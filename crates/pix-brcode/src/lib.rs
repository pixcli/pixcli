#![deny(unsafe_code)]
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
    /// Description text (inside merchant account information tag 26, sub-tag 02).
    pub description: Option<String>,
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
    #[must_use]
    pub fn builder(
        pix_key: impl Into<String>,
        merchant_name: impl Into<String>,
        merchant_city: impl Into<String>,
    ) -> BrCodeBuilder {
        BrCodeBuilder {
            pix_key: pix_key.into(),
            merchant_name: merchant_name.into(),
            merchant_city: merchant_city.into(),
            point_of_initiation: None,
            merchant_category_code: "0000".to_string(),
            transaction_amount: None,
            description: None,
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
    description: Option<String>,
    txid: Option<String>,
}

impl BrCodeBuilder {
    /// Sets the point of initiation method.
    ///
    /// - `"11"` for static (reusable) QR codes
    /// - `"12"` for dynamic (one-time) QR codes
    #[must_use]
    pub fn point_of_initiation(mut self, method: impl Into<String>) -> Self {
        self.point_of_initiation = Some(method.into());
        self
    }

    /// Sets the merchant category code (MCC).
    #[must_use]
    pub fn merchant_category_code(mut self, mcc: impl Into<String>) -> Self {
        self.merchant_category_code = mcc.into();
        self
    }

    /// Sets the transaction amount.
    #[must_use]
    pub fn transaction_amount(mut self, amount: impl Into<String>) -> Self {
        self.transaction_amount = Some(amount.into());
        self
    }

    /// Sets the description text (included in merchant account info, sub-tag 02).
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the transaction ID (txid).
    #[must_use]
    pub fn txid(mut self, txid: impl Into<String>) -> Self {
        self.txid = Some(txid.into());
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
            description: self.description,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        assert_eq!(brcode.pix_key, "key@test.com");
        assert_eq!(brcode.merchant_name, "Name");
        assert_eq!(brcode.merchant_city, "City");
        assert_eq!(brcode.payload_format_indicator, "01");
        assert_eq!(brcode.transaction_currency, "986");
        assert_eq!(brcode.country_code, "BR");
        assert_eq!(brcode.merchant_category_code, "0000");
        assert!(brcode.transaction_amount.is_none());
        assert!(brcode.txid.is_none());
        assert!(brcode.description.is_none());
    }

    #[test]
    fn test_builder_with_all_fields() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("12")
            .merchant_category_code("5812")
            .transaction_amount("50.00")
            .description("Test payment")
            .txid("TX123")
            .build()
            .unwrap();
        assert_eq!(brcode.point_of_initiation, Some("12".to_string()));
        assert_eq!(brcode.merchant_category_code, "5812");
        assert_eq!(brcode.transaction_amount, Some("50.00".to_string()));
        assert_eq!(brcode.description, Some("Test payment".to_string()));
        assert_eq!(brcode.txid, Some("TX123".to_string()));
    }

    #[test]
    fn test_builder_rejects_long_merchant_name() {
        let result = BrCode::builder("key", &"A".repeat(26), "City").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_long_merchant_city() {
        let result = BrCode::builder("key", "Name", &"B".repeat(16)).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_long_amount() {
        let result = BrCode::builder("key", "Name", "City")
            .transaction_amount(&"9".repeat(14))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_long_txid() {
        let result = BrCode::builder("key", "Name", "City")
            .txid(&"X".repeat(26))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_accepts_max_length_fields() {
        let result = BrCode::builder("key", &"A".repeat(25), &"B".repeat(15))
            .transaction_amount(&"9".repeat(13))
            .txid(&"X".repeat(25))
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_brcode_serialize_deserialize() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("10.00")
            .build()
            .unwrap();
        let json = serde_json::to_string(&brcode).unwrap();
        let deserialized: BrCode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pix_key, brcode.pix_key);
        assert_eq!(deserialized.transaction_amount, brcode.transaction_amount);
    }
}

#[cfg(test)]
mod additional_builder_tests {
    use super::*;

    #[test]
    fn test_builder_merchant_name_exactly_25() {
        let name = "A".repeat(25);
        let brcode = BrCode::builder("key", &name, "City").build().unwrap();
        assert_eq!(brcode.merchant_name, name);
    }

    #[test]
    fn test_builder_merchant_name_26_fails() {
        let name = "A".repeat(26);
        let result = BrCode::builder("key", &name, "City").build();
        assert!(result.is_err());
        match result.unwrap_err() {
            BrCodeError::FieldTooLong { field, max, actual } => {
                assert_eq!(field, "merchant_name");
                assert_eq!(max, 25);
                assert_eq!(actual, 26);
            }
            e => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_builder_merchant_city_exactly_15() {
        let city = "B".repeat(15);
        let brcode = BrCode::builder("key", "Name", &city).build().unwrap();
        assert_eq!(brcode.merchant_city, city);
    }

    #[test]
    fn test_builder_merchant_city_16_fails() {
        let city = "B".repeat(16);
        let result = BrCode::builder("key", "Name", &city).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_amount_exactly_13_chars() {
        let amount = "9".repeat(13);
        let brcode = BrCode::builder("key", "Name", "City")
            .transaction_amount(&amount)
            .build()
            .unwrap();
        assert_eq!(brcode.transaction_amount, Some(amount));
    }

    #[test]
    fn test_builder_amount_14_chars_fails() {
        let amount = "9".repeat(14);
        let result = BrCode::builder("key", "Name", "City")
            .transaction_amount(&amount)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_txid_exactly_25() {
        let txid = "X".repeat(25);
        let brcode = BrCode::builder("key", "Name", "City")
            .txid(&txid)
            .build()
            .unwrap();
        assert_eq!(brcode.txid, Some(txid));
    }

    #[test]
    fn test_builder_txid_26_fails() {
        let txid = "X".repeat(26);
        let result = BrCode::builder("key", "Name", "City").txid(&txid).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_empty_fields() {
        let brcode = BrCode::builder("k", "N", "C").build().unwrap();
        assert_eq!(brcode.pix_key, "k");
        assert_eq!(brcode.merchant_name, "N");
        assert_eq!(brcode.merchant_city, "C");
    }

    #[test]
    fn test_builder_chaining() {
        let brcode = BrCode::builder("key", "Name", "City")
            .point_of_initiation("12")
            .merchant_category_code("5812")
            .transaction_amount("10.00")
            .description("Desc")
            .txid("TX1")
            .build()
            .unwrap();
        assert_eq!(brcode.point_of_initiation, Some("12".to_string()));
        assert_eq!(brcode.merchant_category_code, "5812");
        assert_eq!(brcode.transaction_amount, Some("10.00".to_string()));
        assert_eq!(brcode.description, Some("Desc".to_string()));
        assert_eq!(brcode.txid, Some("TX1".to_string()));
    }

    #[test]
    fn test_brcode_defaults() {
        let brcode = BrCode::builder("key", "Name", "City").build().unwrap();
        assert_eq!(brcode.payload_format_indicator, "01");
        assert_eq!(brcode.transaction_currency, "986");
        assert_eq!(brcode.country_code, "BR");
        assert_eq!(brcode.merchant_category_code, "0000");
        assert!(brcode.point_of_initiation.is_none());
        assert!(brcode.transaction_amount.is_none());
        assert!(brcode.description.is_none());
        assert!(brcode.txid.is_none());
        assert!(brcode.crc.is_empty());
    }

    #[test]
    fn test_brcode_clone() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("10.00")
            .build()
            .unwrap();
        let cloned = brcode.clone();
        assert_eq!(cloned.pix_key, brcode.pix_key);
        assert_eq!(cloned.transaction_amount, brcode.transaction_amount);
    }

    #[test]
    fn test_brcode_debug() {
        let brcode = BrCode::builder("key", "Name", "City").build().unwrap();
        let debug = format!("{:?}", brcode);
        assert!(debug.contains("BrCode"));
    }
}
