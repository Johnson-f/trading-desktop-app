//! Environment configuration for historical-service. Yahoo-only — no FMP,
//! no Redis, no per-page concurrency knobs. Yahoo throttles aggressive
//! fetchers so we cap concurrency low (8 workers default).

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub clickhouse_database: String,
    /// Hour (0–23, UTC) at which the daily sync runs. Default 2 (02:00 UTC,
    /// safely after US market close + late settlement reconciliation).
    pub schedule_hour_utc: u32,
    /// Bounded concurrency for Yahoo fetches. Yahoo silently throttles
    /// fetchers above ~10 req/s; 8 workers gives us headroom.
    pub yahoo_workers: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let clickhouse_url =
            std::env::var("CLICKHOUSE_URL").context("CLICKHOUSE_URL is required")?;
        let clickhouse_user =
            std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string());
        let clickhouse_password =
            std::env::var("CLICKHOUSE_PASSWORD").context("CLICKHOUSE_PASSWORD is required")?;
        let clickhouse_database =
            std::env::var("CLICKHOUSE_DATABASE").unwrap_or_else(|_| "market_data".to_string());

        let schedule_hour_utc = std::env::var("SCHEDULE_HOUR_UTC")
            .ok()
            .map(|s| s.parse::<u32>())
            .transpose()
            .context("SCHEDULE_HOUR_UTC must be an integer in 0..=23")?
            .unwrap_or(2);
        if schedule_hour_utc > 23 {
            anyhow::bail!("SCHEDULE_HOUR_UTC must be 0..=23");
        }

        let yahoo_workers = std::env::var("YAHOO_WORKERS")
            .ok()
            .map(|s| s.parse::<usize>())
            .transpose()
            .context("YAHOO_WORKERS must be a positive integer")?
            .unwrap_or(8);
        if yahoo_workers == 0 {
            anyhow::bail!("YAHOO_WORKERS must be > 0");
        }

        Ok(Self {
            clickhouse_url,
            clickhouse_user,
            clickhouse_password,
            clickhouse_database,
            schedule_hour_utc,
            yahoo_workers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        use std::sync::Mutex;
        static SERIAL_ENV: Mutex<()> = Mutex::new(());
        let _g = SERIAL_ENV.lock().unwrap();
        let prev: Vec<_> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
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

    #[test]
    fn defaults_apply() {
        with_env(
            &[
                ("CLICKHOUSE_URL", Some("http://localhost:8123")),
                ("CLICKHOUSE_PASSWORD", Some("test")),
                ("CLICKHOUSE_USER", None),
                ("CLICKHOUSE_DATABASE", None),
                ("SCHEDULE_HOUR_UTC", None),
                ("YAHOO_WORKERS", None),
            ],
            || {
                let cfg = Config::from_env().unwrap();
                assert_eq!(cfg.clickhouse_user, "default");
                assert_eq!(cfg.clickhouse_database, "market_data");
                assert_eq!(cfg.schedule_hour_utc, 2);
                assert_eq!(cfg.yahoo_workers, 8);
            },
        );
    }

    #[test]
    fn rejects_zero_workers() {
        with_env(
            &[
                ("CLICKHOUSE_URL", Some("http://localhost:8123")),
                ("CLICKHOUSE_PASSWORD", Some("test")),
                ("YAHOO_WORKERS", Some("0")),
            ],
            || assert!(Config::from_env().is_err()),
        );
    }

    #[test]
    fn rejects_invalid_schedule_hour() {
        with_env(
            &[
                ("CLICKHOUSE_URL", Some("http://localhost:8123")),
                ("CLICKHOUSE_PASSWORD", Some("test")),
                ("SCHEDULE_HOUR_UTC", Some("24")),
            ],
            || assert!(Config::from_env().is_err()),
        );
    }
}
