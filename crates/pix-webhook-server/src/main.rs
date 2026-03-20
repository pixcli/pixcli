//! Standalone Pix webhook receiver server.
//!
//! Listens for POST requests at `/pix` from Efí payment notifications,
//! printing events to stdout, optionally saving to file, and forwarding via HTTP.

use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use pix_webhook_server::AppState;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// Standalone Pix webhook receiver.
#[derive(Parser)]
#[command(
    name = "pix-webhook-server",
    about = "Standalone Pix webhook receiver for payment notifications"
)]
struct Args {
    /// Port to listen on.
    #[arg(short, long, default_value = "8080")]
    port: u16,
    /// Bind address.
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,
    /// Forward events to this URL via POST.
    #[arg(long)]
    forward_url: Option<String>,
    /// Append events to this file (JSONL format).
    #[arg(long)]
    output_file: Option<String>,
    /// Suppress stdout output of events.
    #[arg(long)]
    quiet: bool,
    /// Optional API key for webhook authentication (checked via X-Api-Key header).
    #[arg(long)]
    api_key: Option<String>,
    /// Optional HMAC-SHA256 secret for webhook signature verification (checked via X-Webhook-Signature header).
    #[arg(long)]
    hmac_secret: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .init();

    let state = Arc::new(AppState {
        forward_url: args.forward_url.clone(),
        output_file: args.output_file.clone(),
        quiet: args.quiet,
        http_client: reqwest::Client::new(),
        api_key: args.api_key.clone(),
        hmac_secret: args.hmac_secret.clone(),
    });

    let app = Router::new()
        .route("/pix", post(pix_webhook_server::handlers::handle_webhook))
        .route("/health", get(|| async { "OK" }))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state);

    let addr = format!("{}:{}", args.bind, args.port);
    tracing::info!("🔔 Webhook server listening on {addr}");
    tracing::info!("   Endpoint: POST http://{addr}/pix");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
