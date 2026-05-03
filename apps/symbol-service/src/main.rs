mod config;
mod filter;
mod scheduler;
mod state;
mod sync;
mod typesense;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "symbol_service=info,markets=warn".into()),
        )
        .init();

    tracing::info!("symbol-service starting");
    Ok(())
}
