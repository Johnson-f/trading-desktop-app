//! Full OAuth 2.0 Authorization Code + PKCE flow against Clerk:
//!
//!   1. Generate PKCE pair + CSRF state.
//!   2. Bind a one-shot HTTP listener on `127.0.0.1:0` (random port).
//!   3. Build the authorize URL and open it in the system browser.
//!   4. Wait for the browser to hit our localhost callback with `?code=`.
//!   5. POST `code + verifier` to `/oauth/token`, parse the response.
//!   6. Persist refresh token, return the in-memory `TokenSet`.
//!
//! Errors at any step are surfaced as `anyhow::Error` so the caller can
//! transition `AuthState` to `Failed` with the message.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::Query;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use super::config::ClerkConfig;
use super::pkce::{new_pair, random_state};
use super::storage;
use super::tokens::{TokenResponse, TokenSet};

/// Run the full sign-in flow. Blocks (asynchronously) until either tokens
/// are obtained, the user cancels in the browser, or the callback times
/// out. Five-minute timeout matches Clerk's authorize-URL TTL.
pub async fn sign_in(cfg: &ClerkConfig) -> Result<TokenSet> {
    // 1. PKCE + state
    let pkce = new_pair();
    let state = random_state();

    // 2. Bind a one-shot listener on a FIXED port. Clerk's redirect URI
    // validator doesn't accept wildcard ports (`http://127.0.0.1:*/callback`
    // is rejected), so we register a single port in the dashboard and bind
    // it here. Trade-off: if 8787 is already in use the sign-in fails;
    // single-user desktop, low collision risk.
    const CALLBACK_PORT: u16 = 8787;
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).await
        .with_context(|| format!("bind localhost callback listener on port {CALLBACK_PORT} (in use?)"))?;
    let redirect_uri = format!("http://127.0.0.1:{CALLBACK_PORT}/callback");

    // Channel from the route handler back to this function.
    let (tx, rx) = oneshot::channel::<Result<String>>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));
    let expected_state = state.clone();

    let app = Router::new().route(
        "/callback",
        get({
            let tx = tx.clone();
            move |Query(params): Query<HashMap<String, String>>| {
                let tx = tx.clone();
                let expected_state = expected_state.clone();
                async move {
                    let result = match (params.get("code"), params.get("state"), params.get("error")) {
                        (_, _, Some(err)) => {
                            Err(anyhow::anyhow!("authorize error: {err}"))
                        }
                        (Some(code), Some(s), _) if *s == expected_state => {
                            Ok(code.clone())
                        }
                        (Some(_), Some(_), _) => Err(anyhow::anyhow!("CSRF state mismatch")),
                        _ => Err(anyhow::anyhow!("missing code/state in callback")),
                    };
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(match &result {
                            Ok(c) => Ok(c.clone()),
                            Err(e) => Err(anyhow::anyhow!(e.to_string())),
                        });
                    }
                    Html(match &result {
                        Ok(_) => "<html><body><h2>Sign-in complete.</h2><p>You can close this tab.</p></body></html>",
                        Err(_) => "<html><body><h2>Sign-in failed.</h2><p>Return to the app to retry.</p></body></html>",
                    })
                }
            }
        }),
    );

    // Spawn the server. Aborted via the JoinHandle when we have a result.
    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // 3. Build authorize URL + open browser.
    let authorize_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        cfg.authorize_url(),
        urlencoding::encode(&cfg.client_id),
        urlencoding::encode(&redirect_uri),
        // `offline_access` is required for Clerk to issue a refresh_token.
        // Without it, the response only contains an access_token (2h life)
        // and the desktop will have to re-sign-in on every restart.
        urlencoding::encode("openid profile email offline_access"),
        urlencoding::encode(&state),
        urlencoding::encode(pkce.challenge.as_str()),
    );
    if let Err(e) = webbrowser::open(&authorize_url) {
        tracing::warn!(error = %e, "failed to open system browser; copy this URL: {authorize_url}");
    }

    // 4. Wait for callback (or timeout).
    let code = tokio::select! {
        biased;
        res = rx => res.context("callback channel closed")?.context("callback error")?,
        _ = tokio::time::sleep(Duration::from_secs(300)) => {
            server_handle.abort();
            anyhow::bail!("sign-in timed out after 5 minutes");
        }
    };
    server_handle.abort();

    // 5. Exchange code for tokens.
    let resp: TokenResponse = exchange_code(cfg, &code, &redirect_uri, pkce.verifier.secret()).await?;
    let token_set = TokenSet::from_response(resp, None);

    // 6. Persist refresh token (best effort — sign-in still succeeds even
    // if the keychain is locked; user just has to re-auth on next launch).
    // If Clerk didn't issue one, surface a loud warning — silent absence
    // here means every cold start will require a fresh sign-in.
    match &token_set.refresh_token {
        Some(rt) => {
            if let Err(e) = storage::store_refresh_token(rt) {
                tracing::warn!(error = %e, "failed to persist refresh token");
            }
        }
        None => tracing::warn!(
            "Clerk did not return a refresh token; user will need to re-sign-in on next launch \
             (check that `offline_access` is in the OAuth Application's allowed scopes)"
        ),
    }

    Ok(token_set)
}

/// POST to `/oauth/token` with `grant_type=authorization_code`.
async fn exchange_code(
    cfg: &ClerkConfig,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(cfg.token_url())
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &cfg.client_id),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .context("POST token endpoint")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("token endpoint {status}: {body}");
    }
    serde_json::from_str(&body).with_context(|| format!("parse token response: {body}"))
}

/// Refresh an expired access token. Returns a new `TokenSet` (with the
/// refresh token rotated if Clerk issued a new one).
pub async fn refresh(cfg: &ClerkConfig, refresh_token: &str) -> Result<TokenSet> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(cfg.token_url())
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &cfg.client_id),
        ])
        .send()
        .await
        .context("POST refresh endpoint")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("refresh {status}: {body}");
    }
    let parsed: TokenResponse = serde_json::from_str(&body)
        .with_context(|| format!("parse refresh response: {body}"))?;
    let token_set = TokenSet::from_response(parsed, Some(refresh_token.to_string()));

    // Rotate the persisted refresh token if Clerk issued a new one.
    if let Some(rt) = &token_set.refresh_token {
        if rt != refresh_token {
            if let Err(e) = storage::store_refresh_token(rt) {
                tracing::warn!(error = %e, "failed to persist rotated refresh token");
            }
        }
    }
    Ok(token_set)
}
