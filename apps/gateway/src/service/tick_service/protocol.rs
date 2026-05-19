//! WebSocket protocol between desktop clients and tick-service.
//!
//! All frames are JSON. The desktop sends `ClientMsg`, the server sends
//! `ServerMsg`. Both are externally-tagged enums via `#[serde(tag = "type")]`
//! so a wire payload looks like `{"type":"subscribe","symbols":["AAPL"]}`.
//!
//! Prices are `i32` cents (matching historical-service's `Bar`). The desktop
//! divides by 100.0 to render. Volume is `i64` (Yahoo's `day_volume` can run
//! into the billions for high-volume days, beyond u32 range).

use serde::{Deserialize, Serialize};

/// Compact bar payload. Used in `Seed`, `Today`, and `Finalize` server frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarPayload {
    /// Minute-boundary unix seconds (UTC).
    pub ts: i64,
    /// Open price (cents).
    pub o: i32,
    /// High price (cents).
    pub h: i32,
    /// Low price (cents).
    pub l: i32,
    /// Close price (cents).
    pub c: i32,
    /// Accumulated bucket volume.
    pub v: i64,
}

/// Frames sent by the desktop client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Add `symbols` to this connection's active subscription set.
    Subscribe { symbols: Vec<String> },
    /// Remove `symbols` from this connection's active subscription set.
    Unsubscribe { symbols: Vec<String> },
    /// Heartbeat — server replies with `Pong`.
    Ping,
}

/// Frames sent by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// One-time delivery on subscribe: the current in-progress candle for
    /// the symbol, if any. May be `None` for cold symbols whose first tick
    /// hasn't landed yet.
    Seed {
        symbol: String,
        candle: Option<BarPayload>,
    },
    /// One-time delivery on subscribe: every finalized 1-min bar for the
    /// symbol from this trading session up to the latest finalized minute.
    /// Empty if there's no today data yet (e.g. before the session opens).
    Today {
        symbol: String,
        bars: Vec<BarPayload>,
    },
    /// Live update inside the current minute. The server only emits fields
    /// that changed since the last delta — so `high` / `low` are `None`
    /// most of the time. The client merges into its in-progress candle.
    Delta {
        symbol: String,
        /// Tick timestamp, **Unix milliseconds** (matches Yahoo's tick `time`
        /// field). Note this is a different unit than `BarPayload.ts` which is
        /// minute-boundary unix seconds. Truncate via `(ts / 60_000) * 60` to
        /// recover the bucket start (in seconds).
        ts: i64,
        c: i32,
        h: Option<i32>,
        l: Option<i32>,
        v: Option<i64>,
    },
    /// Minute boundary crossed: this candle is now finalized. The client
    /// locks it in (full opacity) and starts rendering the next minute.
    Finalize { symbol: String, candle: BarPayload },
    /// Reply to `Ping`.
    Pong,
    /// Server-side error — the connection stays open; the client may retry.
    Error { code: String, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_subscribe_round_trips() {
        let msg = ClientMsg::Subscribe {
            symbols: vec!["AAPL".to_string(), "MSFT".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"subscribe\""));
        assert!(json.contains("AAPL"));
        let parsed: ClientMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn server_seed_with_candle_round_trips() {
        let msg = ServerMsg::Seed {
            symbol: "AAPL".to_string(),
            candle: Some(BarPayload {
                ts: 1_700_000_000,
                o: 18422,
                h: 18450,
                l: 18415,
                c: 18438,
                v: 14523,
            }),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn server_seed_with_no_candle_round_trips() {
        let msg = ServerMsg::Seed {
            symbol: "ZZZZ".to_string(),
            candle: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn server_delta_omits_unchanged_fields() {
        let msg = ServerMsg::Delta {
            symbol: "AAPL".to_string(),
            ts: 1_700_000_000_000,
            c: 18438,
            h: None,
            l: None,
            v: Some(14523),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"h\":null"));
        let parsed: ServerMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn server_finalize_round_trips() {
        let msg = ServerMsg::Finalize {
            symbol: "AAPL".to_string(),
            candle: BarPayload {
                ts: 1_700_000_000,
                o: 18422,
                h: 18450,
                l: 18415,
                c: 18438,
                v: 14523,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"finalize\""));
        let parsed: ServerMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
