//! GraphQL custom-scalar mappings used by `graphql_client` codegen.
//!
//! Each `pub type X = Y` here lets a `scalar X` declared in
//! `schema.graphql` be represented in Rust as `Y` for serde's
//! Serialize/Deserialize. We use `String` for `DateTime` so the
//! ISO-8601 wire format passes through unchanged; the public wrappers
//! on `ApiClient` accept `chrono::DateTime<Utc>` and convert to RFC-3339
//! strings on the way in.

pub type DateTime = String;
