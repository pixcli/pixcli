//! Error types for BRCode encoding and decoding.

use thiserror::Error;

/// Errors that can occur during BRCode encoding or decoding.
#[derive(Debug, Error)]
pub enum BrCodeError {
    /// A field exceeds the maximum allowed length.
    #[error("field '{field}' is too long: max {max}, got {actual}")]
    FieldTooLong {
        /// Name of the field.
        field: String,
        /// Maximum allowed length.
        max: usize,
        /// Actual length.
        actual: usize,
    },

    /// The payload is incomplete or malformed.
    #[error("malformed TLV payload: {0}")]
    MalformedTlv(String),

    /// A required tag is missing from the payload.
    #[error("missing required tag: {0}")]
    MissingTag(String),

    /// CRC checksum mismatch during decoding.
    #[error("CRC mismatch: expected {expected}, got {actual}")]
    CrcMismatch {
        /// Expected CRC value.
        expected: String,
        /// Actual CRC value.
        actual: String,
    },

    /// Invalid field value.
    #[error("invalid field value for '{field}': {reason}")]
    InvalidField {
        /// Name of the field.
        field: String,
        /// Reason the value is invalid.
        reason: String,
    },
}
