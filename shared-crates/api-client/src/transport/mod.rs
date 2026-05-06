//! Transport layer — generic HTTP and WebSocket helpers used by every
//! operation and subscription. The only place that touches `reqwest`,
//! bearer headers, `tokio_tungstenite`, and `graphql_ws_client`
//! directly. Adding a new query or subscription should never require
//! touching this folder.

pub mod http;
pub mod ws;

pub use http::execute_query;
pub use ws::open_subscription;
