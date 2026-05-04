//! Once a minute, finalize any in-progress candle whose bucket has already
//! ended (because no new tick arrived for the next minute, or because the
//! aggregator's atomic transition didn't write to today-bars).
//!
//! Most candles are finalized by the aggregator's Lua script when a tick
//! crosses the minute boundary. The sweeper handles two edge cases:
//! 1. Symbols that briefly stop trading (gap > 1 min) — last tick's bucket
//!    sits in `tick:current` past its close.
//! 2. End-of-session (16:00 ET) — no more ticks for hours, so the 15:59
//!    candle has nothing to push it out.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::Mutex;

use super::protocol::BarPayload;
use super::redis_state::RedisState;
use super::subscriptions::Subscriptions;

/// Wait until the next minute boundary plus a small grace period (1s) so
/// late ticks of the just-completed minute have time to land.
async fn wait_until_next_minute_boundary() {
    let now = Utc::now();
    let next_minute = (now.timestamp() / 60 + 1) * 60;
    let target_ms = next_minute * 1000 + 1000; // +1s grace
    let wait_ms = (target_ms - now.timestamp_millis()).max(0) as u64;
    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
}

/// Drive the sweeper loop forever. Spawned by `main` as a background task.
pub async fn run(redis: Arc<RedisState>, subs: Arc<Mutex<Subscriptions>>) -> Result<()> {
    tracing::info!("boundary sweeper: starting");

    loop {
        wait_until_next_minute_boundary().await;
        let now = Utc::now().timestamp();
        let prior_bucket = (now / 60 - 1) * 60;
        let mut finalized = 0usize;

        // Snapshot symbols quickly so we don't hold the lock across Redis I/O.
        let snapshot = {
            let s = subs.lock().await;
            s.snapshot()
        };

        for symbol in &snapshot {
            match redis.get_current(symbol).await {
                Ok(Some(candle)) if candle.ts <= prior_bucket => {
                    let bar = BarPayload {
                        ts: candle.ts,
                        o: candle.o,
                        h: candle.h,
                        l: candle.l,
                        c: candle.c,
                        v: candle.v,
                    };
                    if let Err(e) = redis.finalize_bar(symbol, bar).await {
                        tracing::warn!(symbol, error = %e, "finalize_bar failed");
                    } else {
                        let _ = redis.clear_current_if_bucket(symbol, candle.ts).await;
                        finalized += 1;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(symbol, error = %e, "get_current failed");
                }
            }
        }

        if finalized > 0 {
            tracing::info!(
                finalized,
                total = snapshot.len(),
                "boundary sweeper: finalized stale candles"
            );
        }
    }
}
