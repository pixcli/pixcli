//! EMV BRCode encoder and decoder for Brazilian Pix QR codes.
//!
//! Implements the EMV TLV (Tag-Length-Value) format used by the Brazilian Central Bank
//! for Pix static and dynamic QR codes.

pub mod decoder;
pub mod encoder;
pub mod error;
pub mod tlv;

pub use decoder::decode_brcode;
pub use encoder::encode_brcode;
pub use error::BrCodeError;
pub use tlv::TlvEntry;

use serde::{Deserialize, Serialize};

/// Represents a parsed BRCode payload with all relevant fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrCode {
    /// Payload format indicator (always "01").
    pub payload_format_indicator: String,
    /// Point of initiation method: "11" for static, "12" for dynamic.
    pub point_of_initiation: Option<String>,
    /// The Pix key (inside merchant account information tag 26, sub-tag 01).
    pub pix_key: String,
    /// Merchant category code (MCC).
    pub merchant_category_code: String,
    /// Transaction currency (ISO 4217 numeric code, "986" for BRL).
    pub transaction_currency: String,
    /// Transaction amount (optional for static QR codes).
    pub transaction_amount: Option<String>,
    /// Country code ("BR").
    pub country_code: String,
    /// Merchant name (max 25 characters).
    pub merchant_name: String,
    /// Merchant city (max 15 characters).
    pub merchant_city: String,
    /// Transaction ID (txid, inside additional data field tag 62, sub-tag 05).
    pub txid: Option<String>,
    /// CRC16 checksum as a 4-char hex string.
    pub crc: String,
}

impl BrCode {
    /// Creates a new `BrCode` builder with required fields.
    pub fn builder(pix_key: &str, merchant_name: &str, merchant_city: &str) -> BrCodeBuilder {
        BrCodeBuilder {
            pix_key: pix_key.to_string(),
            merchant_name: merchant_name.to_string(),
            merchant_city: merchant_city.to_string(),
            point_of_initiation: None,
            merchant_category_code: "0000".to_string(),
            transaction_amount: None,
            txid: None,
        }
    }
}

/// Builder for constructing a `BrCode` payload.
#[derive(Debug, Clone)]
pub struct BrCodeBuilder {
    pix_key: String,
    merchant_name: String,
    merchant_city: String,
    point_of_initiation: Option<String>,
    merchant_category_code: String,
    transaction_amount: Option<String>,
    txid: Option<String>,
}

impl BrCodeBuilder {
    /// Sets the point of initiation method.
    ///
    /// - `"11"` for static (reusable) QR codes
    /// - `"12"` for dynamic (one-time) QR codes
    pub fn point_of_initiation(mut self, method: &str) -> Self {
        self.point_of_initiation = Some(method.to_string());
        self
    }

    /// Sets the merchant category code (MCC).
    pub fn merchant_category_code(mut self, mcc: &str) -> Self {
        self.merchant_category_code = mcc.to_string();
        self
    }

    /// Sets the transaction amount.
    pub fn transaction_amount(mut self, amount: &str) -> Self {
        self.transaction_amount = Some(amount.to_string());
        self
    }

    /// Sets the transaction ID (txid).
    pub fn txid(mut self, txid: &str) -> Self {
        self.txid = Some(txid.to_string());
        self
    }

    /// Builds the BRCode and encodes it as a string payload.
    ///
    /// # Errors
    ///
    /// Returns `BrCodeError` if field validation fails.
    pub fn build(self) -> Result<BrCode, BrCodeError> {
        if self.merchant_name.len() > 25 {
            return Err(BrCodeError::FieldTooLong {
                field: "merchant_name".into(),
                max: 25,
                actual: self.merchant_name.len(),
            });
        }

        if self.merchant_city.len() > 15 {
            return Err(BrCodeError::FieldTooLong {
                field: "merchant_city".into(),
                max: 15,
                actual: self.merchant_city.len(),
            });
        }

        if let Some(ref amount) = self.transaction_amount {
            if amount.len() > 13 {
                return Err(BrCodeError::FieldTooLong {
                    field: "transaction_amount".into(),
                    max: 13,
                    actual: amount.len(),
                });
            }
        }

        if let Some(ref txid) = self.txid {
            if txid.len() > 25 {
                return Err(BrCodeError::FieldTooLong {
                    field: "txid".into(),
                    max: 25,
                    actual: txid.len(),
                });
            }
        }

        let brcode = BrCode {
            payload_format_indicator: "01".to_string(),
            point_of_initiation: self.point_of_initiation,
            pix_key: self.pix_key,
            merchant_category_code: self.merchant_category_code,
            transaction_currency: "986".to_string(),
            transaction_amount: self.transaction_amount,
            country_code: "BR".to_string(),
            merchant_name: self.merchant_name,
            merchant_city: self.merchant_city,
            txid: self.txid,
            crc: String::new(), // Will be computed during encoding
        };

        Ok(brcode)
    }
}
