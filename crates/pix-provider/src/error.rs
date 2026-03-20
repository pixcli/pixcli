//! Error types for Pix provider operations.

use thiserror::Error;

/// Errors that can occur during Pix provider operations.
#[derive(Debug, Error)]
#[non_exhaustive]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authentication_display() {
        let err = ProviderError::Authentication("bad token".into());
        assert!(err.to_string().contains("bad token"));
    }

    #[test]
    fn test_http_display() {
        let err = ProviderError::Http {
            status: 500,
            message: "server error".into(),
        };
        let s = err.to_string();
        assert!(s.contains("500"));
        assert!(s.contains("server error"));
    }

    #[test]
    fn test_network_display() {
        let err = ProviderError::Network("timeout".into());
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_invalid_response_display() {
        let err = ProviderError::InvalidResponse("bad json".into());
        assert!(err.to_string().contains("bad json"));
    }

    #[test]
    fn test_certificate_display() {
        let err = ProviderError::Certificate("expired".into());
        assert!(err.to_string().contains("expired"));
    }

    #[test]
    fn test_not_found_display() {
        let err = ProviderError::NotFound("charge".into());
        assert!(err.to_string().contains("charge"));
    }

    #[test]
    fn test_rate_limited_display() {
        let err = ProviderError::RateLimited {
            retry_after_secs: 60,
        };
        assert!(err.to_string().contains("60"));
    }

    #[test]
    fn test_serialization_display() {
        let err = ProviderError::Serialization("parse error".into());
        assert!(err.to_string().contains("parse error"));
    }

    #[test]
    fn test_timeout_display() {
        let err = ProviderError::Timeout(30);
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_io_display() {
        let err = ProviderError::Io("file not found".into());
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_error_is_debug() {
        let err = ProviderError::Network("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Network"));
    }
}
