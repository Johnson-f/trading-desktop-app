//! Real-time tick aggregation subsystem: subscribes to Yahoo's WebSocket
//! per active symbol, aggregates ticks into 1-min OHLCV bars in Redis, and
//! fans out live updates to desktop clients over a WebSocket at `/ws`.
//!
//! The host process boots this via `runtime::start`, mounts the returned
//! Router into its top-level axum app, and aborts the returned handles on
//! shutdown.

pub mod boundary;
pub mod config;
pub mod coordinator;
pub mod protocol;
pub mod redis_state;
pub mod runtime;
pub mod subscriptions;
pub mod ws_server;
