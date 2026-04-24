use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::path::PathBuf;
use crate::error::{DatabaseError, Result};
use crate::schema;

/// Main database client
pub struct Database {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl Database {
    /// Initialize the database with automatic migration
    pub async fn init() -> Result<Self> {
        let db_path = Self::get_database_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let db_exists = db_path.exists();
        
        // Create connection options
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        
        // Create connection pool
        let pool = SqlitePool::connect_with(options).await?;
        
        if !db_exists {
            println!("Creating new database at: {}", db_path.display());
        }
        
        // Run declarative schema migration
        schema::migrate(&pool).await?;
        
        Ok(Self { pool, db_path })
    }
    
    /// Get the database file path based on the operating system
    fn get_database_path() -> Result<PathBuf> {
        let data_dir = if cfg!(target_os = "macos") {
            dirs::data_local_dir()
                .ok_or_else(|| DatabaseError::PathError(
                    "Could not determine local data directory".to_string()
                ))?
                .join("Zaned")
        } else if cfg!(target_os = "windows") {
            dirs::data_local_dir()
                .ok_or_else(|| DatabaseError::PathError(
                    "Could not determine local data directory".to_string()
                ))?
                .join("Zaned")
        } else {
            // Linux and other Unix-like systems
            dirs::data_local_dir()
                .ok_or_else(|| DatabaseError::PathError(
                    "Could not determine local data directory".to_string()
                ))?
                .join("zaned")
        };
        
        Ok(data_dir.join("zaned.db"))
    }
    
    /// Get the database pool for executing queries
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    
    /// Get the database file path
    pub fn path(&self) -> &PathBuf {
        &self.db_path
    }
    
    /// Get the currently applied schema version
    pub async fn get_schema_version(&self) -> Result<Option<String>> {
        schema::logic::get_applied_version(&self.pool).await
    }
    
    /// Close the database connection
    pub async fn close(self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_path() {
        let path = Database::get_database_path().unwrap();
        assert!(path.to_string_lossy().contains("zaned"));
        assert!(path.to_string_lossy().ends_with(".db"));
    }
}
