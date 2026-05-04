use anyhow::{Context, Result};

const PAGE_SIZE: u32 = 250;
const EXCHANGES: &[&str] = &["NMS", "NYQ", "ASE"];
const MIN_AVG_DAILY_VOL_3M: f64 = 50_000.0;
const MIN_MARKET_CAP: f64 = 1_000_000.0;

/// Fetch the active US equity universe ordered by intraday market cap
/// descending, so the backfill processes the most-traded names first.
///
/// We use Yahoo's screener directly (not Typesense) so this service stays
/// independent of `symbol-service`'s collection schema. Each exchange is
/// queried separately, results are merged and re-sorted client-side.
pub async fn fetch_universe() -> Result<Vec<String>> {
    use markets::{EquityField, EquityScreenerQuery, ScreenerFieldExt};

    let mut all: Vec<(String, i64)> = Vec::new();

    for exchange in EXCHANGES {
        let mut offset: u32 = 0;
        loop {
            let query = EquityScreenerQuery::new()
                .size(PAGE_SIZE)
                .offset(offset)
                .sort_by(EquityField::IntradayMarketCap, false)
                .add_condition(EquityField::Region.eq_str("us"))
                .add_condition(EquityField::Exchange.eq_str(*exchange))
                .add_condition(EquityField::AvgDailyVol3M.gt(MIN_AVG_DAILY_VOL_3M))
                .add_condition(EquityField::IntradayMarketCap.gt(MIN_MARKET_CAP));

            let results = markets::finance::custom_screener(query)
                .await
                .with_context(|| format!("yahoo screener {exchange}@{offset}"))?;

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
                let market_cap = quote
                    .market_cap
                    .as_ref()
                    .and_then(|fv| fv.raw)
                    .unwrap_or(0);
                all.push((quote.symbol.clone(), market_cap));
            }

            if (results.quotes.len() as u32) < PAGE_SIZE {
                break;
            }
            offset += PAGE_SIZE;
        }
    }

    all.sort_by(|a, b| b.1.cmp(&a.1));

    let mut seen = std::collections::HashSet::new();
    let symbols: Vec<String> = all
        .into_iter()
        .filter_map(|(s, _)| if seen.insert(s.clone()) { Some(s) } else { None })
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
