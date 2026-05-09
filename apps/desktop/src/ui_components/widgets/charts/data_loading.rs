use super::ChartWidget;
use super::candle::{self, CandleData};
use super::drawings;
use std::sync::Arc;
use zaned_chart_core::BaseScale;

pub(super) const DRAWINGS_SAVE_DEBOUNCE: std::time::Duration =
    std::time::Duration::from_millis(500);

/// Trigger a backfill fetch when the camera's `x_offset` (index of the
/// leftmost visible candle) drops below this many bars. With a 1,250-bar
/// chunk per fetch, 100 bars of buffer is enough that the user almost
/// never sees empty space at the left edge: by the time they pan that
/// far, the next chunk has typically landed.
pub(super) const OLDER_LOAD_THRESHOLD_BARS: f64 = 100.0;

/// Parse a `YYYY-MM-DD` date string (the format
/// `candle_loader::bars_to_candle_data` produces) into a UTC midnight
/// `DateTime`. Used to anchor the next backfill fetch at the earliest
/// loaded bar. `None` for any input that isn't a 10-char ISO date prefix.
pub(super) fn parse_date_to_utc_midnight(
    s: Option<&str>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = s?;
    let date = chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()?;
    Some(date.and_hms_opt(0, 0, 0)?.and_utc())
}

/// Convert integer cents (the wire format from the gateway's GraphQL
/// `Int` price fields) to f32 dollars. Server-side prices are
/// `Decimal32(2)` in ClickHouse, decoded as `i32` cents on the way out
/// and widened to `i64` by graphql_client's default `Int` mapping.
fn cents_to_dollars(cents: i64) -> f32 {
    cents as f32 / 100.0
}

impl ChartWidget {
    /// Drain a completed async drawings load, if one finished. Called once per
    /// frame from `show()`.
    pub(super) fn poll_pending_drawings_load(&mut self) {
        let Some(rx) = self.pending_drawings_load.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(loaded) => {
                self.drawings.replace_committed(loaded);
                self.drawings.remap_to_data(&self.data);
                self.pending_drawings_load = None;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Still loading; check again next frame.
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                // Sender dropped without sending — treat as empty.
                self.pending_drawings_load = None;
            }
        }
    }

    /// Drain a completed async candles load. On success, feed the new
    /// data through `set_data` so camera + indicators + drawing remap
    /// run consistently. On error, log and leave the chart's existing
    /// data alone.
    pub(super) fn poll_pending_candles_load(&mut self) {
        let Some(rx) = self.pending_candles_load.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(data)) => {
                self.set_data(data);
                self.pending_candles_load = None;
            }
            Ok(Err(msg)) => {
                tracing::warn!(error = %msg, "candle load failed");
                self.pending_candles_load = None;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Still loading; check again next frame.
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.pending_candles_load = None;
            }
        }
    }

    /// If the user has scrolled within `OLDER_LOAD_THRESHOLD_BARS` of
    /// the leftmost loaded bar (camera index 0), kick off a backfill
    /// fetch. Gated on no-fetch-already-in-flight, no-history-exhausted,
    /// and the initial fetch having landed (we need an `earliest_loaded_ts`).
    pub(super) fn maybe_load_older_candles(&mut self) {
        if self.pending_older_candles_load.is_some() || self.older_history_exhausted {
            return;
        }
        if self.pending_candles_load.is_some() {
            // Initial fetch hasn't landed yet — don't race it.
            return;
        }
        let Some(symbol) = self.symbol.clone() else {
            return;
        };
        let Some(before) = self.earliest_loaded_ts else {
            return;
        };
        let camera = self.camera.lock();
        if camera.x_offset > OLDER_LOAD_THRESHOLD_BARS {
            return;
        }
        drop(camera);
        self.pending_older_candles_load = Some(crate::api::candle_loader::load_older_async(
            symbol,
            self.timeframe.base_scale(),
            before,
        ));
    }

    /// Drain a completed backfill fetch. On a non-empty result, prepend
    /// the older bars to `raw_data`, re-index, and shift the camera so
    /// the user's view doesn't jump. An empty result latches
    /// `older_history_exhausted` so we don't keep poking the server.
    pub(super) fn poll_pending_older_candles_load(&mut self) {
        let Some(rx) = self.pending_older_candles_load.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(older)) => {
                self.pending_older_candles_load = None;
                if older.instances.is_empty() {
                    self.older_history_exhausted = true;
                    return;
                }
                self.prepend_older_bars(older);
            }
            Ok(Err(msg)) => {
                tracing::warn!(error = %msg, "older candles load failed");
                self.pending_older_candles_load = None;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Still loading; check again next frame.
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.pending_older_candles_load = None;
            }
        }
    }

    /// Merge a chunk of older bars into `raw_data` and shift the camera
    /// by the chunk's length so the user's currently-visible bars stay
    /// pinned. Re-runs aggregation, invalidates indicator caches, and
    /// remaps drawings against the extended series. Mirrors
    /// `pending_symbol_change` like `set_data` so multi-chart sync sees
    /// the new base.
    fn prepend_older_bars(&mut self, older: CandleData) {
        let added = older.instances.len();
        if added == 0 {
            return;
        }

        let mut instances = Vec::with_capacity(added + self.raw_data.instances.len());
        let mut dates = Vec::with_capacity(added + self.raw_data.dates.len());
        instances.extend(older.instances.iter().copied());
        instances.extend(self.raw_data.instances.iter().copied());
        dates.extend(older.dates.iter().cloned());
        dates.extend(self.raw_data.dates.iter().cloned());
        for (i, inst) in instances.iter_mut().enumerate() {
            inst.index = i as f32;
        }
        let merged = CandleData { instances, dates };

        // Pin the user's view: their world-position is at
        // `camera.x_offset` (in index units of the OLD series); after
        // prepending `added` bars every old bar's index gains `added`,
        // so the camera must too for the visible window to stay still.
        {
            let mut camera = self.camera.lock();
            camera.x_offset += added as f64;
        }

        self.earliest_loaded_ts =
            parse_date_to_utc_midnight(merged.dates.first().map(|s| s.as_str()));
        self.pending_symbol_change = Some(merged.clone());
        self.raw_data = Arc::new(merged);
        self.data = Arc::new(self.raw_data.aggregated(self.timeframe));
        self.manager.invalidate_cache();
        self.drawings.remap_to_data(&self.data);
    }

    /// Drain every queued tick event for the active symbol, applying
    /// each one to today's bar in `raw_data`. Re-aggregates + invalidates
    /// indicator caches once at the end so a burst of deltas only pays
    /// the recompute cost once per frame.
    pub(super) fn poll_pending_ticks(&mut self) {
        // Drain into a local Vec first so the borrow on `pending_ticks`
        // is released before we re-borrow `self` for `apply_tick_event`.
        let events: Vec<_> = match self.pending_ticks.as_mut() {
            Some(rx) => std::iter::from_fn(|| rx.try_recv().ok()).collect(),
            None => return,
        };
        let mut applied = false;
        for event in events {
            if self.apply_tick_event(event) {
                applied = true;
            }
        }
        if applied {
            self.data = Arc::new(self.raw_data.aggregated(self.timeframe));
            self.manager.invalidate_cache();
        }
    }

    /// Route one tick event to today's bar. Returns `true` if anything
    /// changed.
    ///
    /// Symbol mismatch (e.g. a buffered event from the previous
    /// subscription arriving after `set_symbol`) is silently dropped.
    /// `TodayEvent` is ignored — for daily charts the running deltas
    /// already update us correctly; the catch-up replay is only useful
    /// for sub-day timeframes that we don't render yet.
    fn apply_tick_event(&mut self, event: zaned_api_client::TickEvent) -> bool {
        use zaned_api_client::TickEvent;

        let event_symbol = match &event {
            TickEvent::SeedEvent(e) => &e.symbol,
            TickEvent::TodayEvent(e) => &e.symbol,
            TickEvent::DeltaEvent(e) => &e.symbol,
            TickEvent::FinalizeEvent(e) => &e.symbol,
        };
        if let Some(active) = self.symbol.as_deref() {
            if active != event_symbol {
                return false;
            }
        } else {
            return false;
        }

        match event {
            TickEvent::DeltaEvent(d) => self.upsert_today_bar(
                d.ts,
                None,
                d.h.map(cents_to_dollars),
                d.l.map(cents_to_dollars),
                Some(cents_to_dollars(d.c)),
                d.v.map(|v| v as f32),
            ),
            TickEvent::SeedEvent(seed) => {
                let Some(c) = seed.candle else { return false };
                self.upsert_today_bar(
                    c.ts,
                    Some(cents_to_dollars(c.o)),
                    Some(cents_to_dollars(c.h)),
                    Some(cents_to_dollars(c.l)),
                    Some(cents_to_dollars(c.c)),
                    Some(c.v as f32),
                )
            }
            TickEvent::FinalizeEvent(fin) => {
                let c = fin.candle;
                self.upsert_today_bar(
                    c.ts,
                    Some(cents_to_dollars(c.o)),
                    Some(cents_to_dollars(c.h)),
                    Some(cents_to_dollars(c.l)),
                    Some(cents_to_dollars(c.c)),
                    Some(c.v as f32),
                )
            }
            TickEvent::TodayEvent(_) => false,
        }
    }

    /// Mutate-or-append the current bar in `raw_data`. "Current" means
    /// today's daily bar on the daily base, or the in-progress 1-min
    /// bar on the minute base. If `raw_data` already ends with the
    /// bucket-key derived from `ts`, mutate the last `CandleInstance`
    /// in place (high/low expand monotonically; close and volume
    /// overwrite). Otherwise append a new bar with index = previous
    /// `len()`. `None` fields are left at their existing value when
    /// mutating, or seeded from `close` when appending.
    ///
    /// Bucket-key format follows the active base scale and matches
    /// what `candle_loader::bars_to_candle_data` produces for fetched
    /// bars, so live ticks line up cleanly with historical rows:
    ///   - `Daily`  → `YYYY-MM-DD` (one bucket per UTC day)
    ///   - `Minute` → `YYYY-MM-DD HH:MM` (one bucket per UTC minute)
    fn upsert_today_bar(
        &mut self,
        ts: i64,
        open: Option<f32>,
        high: Option<f32>,
        low: Option<f32>,
        close: Option<f32>,
        volume: Option<f32>,
    ) -> bool {
        // The server sends `ts` in inconsistent units across event types:
        // `FinalizeEvent.candle.ts` and `SeedEvent.candle.ts` use unix
        // SECONDS (the bar's bucket_start), while `DeltaEvent.ts` uses
        // unix MILLISECONDS (the raw tick timestamp). Normalize: any
        // value past the year-2286 threshold (10 billion seconds since
        // 1970) is milliseconds and must be divided down. Without this,
        // delta events parsed as far-future dates and got appended as
        // ghost candles instead of updating today's bar — making the
        // chart appear frozen between 1-minute boundary finalizes.
        let ts_secs = if ts > 10_000_000_000 { ts / 1000 } else { ts };
        let scale = self.timeframe.base_scale();
        let date = chrono::DateTime::<chrono::Utc>::from_timestamp(ts_secs, 0)
            .map(|dt| match scale {
                BaseScale::Daily => dt.format("%Y-%m-%d").to_string(),
                BaseScale::Minute => dt.format("%Y-%m-%d %H:%M").to_string(),
            })
            .unwrap_or_else(|| {
                let now = chrono::Utc::now();
                match scale {
                    BaseScale::Daily => now.format("%Y-%m-%d").to_string(),
                    BaseScale::Minute => now.format("%Y-%m-%d %H:%M").to_string(),
                }
            });

        let raw = Arc::make_mut(&mut self.raw_data);
        let last_date = raw.dates.last().cloned();

        match last_date.as_deref() {
            Some(last) if last == date => {
                let last_inst = raw
                    .instances
                    .last_mut()
                    .expect("dates non-empty implies instances non-empty");
                if let Some(o) = open {
                    last_inst.open = o;
                }
                if let Some(h) = high {
                    last_inst.high = last_inst.high.max(h);
                }
                if let Some(l) = low {
                    last_inst.low = if last_inst.low > 0.0 {
                        last_inst.low.min(l)
                    } else {
                        l
                    };
                }
                if let Some(c) = close {
                    last_inst.close = c;
                }
                if let Some(v) = volume {
                    last_inst.volume = v;
                }
                true
            }
            None | Some(_) if last_date.as_deref().is_none_or(|l| l < date.as_str()) => {
                // Append a new bar — covers two cases:
                //   1. `raw_data` is empty (no historical bars
                //      returned, e.g. Minute1 against an unpopulated
                //      `bars_1m` table). The first live tick seeds
                //      the series.
                //   2. The current bucket key has advanced past the
                //      last loaded bar (a fresh minute / day rolled
                //      over while we were watching).
                // Seed missing OHL from `close` so the candle has
                // reasonable values until the next event refines them.
                let close_val = close.unwrap_or(0.0);
                let new_index = raw.instances.len() as f32;
                raw.instances.push(candle::CandleInstance {
                    index: new_index,
                    open: open.unwrap_or(close_val),
                    high: high.unwrap_or(close_val),
                    low: low.unwrap_or(close_val),
                    close: close_val,
                    volume: volume.unwrap_or(0.0),
                });
                raw.dates.push(date);
                true
            }
            // Last date is in the future relative to this tick — out-
            // of-order delivery. Ignore rather than rewrite history.
            _ => false,
        }
    }

    /// If the manager is dirty AND it's been at least `DRAWINGS_SAVE_DEBOUNCE`
    /// since the last edit, kick off an async save and clear the flag. Called
    /// once per frame from `show()`.
    pub(super) fn flush_dirty_drawings(&mut self) {
        if !self.drawings.dirty {
            self.drawings_dirty_since = None;
            return;
        }
        let Some(symbol) = self.symbol.clone() else {
            // No symbol = nothing to persist against. Keep the flag set in case
            // a symbol is bound later, but don't write.
            return;
        };
        let now = std::time::Instant::now();
        let since = self.drawings_dirty_since.get_or_insert(now);
        if now.duration_since(*since) >= DRAWINGS_SAVE_DEBOUNCE {
            drawings::persistence::save_all(symbol, self.drawings.committed.clone());
            self.drawings.dirty = false;
            self.drawings_dirty_since = None;
        }
    }

    /// Wipe every committed drawing on this chart (in-memory + DB).
    /// Triggered by the toolbar's trash icon after the user confirms.
    pub fn clear_all_drawings(&mut self) {
        self.drawings.clear_all();
        if let Some(symbol) = &self.symbol {
            drawings::persistence::delete_all(symbol.clone());
        }
        // We've already deleted from the DB; suppress the debounced save that
        // `clear_all` would otherwise schedule (it would be a redundant DELETE +
        // empty-set INSERT).
        self.drawings.dirty = false;
        self.drawings_dirty_since = None;
    }

    pub fn has_user_drawings(&self) -> bool {
        !self.drawings.committed.is_empty()
    }
}
