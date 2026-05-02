mod client;
mod error;
pub mod operations;
mod schema;

pub use client::Database;
pub use error::{DatabaseError, Result};
pub use schema::logic::SCHEMA_VERSION;
