//! Common error types for the Pix ecosystem.

use thiserror::Error;

/// Top-level error type for all Pix operations.
#[derive(Debug, Error)]
pub enum PixError {
    /// Invalid Pix key format or check digit.
    #[error("invalid pix key: {0}")]
    InvalidPixKey(String),

    /// Invalid BRCode payload.
    #[error("invalid brcode: {0}")]
    InvalidBrCode(String),

    /// CRC checksum mismatch.
    #[error("CRC mismatch: expected {expected}, got {actual}")]
    CrcMismatch {
        /// Expected CRC value.
        expected: String,
        /// Actual CRC value.
        actual: String,
    },

    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Generic validation error.
    #[error("validation error: {0}")]
    Validation(String),
}
