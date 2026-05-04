//! Redis-backed state for the in-progress candle, live delta stream, and
//! today's finalized bars.
//!
//! All mutating operations on `tick:current:{symbol}` go through one Lua
//! script (`apply_tick`) so that "compute new bucket / extend high-low /
//! recompute volume / append to stream" is atomic — no two ticks can race
//! and produce inconsistent state.

use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use redis::{AsyncCommands, Script, aio::ConnectionManager};
use serde::{Deserialize, Serialize};

use super::protocol::BarPayload;

pub fn current_key(symbol: &str) -> String {
    format!("tick:current:{symbol}")
}

pub fn updates_key(symbol: &str) -> String {
    format!("tick:updates:{symbol}")
}

/// `tick:bars:{YYYY-MM-DD}:{symbol}` — date is the trading day in US/Eastern.
/// Switching at the ET midnight boundary keeps a session's bars in one key
/// even though our timestamps are UTC.
pub fn today_bars_key(symbol: &str, ts: DateTime<Utc>) -> String {
    let et = ts.with_timezone(&New_York).date_naive();
    format!("tick:bars:{}:{symbol}", et)
}

/// Output of `apply_tick`. Mirrors the candle state after this tick was
/// folded in. Used by the WS server when it needs to push an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyResult {
    pub bucket_start: i64,
    pub open: i32,
    pub high: i32,
    pub low: i32,
    pub close: i32,
    pub volume: i64,
    /// True if this tick caused a bucket transition. If true, the *previous*
    /// bucket should be finalized (it was just popped out of `tick:current`).
    pub bucket_rolled_over: bool,
}

/// Lua script body. Runs in a single round-trip per tick.
///
/// KEYS[1] = tick:current:{symbol}
/// KEYS[2] = tick:updates:{symbol}
/// KEYS[3] = tick:bars:{YYYY-MM-DD}:{symbol}  -- for the *prior* bar's date
/// ARGV[1] = bucket_start (unix seconds, minute boundary)
/// ARGV[2] = price (cents, integer)
/// ARGV[3] = day_volume (integer)
/// ARGV[4] = tick_ts (unix ms)
/// ARGV[5] = updates_maxlen (positive integer)
/// ARGV[6] = today_bars_ttl_secs
///
/// Returns: { bucket_start, open, high, low, close, volume, rolled_over_int }
const APPLY_TICK_SCRIPT: &str = r#"
local current = redis.call('HGET', KEYS[1], 'bucket_start')
local bucket = tonumber(ARGV[1])
local price = tonumber(ARGV[2])
local day_vol = tonumber(ARGV[3])
local ts = ARGV[4]
local maxlen = tonumber(ARGV[5])
local ttl = tonumber(ARGV[6])

-- Drop late ticks for an already-finalized bucket. The boundary sweeper
-- (and prior bucket-roll finalizes) write into KEYS[3] keyed by bucket_start,
-- so HEXISTS is a definitive "this minute is sealed" signal. Without this
-- check, a late Yahoo tick for the just-finalized minute would re-create
-- tick:current and trigger a spurious second `finalize` on the next roll.
if redis.call('HEXISTS', KEYS[3], bucket) == 1 then
    return { bucket, 0, 0, 0, 0, 0, 0 }
end

local rolled = 0
if (not current) or (tonumber(current) ~= bucket) then
    -- Yahoo sometimes delivers ticks out of order across minute boundaries.
    -- If this tick is for an older bucket than the one currently in flight,
    -- drop it — same reasoning as the HEXISTS check above.
    if current and bucket < tonumber(current) then
        return { tonumber(current), 0, 0, 0, 0, 0, 0 }
    end
    if current then
        local prev_bucket = tonumber(current)
        -- Only finalize the prior bar if it isn't already in today-bars.
        -- Guards against the boundary sweeper having finalized first.
        if redis.call('HEXISTS', KEYS[3], prev_bucket) == 0 then
            rolled = 1
            local prev_open  = tonumber(redis.call('HGET', KEYS[1], 'open'))
            local prev_high  = tonumber(redis.call('HGET', KEYS[1], 'high'))
            local prev_low   = tonumber(redis.call('HGET', KEYS[1], 'low'))
            local prev_close = tonumber(redis.call('HGET', KEYS[1], 'close'))
            local prev_vol   = tonumber(redis.call('HGET', KEYS[1], 'volume'))
            local bar_json = string.format(
                '{"ts":%d,"o":%d,"h":%d,"l":%d,"c":%d,"v":%d}',
                prev_bucket, prev_open, prev_high, prev_low, prev_close, prev_vol)
            redis.call('HSET', KEYS[3], prev_bucket, bar_json)
            redis.call('EXPIRE', KEYS[3], ttl)
            redis.call('XADD', KEYS[2], 'MAXLEN', '~', maxlen, '*',
                'type', 'finalize',
                'ts', prev_bucket,
                'o', prev_open, 'h', prev_high, 'l', prev_low,
                'c', prev_close, 'v', prev_vol)
        end
    end
    redis.call('HSET', KEYS[1],
        'bucket_start', bucket,
        'open', price, 'high', price, 'low', price, 'close', price,
        'volume', 0, 'volume_anchor', day_vol, 'last_tick_ts', ts)
    redis.call('XADD', KEYS[2], 'MAXLEN', '~', maxlen, '*',
        'type', 'delta',
        'open', price, 'high', price, 'low', price, 'close', price,
        'volume', 0, 'ts', ts)
    return { bucket, price, price, price, price, 0, rolled }
end

-- same bucket: extend
local high = tonumber(redis.call('HGET', KEYS[1], 'high'))
local low = tonumber(redis.call('HGET', KEYS[1], 'low'))
local anchor = tonumber(redis.call('HGET', KEYS[1], 'volume_anchor'))

if price > high then high = price end
if price < low then low = price end

-- volume = day_volume_now - volume_anchor.
-- If Yahoo's day_volume regressed (rare; happens at session boundaries when
-- we cross from post-market into next-day pre-market), reset the anchor so
-- volume can never go negative.
local volume = day_vol - anchor
if volume < 0 then
    redis.call('HSET', KEYS[1], 'volume_anchor', day_vol)
    anchor = day_vol
    volume = 0
end

redis.call('HSET', KEYS[1],
    'high', high, 'low', low, 'close', price,
    'volume', volume, 'last_tick_ts', ts)

local open = tonumber(redis.call('HGET', KEYS[1], 'open'))

redis.call('XADD', KEYS[2], 'MAXLEN', '~', maxlen, '*',
    'type', 'delta',
    'high', high, 'low', low, 'close', price,
    'volume', volume, 'ts', ts)

return { bucket, open, high, low, price, volume, rolled }
"#;

const CLEAR_IF_BUCKET_SCRIPT: &str = r#"
local current = redis.call('HGET', KEYS[1], 'bucket_start')
if current and tonumber(current) == tonumber(ARGV[1]) then
    redis.call('DEL', KEYS[1])
    return 1
end
return 0
"#;

#[derive(Clone)]
pub struct RedisState {
    conn: ConnectionManager,
    apply_tick: Script,
    clear_if_bucket: Script,
    updates_maxlen: u64,
    today_bars_ttl_secs: u64,
}

impl RedisState {
    pub async fn connect(
        url: &str,
        updates_maxlen: u64,
        today_bars_ttl_secs: u64,
    ) -> Result<Self> {
        let client = redis::Client::open(url).context("open redis client")?;
        // redis 1.x defaults: 500ms response_timeout, 1s connection_timeout —
        // way too aggressive for a WAN-hosted Redis. Override generously so
        // batch HSETs and the occasional slow auth handshake don't trip.
        let cm_config = redis::aio::ConnectionManagerConfig::new()
            .set_connection_timeout(Some(std::time::Duration::from_secs(10)))
            .set_response_timeout(Some(std::time::Duration::from_secs(30)));
        let conn = client
            .get_connection_manager_with_config(cm_config)
            .await
            .context("redis connection manager")?;
        Ok(Self {
            conn,
            apply_tick: Script::new(APPLY_TICK_SCRIPT),
            clear_if_bucket: Script::new(CLEAR_IF_BUCKET_SCRIPT),
            updates_maxlen,
            today_bars_ttl_secs,
        })
    }

    /// A shareable clone of the underlying ConnectionManager. Useful for
    /// modules that need to issue commands not covered by the high-level
    /// API (e.g. `XREAD BLOCK` in the WS server).
    pub fn connection(&self) -> ConnectionManager {
        self.conn.clone()
    }

    /// Apply one Yahoo tick to the symbol's in-progress candle, atomically.
    /// On bucket transition the prior bar is persisted to today-bars and a
    /// `finalize` stream event is emitted — all inside the Lua script.
    pub async fn apply_tick(
        &self,
        symbol: &str,
        bucket_start: i64,
        price: i32,
        day_volume: i64,
        tick_ts_ms: i64,
    ) -> Result<ApplyResult> {
        let mut conn = self.conn.clone();
        let bucket_ts = Utc.timestamp_opt(bucket_start, 0).single().ok_or_else(|| {
            anyhow::anyhow!("invalid bucket_start {} for {symbol}", bucket_start)
        })?;
        let bars_key = today_bars_key(symbol, bucket_ts);
        let result: Vec<i64> = self
            .apply_tick
            .key(current_key(symbol))
            .key(updates_key(symbol))
            .key(bars_key)
            .arg(bucket_start)
            .arg(price as i64)
            .arg(day_volume)
            .arg(tick_ts_ms)
            .arg(self.updates_maxlen as i64)
            .arg(self.today_bars_ttl_secs as i64)
            .invoke_async(&mut conn)
            .await
            .with_context(|| format!("apply_tick for {symbol}"))?;
        if result.len() != 7 {
            anyhow::bail!("apply_tick returned {} fields, expected 7", result.len());
        }
        Ok(ApplyResult {
            bucket_start: result[0],
            open: result[1] as i32,
            high: result[2] as i32,
            low: result[3] as i32,
            close: result[4] as i32,
            volume: result[5],
            bucket_rolled_over: result[6] != 0,
        })
    }

    /// Read the current in-progress candle, if one exists.
    pub async fn get_current(&self, symbol: &str) -> Result<Option<BarPayload>> {
        let mut conn = self.conn.clone();
        let h: std::collections::HashMap<String, String> = conn
            .hgetall(current_key(symbol))
            .await
            .with_context(|| format!("HGETALL {symbol}"))?;
        if h.is_empty() {
            return Ok(None);
        }
        let bucket_start: i64 = h
            .get("bucket_start")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("missing bucket_start in {symbol} hash"))?;
        let parse = |k: &str| -> Result<i64> {
            h.get(k)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| anyhow::anyhow!("missing {k} in {symbol} hash"))
        };
        Ok(Some(BarPayload {
            ts: bucket_start,
            o: parse("open")? as i32,
            h: parse("high")? as i32,
            l: parse("low")? as i32,
            c: parse("close")? as i32,
            v: parse("volume")?,
        }))
    }

    /// Persist a finalized bar into today's hash (field = bucket_start, value
    /// = JSON) and emit a `finalize` stream event. The TTL is refreshed on
    /// every write. Using a hash (vs a sorted set keyed by JSON value)
    /// guarantees one entry per bucket — late or revised data overwrites the
    /// existing field rather than appending a duplicate.
    pub async fn finalize_bar(&self, symbol: &str, bar: BarPayload) -> Result<()> {
        let mut conn = self.conn.clone();
        let bar_json = serde_json::to_string(&bar)?;
        let ts = Utc.timestamp_opt(bar.ts, 0).single().ok_or_else(|| {
            anyhow::anyhow!("invalid bucket_start {} for {symbol}", bar.ts)
        })?;
        let bars_key = today_bars_key(symbol, ts);
        redis::pipe()
            .atomic()
            .hset(&bars_key, bar.ts, &bar_json)
            .expire(&bars_key, self.today_bars_ttl_secs as i64)
            .xadd_maxlen(
                updates_key(symbol),
                redis::streams::StreamMaxlen::Approx(self.updates_maxlen as usize),
                "*",
                &[
                    ("type", "finalize"),
                    ("ts", &bar.ts.to_string()),
                    ("o", &bar.o.to_string()),
                    ("h", &bar.h.to_string()),
                    ("l", &bar.l.to_string()),
                    ("c", &bar.c.to_string()),
                    ("v", &bar.v.to_string()),
                ],
            )
            .query_async::<()>(&mut conn)
            .await
            .with_context(|| format!("finalize_bar for {symbol}"))?;
        Ok(())
    }

    /// Read every finalized bar in today's session, sorted ascending by ts.
    pub async fn get_today_bars(&self, symbol: &str) -> Result<Vec<BarPayload>> {
        let mut conn = self.conn.clone();
        let key = today_bars_key(symbol, Utc::now());
        let entries: std::collections::HashMap<String, String> = conn
            .hgetall(&key)
            .await
            .with_context(|| format!("HGETALL {key}"))?;
        let mut out = Vec::with_capacity(entries.len());
        for (_field, json) in entries {
            match serde_json::from_str::<BarPayload>(&json) {
                Ok(b) => out.push(b),
                Err(err) => {
                    tracing::warn!(symbol, error = %err, "skipping malformed bar in today's hash");
                }
            }
        }
        out.sort_by_key(|b| b.ts);
        Ok(out)
    }

    /// Remove the in-progress candle hash only if it still holds the expected
    /// bucket. Guards against racing with the aggregator: if a new tick has
    /// already rolled the bucket forward, we leave the fresh state intact.
    /// Returns `true` if the key was deleted, `false` if it was already gone
    /// or held a different bucket.
    pub async fn clear_current_if_bucket(
        &self,
        symbol: &str,
        expected_bucket_start: i64,
    ) -> Result<bool> {
        let mut conn = self.conn.clone();
        let result: i64 = self
            .clear_if_bucket
            .key(current_key(symbol))
            .arg(expected_bucket_start)
            .invoke_async(&mut conn)
            .await
            .with_context(|| format!("clear_current_if_bucket for {symbol}"))?;
        Ok(result == 1)
    }

    /// Add `symbol` to the `tick:active` Redis SET (persists across restarts).
    pub async fn add_active(&self, symbol: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let _: () = conn.sadd("tick:active", symbol).await?;
        Ok(())
    }

    /// Remove `symbol` from the `tick:active` Redis SET.
    pub async fn remove_active(&self, symbol: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let _: () = conn.srem("tick:active", symbol).await?;
        Ok(())
    }

    /// Fetch all members of `tick:active`.
    pub async fn get_active(&self) -> Result<HashSet<String>> {
        let mut conn = self.conn.clone();
        let members: HashSet<String> = conn.smembers("tick:active").await?;
        Ok(members)
    }

    /// Clear in-progress candle hash and delta stream for an evicted symbol.
    /// Does NOT touch `tick:bars` — TTL handles that.
    pub async fn clear_symbol_state(&self, symbol: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let _: () = redis::pipe()
            .atomic()
            .del(current_key(symbol))
            .del(updates_key(symbol))
            .query_async(&mut conn)
            .await
            .with_context(|| format!("clear_symbol_state for {symbol}"))?;
        Ok(())
    }

    /// Idempotent batch seed: write many finalized bars into today's hash
    /// in one pipeline, with a single TTL refresh at the end. Each bar is
    /// HSET with field = bucket_start so re-seeding overwrites rather than
    /// duplicating. No-op for empty input.
    pub async fn seed_today_bars(&self, symbol: &str, bars: &[BarPayload]) -> Result<()> {
        if bars.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.clone();
        let first_ts = Utc.timestamp_opt(bars[0].ts, 0).single().ok_or_else(|| {
            anyhow::anyhow!("invalid bucket_start {} for {symbol}", bars[0].ts)
        })?;
        let bars_key = today_bars_key(symbol, first_ts);
        let mut pipe = redis::pipe();
        pipe.atomic();
        for bar in bars {
            let bar_json = serde_json::to_string(bar)?;
            pipe.hset(&bars_key, bar.ts, bar_json).ignore();
        }
        pipe.expire(&bars_key, self.today_bars_ttl_secs as i64).ignore();
        pipe.query_async::<()>(&mut conn)
            .await
            .with_context(|| format!("seed_today_bars for {symbol}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_key_format() {
        assert_eq!(current_key("AAPL"), "tick:current:AAPL");
    }

    #[test]
    fn updates_key_format() {
        assert_eq!(updates_key("AAPL"), "tick:updates:AAPL");
    }

    #[test]
    fn today_bars_key_uses_eastern_date() {
        // 2024-01-03 03:00 UTC = 2024-01-02 22:00 ET → uses 2024-01-02
        let ts = chrono::Utc
            .with_ymd_and_hms(2024, 1, 3, 3, 0, 0)
            .unwrap();
        assert_eq!(today_bars_key("AAPL", ts), "tick:bars:2024-01-02:AAPL");
        // 2024-01-03 14:30 UTC = 2024-01-03 09:30 ET → uses 2024-01-03
        let ts2 = chrono::Utc
            .with_ymd_and_hms(2024, 1, 3, 14, 30, 0)
            .unwrap();
        assert_eq!(today_bars_key("AAPL", ts2), "tick:bars:2024-01-03:AAPL");
    }
}
