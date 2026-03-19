//! Common types for Pix provider operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Request to create a new Pix charge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargeRequest {
    /// The Pix key to receive the payment.
    pub pix_key: String,
    /// Description or info text for the charge.
    pub description: Option<String>,
    /// Amount in BRL (e.g., "10.50").
    pub amount: String,
    /// Time-to-live in seconds for the charge (how long it remains payable).
    pub expiration_secs: u32,
    /// Optional debtor information.
    pub debtor: Option<Debtor>,
}

/// Information about the person paying the charge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Debtor {
    /// Debtor's name.
    pub name: String,
    /// Debtor's CPF (11 digits) or CNPJ (14 digits).
    pub document: String,
}

/// Response after creating a charge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargeResponse {
    /// The transaction ID assigned to this charge.
    pub txid: String,
    /// The BRCode payload (copy-and-paste string).
    pub brcode: String,
    /// Current status of the charge.
    pub status: ChargeStatus,
    /// When the charge was created.
    pub created_at: DateTime<Utc>,
    /// When the charge expires.
    pub expires_at: DateTime<Utc>,
}

/// Status of a Pix charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChargeStatus {
    /// Charge is active and can be paid.
    Active,
    /// Charge has been completed (payment received).
    Completed,
    /// Charge was removed or cancelled.
    RemovedByUser,
    /// Charge was removed by the PSP.
    RemovedByPsp,
    /// Charge has expired.
    Expired,
}

impl std::fmt::Display for ChargeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChargeStatus::Active => write!(f, "ACTIVE"),
            ChargeStatus::Completed => write!(f, "COMPLETED"),
            ChargeStatus::RemovedByUser => write!(f, "REMOVED_BY_USER"),
            ChargeStatus::RemovedByPsp => write!(f, "REMOVED_BY_PSP"),
            ChargeStatus::Expired => write!(f, "EXPIRED"),
        }
    }
}

/// Full details of a Pix charge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixCharge {
    /// The transaction ID.
    pub txid: String,
    /// Current status of the charge.
    pub status: ChargeStatus,
    /// Amount in BRL.
    pub amount: String,
    /// The Pix key that receives the payment.
    pub pix_key: String,
    /// Description or info text.
    pub description: Option<String>,
    /// The BRCode payload.
    pub brcode: Option<String>,
    /// Debtor information (if provided).
    pub debtor: Option<Debtor>,
    /// When the charge was created.
    pub created_at: DateTime<Utc>,
    /// When the charge expires.
    pub expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charge_status_display() {
        assert_eq!(ChargeStatus::Active.to_string(), "ACTIVE");
        assert_eq!(ChargeStatus::Completed.to_string(), "COMPLETED");
        assert_eq!(ChargeStatus::RemovedByUser.to_string(), "REMOVED_BY_USER");
        assert_eq!(ChargeStatus::RemovedByPsp.to_string(), "REMOVED_BY_PSP");
        assert_eq!(ChargeStatus::Expired.to_string(), "EXPIRED");
    }

    #[test]
    fn test_charge_status_serialize() {
        let json = serde_json::to_string(&ChargeStatus::Active).unwrap();
        assert_eq!(json, "\"ACTIVE\"");
    }

    #[test]
    fn test_charge_status_deserialize() {
        let status: ChargeStatus = serde_json::from_str("\"COMPLETED\"").unwrap();
        assert_eq!(status, ChargeStatus::Completed);
    }

    #[test]
    fn test_charge_request_serialize() {
        let req = ChargeRequest {
            pix_key: "user@example.com".to_string(),
            description: Some("Test charge".to_string()),
            amount: "10.50".to_string(),
            expiration_secs: 3600,
            debtor: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("user@example.com"));
        assert!(json.contains("10.50"));
    }

    #[test]
    fn test_charge_request_with_debtor() {
        let req = ChargeRequest {
            pix_key: "user@example.com".to_string(),
            description: None,
            amount: "25.00".to_string(),
            expiration_secs: 1800,
            debtor: Some(Debtor {
                name: "João Silva".to_string(),
                document: "52998224725".to_string(),
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("João Silva"));
        assert!(json.contains("52998224725"));
    }
}
