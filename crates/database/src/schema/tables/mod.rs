/// Declarative schema. Bump SCHEMA_VERSION in logic.rs when you change this.
///
/// The migrator will automatically:
///   - Create new tables
///   - Drop removed tables (safely)
///   - Add new columns
///   - Drop removed columns (via table rebuild if constrained)
///   - Rename columns (via `-- rename: table.old_col -> new_col` comments)
///   - Rename tables  (via `-- rename_table: old_name -> new_name` comments)
///   - Create/drop indexes
///   - Create/drop triggers
///
/// Use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` as usual.
pub const SCHEMA_SQL: &str = r#"
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
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS drawings (
    symbol           TEXT NOT NULL,
    drawing_id       INTEGER NOT NULL,
    def_id           TEXT NOT NULL,
    points_json      TEXT NOT NULL,
    point_dates_json TEXT NOT NULL,
    style_json       TEXT NOT NULL,
    kind_style_json  TEXT NOT NULL,
    locked           INTEGER NOT NULL,
    updated_at       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (symbol, drawing_id)
);

CREATE INDEX IF NOT EXISTS idx_drawings_symbol ON drawings (symbol);
"#;
