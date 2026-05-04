use anyhow::{Context, Result};
use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Config {
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub clickhouse_database: String,
    pub fmp_api_key: String,
    pub fmp_base_url: String,
    pub fmp_rate_limit_rpm: u32,
    pub redis_url: String,
    pub schedule_hour_utc: u32,
    pub backfill_earliest: NaiveDate,
    /// Number of symbols processed concurrently. Each in-flight task makes
    /// one fetch call against FMP or Yahoo at a time, so this is also the
    /// effective ceiling on concurrent FMP requests.
    pub concurrency: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let clickhouse_url = std::env::var("CLICKHOUSE_URL")
            .context("CLICKHOUSE_URL is required")?;
        let clickhouse_user = std::env::var("CLICKHOUSE_USER")
            .unwrap_or_else(|_| "default".to_string());
        let clickhouse_password = std::env::var("CLICKHOUSE_PASSWORD")
            .unwrap_or_else(|_| String::new());
        let clickhouse_database = std::env::var("CLICKHOUSE_DATABASE")
            .unwrap_or_else(|_| "market_data".to_string());

        let fmp_api_key = std::env::var("FMP_API_KEY")
            .context("FMP_API_KEY is required")?;
        let fmp_base_url = std::env::var("FMP_BASE_URL")
            .unwrap_or_else(|_| "https://financialmodelingprep.com/stable".to_string());
        let fmp_rate_limit_rpm = std::env::var("FMP_RATE_LIMIT_RPM")
            .ok()
            .map(|s| s.parse::<u32>())
            .transpose()
            .context("FMP_RATE_LIMIT_RPM must be a positive integer")?
            .unwrap_or(300);
        if fmp_rate_limit_rpm == 0 {
            anyhow::bail!("FMP_RATE_LIMIT_RPM must be > 0");
        }

        let redis_url = std::env::var("REDIS_URL").context("REDIS_URL is required")?;

        let schedule_hour_utc = std::env::var("HISTORICAL_SERVICE_SCHEDULE_HOUR_UTC")
            .ok()
            .map(|s| s.parse::<u32>())
            .transpose()
            .context("HISTORICAL_SERVICE_SCHEDULE_HOUR_UTC must be 0..=23")?
            .unwrap_or(2);
        if schedule_hour_utc > 23 {
            anyhow::bail!("HISTORICAL_SERVICE_SCHEDULE_HOUR_UTC must be 0..=23");
        }

        let backfill_earliest_str = std::env::var("BACKFILL_EARLIEST")
            .unwrap_or_else(|_| "2005-01-01".to_string());
        let backfill_earliest = NaiveDate::parse_from_str(&backfill_earliest_str, "%Y-%m-%d")
            .with_context(|| format!("BACKFILL_EARLIEST must be YYYY-MM-DD, got {backfill_earliest_str}"))?;

        let concurrency = std::env::var("HISTORICAL_SERVICE_CONCURRENCY")
            .ok()
            .map(|s| s.parse::<usize>())
            .transpose()
            .context("HISTORICAL_SERVICE_CONCURRENCY must be a positive integer")?
            .unwrap_or(4);
        if concurrency == 0 {
            anyhow::bail!("HISTORICAL_SERVICE_CONCURRENCY must be > 0");
        }

        Ok(Self {
            clickhouse_url,
            clickhouse_user,
            clickhouse_password,
            clickhouse_database,
            fmp_api_key,
            fmp_base_url,
            fmp_rate_limit_rpm,
            redis_url,
            schedule_hour_utc,
            backfill_earliest,
            concurrency,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: set env vars, run f, restore. Tests run serially via `SERIAL_ENV`
    /// because env mutation is process-global.
    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        use std::sync::Mutex;
        static SERIAL_ENV: Mutex<()> = Mutex::new(());
        let _guard = SERIAL_ENV.lock().unwrap();
        let prev: Vec<_> = vars.iter().map(|(k, _)| (*k, std::env::var(k).ok())).collect();
        for (k, v) in vars {
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        for (k, v) in prev {
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    fn min_env() -> Vec<(&'static str, Option<&'static str>)> {
        vec![
            ("CLICKHOUSE_URL", Some("http://localhost:8123")),
            ("FMP_API_KEY", Some("k")),
            ("REDIS_URL", Some("redis://localhost")),
            ("CLICKHOUSE_USER", None),
            ("CLICKHOUSE_PASSWORD", None),
            ("CLICKHOUSE_DATABASE", None),
            ("FMP_BASE_URL", None),
            ("FMP_RATE_LIMIT_RPM", None),
            ("HISTORICAL_SERVICE_SCHEDULE_HOUR_UTC", None),
            ("BACKFILL_EARLIEST", None),
            ("HISTORICAL_SERVICE_CONCURRENCY", None),
        ]
    }

    #[test]
    fn loads_minimal_required_env_with_defaults() {
        with_env(&min_env(), || {
            let cfg = Config::from_env().expect("config loads");
            assert_eq!(cfg.clickhouse_url, "http://localhost:8123");
            assert_eq!(cfg.clickhouse_user, "default");
            assert_eq!(cfg.clickhouse_password, "");
            assert_eq!(cfg.clickhouse_database, "market_data");
            assert_eq!(cfg.fmp_api_key, "k");
            assert_eq!(cfg.fmp_base_url, "https://financialmodelingprep.com/stable");
            assert_eq!(cfg.fmp_rate_limit_rpm, 300);
            assert_eq!(cfg.redis_url, "redis://localhost");
            assert_eq!(cfg.schedule_hour_utc, 2);
            assert_eq!(
                cfg.backfill_earliest,
                NaiveDate::from_ymd_opt(2005, 1, 1).unwrap()
            );
            assert_eq!(cfg.concurrency, 4);
        });
    }

    #[test]
    fn rejects_zero_concurrency() {
        let mut env = min_env();
        env.push(("HISTORICAL_SERVICE_CONCURRENCY", Some("0")));
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn rejects_missing_required_env() {
        let mut env = min_env();
        env[0] = ("CLICKHOUSE_URL", None);
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn rejects_out_of_range_schedule_hour() {
        let mut env = min_env();
        env.push(("HISTORICAL_SERVICE_SCHEDULE_HOUR_UTC", Some("25")));
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn rejects_zero_rate_limit() {
        let mut env = min_env();
        env.push(("FMP_RATE_LIMIT_RPM", Some("0")));
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn rejects_invalid_backfill_earliest() {
        let mut env = min_env();
        env.push(("BACKFILL_EARLIEST", Some("not-a-date")));
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }
}
