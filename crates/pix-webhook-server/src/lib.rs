//! Shared types and handlers for the Pix webhook server.
//!
//! This library exposes the webhook handler and application state so that
//! other crates (such as the CLI) can reuse them without duplicating logic.

pub mod handlers;

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
    /// Optional API key for authentication.
    pub api_key: Option<String>,
    /// Optional HMAC-SHA256 secret for signature verification.
    pub hmac_secret: Option<String>,
}
