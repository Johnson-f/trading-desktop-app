//! Quote-based enrichment of `TickerDoc`s.
//!
//! Yahoo's screener endpoint returns sparse data — `longName` and a
//! human-friendly exchange code are often missing. The batch-quote
//! endpoint (`/v7/finance/quote`) reliably returns both for every
//! plain-equity symbol, so after `quote_to_doc` produces a sparse
//! batch we re-call Yahoo and overwrite the empty fields.
//!
//! A failure here is non-fatal: the upsert will proceed with
//! whatever data we have. Search-by-ticker still works; the dropdown
//! will just show the ticker until the next successful enrichment.

use anyhow::{Context, Result};
use markets::Tickers;

use crate::filter::TickerDoc;

/// Batch-fetch Yahoo quotes for every doc and overwrite `long_name`
/// and `exchange` with the richer values. Mutates in place; symbols
/// Yahoo doesn't recognize are skipped.
///
/// `exchange` is overwritten with `quote.exchange_name` (e.g.
/// "NasdaqGS") rather than `quote.exchange` (e.g. "NMS") because the
/// long form is what users expect to see in a search dropdown.
pub async fn enrich_with_quotes(docs: &mut [TickerDoc]) -> Result<()> {
    if docs.is_empty() {
        return Ok(());
    }

    let symbols: Vec<String> = docs.iter().map(|d| d.symbol.clone()).collect();
    let tickers = Tickers::new(symbols.iter().cloned())
        .await
        .context("Tickers::new for enrichment")?;
    let response = tickers
        .quotes()
        .await
        .context("batch quotes for enrichment")?;

    for doc in docs.iter_mut() {
        let Some(quote) = response.quotes.get(&doc.symbol) else {
            continue;
        };
        if let Some(name) = &quote.long_name {
            doc.long_name = Some(name.clone());
        }
        if let Some(exch_name) = &quote.exchange_name {
            doc.exchange = exch_name.clone();
        }
    }
    Ok(())
}
