use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::bar::Bar;

/// FMP's `/stable/historical-chart/1min` returns timestamps as naive
/// `YYYY-MM-DD HH:MM:SS` strings *in US Eastern Time* (the exchange's local
/// time, DST-aware). Convert to UTC explicitly so downstream consumers can
/// trust every `Bar.ts` as a real UTC instant.
///
/// Ambiguous local times (the 1am-2am hour during fall back) get the earliest
/// valid instant — same minute will only ever appear once per FMP response in
/// practice, but be defensive.
fn parse_fmp_ts(s: &str) -> Result<DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .with_context(|| format!("parse FMP timestamp {s}"))?;
    let et = match New_York.from_local_datetime(&naive) {
        chrono::offset::LocalResult::Single(dt) => dt,
        chrono::offset::LocalResult::Ambiguous(earliest, _later) => earliest,
        chrono::offset::LocalResult::None => {
            anyhow::bail!("FMP timestamp {s} falls in spring-forward DST gap")
        }
    };
    Ok(et.with_timezone(&Utc))
}

#[derive(Debug, Clone, Copy)]
struct DelayState {
    min_delay: Duration,
    current_delay: Duration,
    consecutive_ok: u32,
}

#[derive(Clone)]
pub struct FmpClient {
    http: reqwest::Client,
    base: String,
    api_key: String,
    delay: Arc<Mutex<DelayState>>,
}

impl FmpClient {
    pub fn new(base_url: &str, api_key: &str, rate_limit_rpm: u32) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("build reqwest client")?;
        let min_delay = Duration::from_millis(60_000 / rate_limit_rpm.max(1) as u64);
        Ok(Self {
            http,
            base: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            delay: Arc::new(Mutex::new(DelayState {
                min_delay,
                current_delay: min_delay,
                consecutive_ok: 0,
            })),
        })
    }

    pub async fn current_delay_ms(&self) -> u64 {
        self.delay.lock().await.current_delay.as_millis() as u64
    }

    pub async fn fetch_1min(
        &self,
        symbol: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Bar>> {
        let from_str = from.format("%Y-%m-%d").to_string();
        let to_str = to.format("%Y-%m-%d").to_string();

        let url = format!(
            "{}/historical-chart/1min?symbol={symbol}&from={from_str}&to={to_str}&apikey={key}",
            self.base,
            key = self.api_key,
        );

        let pause = self.delay.lock().await.current_delay;
        if !pause.is_zero() {
            tokio::time::sleep(pause).await;
        }

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("fmp GET {symbol} {from_str}→{to_str}"))?;
        let status = resp.status();

        if status.as_u16() == 429 {
            self.on_rate_limited().await;
            anyhow::bail!("fmp 429 for {symbol} {from_str}→{to_str}");
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("fmp {} for {symbol}: {}", status, body);
        }

        let text = resp.text().await.context("fmp body")?;
        let parsed: Result<Vec<RawBar>, _> = serde_json::from_str(&text);
        let raw = match parsed {
            Ok(v) => v,
            Err(e) => {
                let body_len = text.len();
                let head: String = text.chars().take(500).collect();
                let tail: String = text.chars().rev().take(500).collect::<String>().chars().rev().collect();
                anyhow::bail!(
                    "fmp parse error for {symbol}: {e}; body_len={body_len}; head=<<<{head}>>>; tail=<<<{tail}>>>"
                );
            }
        };

        self.on_success().await;

        let version = Utc::now().timestamp() as u32;
        let mut bars = Vec::with_capacity(raw.len());
        for r in raw {
            let ts = parse_fmp_ts(&r.date)?;
            bars.push(Bar {
                symbol: symbol.to_string(),
                ts,
                open: Bar::cents_from_dollars(r.open),
                high: Bar::cents_from_dollars(r.high),
                low: Bar::cents_from_dollars(r.low),
                close: Bar::cents_from_dollars(r.close),
                volume: r.volume.round().max(0.0) as u32,
                version,
            });
        }
        bars.sort_by_key(|b| b.ts);
        Ok(bars)
    }

    async fn on_rate_limited(&self) {
        let mut d = self.delay.lock().await;
        d.consecutive_ok = 0;
        d.current_delay = (d.current_delay * 2).min(Duration::from_secs(60));
        tracing::warn!(
            new_delay_ms = d.current_delay.as_millis() as u64,
            "fmp 429 — backing off"
        );
    }

    async fn on_success(&self) {
        let mut d = self.delay.lock().await;
        d.consecutive_ok += 1;
        if d.consecutive_ok >= 5 && d.current_delay > d.min_delay {
            d.current_delay = (d.current_delay / 2).max(d.min_delay);
            d.consecutive_ok = 0;
            tracing::info!(
                new_delay_ms = d.current_delay.as_millis() as u64,
                "fmp recovered — relaxing delay"
            );
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawBar {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    // FMP returns fractional volume for some securities (ADRs, foreign-listed
    // stocks like RYAAY). Parse as f64 and round at conversion time.
    volume: f64,
}

#[async_trait::async_trait]
impl crate::sync::BarSource for FmpClient {
    async fn fetch(
        &self,
        symbol: &str,
        from: ::chrono::DateTime<::chrono::Utc>,
        to: ::chrono::DateTime<::chrono::Utc>,
    ) -> anyhow::Result<Vec<crate::bar::Bar>> {
        FmpClient::fetch_1min(self, symbol, from, to).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parse_fmp_ts_winter_est_to_utc() {
        // FMP sends "09:30:00" as US/Eastern; in January (EST = UTC-5) the
        // NYSE open should land at 14:30:00 UTC.
        let ts = parse_fmp_ts("2024-01-03 09:30:00").unwrap();
        assert_eq!(ts.format("%Y-%m-%d %H:%M:%S").to_string(), "2024-01-03 14:30:00");
    }

    #[tokio::test]
    async fn parse_fmp_ts_summer_edt_to_utc() {
        // In July (EDT = UTC-4) the NYSE open should land at 13:30:00 UTC.
        let ts = parse_fmp_ts("2024-07-15 09:30:00").unwrap();
        assert_eq!(ts.format("%Y-%m-%d %H:%M:%S").to_string(), "2024-07-15 13:30:00");
    }

    #[tokio::test]
    async fn fetch_returns_bars_oldest_first() {
        let server = MockServer::start().await;
        let body = r#"[
            {"date":"2024-01-03 09:31:00","open":190.0,"high":190.5,"low":189.8,"close":190.2,"volume":1000},
            {"date":"2024-01-03 09:30:00","open":189.5,"high":190.0,"low":189.4,"close":189.9,"volume":2000}
        ]"#;
        Mock::given(method("GET"))
            .and(path("/historical-chart/1min"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let fmp = FmpClient::new(&server.uri(), "k", 60_000).unwrap();
        let bars = fmp
            .fetch_1min(
                "AAPL",
                Utc.with_ymd_and_hms(2024, 1, 3, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 4, 0, 0, 0).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bars.len(), 2);
        assert!(bars[0].ts < bars[1].ts);
        assert_eq!(bars[0].symbol, "AAPL");
        assert_eq!(bars[0].volume, 2000);
    }

    #[tokio::test]
    async fn fetch_returns_empty_for_empty_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/historical-chart/1min"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .mount(&server)
            .await;
        let fmp = FmpClient::new(&server.uri(), "k", 60_000).unwrap();
        let bars = fmp
            .fetch_1min(
                "ZZZZ",
                Utc.with_ymd_and_hms(1990, 1, 1, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(1990, 1, 2, 0, 0, 0).unwrap(),
            )
            .await
            .unwrap();
        assert!(bars.is_empty());
    }

    #[tokio::test]
    async fn fetch_429_grows_delay() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let fmp = FmpClient::new(&server.uri(), "k", 6000).unwrap();
        let initial = fmp.current_delay_ms().await;
        let _ = fmp
            .fetch_1min(
                "AAPL",
                Utc.with_ymd_and_hms(2024, 1, 3, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 4, 0, 0, 0).unwrap(),
            )
            .await;
        let after = fmp.current_delay_ms().await;
        assert!(after > initial, "delay should grow after 429: {initial} → {after}");
    }
}
