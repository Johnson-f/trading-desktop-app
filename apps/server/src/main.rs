mod service;

use anyhow::Result;
use axum::{Json, Router, routing::get};
use serde::Serialize;
use tokio::signal::unix::{SignalKind, signal};

use crate::service::auth;
use crate::service::graphql;
use crate::service::tick_service::config::Config;
use crate::service::tick_service::runtime;

/// Strip userinfo from a Redis URL for safe logging.
fn redact_redis_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let scheme = &url[..scheme_end + 3];
        let rest = &url[scheme_end + 3..];
        if let Some(at) = rest.rfind('@') {
            return format!("{scheme}***@{}", &rest[at + 1..]);
        }
    }
    url.to_string()
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[tokio::main]
async fn main() -> Result<()> {
    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&crate_env).is_err() {
        let _ = dotenvy::dotenv();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zaned_server=info,markets=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!(
        redis = %redact_redis_url(&cfg.redis_url),
        max_warm_symbols = cfg.max_warm_symbols,
        ws_bind = %cfg.ws_bind,
        typesense_url = %cfg.typesense_url,
        typesense_collection = %cfg.typesense_collection,
        "zaned-server starting"
    );

    let (tick_router, handles, cmd_tx, redis) = runtime::start(&cfg).await?;

    // Build the Clerk auth layer once and apply it to every protected
    // subtree. /health stays public so load balancers / monitoring can hit
    // it without credentials.
    let clerk_layer = auth::runtime::from_env()?;
    tracing::info!("clerk auth layer initialized");

    // Build read-only Typesense client for symbol search (writes are owned
    // by apps/symbol-service on the VPS).
    let typesense = std::sync::Arc::new(
        crate::service::typesense::TypesenseClient::new(
            &cfg.typesense_url,
            &cfg.typesense_api_key,
            &cfg.typesense_collection,
        )?,
    );
    tracing::info!("typesense client initialized");

    // Build the GraphQL schema. Reuse the Redis Arc returned by runtime::start
    // instead of opening a second connection.
    let schema = graphql::build_schema(redis, cfg.redis_url.clone(), cmd_tx, typesense);
    let graphql_router = graphql::router(schema);

    let app = Router::new()
        // /health is the public liveness probe (no auth, no GraphQL).
        .route("/health", get(health))
        // GraphQL: /graphql + /graphql/ws + /graphiql, all behind Clerk.
        .merge(graphql_router.layer(clerk_layer.clone()))
        // Legacy WS — kept alive while desktop migrates.
        .merge(tick_router.layer(clerk_layer));

    let listener = tokio::net::TcpListener::bind(&cfg.ws_bind).await?;
    tracing::info!(bind = %cfg.ws_bind, "server listening");
    let serve_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error = %e, "server exited with error");
        }
    });

    let mut sigterm =
        signal(SignalKind::terminate()).map_err(|e| anyhow::anyhow!("install SIGTERM: {e}"))?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT received; shutting down"),
        _ = sigterm.recv() => tracing::info!("SIGTERM received; shutting down"),
    }

    handles.abort_all();
    serve_handle.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_redis_url_scrubs_credentials() {
        assert_eq!(
            redact_redis_url("redis://:secret@host:6379"),
            "redis://***@host:6379"
        );
        assert_eq!(
            redact_redis_url("redis://user:pass@host:6379/0"),
            "redis://***@host:6379/0"
        );
    }

    #[test]
    fn redact_redis_url_passes_through_no_credentials() {
        assert_eq!(redact_redis_url("redis://host:6379"), "redis://host:6379");
        assert_eq!(redact_redis_url("redis://localhost"), "redis://localhost");
    }
}
