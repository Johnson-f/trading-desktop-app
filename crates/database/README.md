# Zaned Database Crate

SQLite-based database layer for the Zaned application with automatic migrations and schema versioning.

## Features

- **Automatic Migrations**: Database schema is automatically created and updated on startup
- **Schema Versioning**: Tracks all schema changes with version numbers (0.01, 0.02, etc.)
- **Cross-Platform**: Database location adapts to the operating system:
  - **macOS**: `~/Library/Application Support/Zaned/zaned.db`
  - **Linux**: `~/.local/share/zaned/zaned.db`
  - **Windows**: `%LOCALAPPDATA%\Zaned\zaned.db`

## Usage

### Initialization

```rust
use zaned_database::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database (creates if doesn't exist, runs migrations)
    let db = Database::init().await?;
    
    // Get the database pool for queries
    let pool = db.pool();
    
    // Check current schema version
    if let Some(version) = db.get_schema_version().await? {
        println!("Schema version: {}", version);
    }
    
    Ok(())
}
```

### Adding Migrations

Migrations are defined in `src/migrations.rs`. To add a new migration:

1. Add a new `Migration` struct to the `MIGRATIONS` array
2. Increment the version number (e.g., "0.02", "0.03")
3. Provide a description and SQL statements

```rust
Migration {
    version: "0.02",
    description: "Add user preferences table",
    up: r#"
        CREATE TABLE IF NOT EXISTS user_preferences (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
        );
    "#,
},
```

## Current Schema

### Version 0.01

**Tables:**
- `schema_version`: Tracks applied migrations
  - `version` (TEXT, PRIMARY KEY): Migration version
  - `applied_at` (DATETIME): When the migration was applied
  - `description` (TEXT): Migration description

- `drawing_defaults`: Stores user's default drawing style preferences
  - `id` (INTEGER, PRIMARY KEY): Always 1 (single row table)
  - `color_r`, `color_g`, `color_b`, `color_a` (INTEGER): RGBA color components
  - `width` (REAL): Line width
  - `dash` (TEXT): Dash style (Solid, Dashed, Dotted)
  - `opacity` (REAL): Opacity value (0.0-1.0)
  - `extend_left`, `extend_right` (INTEGER): Extension flags (0 or 1)
  - `updated_at` (DATETIME): Last update timestamp

**Indexes:**
- `idx_schema_version_applied`: Index on `schema_version.applied_at` for efficient version queries

## Architecture

- **`client.rs`**: Main database client with initialization and connection pooling
- **`migrations.rs`**: Migration definitions and execution logic
- **`error.rs`**: Error types and result aliases

## Dependencies

- `sqlx`: Async SQL toolkit with SQLite support
- `tokio`: Async runtime
- `dirs`: Cross-platform directory paths
- `thiserror`: Error handling
- `serde`/`serde_json`: Serialization support
