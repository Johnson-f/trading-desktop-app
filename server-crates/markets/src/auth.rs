use crate::client::ClientConfig;
use crate::endpoints::urls::{api, base};
use crate::error::{FinanceError, Result};
use rquest::Proxy;
use rquest_util::Emulation;
use std::time::{Duration, Instant};
use tokio::sync::{OnceCell, RwLock};
use tracing::{debug, info, warn};

// ============================================================================
// Authentication Constants
// ============================================================================

/// Browser to impersonate when talking to Yahoo. Picks the TLS Client
/// Hello, HTTP/2 SETTINGS, and User-Agent that match this Chrome
/// version. Bump to the latest variant when newer ones land — older
/// fingerprints get more attention from bot detectors over time.
const YAHOO_EMULATION: Emulation = Emulation::Chrome136;

/// Timeout for authentication requests
const AUTH_TIMEOUT: Duration = Duration::from_secs(15);

/// Minimum interval between auth refreshes (prevent excessive refreshing)
#[cfg(test)]
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum age of auth before considering it stale
#[cfg(test)]
const AUTH_MAX_AGE: Duration = Duration::from_secs(3600); // 1 hour

/// How long a fetched crumb is considered fresh. Yahoo's actual lifetime
/// appears to be ~30 min — we cache slightly under that to refetch before
/// it would expire mid-request and cause an `AuthenticationFailed`.
const CRUMB_TTL: Duration = Duration::from_secs(25 * 60);

/// Cached authentication: shared across every `YahooAuth::authenticate_with_config`
/// caller in the process. Cloning is cheap (the `rquest::Client` is internally
/// `Arc`-wrapped and the crumb is a short String), so the cache hands out
/// snapshots rather than references.
#[derive(Clone)]
struct CachedAuth {
    crumb: String,
    http_client: rquest::Client,
    acquired_at: Instant,
}

impl CachedAuth {
    fn is_fresh(&self) -> bool {
        self.acquired_at.elapsed() < CRUMB_TTL
    }
}

/// Process-wide cache. `OnceCell` so we only allocate the lock on first use.
static AUTH_CACHE: OnceCell<RwLock<Option<CachedAuth>>> = OnceCell::const_new();

async fn auth_cache() -> &'static RwLock<Option<CachedAuth>> {
    AUTH_CACHE.get_or_init(|| async { RwLock::new(None) }).await
}

/// Yahoo Finance authentication data
#[derive(Clone)]
pub struct YahooAuth {
    /// CSRF crumb token
    pub crumb: String,
    /// Last time auth was refreshed
    pub last_refresh: Instant,
    /// HTTP client with cookies. `rquest::Client` mimics Chrome's TLS
    /// + HTTP/2 fingerprint so Yahoo's bot detection can't tell us
    /// apart from a real browser.
    pub(crate) http_client: rquest::Client,
}

impl std::fmt::Debug for YahooAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YahooAuth")
            .field("crumb", &self.crumb)
            .field("last_refresh", &self.last_refresh)
            .finish()
    }
}

impl YahooAuth {
    /// Authenticate with Yahoo Finance using custom configuration.
    ///
    /// Allows specifying timeout and proxy settings for the HTTP client.
    ///
    /// Crumbs are cached process-wide with a 25-minute TTL (see `CRUMB_TTL`).
    /// On a cache miss the slow path holds a write lock across two HTTP calls
    /// (the `fc.yahoo.com` session GET and the crumb fetch); concurrent callers
    /// wait. This is intentional single-flight — we never have two in-flight
    /// crumb fetches process-wide. The worst-case foreground stall is one full
    /// Yahoo round-trip plus the session-cookie GET, bounded by `config.timeout`
    /// + `AUTH_TIMEOUT`. Callers that detect a rejected crumb (HTTP 401 /
    /// auth-failure on the chart endpoint) should call `YahooAuth::invalidate()`
    /// so the next call refetches.
    pub async fn authenticate_with_config(config: &ClientConfig) -> Result<Self> {
        // Cache fast path: if we have a fresh crumb, hand out a clone.
        {
            let guard = auth_cache().await.read().await;
            if let Some(cached) = guard.as_ref() {
                if cached.is_fresh() {
                    debug!("Yahoo crumb cache hit");
                    return Ok(Self {
                        crumb: cached.crumb.clone(),
                        last_refresh: cached.acquired_at,
                        http_client: cached.http_client.clone(),
                    });
                }
            }
        }

        // Slow path: take the write lock and refresh. Double-check after
        // acquiring to handle the thundering-herd case where many tasks
        // raced past the read-lock check.
        let mut guard = auth_cache().await.write().await;
        if let Some(cached) = guard.as_ref() {
            if cached.is_fresh() {
                debug!("Yahoo crumb cache hit (after write-lock contention)");
                return Ok(Self {
                    crumb: cached.crumb.clone(),
                    last_refresh: cached.acquired_at,
                    http_client: cached.http_client.clone(),
                });
            }
        }

        info!("Fetching fresh Yahoo crumb (cache miss or expired)");

        // Build a cookie-bearing client per refresh. The cookies established
        // by visiting fc.yahoo.com are required for the crumb endpoint.
        // The `.emulation(...)` call configures the TLS Client Hello,
        // HTTP/2 SETTINGS frame, header order, and User-Agent to match
        // the chosen Chrome version — Yahoo's bot detection can't tell
        // us apart from a real browser at the wire level.
        let mut builder = rquest::Client::builder()
            .emulation(YAHOO_EMULATION)
            .cookie_store(true)
            .timeout(config.timeout)
            .connect_timeout(AUTH_TIMEOUT);

        if let Some(proxy_url) = &config.proxy {
            debug!("Configuring proxy: {}", proxy_url);
            let proxy = Proxy::all(proxy_url)
                .map_err(|e| FinanceError::InternalError(format!("Invalid proxy URL: {}", e)))?;
            builder = builder.proxy(proxy);
        }

        let client = builder.build().map_err(|e| {
            FinanceError::InternalError(format!("Failed to create HTTP client: {}", e))
        })?;

        debug!("Visiting {} to establish session", base::YAHOO_FC);
        crate::client::yahoo_rate_limiter().await.acquire().await;
        client.get(base::YAHOO_FC).send().await.map_err(|e| {
            FinanceError::InternalError(format!("Failed to establish session: {}", e))
        })?;

        debug!("Attempting to fetch crumb from query1");
        crate::client::yahoo_rate_limiter().await.acquire().await;
        let crumb = get_crumb(&client, api::CRUMB_QUERY1).await.map_err(|e| {
            warn!("Failed to fetch crumb: {}", e);
            FinanceError::AuthenticationFailed {
                context: format!("Failed to fetch crumb: {}", e),
            }
        })?;

        let acquired_at = Instant::now();
        *guard = Some(CachedAuth {
            crumb: crumb.clone(),
            http_client: client.clone(),
            acquired_at,
        });

        info!(
            "Successfully authenticated with Yahoo Finance (cached for {} min)",
            CRUMB_TTL.as_secs() / 60
        );
        Ok(Self {
            crumb,
            last_refresh: acquired_at,
            http_client: client,
        })
    }

    /// Drop the cached crumb so the next `authenticate_with_config` call
    /// will refetch. Use when a downstream request returns
    /// `AuthenticationFailed`, which is Yahoo's signal that our crumb is
    /// no longer valid (typically due to crumb rotation before our TTL
    /// expired).
    pub async fn invalidate() {
        let mut guard = auth_cache().await.write().await;
        if guard.take().is_some() {
            info!("Yahoo crumb cache invalidated");
        }
    }

    /// Check if authentication is still valid
    #[cfg(test)]
    pub fn is_expired(&self) -> bool {
        self.last_refresh.elapsed() > AUTH_MAX_AGE
    }

    /// Check if enough time has passed to allow refresh
    #[cfg(test)]
    pub fn can_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= MIN_REFRESH_INTERVAL
    }
}

/// Fetch crumb token from Yahoo Finance
async fn get_crumb(client: &rquest::Client, crumb_url: &str) -> Result<String> {
    let response = client
        .get(crumb_url)
        .send()
        .await
        .map_err(|e| FinanceError::InternalError(format!("Crumb request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(FinanceError::InternalError(format!(
            "Crumb request returned status {}",
            response.status()
        )));
    }

    let crumb = response.text().await.map_err(|e| {
        FinanceError::InternalError(format!("Failed to read crumb response: {}", e))
    })?;

    // Validate crumb (should not contain HTML)
    if crumb.contains("<html") || crumb.contains("<!DOCTYPE") {
        return Err(FinanceError::InternalError(
            "Crumb response contains HTML instead of token".to_string(),
        ));
    }

    debug!(
        "Successfully fetched crumb: {}",
        &crumb[..10.min(crumb.len())]
    );
    Ok(crumb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_authenticate() {
        let auth = YahooAuth::authenticate_with_config(&ClientConfig::default()).await;
        assert!(auth.is_ok());

        let auth = auth.unwrap();
        assert!(!auth.crumb.is_empty());
        assert!(!auth.crumb.contains("<html"));
    }

    #[test]
    fn test_is_expired() {
        let client = rquest::Client::new();
        let auth = YahooAuth {
            crumb: "test".to_string(),
            last_refresh: Instant::now() - std::time::Duration::from_secs(7200),
            http_client: client,
        };

        assert!(auth.is_expired());
    }

    #[test]
    fn test_can_refresh() {
        let client = rquest::Client::new();
        let auth = YahooAuth {
            crumb: "test".to_string(),
            last_refresh: Instant::now() - std::time::Duration::from_secs(60),
            http_client: client,
        };

        assert!(auth.can_refresh());
    }

    /// Mutates the process-wide `AUTH_CACHE` static. Serialized against
    /// other cache-touching tests via `#[serial(yahoo_auth_cache)]` so we
    /// can run the full test suite with default parallelism without flakes.
    #[tokio::test]
    #[serial(yahoo_auth_cache)]
    async fn cache_invalidate_clears_state() {
        YahooAuth::invalidate().await;
        let cache = auth_cache().await;
        let client = rquest::Client::new();
        *cache.write().await = Some(CachedAuth {
            crumb: "stale".into(),
            http_client: client,
            acquired_at: Instant::now(),
        });
        assert!(cache.read().await.is_some());

        YahooAuth::invalidate().await;
        assert!(cache.read().await.is_none());

        YahooAuth::invalidate().await;
    }

    #[tokio::test]
    #[serial(yahoo_auth_cache)]
    async fn authenticate_returns_cached_crumb_when_fresh() {
        YahooAuth::invalidate().await;

        {
            let cache = auth_cache().await;
            *cache.write().await = Some(CachedAuth {
                crumb: "test-crumb-abc".into(),
                http_client: rquest::Client::new(),
                acquired_at: Instant::now(),
            });
        }

        let auth = YahooAuth::authenticate_with_config(&ClientConfig::default())
            .await
            .unwrap();
        assert_eq!(auth.crumb, "test-crumb-abc");

        YahooAuth::invalidate().await;
    }

    #[test]
    fn cached_auth_freshness_uses_ttl() {
        let client = rquest::Client::new();
        let fresh = CachedAuth {
            crumb: "x".into(),
            http_client: client.clone(),
            acquired_at: Instant::now(),
        };
        assert!(fresh.is_fresh());

        let stale = CachedAuth {
            crumb: "x".into(),
            http_client: client,
            acquired_at: Instant::now() - CRUMB_TTL - Duration::from_secs(1),
        };
        assert!(!stale.is_fresh());
    }
}
