//! Boot the tick-service subsystem: Redis state, Subscriptions, coordinator,
//! boundary sweeper, and the axum WS Router. The host process (zaned-server's
//! `main`) owns env loading, tracing init, and the TCP listener; this module
//! returns the pieces it needs to mount and a handle for graceful shutdown.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::boundary;
use super::config::Config;
use super::coordinator::{self, Coordinator};
use super::redis_state::RedisState;
use super::subscriptions::Subscriptions;
use super::ws_server::{ServerState, router};

/// Handles for the spawned tick-service tasks. The caller (host process)
/// can `.abort()` these on shutdown.
pub struct TickServiceHandles {
    pub coordinator: JoinHandle<()>,
    pub boundary: JoinHandle<()>,
}

impl TickServiceHandles {
    pub fn abort_all(self) {
        self.coordinator.abort();
        self.boundary.abort();
    }
}

/// Initialize the tick-service from a `Config`, spawn its background tasks,
/// and return the axum Router that should be merged into the host server's
/// router (it adds `/ws`).
pub async fn start(cfg: &Config) -> Result<(axum::Router, TickServiceHandles)> {
    let redis = Arc::new(
        RedisState::connect(
            &cfg.redis_url,
            cfg.updates_stream_maxlen,
            cfg.today_bars_ttl_secs,
        )
        .await?,
    );

    let subs = Arc::new(Mutex::new(Subscriptions::new(cfg.max_warm_symbols as usize)));
    let (cmd_tx, cmd_rx) = coordinator::make_command_channel();

    let mut coord = Coordinator::new(redis.clone(), subs.clone(), cmd_rx);
    if let Err(e) = coord.rehydrate_from_redis().await {
        tracing::warn!(error = %e, "rehydrate failed; starting cold");
    }
    let coordinator_handle = tokio::spawn(async move {
        if let Err(e) = coord.run().await {
            tracing::error!(error = %e, "coordinator exited with error");
        }
    });

    let boundary_redis = redis.clone();
    let boundary_subs = subs.clone();
    let boundary_handle = tokio::spawn(async move {
        if let Err(e) = boundary::run(boundary_redis, boundary_subs).await {
            tracing::error!(error = %e, "boundary sweeper exited with error");
        }
    });

    let server_state = ServerState {
        redis: redis.clone(),
        redis_url: cfg.redis_url.clone(),
        cmd_tx: cmd_tx.clone(),
    };

    Ok((
        router(server_state),
        TickServiceHandles {
            coordinator: coordinator_handle,
            boundary: boundary_handle,
        },
    ))
}
