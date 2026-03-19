//! Provider trait and error types for Pix payment service providers.
//!
//! This crate defines the abstract interface that all Pix providers (Efí, Mercado Pago, etc.)
//! must implement, along with common request/response types and error handling.

pub mod error;
pub mod types;

pub use error::ProviderError;
pub use types::{
    Balance, ChargeRequest, ChargeResponse, ChargeStatus, Debtor, DueDateChargeRequest, PixCharge,
    PixTransaction, PixTransfer, TransactionFilter,
};

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

    /// Creates a charge with a due date (cobrança com vencimento).
    fn create_due_date_charge(
        &self,
        request: DueDateChargeRequest,
    ) -> impl Future<Output = Result<ChargeResponse, ProviderError>> + Send;

    /// Retrieves the status and details of an existing charge by its txid.
    fn get_charge(
        &self,
        txid: &str,
    ) -> impl Future<Output = Result<PixCharge, ProviderError>> + Send;

    /// Lists charges within a time range.
    fn list_charges(
        &self,
        filter: TransactionFilter,
    ) -> impl Future<Output = Result<Vec<PixCharge>, ProviderError>> + Send;

    /// Sends a Pix payment to a recipient key.
    fn send_pix(
        &self,
        key: &str,
        amount: &str,
        description: Option<&str>,
    ) -> impl Future<Output = Result<PixTransfer, ProviderError>> + Send;

    /// Retrieves a Pix transaction by its end-to-end ID.
    fn get_pix(
        &self,
        e2eid: &str,
    ) -> impl Future<Output = Result<PixTransaction, ProviderError>> + Send;

    /// Lists received Pix transactions within a time range.
    fn list_received_pix(
        &self,
        filter: TransactionFilter,
    ) -> impl Future<Output = Result<Vec<PixTransaction>, ProviderError>> + Send;

    /// Gets the current account balance.
    fn get_balance(&self) -> impl Future<Output = Result<Balance, ProviderError>> + Send;

    /// Returns the provider name (e.g., "efi", "mercadopago").
    fn provider_name(&self) -> &str;
}
