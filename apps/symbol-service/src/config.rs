use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub typesense_url: String,
    pub typesense_api_key: String,
    pub redis_url: String,
    /// Daily run hour in UTC (0–23). Defaults to 2 (02:00 UTC ≈ 21:00 ET).
    pub schedule_hour_utc: u32,
    /// Typesense collection name. Defaults to "tickers".
    pub collection: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let typesense_url = std::env::var("TYPESENSE_URL")
            .context("TYPESENSE_URL is required")?;
        let typesense_api_key = std::env::var("TYPESENSE_API_KEY")
            .context("TYPESENSE_API_KEY is required")?;
        let redis_url = std::env::var("REDIS_URL").context("REDIS_URL is required")?;
        let schedule_hour_utc = std::env::var("SYMBOL_SERVICE_SCHEDULE_HOUR_UTC")
            .ok()
            .map(|s| s.parse::<u32>())
            .transpose()
            .context("SYMBOL_SERVICE_SCHEDULE_HOUR_UTC must be 0..=23")?
            .unwrap_or(2);
        if schedule_hour_utc > 23 {
            anyhow::bail!("SYMBOL_SERVICE_SCHEDULE_HOUR_UTC must be 0..=23");
        }
        let collection = std::env::var("SYMBOL_SERVICE_COLLECTION")
            .unwrap_or_else(|_| "tickers".to_string());
        Ok(Self {
            typesense_url,
            typesense_api_key,
            redis_url,
            schedule_hour_utc,
            collection,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: set env vars, run f, restore. Tests run serially via the
    /// `SERIAL_ENV` mutex because env is process-global.
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

    #[test]
    fn loads_minimal_required_env() {
        with_env(
            &[
                ("TYPESENSE_URL", Some("http://localhost:8108")),
                ("TYPESENSE_API_KEY", Some("k")),
                ("REDIS_URL", Some("redis://localhost")),
                ("SYMBOL_SERVICE_SCHEDULE_HOUR_UTC", None),
                ("SYMBOL_SERVICE_COLLECTION", None),
            ],
            || {
                let cfg = Config::from_env().expect("config loads");
                assert_eq!(cfg.typesense_url, "http://localhost:8108");
                assert_eq!(cfg.typesense_api_key, "k");
                assert_eq!(cfg.redis_url, "redis://localhost");
                assert_eq!(cfg.schedule_hour_utc, 2);
                assert_eq!(cfg.collection, "tickers");
            },
        );
    }

    #[test]
    fn rejects_missing_required_env() {
        with_env(
            &[
                ("TYPESENSE_URL", None),
                ("TYPESENSE_API_KEY", Some("k")),
                ("REDIS_URL", Some("redis://localhost")),
            ],
            || {
                assert!(Config::from_env().is_err());
            },
        );
    }

    #[test]
    fn rejects_out_of_range_schedule_hour() {
        with_env(
            &[
                ("TYPESENSE_URL", Some("http://x")),
                ("TYPESENSE_API_KEY", Some("k")),
                ("REDIS_URL", Some("redis://x")),
                ("SYMBOL_SERVICE_SCHEDULE_HOUR_UTC", Some("25")),
            ],
            || {
                assert!(Config::from_env().is_err());
            },
        );
    }
}
