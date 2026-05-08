//! Axum WebSocket endpoint at `/ws`.
//!
//! Each client connection runs one tokio task. The task multiplexes
//! per-symbol XREAD against Redis streams: when the client subscribes to N
//! symbols, the task issues one XREAD that reads all N streams concurrently.
//! Subscription changes restart the XREAD with the new symbol set.
//!
//! On subscribe the coordinator is notified (it admits the symbol into the
//! `PriceStream` + persists to `tick:active`). On disconnect (or unsubscribe)
//! the coordinator is notified to decrement the ref count.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};
use tokio::sync::oneshot;

use super::coordinator::CoordCmd;
use super::protocol::{BarPayload, ClientMsg, ServerMsg};
use super::redis_state::{RedisState, updates_key};

#[derive(Clone)]
pub struct ServerState {
    pub redis: Arc<RedisState>,
    /// URL used to open a dedicated XREAD connection per WS handler.
    /// XREAD BLOCK doesn't play well with the shared ConnectionManager — it
    /// ties up the multiplexed cmd queue and starves other commands.
    pub redis_url: String,
    pub cmd_tx: tokio::sync::mpsc::Sender<CoordCmd>,
}

pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<ServerState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: ServerState) {
    tracing::info!("ws: connection upgraded");
    let (mut sink, mut stream) = socket.split();

    let mut cursors: HashMap<String, String> = HashMap::new();

    // Open a DEDICATED multiplexed connection just for this socket's XREAD
    // BLOCK loop. Sharing the service's ConnectionManager would tie up the
    // multiplexed command queue with the BLOCK and starve other commands.
    //
    // Override the redis 1.x defaults (500ms response, 1s connect) which
    // are far too aggressive for WAN: an in-flight `XREAD BLOCK 5000` is
    // cancelled by the connection layer at 500ms and surfaces as
    // `xread_failed: timed out`.
    let client = match redis::Client::open(state.redis_url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "ws: redis::Client::open failed");
            let _ = send_error(&mut sink, "redis_unavailable", &e.to_string()).await;
            return;
        }
    };
    let conn_cfg = redis::AsyncConnectionConfig::new()
        .set_connection_timeout(Some(std::time::Duration::from_secs(10)))
        .set_response_timeout(Some(std::time::Duration::from_secs(30)));
    let xread_conn = match client
        .get_multiplexed_async_connection_with_config(&conn_cfg)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "ws: failed to open xread connection");
            let _ = send_error(&mut sink, "redis_unavailable", &e.to_string()).await;
            return;
        }
    };
    let xread_conn = Arc::new(tokio::sync::Mutex::new(xread_conn));

    let (ctrl_tx, mut ctrl_rx) = tokio::sync::mpsc::channel::<ClientMsg>(32);

    tokio::spawn(async move {
        while let Some(res) = stream.next().await {
            match res {
                Ok(Message::Text(text)) => {
                    tracing::info!(len = text.len(), "ws: text frame received");
                    match serde_json::from_str::<ClientMsg>(&text) {
                        Ok(parsed) => {
                            tracing::info!("ws: parsed client message ok");
                            if ctrl_tx.send(parsed).await.is_err() {
                                tracing::warn!("ws: ctrl_tx send failed; reader exiting");
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, payload = %&*text, "ws: bad client frame");
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("ws: close frame received");
                    break;
                }
                Ok(other) => {
                    tracing::debug!(?other, "ws: non-text frame ignored");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ws: stream error");
                    break;
                }
            }
        }
        tracing::info!("ws: reader task exited");
    });

    loop {
        let symbols: Vec<String> = cursors.keys().cloned().collect();
        let stream_keys: Vec<String> = symbols.iter().map(|s| updates_key(s)).collect();
        let stream_ids: Vec<String> = symbols.iter().map(|s| cursors[s].clone()).collect();

        if symbols.is_empty() {
            match ctrl_rx.recv().await {
                Some(msg) => {
                    handle_control(&state, &xread_conn, &mut cursors, &mut sink, msg).await;
                }
                None => break,
            }
            continue;
        }

        let xread_fut = {
            let conn = xread_conn.clone();
            let stream_keys = stream_keys.clone();
            let stream_ids = stream_ids.clone();
            async move {
                let mut conn = conn.lock().await;
                // 5s block — fewer wakeups, less stress on the shared
                // ConnectionManager when running over WAN to remote Redis.
                let opts = StreamReadOptions::default().block(5000);
                let result: redis::RedisResult<Option<StreamReadReply>> =
                    conn.xread_options(&stream_keys, &stream_ids, &opts).await;
                result
            }
        };

        tokio::select! {
            biased;

            ctrl = ctrl_rx.recv() => {
                match ctrl {
                    Some(msg) => handle_control(&state, &xread_conn, &mut cursors, &mut sink, msg).await,
                    None => break,
                }
            }

            xread = xread_fut => {
                match xread {
                    Ok(Some(reply)) => {
                        for stream in reply.keys {
                            let symbol = stream.key
                                .strip_prefix("tick:updates:")
                                .unwrap_or(&stream.key)
                                .to_string();
                            for entry in stream.ids {
                                let server_msg =
                                    parse_stream_entry(&symbol, &entry.id, &entry.map);
                                if let Some(msg) = server_msg {
                                    if send_msg(&mut sink, &msg).await.is_err() {
                                        // Connection dropped — send unsubscribes for cleanup.
                                        cleanup_cursors(&state, &cursors).await;
                                        return;
                                    }
                                }
                                cursors.insert(symbol.clone(), entry.id.clone());
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "xread failed");
                        let _ = send_error(&mut sink, "xread_failed", &e.to_string()).await;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }
    }

    // Notify coordinator to decrement ref counts for all remaining symbols.
    cleanup_cursors(&state, &cursors).await;
}

/// Send `Unsubscribe` to the coordinator for every symbol in `cursors`.
async fn cleanup_cursors(state: &ServerState, cursors: &HashMap<String, String>) {
    for symbol in cursors.keys() {
        let _ = state
            .cmd_tx
            .send(CoordCmd::Unsubscribe {
                symbol: symbol.clone(),
            })
            .await;
    }
}

async fn handle_control(
    state: &ServerState,
    xread_conn: &Arc<tokio::sync::Mutex<redis::aio::MultiplexedConnection>>,
    cursors: &mut HashMap<String, String>,
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    msg: ClientMsg,
) {
    match msg {
        ClientMsg::Subscribe { symbols } => {
            for symbol in symbols {
                if cursors.contains_key(&symbol) {
                    continue;
                }
                // Tell coordinator to admit (adds to PriceStream + seeds today).
                let (tx, rx) = oneshot::channel();
                let _ = state
                    .cmd_tx
                    .send(CoordCmd::Subscribe {
                        symbol: symbol.clone(),
                        resp: tx,
                    })
                    .await;
                // Wait for admit to complete (PriceStream subscription is live).
                let _ = rx.await;

                let (current, last_id) = snapshot(state, xread_conn, &symbol)
                    .await
                    .unwrap_or((None, "0".to_string()));
                let _ = send_msg(
                    sink,
                    &ServerMsg::Seed {
                        symbol: symbol.clone(),
                        candle: current,
                    },
                )
                .await;
                let today = state
                    .redis
                    .get_today_bars(&symbol)
                    .await
                    .unwrap_or_default();
                let _ = send_msg(
                    sink,
                    &ServerMsg::Today {
                        symbol: symbol.clone(),
                        bars: today,
                    },
                )
                .await;
                cursors.insert(symbol, last_id);
            }
        }
        ClientMsg::Unsubscribe { symbols } => {
            for s in &symbols {
                let _ = state
                    .cmd_tx
                    .send(CoordCmd::Unsubscribe { symbol: s.clone() })
                    .await;
                cursors.remove(s);
            }
        }
        ClientMsg::Ping => {
            let _ = send_msg(sink, &ServerMsg::Pong).await;
        }
    }
}

async fn snapshot(
    state: &ServerState,
    xread_conn: &Arc<tokio::sync::Mutex<redis::aio::MultiplexedConnection>>,
    symbol: &str,
) -> Option<(Option<BarPayload>, String)> {
    let candle = state.redis.get_current(symbol).await.ok().flatten();
    let mut conn = xread_conn.lock().await;
    let entries: Vec<(String, Vec<(String, String)>)> = conn
        .xrevrange_count(updates_key(symbol), "+", "-", 1)
        .await
        .ok()?;
    let last_id = entries
        .first()
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "0".to_string());
    Some((candle, last_id))
}

fn parse_stream_entry(
    symbol: &str,
    _entry_id: &str,
    map: &HashMap<String, redis::Value>,
) -> Option<ServerMsg> {
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
        "delta" => Some(ServerMsg::Delta {
            symbol: symbol.to_string(),
            ts: parse_i64("ts").unwrap_or(0),
            c: parse_i32("close").unwrap_or(0),
            h: parse_i32("high"),
            l: parse_i32("low"),
            v: parse_i64("volume"),
        }),
        "finalize" => {
            let bar = BarPayload {
                ts: parse_i64("ts").unwrap_or(0),
                o: parse_i32("o").unwrap_or(0),
                h: parse_i32("h").unwrap_or(0),
                l: parse_i32("l").unwrap_or(0),
                c: parse_i32("c").unwrap_or(0),
                v: parse_i64("v").unwrap_or(0),
            };
            Some(ServerMsg::Finalize {
                symbol: symbol.to_string(),
                candle: bar,
            })
        }
        _ => None,
    }
}

async fn send_msg(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    msg: &ServerMsg,
) -> Result<()> {
    let json = serde_json::to_string(msg)?;
    sink.send(Message::Text(json.into())).await?;
    Ok(())
}

async fn send_error(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    code: &str,
    message: &str,
) -> Result<()> {
    send_msg(
        sink,
        &ServerMsg::Error {
            code: code.to_string(),
            message: message.to_string(),
        },
    )
    .await
}
