//! Coordinator: owns the Yahoo `PriceStream`, processes subscribe/unsubscribe
//! commands, and dispatches each tick through `RedisState::apply_tick`.
//!
//! The coordinator is the single authority over:
//! - which symbols are currently subscribed to Yahoo's WebSocket
//! - which symbols persist in `tick:active` (Redis SET)
//! - tick → Redis state pipeline (was `aggregator.rs`)
//!
//! `PriceStream` is created lazily on the first real subscribe so we never
//! have to send an empty symbol list to Yahoo's WS handshake.

use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;
use markets::streaming::{MarketHoursType, PriceStream, PriceUpdate, QuoteType};
use tokio::sync::{Mutex, mpsc, oneshot};

use super::protocol::BarPayload;
use super::redis_state::RedisState;
use super::subscriptions::Subscriptions;

// ── Command channel ──────────────────────────────────────────────────────────

pub enum CoordCmd {
    Subscribe {
        symbol: String,
        resp: oneshot::Sender<()>,
    },
    Unsubscribe {
        symbol: String,
    },
    Shutdown,
}

pub fn make_command_channel() -> (mpsc::Sender<CoordCmd>, mpsc::Receiver<CoordCmd>) {
    mpsc::channel(256)
}

// ── Coordinator ──────────────────────────────────────────────────────────────

pub struct Coordinator {
    redis: Arc<RedisState>,
    subs: Arc<Mutex<Subscriptions>>,
    cmd_rx: mpsc::Receiver<CoordCmd>,
    /// Created lazily on first subscribe; Yahoo WS rejects an empty symbol
    /// list during handshake.
    price_stream: Option<PriceStream>,
}

impl Coordinator {
    pub fn new(
        redis: Arc<RedisState>,
        subs: Arc<Mutex<Subscriptions>>,
        cmd_rx: mpsc::Receiver<CoordCmd>,
    ) -> Self {
        Self {
            redis,
            subs,
            cmd_rx,
            price_stream: None,
        }
    }

    /// Drive the coordinator forever. Multiplexes the command channel and
    /// `PriceStream` ticks via `tokio::select!`.
    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                biased;

                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(CoordCmd::Subscribe { symbol, resp }) => {
                            let outcome = {
                                let mut subs = self.subs.lock().await;
                                subs.admit(&symbol)
                            };

                            if outcome.newly_added {
                                tracing::info!(symbol = %symbol, "coordinator: admitting symbol");
                                // Synchronous: PriceStream membership update.
                                // Fast for an already-connected stream;
                                // lazy-creates on the very first subscribe.
                                self.add_to_price_stream(&symbol).await;

                                // Spawn the seed path: SADD tick:active +
                                // Yahoo chart fetch + batch HSET. The ack
                                // fires only after this completes so the
                                // WS handler's `today` frame reflects the
                                // seeded bars instead of an empty hash.
                                let redis = self.redis.clone();
                                let sym = symbol.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = redis.add_active(&sym).await {
                                        tracing::warn!(symbol = %sym, error = %e, "add_active failed");
                                    }
                                    if let Err(e) = seed_symbol_today(&redis, &sym).await {
                                        tracing::warn!(symbol = %sym, error = ?e, "seed_today failed");
                                    }
                                    let _ = resp.send(());
                                });
                            } else {
                                // Already-warm symbol — ref bumped, no seeding needed.
                                let _ = resp.send(());
                            }

                            for ev in outcome.evicted {
                                tracing::info!(symbol = %ev, "coordinator: evicting (zero-ref oldest)");
                                self.evict(&ev).await;
                            }
                        }
                        Some(CoordCmd::Unsubscribe { symbol }) => {
                            self.handle_unsubscribe(symbol).await;
                        }
                        Some(CoordCmd::Shutdown) | None => break,
                    }
                }

                tick = next_tick(&mut self.price_stream) => {
                    if let Some(t) = tick {
                        self.handle_tick(t).await;
                    }
                    // None means no stream yet — pending, loop back.
                }
            }
        }
        Ok(())
    }

    /// Re-prime the `PriceStream` and `Subscriptions` from the `tick:active`
    /// Redis SET on boot. Symbols are added with ref_count = 0 (clients
    /// increment on re-subscribe).
    pub async fn rehydrate_from_redis(&mut self) -> Result<()> {
        let active = self.redis.get_active().await?;
        if active.is_empty() {
            tracing::info!("rehydrate: no persisted active symbols; starting cold");
            return Ok(());
        }

        let symbols: Vec<&str> = active.iter().map(|s| s.as_str()).collect();
        tracing::info!(count = symbols.len(), "rehydrate: re-priming PriceStream");

        match PriceStream::subscribe(&symbols).await {
            Ok(s) => self.price_stream = Some(s),
            Err(e) => {
                tracing::error!(error = %e, "rehydrate: PriceStream::subscribe failed");
            }
        }

        let mut subs = self.subs.lock().await;
        for s in active {
            subs.admit_at_zero_ref(&s);
        }
        Ok(())
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn handle_unsubscribe(&mut self, symbol: String) {
        let mut subs = self.subs.lock().await;
        subs.release(&symbol);
        // Symbol stays warm for re-subscribe; no eviction here.
    }

    async fn evict(&mut self, symbol: &str) {
        if let Some(stream) = self.price_stream.as_ref() {
            stream.remove_symbols(&[symbol]).await;
        }
        if let Err(e) = self.redis.remove_active(symbol).await {
            tracing::warn!(symbol, error = %e, "remove_active failed");
        }
        if let Err(e) = self.redis.clear_symbol_state(symbol).await {
            tracing::warn!(symbol, error = %e, "clear_symbol_state failed");
        }
    }

    async fn add_to_price_stream(&mut self, symbol: &str) {
        if let Some(stream) = self.price_stream.as_ref() {
            stream.add_symbols(&[symbol]).await;
        } else {
            // Lazily create the stream on first real subscribe.
            match PriceStream::subscribe(&[symbol]).await {
                Ok(s) => {
                    tracing::info!(
                        symbol,
                        "coordinator: PriceStream connected (first subscribe)"
                    );
                    self.price_stream = Some(s);
                }
                Err(e) => {
                    tracing::error!(symbol, error = %e, "PriceStream::subscribe failed");
                }
            }
        }
    }

    async fn handle_tick(&mut self, tick: PriceUpdate) {
        if !matches!(tick.quote_type, QuoteType::Equity | QuoteType::Etf) {
            return;
        }
        if !matches!(tick.market_hours, MarketHoursType::RegularMarket) {
            return;
        }

        let bucket = bucket_start_secs(tick.time);
        let price = cents(tick.price);

        if let Err(e) = self
            .redis
            .apply_tick(&tick.id, bucket, price, tick.day_volume, tick.time)
            .await
        {
            tracing::warn!(symbol = %tick.id, error = ?e, "apply_tick failed");
        }
    }
}

// ── Helper: poll the optional stream without returning None prematurely ───────

/// Yields the next `PriceUpdate` from the stream, or stays pending forever
/// when there is no stream yet (until one is created).
async fn next_tick(stream: &mut Option<PriceStream>) -> Option<PriceUpdate> {
    match stream {
        Some(s) => s.next().await,
        None => std::future::pending().await,
    }
}

// ── Per-symbol today seeding (replaces recovery.rs) ──────────────────────────

/// Fetch today's already-elapsed 1-min bars from Yahoo's chart endpoint and
/// seed them into Redis. Best-effort — errors are logged, not surfaced.
pub async fn seed_symbol_today(redis: &RedisState, symbol: &str) -> Result<usize> {
    use chrono::{TimeZone, Utc};
    use markets::{Interval, Ticker, TimeRange};

    let session_start = today_session_start_utc();

    // Floor "now" to the current minute so we never seed an in-progress bar.
    let now_secs = Utc::now().timestamp();
    let cutoff_secs = now_secs - (now_secs % 60);
    let cutoff = Utc
        .timestamp_opt(cutoff_secs, 0)
        .single()
        .unwrap_or_else(Utc::now);

    let ticker = Ticker::new(symbol).await?;
    let chart = ticker.chart(Interval::OneMinute, TimeRange::OneDay).await?;

    let mut bars = Vec::with_capacity(chart.candles.len());
    for c in &chart.candles {
        let Some(ts) = chrono::Utc.timestamp_opt(c.timestamp, 0).single() else {
            continue;
        };
        if ts < session_start || ts >= cutoff {
            continue;
        }
        bars.push(BarPayload {
            ts: c.timestamp,
            o: ((c.open as f32) * 100.0).round() as i32,
            h: ((c.high as f32) * 100.0).round() as i32,
            l: ((c.low as f32) * 100.0).round() as i32,
            c: ((c.close as f32) * 100.0).round() as i32,
            v: c.volume,
        });
    }

    let n = bars.len();
    redis.seed_today_bars(symbol, &bars).await?;
    tracing::info!(symbol, bars = n, "seed_symbol_today complete");
    Ok(n)
}

/// Today's regular-session open in UTC: 09:30 US/Eastern (DST-aware).
fn today_session_start_utc() -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    let now_et = chrono::Utc::now().with_timezone(&chrono_tz::America::New_York);
    let today = now_et.date_naive();
    let session_local = today
        .and_hms_opt(9, 30, 0)
        .expect("09:30 is always valid")
        .and_local_timezone(chrono_tz::America::New_York)
        .single()
        .expect("09:30 ET is unambiguous outside spring-forward DST gap");
    session_local.with_timezone(&chrono::Utc)
}

// ── Tick helpers (moved from aggregator.rs) ───────────────────────────────────

/// Convert a Yahoo `f32` price (dollars) to `i32` cents.
fn cents(price: f32) -> i32 {
    (price * 100.0).round() as i32
}

/// Compute the minute-boundary unix seconds for a tick timestamp (ms).
fn bucket_start_secs(tick_ts_ms: i64) -> i64 {
    let secs = tick_ts_ms / 1000;
    secs - (secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_start_secs_floors_to_minute() {
        for &ts_ms in &[1_700_000_037_500i64, 1_700_000_059_999, 1_700_000_000_000] {
            let b = bucket_start_secs(ts_ms);
            assert_eq!(b % 60, 0, "bucket_start must be on the minute");
            assert!(b * 1000 <= ts_ms, "bucket_start must be <= tick_ts");
            assert!(
                ts_ms - b * 1000 < 60_000,
                "bucket_start must be within the same minute"
            );
        }
    }

    #[test]
    fn cents_rounds_correctly() {
        assert_eq!(cents(123.45), 12345);
        assert_eq!(cents(0.01), 1);
        assert_eq!(cents(0.0), 0);
    }
}
