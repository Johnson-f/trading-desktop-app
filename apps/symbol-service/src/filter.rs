use serde::{Deserialize, Serialize};

/// Document we upsert to Typesense. `id` mirrors `symbol` so re-runs dedupe.
/// Logo URL is intentionally not stored — the desktop app computes it from
/// the symbol at render time (e.g. `https://assets.parqet.com/logos/symbol/{SYMBOL}`)
/// so the index isn't tied to a specific CDN and stale URLs don't accumulate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickerDoc {
    pub id: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_name: Option<String>,
    pub exchange: String,
    pub quote_type: String,
}

/// Reject symbols we don't want in the search index. We keep plain equity
/// tickers and drop instruments addressed via suffix codes (warrants `.WS`,
/// units `.U`, rights `.R`, when-issued `.WI`) and preferred shares (`^`).
pub fn keep_symbol(symbol: &str) -> bool {
    if symbol.is_empty() || !symbol.is_ascii() {
        return false;
    }
    if symbol.contains('^') {
        return false;
    }
    let bad_suffixes = [".WS", ".U", ".R", ".WI", ".WD", ".P"];
    for suf in bad_suffixes {
        if symbol.ends_with(suf) {
            return false;
        }
    }
    true
}

/// Map a Yahoo screener quote to our Typesense doc. Returns `None` when the
/// quote should be dropped (non-equity, junk symbol, missing symbol).
pub fn quote_to_doc(quote: &markets::ScreenerQuote) -> Option<TickerDoc> {
    if quote.symbol.is_empty() || !keep_symbol(&quote.symbol) {
        return None;
    }
    if quote.quote_type != "EQUITY" {
        return None;
    }
    Some(TickerDoc {
        id: quote.symbol.clone(),
        symbol: quote.symbol.clone(),
        long_name: quote.long_name.clone(),
        exchange: quote.exchange.clone(),
        quote_type: quote.quote_type.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_plain_equity_symbols() {
        assert!(keep_symbol("AAPL"));
        assert!(keep_symbol("BRK.B")); // dotted class shares are OK
        assert!(keep_symbol("MSFT"));
    }

    #[test]
    fn rejects_warrants_units_rights_preferred() {
        assert!(!keep_symbol("ABC.WS"));
        assert!(!keep_symbol("ABC.U"));
        assert!(!keep_symbol("ABC.R"));
        assert!(!keep_symbol("ABC^A"));
        assert!(!keep_symbol(""));
        assert!(!keep_symbol("ÄBC"));
    }
}
