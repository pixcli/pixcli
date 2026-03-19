//! Provider trait and error types for Pix payment service providers.
//!
//! This crate defines the abstract interface that all Pix providers (Efí, Mercado Pago, etc.)
//! must implement, along with common request/response types and error handling.

pub mod error;
pub mod types;

pub use error::ProviderError;
pub use types::{ChargeRequest, ChargeResponse, ChargeStatus, Debtor, PixCharge};

use std::future::Future;

/// The core trait that Pix payment providers must implement.
///
/// Each method represents a Pix API operation. Providers handle
/// authentication, request signing, and response parsing internally.
pub trait PixProvider: Send + Sync {
    /// Creates a new immediate charge (cobrança imediata).
    ///
    /// This generates a Pix charge that can be paid via QR code or copy-and-paste.
    fn create_charge(
        &self,
        request: ChargeRequest,
    ) -> impl Future<Output = Result<ChargeResponse, ProviderError>> + Send;

    /// Retrieves the status and details of an existing charge by its txid.
    fn get_charge(
        &self,
        txid: &str,
    ) -> impl Future<Output = Result<PixCharge, ProviderError>> + Send;

    /// Lists charges within a time range.
    fn list_charges(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> impl Future<Output = Result<Vec<PixCharge>, ProviderError>> + Send;

    /// Returns the provider name (e.g., "efi", "mercadopago").
    fn provider_name(&self) -> &str;
}
