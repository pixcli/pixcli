//! Standalone Pix webhook receiver server.
//!
//! Listens for POST requests at `/pix` from Efí payment notifications,
//! printing events to stdout, optionally saving to file, and forwarding via HTTP.

use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod handlers;

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
}

/// Shared application state accessible from request handlers.
pub struct AppState {
    /// URL to forward events to, if configured.
    pub forward_url: Option<String>,
    /// File path to append events to, if configured.
    pub output_file: Option<String>,
    /// Whether to suppress stdout output.
    pub quiet: bool,
    /// HTTP client for forwarding events.
    pub http_client: reqwest::Client,
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
    });

    let app = Router::new()
        .route("/pix", post(handlers::handle_webhook))
        .route("/health", get(|| async { "OK" }))
        .with_state(state);

    let addr = format!("{}:{}", args.bind, args.port);
    tracing::info!("🔔 Webhook server listening on {addr}");
    tracing::info!("   Endpoint: POST http://{addr}/pix");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
