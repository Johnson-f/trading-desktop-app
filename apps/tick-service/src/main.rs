mod boundary;
mod config;
mod coordinator;
mod protocol;
mod redis_state;
mod subscriptions;
mod ws_server;

fn redact_redis_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let scheme = &url[..scheme_end + 3];
        let rest = &url[scheme_end + 3..];
        if let Some(at) = rest.rfind('@') {
            return format!("{scheme}***@{}", &rest[at + 1..]);
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_redis_url_scrubs_credentials() {
        assert_eq!(
            redact_redis_url("redis://:secret@host:6379"),
            "redis://***@host:6379"
        );
        assert_eq!(
            redact_redis_url("redis://user:pass@host:6379/0"),
            "redis://***@host:6379/0"
        );
    }

    #[test]
    fn redact_redis_url_passes_through_no_credentials() {
        assert_eq!(
            redact_redis_url("redis://host:6379"),
            "redis://host:6379"
        );
        assert_eq!(
            redact_redis_url("redis://localhost"),
            "redis://localhost"
        );
    }
}

use std::sync::Arc;

use anyhow::Result;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::coordinator::Coordinator;
use crate::redis_state::RedisState;
use crate::subscriptions::Subscriptions;
use crate::ws_server::{ServerState, router};

#[tokio::main]
async fn main() -> Result<()> {
    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&crate_env).is_err() {
        let _ = dotenvy::dotenv();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tick_service=info,markets=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!(
        redis = %redact_redis_url(&cfg.redis_url),
        max_warm_symbols = cfg.max_warm_symbols,
        ws_bind = %cfg.ws_bind,
        "tick-service starting"
    );

    // 1. Connect Redis.
    let redis = Arc::new(
        RedisState::connect(
            &cfg.redis_url,
            cfg.updates_stream_maxlen,
            cfg.today_bars_ttl_secs,
        )
        .await?,
    );

    // 2. Subscriptions + command channel.
    let subs = Arc::new(Mutex::new(Subscriptions::new(
        cfg.max_warm_symbols as usize,
    )));
    let (cmd_tx, cmd_rx) = coordinator::make_command_channel();

    // 3. Build coordinator and rehydrate from Redis.
    let mut coord = Coordinator::new(redis.clone(), subs.clone(), cmd_rx);
    if let Err(e) = coord.rehydrate_from_redis().await {
        tracing::warn!(error = %e, "rehydrate failed; starting cold");
    }
    let coord_handle = tokio::spawn(async move {
        if let Err(e) = coord.run().await {
            tracing::error!(error = %e, "coordinator exited with error");
        }
    });

    // 4. Boundary sweeper using shared subs.
    let boundary_redis = redis.clone();
    let boundary_subs = subs.clone();
    let boundary_handle = tokio::spawn(async move {
        if let Err(e) = boundary::run(boundary_redis, boundary_subs).await {
            tracing::error!(error = %e, "boundary sweeper exited with error");
        }
    });

    // 5. WS server (now also gets cmd_tx).
    let server_state = ServerState {
        redis: redis.clone(),
        redis_url: cfg.redis_url.clone(),
        cmd_tx: cmd_tx.clone(),
    };
    let app = router(server_state);
    let listener = tokio::net::TcpListener::bind(&cfg.ws_bind).await?;
    tracing::info!(bind = %cfg.ws_bind, "ws server listening");
    let ws_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error = %e, "ws server exited with error");
        }
    });

    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| anyhow::anyhow!("install SIGTERM handler: {e}"))?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("SIGINT received; shutting down");
        }
        _ = sigterm.recv() => {
            tracing::info!("SIGTERM received; shutting down");
        }
    }

    coord_handle.abort();
    boundary_handle.abort();
    ws_handle.abort();
    Ok(())
}
