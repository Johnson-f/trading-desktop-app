use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub redis_url: String,
    /// Soft cap on the number of symbols kept continuously subscribed.
    /// When a new symbol is admitted past this cap, the oldest zero-ref
    /// symbol is evicted. Active subscriptions are never evicted.
    pub max_warm_symbols: u32,
    /// `host:port` to bind the WebSocket server (e.g. `0.0.0.0:8765`).
    pub ws_bind: String,
    /// Seconds of TTL on `tick:bars:{date}:{symbol}` keys. Should be > 24h so
    /// the day's data outlives the next-morning historical-service backfill
    /// window. Default 36h.
    pub today_bars_ttl_secs: u64,
    /// Stream MAXLEN cap for `tick:updates:{symbol}`. Bounded so old delta
    /// entries auto-evict; ~200 entries ≈ a few minutes of activity at 1Hz.
    pub updates_stream_maxlen: u64,
    /// How long after the user disconnects from a cold symbol to keep its
    /// Yahoo subscription alive (reserved for future use). Default 300s.
    pub cold_subscription_ttl_secs: u64,
    /// Typesense base URL (e.g. `https://typesense.your-vps:8108`).
    /// Read by the symbol-search GraphQL query. Writes are owned by
    /// `apps/symbol-service`; this server is read-only.
    pub typesense_url: String,
    /// Typesense API key with search-only permissions.
    pub typesense_api_key: String,
    /// Typesense collection name. Defaults to `"tickers"`.
    pub typesense_collection: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let redis_url = std::env::var("REDIS_URL").context("REDIS_URL is required")?;

        let max_warm_symbols = std::env::var("MAX_WARM_SYMBOLS")
            .ok()
            .map(|s| s.parse::<u32>())
            .transpose()
            .context("MAX_WARM_SYMBOLS must be a positive integer")?
            .unwrap_or(50);
        if max_warm_symbols == 0 {
            anyhow::bail!("MAX_WARM_SYMBOLS must be > 0");
        }

        let ws_bind = std::env::var("TICK_SERVICE_WS_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8765".to_string());

        let today_bars_ttl_secs = std::env::var("TODAY_BARS_TTL_SECS")
            .ok()
            .map(|s| s.parse::<u64>())
            .transpose()
            .context("TODAY_BARS_TTL_SECS must be a positive integer")?
            .unwrap_or(36 * 3600);

        let updates_stream_maxlen = std::env::var("UPDATES_STREAM_MAXLEN")
            .ok()
            .map(|s| s.parse::<u64>())
            .transpose()
            .context("UPDATES_STREAM_MAXLEN must be a positive integer")?
            .unwrap_or(200);

        let cold_subscription_ttl_secs = std::env::var("COLD_SUBSCRIPTION_TTL_SECS")
            .ok()
            .map(|s| s.parse::<u64>())
            .transpose()
            .context("COLD_SUBSCRIPTION_TTL_SECS must be a positive integer")?
            .unwrap_or(300);

        let typesense_url = std::env::var("TYPESENSE_URL")
            .context("TYPESENSE_URL is required (e.g. https://typesense.your-vps:8108)")?;
        let typesense_api_key = std::env::var("TYPESENSE_API_KEY")
            .context("TYPESENSE_API_KEY is required")?;
        let typesense_collection = std::env::var("TYPESENSE_COLLECTION")
            .unwrap_or_else(|_| "tickers".to_string());

        Ok(Self {
            redis_url,
            max_warm_symbols,
            ws_bind,
            today_bars_ttl_secs,
            updates_stream_maxlen,
            cold_subscription_ttl_secs,
            typesense_url,
            typesense_api_key,
            typesense_collection,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            ("REDIS_URL", Some("redis://localhost")),
            ("MAX_WARM_SYMBOLS", None),
            ("TICK_SERVICE_WS_BIND", None),
            ("TODAY_BARS_TTL_SECS", None),
            ("UPDATES_STREAM_MAXLEN", None),
            ("COLD_SUBSCRIPTION_TTL_SECS", None),
            ("TYPESENSE_URL", Some("http://localhost:8108")),
            ("TYPESENSE_API_KEY", Some("test_key")),
            ("TYPESENSE_COLLECTION", None),
        ]
    }

    #[test]
    fn loads_minimal_required_env_with_defaults() {
        with_env(&min_env(), || {
            let cfg = Config::from_env().expect("config loads");
            assert_eq!(cfg.redis_url, "redis://localhost");
            assert_eq!(cfg.max_warm_symbols, 50);
            assert_eq!(cfg.ws_bind, "0.0.0.0:8765");
            assert_eq!(cfg.today_bars_ttl_secs, 36 * 3600);
            assert_eq!(cfg.updates_stream_maxlen, 200);
            assert_eq!(cfg.cold_subscription_ttl_secs, 300);
            assert_eq!(cfg.typesense_url, "http://localhost:8108");
            assert_eq!(cfg.typesense_api_key, "test_key");
            assert_eq!(cfg.typesense_collection, "tickers");
        });
    }

    #[test]
    fn rejects_missing_redis_url() {
        let mut env = min_env();
        env[0] = ("REDIS_URL", None);
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn rejects_zero_max_warm_symbols() {
        let mut env = min_env();
        env[1] = ("MAX_WARM_SYMBOLS", Some("0"));
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn parses_overrides() {
        let env = vec![
            ("REDIS_URL", Some("redis://r")),
            ("MAX_WARM_SYMBOLS", Some("500")),
            ("TICK_SERVICE_WS_BIND", Some("127.0.0.1:9000")),
            ("TODAY_BARS_TTL_SECS", Some("48000")),
            ("UPDATES_STREAM_MAXLEN", Some("400")),
            ("COLD_SUBSCRIPTION_TTL_SECS", Some("60")),
            ("TYPESENSE_URL", Some("https://ts.example.com:8108")),
            ("TYPESENSE_API_KEY", Some("prod_key")),
            ("TYPESENSE_COLLECTION", Some("symbols")),
        ];
        with_env(&env, || {
            let cfg = Config::from_env().unwrap();
            assert_eq!(cfg.max_warm_symbols, 500);
            assert_eq!(cfg.ws_bind, "127.0.0.1:9000");
            assert_eq!(cfg.today_bars_ttl_secs, 48000);
            assert_eq!(cfg.updates_stream_maxlen, 400);
            assert_eq!(cfg.cold_subscription_ttl_secs, 60);
            assert_eq!(cfg.typesense_url, "https://ts.example.com:8108");
            assert_eq!(cfg.typesense_api_key, "prod_key");
            assert_eq!(cfg.typesense_collection, "symbols");
        });
    }

    #[test]
    fn typesense_collection_defaults_to_tickers() {
        let mut env = min_env();
        // Ensure TYPESENSE_COLLECTION is absent.
        let col_idx = env.iter().position(|(k, _)| *k == "TYPESENSE_COLLECTION").unwrap();
        env[col_idx] = ("TYPESENSE_COLLECTION", None);
        with_env(&env, || {
            let cfg = Config::from_env().unwrap();
            assert_eq!(cfg.typesense_collection, "tickers");
        });
    }

    #[test]
    fn rejects_missing_typesense_url() {
        let mut env = min_env();
        let idx = env.iter().position(|(k, _)| *k == "TYPESENSE_URL").unwrap();
        env[idx] = ("TYPESENSE_URL", None);
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }

    #[test]
    fn rejects_missing_typesense_api_key() {
        let mut env = min_env();
        let idx = env.iter().position(|(k, _)| *k == "TYPESENSE_API_KEY").unwrap();
        env[idx] = ("TYPESENSE_API_KEY", None);
        with_env(&env, || {
            assert!(Config::from_env().is_err());
        });
    }
}
