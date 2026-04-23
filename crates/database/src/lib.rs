mod client;
mod migrations;
mod error;

pub use client::Database;
pub use error::{DatabaseError, Result};

/// Current schema version
pub const CURRENT_SCHEMA_VERSION: &str = "0.01";
