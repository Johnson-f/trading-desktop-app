//! GraphQL types for historical bar aggregation.

use async_graphql::{Enum, InputObject, SimpleObject};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, Enum, PartialEq, Eq)]
pub enum BarUnit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl From<BarUnit> for crate::service::historical_service::BarUnit {
    fn from(u: BarUnit) -> Self {
        match u {
            BarUnit::Minute => Self::Minute,
            BarUnit::Hour => Self::Hour,
            BarUnit::Day => Self::Day,
            BarUnit::Week => Self::Week,
            BarUnit::Month => Self::Month,
        }
    }
}

#[derive(Debug, Clone, Copy, InputObject)]
pub struct BucketInput {
    pub unit: BarUnit,
    pub count: i32,
}

/// One aggregated OHLCV bar. Prices are sent as Int cents (Decimal32(2)
/// in storage); divide by 100 to render dollars.
///
/// Named `HistoricalBar` rather than `Bar` because the live ticks
/// subscription already exports a `Bar` type (with abbreviated fields)
/// — async-graphql panics on duplicate type names at schema build.
#[derive(Debug, Clone, SimpleObject)]
pub struct HistoricalBar {
    pub ts: DateTime<Utc>,
    pub open: i32,
    pub high: i32,
    pub low: i32,
    pub close: i32,
    pub volume: i64,
}
