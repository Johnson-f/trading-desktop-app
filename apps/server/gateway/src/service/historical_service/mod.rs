//! Read-only ClickHouse client for historical OHLCV bars. Writes are
//! owned by `apps/historical-service` (scheduled on the VPS); this
//! client never modifies the source tables.

pub mod interval;
pub mod query;

mod reader;

pub use interval::{BarUnit, BucketSpec, validate_symbol};
pub use reader::{Bar, ClickhouseReader};
