mod config;
mod filter;
mod scheduler;
mod state;
mod sync;
mod typesense;

use anyhow::Result;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::Config;
use crate::scheduler::wait_until_next_run;
use crate::state::RedisJobState;
use crate::sync::{run_sync, YahooScreener};
use crate::typesense::TypesenseClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present. Try this binary's own crate dir first (so
    // `cargo run -p symbol-service` from any CWD works), then fall back
    // to dotenvy's CWD-walk (covers running the built binary from elsewhere).
    // `CARGO_MANIFEST_DIR` is baked in at compile time = apps/symbol-service.
    // Existing env vars are never overwritten — production (systemd's
    // EnvironmentFile=) takes precedence and these calls become no-ops.
    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&crate_env).is_err() {
        let _ = dotenvy::dotenv();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "symbol_service=info,markets=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!(
        typesense = %cfg.typesense_url,
        collection = %cfg.collection,
        schedule_hour_utc = cfg.schedule_hour_utc,
        "symbol-service starting"
    );

    let typesense = TypesenseClient::new(
        &cfg.typesense_url,
        &cfg.typesense_api_key,
        &cfg.collection,
    )?;
    typesense.ensure_collection().await?;

    let state = RedisJobState::connect(&cfg.redis_url).await?;
    let screener = YahooScreener;

    // On boot, three possible paths before entering the scheduler loop:
    //   1. An interrupted cursor exists → resume that run (release the stale
    //      lock first since the previous process couldn't).
    //   2. The Typesense collection is empty (first deploy or a reset) →
    //      run an initial seed sync so the desktop app has data to search
    //      without waiting until the next 02:00 UTC window.
    //   3. Otherwise → just enter the scheduler loop.
    use crate::state::JobState;
    if state.load_current().await?.is_some() {
        tracing::info!("interrupted run found in redis; resuming immediately");
        state.release_lock().await.ok();
        run_sync(&state, &typesense, &screener).await?;
    } else if typesense.count_documents().await? == 0 {
        tracing::info!("typesense collection is empty; running initial seed sync");
        run_sync(&state, &typesense, &screener).await?;
    }

    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| anyhow::anyhow!("install SIGTERM handler: {e}"))?;

    loop {
        tokio::select! {
            _ = wait_until_next_run(cfg.schedule_hour_utc) => {
                if let Err(e) = run_sync(&state, &typesense, &screener).await {
                    tracing::error!(error = %e, "scheduled run failed; will retry next window");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("SIGINT received; shutting down");
                break;
            }
            _ = sigterm.recv() => {
                tracing::info!("SIGTERM received; shutting down");
                break;
            }
        }
    }
    Ok(())
}
