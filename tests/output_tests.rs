//! Tests for CLI output formatting (JSON validity, table rendering, edge cases).

use chrono::Utc;
use pix_provider::{Balance, ChargeStatus, PixCharge, PixTransaction};

#[test]
fn test_balance_json_valid() {
    let balance = Balance {
        available: "1234.56".to_string(),
    };
    let json = serde_json::to_string_pretty(&balance).unwrap();
    // Verify it's valid JSON by parsing it back
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["available"], "1234.56");
}

#[test]
fn test_charge_json_valid() {
    let charge = PixCharge {
        txid: "pix550e8400e29b41d4a716446655440000".to_string(),
        status: ChargeStatus::Active,
        amount: "100.00".to_string(),
        pix_key: "user@example.com".to_string(),
        description: Some("Test charge".to_string()),
        brcode: Some("brcode_payload".to_string()),
        debtor: None,
        created_at: Utc::now(),
        expires_at: Utc::now(),
        e2eids: vec![],
    };
    let json = serde_json::to_string_pretty(&charge).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["txid"], "pix550e8400e29b41d4a716446655440000");
    assert_eq!(parsed["status"], "ACTIVE");
}

#[test]
fn test_empty_charges_json() {
    let charges: Vec<PixCharge> = vec![];
    let json = serde_json::to_string_pretty(&charges).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.as_array().unwrap().is_empty());
}

#[test]
fn test_pix_transaction_json_with_optionals_none() {
    let tx = PixTransaction {
        e2eid: "E12345678901234567890123456789AB".to_string(),
        txid: None,
        amount: "50.00".to_string(),
        payer_name: None,
        payer_document: None,
        description: None,
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string_pretty(&tx).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["txid"].is_null());
    assert!(parsed["payer_name"].is_null());
    assert_eq!(parsed["amount"], "50.00");
}

#[test]
fn test_pix_transaction_json_with_all_fields() {
    let tx = PixTransaction {
        e2eid: "E12345678901234567890123456789AB".to_string(),
        txid: Some("txid123456789012345678901234567".to_string()),
        amount: "999.99".to_string(),
        payer_name: Some("João Silva".to_string()),
        payer_document: Some("52998224725".to_string()),
        description: Some("Pagamento de teste".to_string()),
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string_pretty(&tx).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["payer_name"], "João Silva");
    assert_eq!(parsed["payer_document"], "52998224725");
}

#[test]
fn test_charge_status_roundtrip_json() {
    for status in [
        ChargeStatus::Active,
        ChargeStatus::Completed,
        ChargeStatus::RemovedByUser,
        ChargeStatus::RemovedByPsp,
        ChargeStatus::Expired,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let back: ChargeStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}

#[test]
fn test_multiple_charges_json_array() {
    let charges: Vec<PixCharge> = (0..5)
        .map(|i| PixCharge {
            txid: format!("pix{:0>32}", i),
            status: ChargeStatus::Active,
            amount: format!("{}.00", i * 10),
            pix_key: "key@test.com".to_string(),
            description: None,
            brcode: None,
            debtor: None,
            created_at: Utc::now(),
            expires_at: Utc::now(),
            e2eids: vec![],
        })
        .collect();
    let json = serde_json::to_string_pretty(&charges).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 5);
}
