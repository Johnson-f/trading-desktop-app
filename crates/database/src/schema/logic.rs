use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};

use super::tables::SCHEMA_SQL;
use crate::error::{DatabaseError, Result};

/// Bump this when you change SCHEMA_SQL and want the diff re-applied.
pub const SCHEMA_VERSION: &str = "0.1";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run declarative schema migration if version has changed.
///
/// Parses SCHEMA_SQL to determine the desired schema, introspects the live DB,
/// then applies the minimal diff: create/drop tables, add/drop columns,
/// rename columns/tables, create/drop indexes and triggers.
pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    ensure_version_table(pool).await?;

    let applied = get_applied_version(pool).await?;
    if applied.as_deref() == Some(SCHEMA_VERSION) {
        println!("Schema is up to date at v{}", SCHEMA_VERSION);
        return Ok(());
    }

    println!(
        "Migrating schema to v{} (was: {})",
        SCHEMA_VERSION,
        applied.as_deref().unwrap_or("none")
    );

    let table_renames = parse_table_renames(SCHEMA_SQL);
    let column_renames = parse_column_renames(SCHEMA_SQL);

    // Step 1: Apply table renames first (so the rest of the diff sees correct names)
    for (old, new) in &table_renames {
        if table_exists(pool, old).await? {
            println!("Renaming table {} -> {}", old, new);
            sqlx::query(&format!("ALTER TABLE \"{}\" RENAME TO \"{}\"", old, new))
                .execute(pool)
                .await
                .map_err(|e| {
                    DatabaseError::Migration(format!(
                        "Failed to rename table {} -> {}: {}",
                        old, new, e
                    ))
                })?;
        }
    }

    // Step 2: Apply column renames (before we diff columns)
    for ((table, old_col), new_col) in &column_renames {
        if table_exists(pool, table).await? && column_exists(pool, table, old_col).await? {
            println!("Renaming column {}.{} -> {}", table, old_col, new_col);
            sqlx::query(&format!(
                "ALTER TABLE \"{}\" RENAME COLUMN \"{}\" TO \"{}\"",
                table, old_col, new_col
            ))
            .execute(pool)
            .await
            .map_err(|e| {
                DatabaseError::Migration(format!(
                    "Failed to rename column {}.{} -> {}: {}",
                    table, old_col, new_col, e
                ))
            })?;
        }
    }

    // Step 3: Create tables first so column diffs can be applied before any
    // indexes or triggers reference newly added columns.
    let statements = split_sql_statements(SCHEMA_SQL);
    for stmt in &statements {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || !is_create_table_statement(trimmed) {
            continue;
        }
        sqlx::query(trimmed).execute(pool).await?;
    }

    // Step 4: Diff columns — find columns to add or drop
    let desired_tables = parse_desired_tables(SCHEMA_SQL);
    let live_tables = get_live_tables(pool).await?;

    for (table_name, desired_cols) in &desired_tables {
        if let Some(live_cols) = live_tables.get(table_name.as_str()) {
            let live_col_names: HashSet<&str> = live_cols.iter().map(|c| c.name.as_str()).collect();
            let desired_col_names: HashSet<&str> =
                desired_cols.iter().map(|c| c.name.as_str()).collect();

            // Add new columns
            for col in desired_cols {
                if !live_col_names.contains(col.name.as_str()) {
                    let default_clause = if col.notnull && col.default.is_none() {
                        match col.col_type.to_uppercase().as_str() {
                            t if t.contains("INT") => " DEFAULT 0".to_string(),
                            t if t.contains("REAL") || t.contains("DECIMAL") => {
                                " DEFAULT 0.0".to_string()
                            }
                            t if t.contains("BOOL") => " DEFAULT 0".to_string(),
                            t if t.contains("BLOB") => " DEFAULT X''".to_string(),
                            _ => " DEFAULT ''".to_string(),
                        }
                    } else if let Some(ref def) = col.default {
                        format!(" DEFAULT {}", def)
                    } else {
                        String::new()
                    };

                    let null_clause = if col.notnull { " NOT NULL" } else { "" };

                    let sql = format!(
                        "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}{}{}",
                        table_name, col.name, col.col_type, null_clause, default_clause
                    );
                    println!("Adding column: {}.{}", table_name, col.name);
                    sqlx::query(&sql).execute(pool).await.map_err(|e| {
                        DatabaseError::Migration(format!(
                            "Failed to add column {}.{}: {}",
                            table_name, col.name, e
                        ))
                    })?;
                }
            }

            // Drop removed columns (via ALTER TABLE DROP COLUMN where safe,
            // otherwise rebuild the table)
            let cols_to_drop: Vec<&str> = live_col_names
                .difference(&desired_col_names)
                .copied()
                .collect();

            for col_name in cols_to_drop {
                println!("Dropping column: {}.{}", table_name, col_name);
                let drop_result = sqlx::query(&format!(
                    "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
                    table_name, col_name
                ))
                .execute(pool)
                .await;

                if drop_result.is_err() {
                    println!(
                        "Direct drop failed for {}.{}, rebuilding table",
                        table_name, col_name
                    );
                    rebuild_table_without_columns(pool, table_name, &[col_name], desired_cols)
                        .await
                        .map_err(|e| {
                            DatabaseError::Migration(format!(
                                "Failed to rebuild table {} to drop column {}: {}",
                                table_name, col_name, e
                            ))
                        })?;
                    break; // rebuild handles all drops at once
                }
            }
        }
    }

    // Step 5: Drop tables that no longer exist in SCHEMA_SQL
    let desired_table_names: HashSet<String> = desired_tables.keys().cloned().collect();
    let internal_tables: HashSet<String> = ["_schema_version".to_string()].into();

    for live_table in live_tables.keys() {
        if !desired_table_names.contains(live_table)
            && !internal_tables.contains(live_table)
            && !live_table.starts_with("sqlite_")
        {
            println!("Dropping removed table: {}", live_table);
            sqlx::query(&format!("DROP TABLE IF EXISTS \"{}\"", live_table))
                .execute(pool)
                .await?;
        }
    }

    // Step 6: Drop indexes/triggers not in desired schema
    drop_stale_indexes(pool, SCHEMA_SQL).await?;
    drop_stale_triggers(pool, SCHEMA_SQL).await?;

    // Step 7: Recreate desired indexes/triggers
    for stmt in &statements {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || !is_create_index_or_trigger_statement(trimmed) {
            continue;
        }
        sqlx::query(trimmed).execute(pool).await?;
    }

    // Record version
    sqlx::query("INSERT OR REPLACE INTO _schema_version (id, version) VALUES (1, ?)")
        .bind(SCHEMA_VERSION)
        .execute(pool)
        .await?;

    println!("Schema v{} applied successfully", SCHEMA_VERSION);
    Ok(())
}

/// Get the currently applied schema version, or None if never migrated.
pub async fn get_applied_version(pool: &SqlitePool) -> Result<Option<String>> {
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='_schema_version'",
    )
    .fetch_one(pool)
    .await?;

    if !table_exists {
        return Ok(None);
    }

    let version: Option<String> =
        sqlx::query_scalar("SELECT version FROM _schema_version WHERE id = 1")
            .fetch_optional(pool)
            .await?;

    Ok(version)
}

// ---------------------------------------------------------------------------
// Version table
// ---------------------------------------------------------------------------

async fn ensure_version_table(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _schema_version (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            version TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// DB introspection helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ColumnInfo {
    name: String,
    col_type: String,
    notnull: bool,
    default: Option<String>,
}

async fn table_exists(pool: &SqlitePool, table: &str) -> Result<bool> {
    let exists: bool =
        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?")
            .bind(table)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}

async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> Result<bool> {
    let cols = get_table_columns(pool, table).await?;
    Ok(cols.iter().any(|c| c.name == column))
}

async fn get_table_columns(pool: &SqlitePool, table: &str) -> Result<Vec<ColumnInfo>> {
    // PRAGMA doesn't accept bind params; table name is internal, not user input.
    let rows = sqlx::query(&format!("PRAGMA table_info(\"{}\")", table))
        .fetch_all(pool)
        .await?;

    let mut cols = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        let col_type: String = row.try_get("type")?;
        let notnull: i64 = row.try_get("notnull")?;
        let default: Option<String> = row.try_get("dflt_value").ok();
        cols.push(ColumnInfo {
            name,
            col_type,
            notnull: notnull != 0,
            default,
        });
    }
    Ok(cols)
}

async fn get_live_tables(pool: &SqlitePool) -> Result<HashMap<String, Vec<ColumnInfo>>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(pool)
    .await?;

    let mut tables = HashMap::new();
    for name in names {
        let cols = get_table_columns(pool, &name).await?;
        tables.insert(name, cols);
    }
    Ok(tables)
}

async fn get_live_index_names(pool: &SqlitePool) -> Result<HashSet<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(pool)
    .await?;
    Ok(names.into_iter().collect())
}

async fn get_live_trigger_names(pool: &SqlitePool) -> Result<HashSet<String>> {
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='trigger'")
            .fetch_all(pool)
            .await?;
    Ok(names.into_iter().collect())
}

// ---------------------------------------------------------------------------
// SQL parsing helpers (extract desired state from SCHEMA_SQL)
// ---------------------------------------------------------------------------

fn parse_desired_tables(sql: &str) -> HashMap<String, Vec<ColumnInfo>> {
    let mut tables = HashMap::new();
    let statements = split_sql_statements(sql);

    for stmt in &statements {
        let upper = stmt.trim().to_uppercase();
        if !upper.starts_with("CREATE TABLE") {
            continue;
        }

        let name = extract_table_name(stmt);
        if name.is_empty() {
            continue;
        }

        if let Some(cols) = extract_columns_from_create(stmt) {
            tables.insert(name, cols);
        }
    }
    tables
}

fn extract_table_name(stmt: &str) -> String {
    let upper = stmt.to_uppercase();
    let after_table = if let Some(pos) = upper.find("EXISTS") {
        &stmt[pos + 6..]
    } else if let Some(pos) = upper.find("TABLE") {
        &stmt[pos + 5..]
    } else {
        return String::new();
    };

    after_table
        .trim()
        .split(|c: char| c == '(' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .trim_matches('"')
        .to_string()
}

fn extract_columns_from_create(stmt: &str) -> Option<Vec<ColumnInfo>> {
    let open = stmt.find('(')?;
    let body = &stmt[open + 1..];
    let mut depth = 1i32;
    let mut end = 0;
    for (i, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }

    let columns_str = &body[..end];
    let mut cols = Vec::new();

    for part in split_column_defs(columns_str) {
        let trimmed = part.trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("PRIMARY KEY")
            || upper.starts_with("FOREIGN KEY")
            || upper.starts_with("UNIQUE")
            || upper.starts_with("CHECK")
            || upper.starts_with("CONSTRAINT")
        {
            continue;
        }

        if let Some(col) = parse_column_def(trimmed) {
            cols.push(col);
        }
    }

    Some(cols)
}

fn split_column_defs(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

fn parse_column_def(def: &str) -> Option<ColumnInfo> {
    let tokens: Vec<&str> = def.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let name = tokens[0].trim_matches('"').to_string();
    let upper_name = name.to_uppercase();
    if upper_name == "PRIMARY"
        || upper_name == "FOREIGN"
        || upper_name == "UNIQUE"
        || upper_name == "CHECK"
    {
        return None;
    }

    let col_type = tokens.get(1).copied().unwrap_or("TEXT").to_string();
    let upper_def = def.to_uppercase();
    let notnull = upper_def.contains("NOT NULL");

    let default = if let Some(pos) = upper_def.find("DEFAULT") {
        let after = def[pos + 7..].trim();
        if after.starts_with('(') {
            let mut d = 0i32;
            let mut end = 0;
            for (i, ch) in after.char_indices() {
                match ch {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Some(after[..end].to_string())
        } else {
            Some(
                after
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .next()
                    .unwrap_or("")
                    .to_string(),
            )
        }
    } else {
        None
    };

    Some(ColumnInfo {
        name,
        col_type,
        notnull,
        default,
    })
}

/// Parse `-- rename_table: old_name -> new_name` directives
fn parse_table_renames(sql: &str) -> Vec<(String, String)> {
    let mut renames = Vec::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("-- rename_table:") {
            if let Some((old, new)) = rest.split_once("->") {
                renames.push((old.trim().to_string(), new.trim().to_string()));
            }
        }
    }
    renames
}

/// Parse `-- rename: table.old_col -> new_col` directives
fn parse_column_renames(sql: &str) -> Vec<((String, String), String)> {
    let mut renames = Vec::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("-- rename:") {
            if let Some((left, new_col)) = rest.split_once("->") {
                let left = left.trim();
                if let Some((table, old_col)) = left.split_once('.') {
                    renames.push((
                        (table.trim().to_string(), old_col.trim().to_string()),
                        new_col.trim().to_string(),
                    ));
                }
            }
        }
    }
    renames
}

fn parse_desired_index_names(sql: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let statements = split_sql_statements(sql);
    for stmt in &statements {
        let upper = stmt.trim().to_uppercase();
        if upper.starts_with("CREATE INDEX") || upper.starts_with("CREATE UNIQUE INDEX") {
            let after_exists = if let Some(pos) = upper.find("EXISTS") {
                &stmt[pos + 6..]
            } else if upper.starts_with("CREATE UNIQUE INDEX") {
                &stmt[19..]
            } else {
                &stmt[12..]
            };
            if let Some(name) = after_exists
                .trim()
                .split(|c: char| c.is_whitespace())
                .next()
            {
                names.insert(name.trim_matches('"').to_string());
            }
        }
    }
    names
}

fn parse_desired_trigger_names(sql: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let statements = split_sql_statements(sql);
    for stmt in &statements {
        let upper = stmt.trim().to_uppercase();
        if upper.starts_with("CREATE TRIGGER") {
            let after_exists = if let Some(pos) = upper.find("EXISTS") {
                &stmt[pos + 6..]
            } else {
                &stmt[14..]
            };
            if let Some(name) = after_exists
                .trim()
                .split(|c: char| c.is_whitespace() || c == '\n')
                .next()
            {
                names.insert(name.trim_matches('"').to_string());
            }
        }
    }
    names
}

fn is_create_table_statement(sql: &str) -> bool {
    sql.trim().to_uppercase().starts_with("CREATE TABLE")
}

fn is_create_index_or_trigger_statement(sql: &str) -> bool {
    let upper = sql.trim().to_uppercase();
    upper.starts_with("CREATE INDEX")
        || upper.starts_with("CREATE UNIQUE INDEX")
        || upper.starts_with("CREATE TRIGGER")
}

async fn drop_stale_indexes(pool: &SqlitePool, schema_sql: &str) -> Result<()> {
    let desired = parse_desired_index_names(schema_sql);
    let live = get_live_index_names(pool).await?;

    for name in &live {
        if !desired.contains(name) && !name.starts_with("sqlite_") {
            println!("Dropping stale index: {}", name);
            sqlx::query(&format!("DROP INDEX IF EXISTS \"{}\"", name))
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

async fn drop_stale_triggers(pool: &SqlitePool, schema_sql: &str) -> Result<()> {
    let desired = parse_desired_trigger_names(schema_sql);
    let live = get_live_trigger_names(pool).await?;

    for name in &live {
        if !desired.contains(name) {
            println!("Dropping stale trigger: {}", name);
            sqlx::query(&format!("DROP TRIGGER IF EXISTS \"{}\"", name))
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Table rebuild (SQLite 12-step procedure for dropping constrained columns)
// ---------------------------------------------------------------------------

async fn rebuild_table_without_columns(
    pool: &SqlitePool,
    table: &str,
    drop_cols: &[&str],
    desired_cols: &[ColumnInfo],
) -> Result<()> {
    let keep_cols: Vec<&ColumnInfo> = desired_cols
        .iter()
        .filter(|c| !drop_cols.contains(&c.name.as_str()))
        .collect();

    let col_list: String = keep_cols
        .iter()
        .map(|c| format!("\"{}\"", c.name))
        .collect::<Vec<_>>()
        .join(", ");

    let tmp = format!("_rebuild_{}", table);

    let col_defs: String = keep_cols
        .iter()
        .map(|c| {
            let mut def = format!("\"{}\" {}", c.name, c.col_type);
            if c.notnull {
                def.push_str(" NOT NULL");
            }
            if let Some(ref d) = c.default {
                def.push_str(&format!(" DEFAULT {}", d));
            }
            def
        })
        .collect::<Vec<_>>()
        .join(", ");

    sqlx::query(&format!("CREATE TABLE \"{}\" ({})", tmp, col_defs))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        "INSERT INTO \"{}\" ({}) SELECT {} FROM \"{}\"",
        tmp, col_list, col_list, table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!("DROP TABLE \"{}\"", table))
        .execute(pool)
        .await?;

    sqlx::query(&format!("ALTER TABLE \"{}\" RENAME TO \"{}\"", tmp, table))
        .execute(pool)
        .await?;

    println!("Rebuilt table {} (dropped columns: {:?})", table, drop_cols);
    Ok(())
}

// ---------------------------------------------------------------------------
// SQL statement splitter (respects BEGIN...END blocks for triggers)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn mem_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn migrate_creates_declared_tables() {
        let pool = mem_pool().await;
        migrate(&pool).await.unwrap();

        let live = get_live_tables(&pool).await.unwrap();
        assert!(live.contains_key("drawing_defaults"));

        let version = get_applied_version(&pool).await.unwrap();
        assert_eq!(version.as_deref(), Some(SCHEMA_VERSION));
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let pool = mem_pool().await;
        migrate(&pool).await.unwrap();
        migrate(&pool).await.unwrap();

        let version = get_applied_version(&pool).await.unwrap();
        assert_eq!(version.as_deref(), Some(SCHEMA_VERSION));
    }

    #[test]
    fn splits_statements_across_trigger_begin_end() {
        let sql = r#"
CREATE TABLE t (id INTEGER);

CREATE TRIGGER trg_t AFTER UPDATE ON t
FOR EACH ROW
BEGIN
    UPDATE t SET id = id WHERE id = OLD.id;
END;
"#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("CREATE TABLE t"));
        assert!(stmts[1].contains("CREATE TRIGGER"));
    }

    #[test]
    fn extract_table_name_works_with_if_not_exists() {
        assert_eq!(
            extract_table_name("CREATE TABLE IF NOT EXISTS foo ("),
            "foo"
        );
        assert_eq!(extract_table_name("CREATE TABLE bar (id INTEGER)"), "bar");
    }
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for line in sql.lines() {
        let trimmed_upper = line.trim().to_uppercase();

        if trimmed_upper.starts_with("BEGIN") {
            depth += 1;
        }

        current.push_str(line);
        current.push('\n');

        if trimmed_upper.starts_with("END") && depth > 0 {
            depth -= 1;
            if depth == 0 {
                let trimmed = current.trim().trim_end_matches(';').to_string();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
            }
        } else if depth == 0 && line.trim().ends_with(';') {
            let trimmed = current.trim().trim_end_matches(';').to_string();
            if !trimmed.is_empty() {
                statements.push(trimmed);
            }
            current.clear();
        }
    }

    let trimmed = current.trim().trim_end_matches(';').to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}
