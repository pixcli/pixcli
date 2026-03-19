//! Error types for Pix provider operations.

use thiserror::Error;

/// Errors that can occur during Pix provider operations.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Authentication failed (invalid credentials, expired token, etc.).
    #[error("authentication error: {0}")]
    Authentication(String),

    /// HTTP request failed.
    #[error("HTTP error: {status} - {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Error message or response body.
        message: String,
    },

    /// Network or connection error.
    #[error("network error: {0}")]
    Network(String),

    /// The provider returned an unexpected response format.
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// mTLS certificate error.
    #[error("certificate error: {0}")]
    Certificate(String),

    /// The requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded: retry after {retry_after_secs}s")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
    },

    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// A timeout occurred.
    #[error("request timed out after {0}s")]
    Timeout(u64),

    /// An I/O error occurred (e.g., reading certificate files).
    #[error("I/O error: {0}")]
    Io(String),
}
