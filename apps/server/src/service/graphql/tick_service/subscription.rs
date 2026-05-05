//! GraphQL subscription resolvers for the tick service. The single
//! subscription `ticks(symbols)` drives real-time market data: it admits
//! the symbols into the coordinator's PriceStream, replays today's bars
//! from Redis, and tails the per-symbol Redis stream forever.
//!
//! Per-stream cleanup runs via `UnsubscribeOnDrop` so coordinator ref
//! counts drop the moment the GraphQL client disconnects.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_graphql::{Context, Subscription};
use async_stream::stream;
use futures::Stream;
use redis::AsyncCommands;
use redis::streams::{StreamRangeReply, StreamReadOptions, StreamReadReply};
use tokio::sync::{mpsc, oneshot};

use super::types::{Bar, DeltaEvent, FinalizeEvent, SeedEvent, TickEvent, TodayEvent};
use crate::service::tick_service::coordinator::CoordCmd;
use crate::service::tick_service::protocol::BarPayload;
use crate::service::tick_service::redis_state::{RedisState, updates_key};

/// Shared application state injected via `Schema::data` and read out in
/// each subscription via `Context::data`.
#[derive(Clone)]
pub struct SubscriptionContext {
    pub redis: Arc<RedisState>,
    pub redis_url: String,
    pub cmd_tx: mpsc::Sender<CoordCmd>,
}

#[derive(Default)]
pub struct TicksSubscription;

#[Subscription]
impl TicksSubscription {
    /// Stream live OHLCV updates for `symbols`. Yields one `Seed` and one
    /// `Today` per symbol on connect, then a continuous stream of `Delta`
    /// and `Finalize` events as ticks arrive.
    async fn ticks(
        &self,
        ctx: &Context<'_>,
        symbols: Vec<String>,
    ) -> async_graphql::Result<impl Stream<Item = TickEvent>> {
        let cx = ctx.data::<SubscriptionContext>()?.clone();

        if symbols.is_empty() {
            return Err(async_graphql::Error::new("symbols must not be empty"));
        }

        Ok(stream! {
            // RAII: send Unsubscribe for every admitted symbol when this
            // stream is dropped, regardless of where the await happens to be.
            let guard = UnsubscribeOnDrop::new(cx.cmd_tx.clone());

            // Dedicated XREAD connection. The shared ConnectionManager
            // can't be used here — a long BLOCK would starve other commands.
            let conn_cfg = redis::AsyncConnectionConfig::new()
                .set_connection_timeout(Some(Duration::from_secs(10)))
                .set_response_timeout(Some(Duration::from_secs(30)));
            let client = match redis::Client::open(cx.redis_url.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "graphql ticks: open redis client");
                    return;
                }
            };
            let mut xread_conn = match client.get_multiplexed_async_connection_with_config(&conn_cfg).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "graphql ticks: get xread connection");
                    return;
                }
            };

            // Per-symbol XREAD cursor (last delivered stream id).
            let mut cursors: HashMap<String, String> = HashMap::new();

            // Phase 1: admit each symbol, emit Seed + Today, snapshot cursor.
            for symbol in &symbols {
                if cursors.contains_key(symbol) {
                    continue;
                }

                let (tx, rx) = oneshot::channel();
                if cx.cmd_tx.send(CoordCmd::Subscribe { symbol: symbol.clone(), resp: tx }).await.is_err() {
                    tracing::warn!(symbol, "graphql ticks: coordinator channel closed");
                    return;
                }
                if rx.await.is_err() {
                    tracing::warn!(symbol, "graphql ticks: coordinator dropped admit ack; aborting");
                    return;
                }
                guard.track(symbol.clone());

                let candle = cx
                    .redis
                    .get_current(symbol)
                    .await
                    .ok()
                    .flatten()
                    .map(bar_from_payload);
                yield TickEvent::Seed(SeedEvent { symbol: symbol.clone(), candle });

                let today = cx
                    .redis
                    .get_today_bars(symbol)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(bar_from_payload)
                    .collect();
                yield TickEvent::Today(TodayEvent { symbol: symbol.clone(), bars: today });

                // Capture cursor: the latest stream id, or "0" if empty.
                let range_reply: StreamRangeReply = xread_conn
                    .xrevrange_count(updates_key(symbol), "+", "-", 1usize)
                    .await
                    .unwrap_or_else(|_| StreamRangeReply { ids: vec![] });
                let last_id = range_reply
                    .ids
                    .first()
                    .map(|e| e.id.clone())
                    .unwrap_or_else(|| "0".to_string());
                cursors.insert(symbol.clone(), last_id);
            }

            // Phase 2: live tail.
            loop {
                let (stream_keys, stream_ids): (Vec<String>, Vec<String>) = cursors
                    .iter()
                    .map(|(sym, id)| (updates_key(sym), id.clone()))
                    .unzip();

                let opts = StreamReadOptions::default().block(5000);
                let result: redis::RedisResult<Option<StreamReadReply>> = xread_conn
                    .xread_options(&stream_keys, &stream_ids, &opts)
                    .await;

                match result {
                    Ok(Some(reply)) => {
                        for stream_data in reply.keys {
                            let symbol = stream_data
                                .key
                                .strip_prefix("tick:updates:")
                                .unwrap_or(&stream_data.key)
                                .to_string();
                            for entry in stream_data.ids {
                                if let Some(event) = parse_stream_entry(&symbol, &entry.map) {
                                    yield event;
                                }
                                cursors.insert(symbol.clone(), entry.id.clone());
                            }
                        }
                    }
                    Ok(None) => {
                        // Block timeout — no entries; loop and re-issue.
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "graphql ticks: xread failed");
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        })
    }
}

/// RAII: collect symbol names as they're admitted; when dropped (client
/// disconnects), spawn a tokio task that sends `Unsubscribe` for each.
/// The Drop runs on the runtime thread that owns the stream future, so
/// we can't `.await` directly — `tokio::spawn` defers cleanup off that
/// thread.
struct UnsubscribeOnDrop {
    cmd_tx: mpsc::Sender<CoordCmd>,
    symbols: std::sync::Mutex<Vec<String>>,
}

impl UnsubscribeOnDrop {
    fn new(cmd_tx: mpsc::Sender<CoordCmd>) -> Self {
        Self {
            cmd_tx,
            symbols: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn track(&self, symbol: String) {
        self.symbols.lock().expect("guard poisoned").push(symbol);
    }
}

impl Drop for UnsubscribeOnDrop {
    fn drop(&mut self) {
        let cmd_tx = self.cmd_tx.clone();
        let symbols = std::mem::take(&mut *self.symbols.lock().expect("guard poisoned"));
        if symbols.is_empty() {
            return;
        }
        // Guard against the runtime being mid-shutdown — `tokio::spawn`
        // would panic if no runtime is current. On shutdown the
        // coordinator is also being aborted, so dropping the unsubs is
        // acceptable; the next clean restart rehydrates from `tick:active`.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                for s in symbols {
                    let _ = cmd_tx.send(CoordCmd::Unsubscribe { symbol: s }).await;
                }
            });
        }
    }
}

fn bar_from_payload(p: BarPayload) -> Bar {
    Bar { ts: p.ts, o: p.o, h: p.h, l: p.l, c: p.c, v: p.v }
}

/// Translate one Redis stream entry into a `TickEvent`. Returns `None` if
/// the entry's `type` field is missing or unknown.
fn parse_stream_entry(
    symbol: &str,
    map: &HashMap<String, redis::Value>,
) -> Option<TickEvent> {
    let mut fields: HashMap<String, String> = HashMap::new();
    for (k, v) in map {
        if let redis::Value::BulkString(bytes) = v {
            if let Ok(s) = std::str::from_utf8(bytes) {
                fields.insert(k.clone(), s.to_string());
            }
        }
    }
    let typ = fields.get("type").map(|s| s.as_str()).unwrap_or("");
    let parse_i32 = |k: &str| fields.get(k).and_then(|s| s.parse::<i32>().ok());
    let parse_i64 = |k: &str| fields.get(k).and_then(|s| s.parse::<i64>().ok());

    match typ {
        "delta" => Some(TickEvent::Delta(DeltaEvent {
            symbol: symbol.to_string(),
            ts: parse_i64("ts").unwrap_or(0),
            c: parse_i32("close").unwrap_or(0),
            h: parse_i32("high"),
            l: parse_i32("low"),
            v: parse_i64("volume"),
        })),
        "finalize" => Some(TickEvent::Finalize(FinalizeEvent {
            symbol: symbol.to_string(),
            candle: Bar {
                ts: parse_i64("ts").unwrap_or(0),
                o: parse_i32("o").unwrap_or(0),
                h: parse_i32("h").unwrap_or(0),
                l: parse_i32("l").unwrap_or(0),
                c: parse_i32("c").unwrap_or(0),
                v: parse_i64("v").unwrap_or(0),
            },
        })),
        _ => None,
    }
}
