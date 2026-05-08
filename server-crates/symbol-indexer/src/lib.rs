//! symbol-indexer: scheduled job that scrapes Yahoo's screener for the
//! active US equity universe and upserts ticker documents into the
//! Typesense `tickers` collection. Embedded as a library inside the
//! gateway's tokio runtime.
//!
//! On boot:
//!   - If Redis holds an interrupted cursor → resume that run.
//!   - Else if the Typesense collection is empty → run an initial seed
//!     sync so the desktop app has data to search before the next
//!     scheduled window.
//!   - Otherwise → enter the schedule loop directly.
//!
//! On each cycle (default 02:00 UTC), the indexer pulls the screener,
//! filters to plain equities, and upserts the resulting documents.
//!
//! # Embedding
//!
//! The gateway calls [`run`] inside a `tokio::spawn` at boot. The task
//! runs forever; cancellation happens implicitly when the runtime
//! drops on shutdown. A panic inside this task does not bring down the
//! gateway — `JoinHandle` surfaces it and the caller logs.

mod config;
mod enrich;
mod filter;
mod scheduler;
mod state;
mod sync;
mod typesense;

pub use config::Config;

use anyhow::Result;

use crate::scheduler::wait_until_next_run;
use crate::state::{JobState, RedisJobState};
use crate::sync::{YahooScreener, run_sync};
use crate::typesense::TypesenseClient;

/// Run the symbol-indexer scheduler forever. Caller is expected to
/// `tokio::spawn` this; cancellation occurs when the runtime shuts
/// down. Returns `Err` only if initial setup (Typesense collection
/// ensure, Redis connect, boot-time resume/seed sync) fails — once
/// the loop starts, transient cycle errors are logged and the next
/// window proceeds.
pub async fn run(cfg: Config) -> Result<()> {
    tracing::info!(
        typesense = %cfg.typesense_url,
        collection = %cfg.collection,
        schedule_hour_utc = cfg.schedule_hour_utc,
        "symbol-indexer starting"
    );

    let typesense =
        TypesenseClient::new(&cfg.typesense_url, &cfg.typesense_api_key, &cfg.collection)?;
    typesense.ensure_collection().await?;

    let state = RedisJobState::connect(&cfg.redis_url).await?;
    let screener = YahooScreener;

    // Boot-time decision: resume an interrupted run, seed an empty
    // collection, or do nothing and wait for the next window.
    if state.load_current().await?.is_some() {
        tracing::info!("interrupted run found in redis; resuming immediately");
        state.release_lock().await.ok();
        run_sync(&state, &typesense, &screener).await?;
    } else if typesense.count_documents().await? == 0 {
        tracing::info!("typesense collection is empty; running initial seed sync");
        run_sync(&state, &typesense, &screener).await?;
    }

    loop {
        wait_until_next_run(cfg.schedule_hour_utc).await;
        if let Err(e) = run_sync(&state, &typesense, &screener).await {
            tracing::error!(error = %e, "scheduled run failed; will retry next window");
        }
    }
}
