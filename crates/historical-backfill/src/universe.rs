use anyhow::{Result, anyhow};

const PAGE_SIZE: u32 = 250;
const EXCHANGES: &[&str] = &["NMS", "NYQ", "ASE"];
const MIN_AVG_DAILY_VOL_3M: f64 = 50_000.0;
const MIN_MARKET_CAP: f64 = 1_000_000.0;
/// Yahoo's screener throttles aggressive pagination. A small pause per page
/// keeps us under whatever the silent rate cap is.
const INTER_PAGE_DELAY_MS: u64 = 250;

/// Fetch the active US equity universe ordered by intraday market cap
/// descending, so the backfill processes the most-traded names first.
///
/// We use Yahoo's screener directly (not Typesense) so this service stays
/// independent of `symbol-service`'s collection schema. Each exchange is
/// queried separately, results are merged and re-sorted client-side.
///
/// **Per-page tolerance:** an individual page failure (Yahoo 429/5xx, network
/// blip) is retried once after a 2s backoff. If it still fails we log a
/// warning and break out of that exchange's pagination, keeping whatever
/// pages we already collected. Only a total failure (no symbols at all
/// across all three exchanges) propagates as an error.
pub async fn fetch_universe() -> Result<Vec<String>> {
    use markets::{EquityField, EquityScreenerQuery, ScreenerFieldExt};

    let mut all: Vec<(String, i64)> = Vec::new();

    for exchange in EXCHANGES {
        let mut offset: u32 = 0;
        loop {
            let mut last_err: Option<anyhow::Error> = None;
            let mut results: Option<_> = None;
            for attempt in 1..=2 {
                let query = EquityScreenerQuery::new()
                    .size(PAGE_SIZE)
                    .offset(offset)
                    .sort_by(EquityField::IntradayMarketCap, false)
                    .add_condition(EquityField::Region.eq_str("us"))
                    .add_condition(EquityField::Exchange.eq_str(*exchange))
                    .add_condition(EquityField::AvgDailyVol3M.gt(MIN_AVG_DAILY_VOL_3M))
                    .add_condition(EquityField::IntradayMarketCap.gt(MIN_MARKET_CAP));
                match markets::finance::custom_screener(query).await {
                    Ok(r) => {
                        results = Some(r);
                        break;
                    }
                    Err(e) => {
                        last_err = Some(anyhow!("yahoo screener {exchange}@{offset}: {e}"));
                        if attempt < 2 {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
            }
            let results = match results {
                Some(r) => r,
                None => {
                    tracing::warn!(
                        exchange,
                        offset,
                        error = ?last_err,
                        "screener page failed twice; skipping rest of this exchange"
                    );
                    break;
                }
            };

            if results.quotes.is_empty() {
                break;
            }

            for quote in &results.quotes {
                if quote.symbol.is_empty() || !is_plain_equity(&quote.symbol) {
                    continue;
                }
                if quote.quote_type != "EQUITY" {
                    continue;
                }
                let market_cap = quote.market_cap.as_ref().and_then(|fv| fv.raw).unwrap_or(0);
                all.push((quote.symbol.clone(), market_cap));
            }

            if (results.quotes.len() as u32) < PAGE_SIZE {
                break;
            }
            offset += PAGE_SIZE;
            tokio::time::sleep(std::time::Duration::from_millis(INTER_PAGE_DELAY_MS)).await;
        }
    }

    if all.is_empty() {
        anyhow::bail!("yahoo screener returned zero symbols across all exchanges");
    }

    all.sort_by(|a, b| b.1.cmp(&a.1));

    let mut seen = std::collections::HashSet::new();
    let symbols: Vec<String> = all
        .into_iter()
        .filter_map(|(s, _)| {
            if seen.insert(s.clone()) {
                Some(s)
            } else {
                None
            }
        })
        .collect();

    tracing::info!(count = symbols.len(), "universe fetched");
    Ok(symbols)
}

fn is_plain_equity(symbol: &str) -> bool {
    if symbol.is_empty() || !symbol.is_ascii() {
        return false;
    }
    if symbol.contains('^') {
        return false;
    }
    let bad_suffixes = [".WS", ".U", ".R", ".WI", ".WD", ".P"];
    !bad_suffixes.iter().any(|suf| symbol.ends_with(suf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_keeps_plain_symbols() {
        assert!(is_plain_equity("AAPL"));
        assert!(is_plain_equity("BRK.B"));
    }

    #[test]
    fn filter_rejects_non_equity_suffixes() {
        assert!(!is_plain_equity("ABC.WS"));
        assert!(!is_plain_equity("ABC.U"));
        assert!(!is_plain_equity("ABC^A"));
        assert!(!is_plain_equity(""));
        assert!(!is_plain_equity("ÄBC"));
    }
}
