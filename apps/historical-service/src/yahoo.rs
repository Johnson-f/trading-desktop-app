use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};

use crate::bar::Bar;

pub struct YahooSource;

impl YahooSource {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_1min(&self, symbol: &str, since: DateTime<Utc>) -> Result<Vec<Bar>> {
        let ticker = markets::Ticker::new(symbol)
            .await
            .with_context(|| format!("yahoo Ticker::new {symbol}"))?;
        let chart = ticker
            .chart(markets::Interval::OneMinute, markets::TimeRange::OneMonth)
            .await
            .with_context(|| format!("yahoo chart {symbol}"))?;

        let version = Utc::now().timestamp() as u32;
        let mut bars: Vec<Bar> = chart
            .candles
            .iter()
            .filter_map(|c| {
                let ts = Utc.timestamp_opt(c.timestamp, 0).single()?;
                if ts <= since {
                    return None;
                }
                Some(Bar {
                    symbol: symbol.to_string(),
                    ts,
                    open: Bar::cents_from_dollars(c.open),
                    high: Bar::cents_from_dollars(c.high),
                    low: Bar::cents_from_dollars(c.low),
                    close: Bar::cents_from_dollars(c.close),
                    volume: c.volume as u32,
                    version,
                })
            })
            .collect();
        bars.sort_by_key(|b| b.ts);
        Ok(bars)
    }
}

#[async_trait::async_trait]
impl crate::sync::BarSource for YahooSource {
    async fn fetch(
        &self,
        symbol: &str,
        from: ::chrono::DateTime<::chrono::Utc>,
        _to: ::chrono::DateTime<::chrono::Utc>,
    ) -> anyhow::Result<Vec<crate::bar::Bar>> {
        YahooSource::fetch_1min(self, symbol, from).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yahoo_source_constructs() {
        let _ = YahooSource::new();
    }
}
