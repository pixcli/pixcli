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

#[test]
fn test_balance_json_roundtrip() {
    let b = Balance {
        available: "0.01".to_string(),
    };
    let json = serde_json::to_string(&b).unwrap();
    let back: Balance = serde_json::from_str(&json).unwrap();
    assert_eq!(back.available, "0.01");
}

#[test]
fn test_balance_large_amount() {
    let b = Balance {
        available: "9999999.99".to_string(),
    };
    let json = serde_json::to_string_pretty(&b).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["available"], "9999999.99");
}

#[test]
fn test_charge_json_all_statuses() {
    for status in [
        ChargeStatus::Active,
        ChargeStatus::Completed,
        ChargeStatus::RemovedByUser,
        ChargeStatus::RemovedByPsp,
        ChargeStatus::Expired,
    ] {
        let charge = PixCharge {
            txid: "test".to_string(),
            status,
            amount: "10.00".to_string(),
            pix_key: "key".to_string(),
            description: None,
            brcode: None,
            debtor: None,
            created_at: Utc::now(),
            expires_at: Utc::now(),
            e2eids: vec![],
        };
        let json = serde_json::to_string(&charge).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["status"], status.to_string());
    }
}

#[test]
fn test_charge_with_debtor_json() {
    use pix_provider::Debtor;
    let charge = PixCharge {
        txid: "test".to_string(),
        status: ChargeStatus::Active,
        amount: "50.00".to_string(),
        pix_key: "key@test.com".to_string(),
        description: Some("Test".to_string()),
        brcode: Some("brcode".to_string()),
        debtor: Some(Debtor {
            name: "João Silva".to_string(),
            document: "52998224725".to_string(),
        }),
        created_at: Utc::now(),
        expires_at: Utc::now(),
        e2eids: vec!["E1".to_string()],
    };
    let json = serde_json::to_string_pretty(&charge).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["debtor"]["name"], "João Silva");
    assert_eq!(parsed["e2eids"][0], "E1");
}

#[test]
fn test_pix_transaction_deserialize_roundtrip() {
    let tx = PixTransaction {
        e2eid: "E12345".to_string(),
        txid: Some("tx1".to_string()),
        amount: "10.00".to_string(),
        payer_name: Some("Maria".to_string()),
        payer_document: Some("doc".to_string()),
        description: Some("desc".to_string()),
        timestamp: Utc::now(),
    };
    let json = serde_json::to_string(&tx).unwrap();
    let back: PixTransaction = serde_json::from_str(&json).unwrap();
    assert_eq!(back.e2eid, "E12345");
    assert_eq!(back.txid, Some("tx1".to_string()));
}

#[test]
fn test_empty_transactions_json() {
    let txs: Vec<PixTransaction> = vec![];
    let json = serde_json::to_string(&txs).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn test_charge_json_with_brcode() {
    let charge = PixCharge {
        txid: "txid".to_string(),
        status: ChargeStatus::Active,
        amount: "10.00".to_string(),
        pix_key: "key".to_string(),
        description: None,
        brcode: Some("00020126...".to_string()),
        debtor: None,
        created_at: Utc::now(),
        expires_at: Utc::now(),
        e2eids: vec![],
    };
    let json = serde_json::to_string(&charge).unwrap();
    assert!(json.contains("00020126..."));
}

#[test]
fn test_charge_json_without_brcode() {
    let charge = PixCharge {
        txid: "txid".to_string(),
        status: ChargeStatus::Active,
        amount: "10.00".to_string(),
        pix_key: "key".to_string(),
        description: None,
        brcode: None,
        debtor: None,
        created_at: Utc::now(),
        expires_at: Utc::now(),
        e2eids: vec![],
    };
    let json = serde_json::to_string(&charge).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["brcode"].is_null());
}

#[test]
fn test_multiple_e2eids_in_charge() {
    let charge = PixCharge {
        txid: "txid".to_string(),
        status: ChargeStatus::Completed,
        amount: "100.00".to_string(),
        pix_key: "key".to_string(),
        description: None,
        brcode: None,
        debtor: None,
        created_at: Utc::now(),
        expires_at: Utc::now(),
        e2eids: vec!["E1".into(), "E2".into(), "E3".into()],
    };
    let json = serde_json::to_string(&charge).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["e2eids"].as_array().unwrap().len(), 3);
}
