use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;

use crate::filter::{quote_to_doc, TickerDoc};
use crate::state::{JobState, RunCursor};
use crate::typesense::TypesenseClient;

/// Yahoo equity exchange codes we sweep, in order. Order matters for
/// resume — `current_run.exchange` indexes into this list.
pub const EXCHANGES: &[&str] = &["NMS", "NYQ", "ASE"];

const PAGE_SIZE: u32 = 250;
const UPSERT_BATCH: usize = 250;
const MAX_PAGES_PER_EXCHANGE: u32 = 200; // 200 × 250 = 50k symbols ceiling
const MIN_AVG_DAILY_VOL_3M: f64 = 50_000.0;
const MIN_MARKET_CAP: f64 = 1_000_000.0;

/// Page fetcher abstraction so tests can inject deterministic data.
#[async_trait]
pub trait Screener: Send + Sync {
    async fn fetch_page(
        &self,
        exchange: &str,
        offset: u32,
        size: u32,
    ) -> Result<Vec<markets::ScreenerQuote>>;
}

pub struct YahooScreener;

#[async_trait]
impl Screener for YahooScreener {
    async fn fetch_page(
        &self,
        exchange: &str,
        offset: u32,
        size: u32,
    ) -> Result<Vec<markets::ScreenerQuote>> {
        use markets::{EquityField, EquityScreenerQuery, ScreenerFieldExt};
        let query = EquityScreenerQuery::new()
            .size(size)
            .offset(offset)
            .add_condition(EquityField::Region.eq_str("us"))
            .add_condition(EquityField::Exchange.eq_str(exchange))
            .add_condition(EquityField::AvgDailyVol3M.gt(MIN_AVG_DAILY_VOL_3M))
            .add_condition(EquityField::IntradayMarketCap.gt(MIN_MARKET_CAP));
        let results = markets::finance::custom_screener(query)
            .await
            .with_context(|| format!("yahoo screener {exchange}@{offset}"))?;
        Ok(results.quotes)
    }
}

/// Run one full sync. Resumes from `state.load_current()` if a cursor exists,
/// otherwise starts fresh from EXCHANGES[0]@0. Idempotent on every page —
/// safe to invoke after a crash.
pub async fn run_sync<S: JobState, F: Screener>(
    state: &S,
    typesense: &TypesenseClient,
    screener: &F,
) -> Result<()> {
    if !state.acquire_lock().await? {
        tracing::info!("another sync run holds the lock; skipping");
        return Ok(());
    }
    // Best-effort: always release on early return.
    let result = run_sync_inner(state, typesense, screener).await;
    if let Err(ref e) = result {
        tracing::error!(error = %e, "sync failed");
        // Leave current_run intact for resume; release lock so a future
        // boot/run can pick up.
        state.release_lock().await.ok();
    }
    result
}

async fn run_sync_inner<S: JobState, F: Screener>(
    state: &S,
    typesense: &TypesenseClient,
    screener: &F,
) -> Result<()> {
    let mut cursor = match state.load_current().await? {
        Some(c) => {
            tracing::info!(?c, "resuming previous run");
            c
        }
        None => RunCursor {
            run_id: Utc::now().format("%Y%m%d-%H%M%S").to_string(),
            started_at: Utc::now(),
            exchange: EXCHANGES[0].to_string(),
            offset: 0,
            total_upserted: 0,
        },
    };

    let start_idx = EXCHANGES
        .iter()
        .position(|e| *e == cursor.exchange)
        .unwrap_or(0);

    let resumed = start_idx != 0 || cursor.offset != 0;

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, exchange) in EXCHANGES.iter().enumerate().skip(start_idx) {
        if i > start_idx {
            cursor.exchange = exchange.to_string();
            cursor.offset = 0;
        }
        let mut page_count: u32 = 0;
        loop {
            page_count += 1;
            if page_count > MAX_PAGES_PER_EXCHANGE {
                tracing::warn!(exchange, "hit max page limit; truncating");
                break;
            }
            let page = screener
                .fetch_page(exchange, cursor.offset, PAGE_SIZE)
                .await?;
            if page.is_empty() {
                break;
            }
            let docs: Vec<TickerDoc> = page.iter().filter_map(quote_to_doc).collect();
            for d in &docs {
                seen.insert(d.id.clone());
            }
            for chunk in docs.chunks(UPSERT_BATCH) {
                let n = typesense.upsert_batch(chunk).await?;
                cursor.total_upserted += n as u64;
            }
            cursor.offset += page.len() as u32;
            state.save_current(&cursor).await?;
            // If Yahoo returned a short page, this exchange is exhausted.
            if (page.len() as u32) < PAGE_SIZE {
                break;
            }
        }
    }

    let removed = if resumed {
        tracing::info!("resumed run — skipping stale-prune pass");
        0
    } else {
        prune_stale(typesense, &seen).await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "prune_stale failed; leaving stale docs in place");
            0
        })
    };
    tracing::info!(total = cursor.total_upserted, removed, "sync complete");
    state.complete_current().await?;
    Ok(())
}

async fn prune_stale(typesense: &TypesenseClient, kept: &std::collections::HashSet<String>) -> Result<usize> {
    let existing = typesense.list_all_ids().await?;
    let mut removed = 0usize;
    for id in existing {
        if !kept.contains(&id) {
            typesense.delete_document(&id).await?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::InMemoryJobState;

    /// Deterministic screener that returns a configurable per-(exchange) sequence.
    struct FakeScreener {
        // Indexed by exchange; each entry is the full list of "all quotes" for
        // that exchange. fetch_page slices it.
        per_exchange: std::collections::HashMap<&'static str, Vec<markets::ScreenerQuote>>,
        calls: tokio::sync::Mutex<std::collections::HashMap<String, usize>>,
    }
    impl FakeScreener {
        fn new() -> Self {
            Self {
                per_exchange: Default::default(),
                calls: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
        fn with_data(data: Vec<(&'static str, Vec<markets::ScreenerQuote>)>) -> Self {
            Self {
                per_exchange: data.into_iter().collect(),
                calls: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
        async fn calls_for(&self, exchange: &str) -> usize {
            *self.calls.lock().await.get(exchange).unwrap_or(&0)
        }
    }
    #[async_trait]
    impl Screener for FakeScreener {
        async fn fetch_page(
            &self,
            exchange: &str,
            offset: u32,
            size: u32,
        ) -> Result<Vec<markets::ScreenerQuote>> {
            *self.calls.lock().await.entry(exchange.to_string()).or_insert(0) += 1;
            let all = self.per_exchange.get(exchange).cloned().unwrap_or_default();
            let start = offset as usize;
            let end = (start + size as usize).min(all.len());
            if start >= all.len() {
                return Ok(vec![]);
            }
            Ok(all[start..end].to_vec())
        }
    }

    #[tokio::test]
    async fn empty_screener_completes_cleanly() {
        // Use a wiremock-backed Typesense that 200s every import.
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;
        // prune_stale calls list_all_ids (GET export) — return empty so nothing is deleted.
        Mock::given(method("GET"))
            .and(path_regex(r"^/collections/.+/documents/export$"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;
        let ts = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();
        let state = InMemoryJobState::new();
        let screener = FakeScreener::new();
        run_sync(&state, &ts, &screener).await.unwrap();
        assert!(state.load_current().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn prune_runs_on_fresh_runs_only() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use wiremock::matchers::{method, path_regex};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let delete_count = Arc::new(AtomicUsize::new(0));

        // collection create
        Mock::given(method("POST"))
            .and(path_regex(r"^/collections$"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&server)
            .await;
        // import (upsert)
        Mock::given(method("POST"))
            .and(path_regex(r"^/collections/.+/documents/import$"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;
        // export (returns one stale id "GONE")
        Mock::given(method("GET"))
            .and(path_regex(r"^/collections/.+/documents/export$"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("{\"id\":\"GONE\"}\n"),
            )
            .mount(&server)
            .await;
        // delete — count invocations
        let dc = delete_count.clone();
        Mock::given(method("DELETE"))
            .and(path_regex(r"^/collections/.+/documents/.+$"))
            .respond_with(move |_: &wiremock::Request| {
                dc.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200)
            })
            .mount(&server)
            .await;

        let ts = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();
        ts.ensure_collection().await.unwrap();

        let state = InMemoryJobState::new();
        let screener = FakeScreener::new(); // empty — no upserts happen
        run_sync(&state, &ts, &screener).await.unwrap();

        assert_eq!(delete_count.load(Ordering::SeqCst), 1, "GONE should be deleted");
    }

    #[tokio::test]
    async fn resumes_from_saved_cursor_skipping_earlier_exchanges() {
        use chrono::Utc;
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;
        let ts = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();

        let state = InMemoryJobState::new();
        state
            .save_current(&RunCursor {
                run_id: "r1".into(),
                started_at: Utc::now(),
                exchange: "NYQ".into(),
                offset: 0,
                total_upserted: 100,
            })
            .await
            .unwrap();
        // NMS would have data IF fetched — it must NOT be fetched on resume.
        let screener = FakeScreener::with_data(vec![("NMS", vec![])]);

        run_sync(&state, &ts, &screener).await.unwrap();

        assert_eq!(
            screener.calls_for("NMS").await,
            0,
            "NMS must not be re-fetched on resume"
        );
        assert!(
            state.load_current().await.unwrap().is_none(),
            "run should complete"
        );
    }
}
