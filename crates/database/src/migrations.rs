use sqlx::SqlitePool;
use crate::error::{DatabaseError, Result};

/// Migration definition
pub struct Migration {
    pub version: &'static str,
    pub description: &'static str,
    pub up: &'static str,
}

/// All migrations in order
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "0.01",
        description: "Initial schema with schema_version and drawing_defaults tables",
        up: r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version TEXT PRIMARY KEY NOT NULL,
                applied_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL,
                description TEXT
            );
            
            CREATE TABLE IF NOT EXISTS drawing_defaults (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                color_r INTEGER NOT NULL,
                color_g INTEGER NOT NULL,
                color_b INTEGER NOT NULL,
                color_a INTEGER NOT NULL,
                width REAL NOT NULL,
                dash TEXT NOT NULL CHECK (dash IN ('Solid', 'Dashed', 'Dotted')),
                opacity REAL NOT NULL,
                extend_left INTEGER NOT NULL,
                extend_right INTEGER NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_schema_version_applied 
            ON schema_version(applied_at DESC);
        "#,
    },
];

/// Get the current schema version from the database
pub async fn get_current_version(pool: &SqlitePool) -> Result<Option<String>> {
    // Check if schema_version table exists
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'"
    )
    .fetch_one(pool)
    .await?;
    
    if !table_exists {
        return Ok(None);
    }
    
    let version: Option<String> = sqlx::query_scalar(
        "SELECT version FROM schema_version ORDER BY applied_at DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(version)
}

/// Run all pending migrations
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    let current_version = get_current_version(pool).await?;
    
    let start_index = if let Some(ref version) = current_version {
        MIGRATIONS
            .iter()
            .position(|m| m.version == version)
            .map(|i| i + 1)
            .unwrap_or(0)
    } else {
        0
    };
    
    for migration in &MIGRATIONS[start_index..] {
        println!("Running migration {}: {}", migration.version, migration.description);
        
        // Begin transaction
        let mut tx = pool.begin().await?;
        
        // Execute migration
        sqlx::query(migration.up)
            .execute(&mut *tx)
            .await
            .map_err(|e| DatabaseError::Migration(format!(
                "Failed to apply migration {}: {}",
                migration.version, e
            )))?;
        
        // Record migration
        sqlx::query(
            "INSERT INTO schema_version (version, description) VALUES (?, ?)"
        )
        .bind(migration.version)
        .bind(migration.description)
        .execute(&mut *tx)
        .await?;
        
        // Commit transaction
        tx.commit().await?;
        
        println!("✓ Migration {} applied successfully", migration.version);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_migrations_are_ordered() {
        let mut prev_version = "0.00";
        for migration in MIGRATIONS {
            assert!(
                migration.version > prev_version,
                "Migrations must be in ascending order"
            );
            prev_version = migration.version;
        }
    }
    
    #[test]
    fn test_migrations_have_descriptions() {
        for migration in MIGRATIONS {
            assert!(
                !migration.description.is_empty(),
                "Migration {} must have a description",
                migration.version
            );
        }
    }
}
