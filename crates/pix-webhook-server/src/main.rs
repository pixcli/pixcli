// Webhook server binary — stdout output is intentional for event display.
#![allow(clippy::print_stdout, clippy::print_stderr)]
//! Standalone Pix webhook receiver server.
//!
//! Listens for POST requests at `/pix` from Efí payment notifications,
//! printing events to stdout, optionally saving to file, and forwarding via HTTP.

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::EnvFilter;

mod handlers;

/// Maximum request body size (256 KB — webhook payloads are small).
const MAX_BODY_SIZE: usize = 256 * 1024;

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
    /// Bind address (defaults to localhost for security; use 0.0.0.0 to expose externally).
    #[arg(short, long, default_value = "127.0.0.1")]
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
    /// Require this Bearer token in the Authorization header for all webhook requests.
    /// If not set, no authentication is required.
    #[arg(long, env = "PIX_WEBHOOK_AUTH_TOKEN")]
    auth_token: Option<String>,
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
    /// Optional Bearer token for authenticating webhook requests.
    pub auth_token: Option<String>,
}

/// Validates a forward URL, ensuring it uses http(s) scheme.
fn validate_forward_url(url: &str) -> anyhow::Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("forward URL must use http:// or https:// scheme, got: {url}");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .init();

    // Validate forward URL scheme before starting the server.
    if let Some(ref url) = args.forward_url {
        validate_forward_url(url)?;
    }

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(5)
        .build()?;

    if args.auth_token.is_none() {
        tracing::warn!(
            "No --auth-token set. Webhook endpoint is unauthenticated. \
             Set --auth-token or PIX_WEBHOOK_AUTH_TOKEN for production use."
        );
    }

    let state = Arc::new(AppState {
        forward_url: args.forward_url.clone(),
        output_file: args.output_file.clone(),
        quiet: args.quiet,
        http_client,
        auth_token: args.auth_token,
    });

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([axum::http::Method::POST, axum::http::Method::GET]);

    let app = Router::new()
        .route("/pix", post(handlers::handle_webhook))
        .route("/health", get(|| async { "OK" }))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", args.bind, args.port);
    tracing::info!("Webhook server listening on {addr}");
    tracing::info!("   Endpoint: POST http://{addr}/pix");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
