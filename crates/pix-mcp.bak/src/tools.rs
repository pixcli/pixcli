//! Tool parameter types for the Pix MCP server.
//!
//! Each tool's parameters are defined as a separate struct with
//! JSON Schema support for automatic MCP tool schema generation.

use rmcp::schemars;
use serde::{Deserialize, Serialize};

/// Parameters for creating a Pix charge.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateChargeParams {
    /// Amount in BRL (e.g., 10.50).
    pub amount: f64,
    /// Pix key to receive payment. If omitted, uses the default from config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pix_key: Option<String>,
    /// Description / payment request text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Expiry in seconds (default: 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_seconds: Option<u32>,
}

/// Parameters for getting a charge's status.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetChargeParams {
    /// Transaction ID (txid).
    pub txid: String,
}

/// Parameters for listing transactions.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListTransactionsParams {
    /// Number of days to look back (default: 7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<u32>,
}

/// Parameters for sending a Pix payment.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendPaymentParams {
    /// Recipient Pix key (CPF, email, phone, or EVP/UUID).
    pub key: String,
    /// Amount in BRL.
    pub amount: f64,
    /// Payment description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parameters for generating a QR code payload.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GenerateQrParams {
    /// Pix key.
    pub key: String,
    /// Amount in BRL (optional for open-amount QR codes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    /// Merchant name (max 25 chars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<String>,
    /// City (max 15 chars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
}
