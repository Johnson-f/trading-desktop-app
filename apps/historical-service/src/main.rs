mod bar;
mod clickhouse;
mod config;
mod fmp;
mod probe;
mod scheduler;
mod state;
mod sync;
mod universe;
mod yahoo;

use anyhow::Result;
use tokio::signal::unix::{signal, SignalKind};

use crate::clickhouse::ClickhouseClient;
use crate::config::Config;
use crate::fmp::FmpClient;
use crate::probe::discover_universe_earliest;
use crate::scheduler::wait_until_next_run;
use crate::state::{JobLock, RedisJobLock};
use crate::sync::run_sync;
use crate::universe::fetch_universe;
use crate::yahoo::YahooSource;

#[tokio::main]
async fn main() -> Result<()> {
    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&crate_env).is_err() {
        let _ = dotenvy::dotenv();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "historical_service=info,markets=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!(
        clickhouse = %cfg.clickhouse_url,
        database = %cfg.clickhouse_database,
        fmp_rpm = cfg.fmp_rate_limit_rpm,
        schedule_hour_utc = cfg.schedule_hour_utc,
        backfill_earliest = %cfg.backfill_earliest,
        "historical-service starting"
    );

    let ch = ClickhouseClient::new(
        &cfg.clickhouse_url,
        &cfg.clickhouse_user,
        &cfg.clickhouse_password,
        &cfg.clickhouse_database,
    )?;
    ch.ensure_table().await?;

    let lock = RedisJobLock::connect(&cfg.redis_url).await?;
    lock.force_release().await.ok();

    let fmp = FmpClient::new(&cfg.fmp_base_url, &cfg.fmp_api_key, cfg.fmp_rate_limit_rpm)?;
    let yahoo = YahooSource::new();

    // Keep the concrete ClickhouseClient around for metadata ops (load/upsert
    // symbol_metadata) — those aren't on the BarSink trait. Clone is cheap
    // (Arc internally for the inner reqwest pool).
    let ch_for_meta = ch.clone();
    let fmp_for_probe = std::sync::Arc::new(fmp.clone());

    let sink: std::sync::Arc<dyn crate::sync::BarSink> = std::sync::Arc::new(ch);
    let fmp_dyn: std::sync::Arc<dyn crate::sync::BarSource> = std::sync::Arc::new(fmp);
    let yahoo_dyn: std::sync::Arc<dyn crate::sync::BarSource> = std::sync::Arc::new(yahoo);

    let universe = fetch_universe().await?;
    if universe.is_empty() {
        anyhow::bail!("universe is empty — refusing to start with no work to do");
    }

    let earliest = chrono::TimeZone::from_utc_datetime(
        &chrono::Utc,
        &cfg.backfill_earliest.and_hms_opt(0, 0, 0).expect("valid time"),
    );

    // Probe phase: discover (and cache) the earliest date FMP has data for
    // each symbol. New symbols incur the binary-search cost; previously-probed
    // symbols are loaded from `symbol_metadata` for free.
    let symbol_earliest = discover_universe_earliest(
        &ch_for_meta,
        fmp_for_probe,
        &universe,
        earliest,
        cfg.concurrency,
    )
    .await?;

    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| anyhow::anyhow!("install SIGTERM handler: {e}"))?;

    loop {
        let did_work = run_sync(
            &lock,
            sink.clone(),
            fmp_dyn.clone(),
            yahoo_dyn.clone(),
            &universe,
            earliest,
            &symbol_earliest,
            cfg.concurrency,
        )
        .await?;
        if did_work {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => continue,
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received during backfill; shutting down");
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received during backfill; shutting down");
                    break;
                }
            }
        }

        tokio::select! {
            _ = wait_until_next_run(cfg.schedule_hour_utc) => {}
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

    lock.release().await.ok();
    Ok(())
}
