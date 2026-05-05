//! GraphQL types exposed by queries, mutations, and subscriptions.
//!
//! `TickEvent` is a union — each yielded subscription frame is one of
//! `Seed`, `Today`, `Delta`, or `Finalize`. The desktop's GraphQL client
//! pattern-matches on `__typename` to route each event.

use async_graphql::{SimpleObject, Union};

/// One OHLCV bar. `ts` is bucket-start unix seconds (UTC), `o`/`h`/`l`/`c`
/// are integer cents (divide by 100 to render in dollars), `v` is i64.
#[derive(Debug, Clone, SimpleObject)]
pub struct Bar {
    pub ts: i64,
    pub o: i32,
    pub h: i32,
    pub l: i32,
    pub c: i32,
    pub v: i64,
}

/// First frame for a new subscription: the in-progress candle for `symbol`,
/// `null` if no tick has landed yet for the current minute.
#[derive(Debug, Clone, SimpleObject)]
pub struct SeedEvent {
    pub symbol: String,
    pub candle: Option<Bar>,
}

/// Second frame for a new subscription: every finalized 1-min bar from
/// session open through the most recently closed minute.
#[derive(Debug, Clone, SimpleObject)]
pub struct TodayEvent {
    pub symbol: String,
    pub bars: Vec<Bar>,
}

/// Live update inside the current minute. `ts` is the tick's timestamp in
/// **unix milliseconds** (matching Yahoo's tick `time` field — divide by
/// 60_000 then multiply by 60 to recover the bar's bucket-start in seconds).
/// `h` and `l` are `null` when they didn't change since the last delta.
#[derive(Debug, Clone, SimpleObject)]
pub struct DeltaEvent {
    pub symbol: String,
    pub ts: i64,
    pub c: i32,
    pub h: Option<i32>,
    pub l: Option<i32>,
    pub v: Option<i64>,
}

/// Minute boundary crossed: this candle is now sealed.
#[derive(Debug, Clone, SimpleObject)]
pub struct FinalizeEvent {
    pub symbol: String,
    pub candle: Bar,
}

/// Union of every frame the `ticks` subscription can yield.
#[derive(Debug, Clone, Union)]
pub enum TickEvent {
    Seed(SeedEvent),
    Today(TodayEvent),
    Delta(DeltaEvent),
    Finalize(FinalizeEvent),
}
