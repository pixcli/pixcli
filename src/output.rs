//! Output formatting for the Pix CLI.
//!
//! Supports three output modes: human-readable, JSON, and table.

use clap::ValueEnum;
use colored::Colorize;
use comfy_table::{presets, Table};
use pix_provider::{Balance, ChargeStatus, PixCharge, PixTransaction, PixTransfer};
use serde::Serialize;

/// Output format for CLI results.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text with colours and emojis.
    Human,
    /// Machine-readable JSON.
    Json,
    /// Tabular output.
    Table,
}

/// Prints any serialisable value as JSON.
pub fn print_json<T: Serialize + ?Sized>(data: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(data)?);
    Ok(())
}

// ── Balance ─────────────────────────────────────────────────────────────────

/// Prints a balance result.
pub fn print_balance(balance: &Balance, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => print_json(balance),
        OutputFormat::Human | OutputFormat::Table => {
            println!("💰 Balance: R${}", balance.available.green().bold());
            Ok(())
        }
    }
}

// ── Charge ──────────────────────────────────────────────────────────────────

/// Prints a single charge.
pub fn print_charge(charge: &PixCharge, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => print_json(charge),
        OutputFormat::Human => {
            print_charge_human(charge);
            Ok(())
        }
        OutputFormat::Table => {
            print_charges_table(std::slice::from_ref(charge));
            Ok(())
        }
    }
}

/// Prints a list of charges.
pub fn print_charges(charges: &[PixCharge], format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => print_json(charges),
        OutputFormat::Human => {
            if charges.is_empty() {
                println!("No charges found.");
            } else {
                for c in charges {
                    print_charge_human(c);
                    println!();
                }
            }
            Ok(())
        }
        OutputFormat::Table => {
            print_charges_table(charges);
            Ok(())
        }
    }
}

fn print_charge_human(charge: &PixCharge) {
    let status_str = format_status(charge.status);
    println!("📋 Charge: {}", charge.txid.bold());
    println!("   Status:  {}", status_str);
    println!("   Amount:  R${}", charge.amount.green());
    println!("   Key:     {}", charge.pix_key);
    if let Some(ref desc) = charge.description {
        println!("   Desc:    {}", desc);
    }
    if let Some(ref code) = charge.brcode {
        println!("   Copy/Paste: {}", code.dimmed());
    }
    println!("   Created: {}", charge.created_at);
    println!("   Expires: {}", charge.expires_at);
}

fn print_charges_table(charges: &[PixCharge]) {
    if charges.is_empty() {
        println!("No charges found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_header(vec!["TxID", "Status", "Amount", "Key", "Created"]);
    for c in charges {
        table.add_row(vec![
            &c.txid,
            &c.status.to_string(),
            &format!("R${}", c.amount),
            &c.pix_key,
            &c.created_at.format("%Y-%m-%d %H:%M").to_string(),
        ]);
    }
    println!("{table}");
}

// ── Pix Transactions ────────────────────────────────────────────────────────

/// Prints a single Pix transaction.
pub fn print_pix_transaction(tx: &PixTransaction, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => print_json(tx),
        OutputFormat::Human => {
            print_pix_transaction_human(tx);
            Ok(())
        }
        OutputFormat::Table => {
            print_pix_transactions_table(std::slice::from_ref(tx));
            Ok(())
        }
    }
}

/// Prints a list of Pix transactions.
pub fn print_pix_transactions(txs: &[PixTransaction], format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => print_json(txs),
        OutputFormat::Human => {
            if txs.is_empty() {
                println!("No transactions found.");
            } else {
                for tx in txs {
                    print_pix_transaction_human(tx);
                    println!();
                }
            }
            Ok(())
        }
        OutputFormat::Table => {
            print_pix_transactions_table(txs);
            Ok(())
        }
    }
}

fn print_pix_transaction_human(tx: &PixTransaction) {
    println!("💸 Pix: {}", tx.e2eid.bold());
    if let Some(ref txid) = tx.txid {
        println!("   TxID:    {}", txid);
    }
    println!("   Amount:  R${}", tx.amount.green());
    if let Some(ref name) = tx.payer_name {
        println!("   Payer:   {}", name);
    }
    if let Some(ref desc) = tx.description {
        println!("   Info:    {}", desc);
    }
    println!("   Time:    {}", tx.timestamp);
}

fn print_pix_transactions_table(txs: &[PixTransaction]) {
    if txs.is_empty() {
        println!("No transactions found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_header(vec!["E2EID", "TxID", "Amount", "Payer", "Time"]);
    for tx in txs {
        table.add_row(vec![
            &tx.e2eid,
            tx.txid.as_deref().unwrap_or("-"),
            &format!("R${}", tx.amount),
            tx.payer_name.as_deref().unwrap_or("-"),
            &tx.timestamp.format("%Y-%m-%d %H:%M").to_string(),
        ]);
    }
    println!("{table}");
}

// ── Transfer ────────────────────────────────────────────────────────────────

/// Prints a Pix transfer result.
#[allow(dead_code)]
pub fn print_transfer(transfer: &PixTransfer, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => print_json(transfer),
        OutputFormat::Human | OutputFormat::Table => {
            println!("✅ Pix Sent!");
            println!("   E2EID:   {}", transfer.e2eid.bold());
            println!("   Amount:  R${}", transfer.amount.green());
            println!("   Status:  {}", transfer.status);
            println!("   Time:    {}", transfer.timestamp);
            Ok(())
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn format_status(status: ChargeStatus) -> String {
    match status {
        ChargeStatus::Active => "ACTIVE".yellow().to_string(),
        ChargeStatus::Completed => "COMPLETED".green().to_string(),
        ChargeStatus::RemovedByUser => "REMOVED_BY_USER".red().to_string(),
        ChargeStatus::RemovedByPsp => "REMOVED_BY_PSP".red().to_string(),
        ChargeStatus::Expired => "EXPIRED".dimmed().to_string(),
        _ => "UNKNOWN".dimmed().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_balance() -> Balance {
        Balance {
            available: "1234.56".to_string(),
        }
    }

    fn sample_charge() -> PixCharge {
        PixCharge {
            txid: "pix550e8400e29b41d4a716446655440000".to_string(),
            status: ChargeStatus::Active,
            amount: "100.00".to_string(),
            pix_key: "user@example.com".to_string(),
            description: Some("Test charge".to_string()),
            brcode: Some("00020126...".to_string()),
            debtor: None,
            created_at: Utc::now(),
            expires_at: Utc::now(),
            e2eids: vec![],
        }
    }

    fn sample_charge_no_optionals() -> PixCharge {
        PixCharge {
            txid: "txid123".to_string(),
            status: ChargeStatus::Completed,
            amount: "50.00".to_string(),
            pix_key: "key@test.com".to_string(),
            description: None,
            brcode: None,
            debtor: None,
            created_at: Utc::now(),
            expires_at: Utc::now(),
            e2eids: vec![],
        }
    }

    fn sample_transaction() -> PixTransaction {
        PixTransaction {
            e2eid: "E12345678901234567890123456789AB".to_string(),
            txid: Some("txid123".to_string()),
            amount: "99.99".to_string(),
            payer_name: Some("João Silva".to_string()),
            payer_document: Some("52998224725".to_string()),
            description: Some("Payment description".to_string()),
            timestamp: Utc::now(),
        }
    }

    fn sample_transaction_no_optionals() -> PixTransaction {
        PixTransaction {
            e2eid: "E00000000000000000000000000000001".to_string(),
            txid: None,
            amount: "10.00".to_string(),
            payer_name: None,
            payer_document: None,
            description: None,
            timestamp: Utc::now(),
        }
    }

    fn sample_transfer() -> PixTransfer {
        PixTransfer {
            e2eid: "E12345".to_string(),
            id_envio: "ID1".to_string(),
            amount: "25.00".to_string(),
            status: "REALIZADO".to_string(),
            timestamp: Utc::now(),
        }
    }

    // ── print_json ──

    #[test]
    fn test_print_json_balance() {
        let b = sample_balance();
        assert!(print_json(&b).is_ok());
    }

    #[test]
    fn test_print_json_charge() {
        let c = sample_charge();
        assert!(print_json(&c).is_ok());
    }

    #[test]
    fn test_print_json_vec() {
        let charges: Vec<PixCharge> = vec![sample_charge()];
        assert!(print_json(&charges).is_ok());
    }

    // ── print_balance ──

    #[test]
    fn test_print_balance_json() {
        assert!(print_balance(&sample_balance(), OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_balance_human() {
        assert!(print_balance(&sample_balance(), OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_balance_table() {
        assert!(print_balance(&sample_balance(), OutputFormat::Table).is_ok());
    }

    // ── print_charge ──

    #[test]
    fn test_print_charge_json() {
        assert!(print_charge(&sample_charge(), OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_charge_human() {
        assert!(print_charge(&sample_charge(), OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_charge_human_no_optionals() {
        assert!(print_charge(&sample_charge_no_optionals(), OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_charge_table() {
        assert!(print_charge(&sample_charge(), OutputFormat::Table).is_ok());
    }

    // ── print_charges ──

    #[test]
    fn test_print_charges_json_empty() {
        assert!(print_charges(&[], OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_charges_json_nonempty() {
        let charges = vec![sample_charge(), sample_charge_no_optionals()];
        assert!(print_charges(&charges, OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_charges_human_empty() {
        assert!(print_charges(&[], OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_charges_human_nonempty() {
        let charges = vec![sample_charge()];
        assert!(print_charges(&charges, OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_charges_table_empty() {
        assert!(print_charges(&[], OutputFormat::Table).is_ok());
    }

    #[test]
    fn test_print_charges_table_nonempty() {
        let charges = vec![sample_charge(), sample_charge_no_optionals()];
        assert!(print_charges(&charges, OutputFormat::Table).is_ok());
    }

    // ── print_pix_transaction ──

    #[test]
    fn test_print_pix_transaction_json() {
        assert!(print_pix_transaction(&sample_transaction(), OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_pix_transaction_human() {
        assert!(print_pix_transaction(&sample_transaction(), OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_pix_transaction_human_no_optionals() {
        assert!(
            print_pix_transaction(&sample_transaction_no_optionals(), OutputFormat::Human).is_ok()
        );
    }

    #[test]
    fn test_print_pix_transaction_table() {
        assert!(print_pix_transaction(&sample_transaction(), OutputFormat::Table).is_ok());
    }

    // ── print_pix_transactions ──

    #[test]
    fn test_print_pix_transactions_json_empty() {
        assert!(print_pix_transactions(&[], OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_pix_transactions_json_nonempty() {
        let txs = vec![sample_transaction()];
        assert!(print_pix_transactions(&txs, OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_pix_transactions_human_empty() {
        assert!(print_pix_transactions(&[], OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_pix_transactions_human_nonempty() {
        let txs = vec![sample_transaction(), sample_transaction_no_optionals()];
        assert!(print_pix_transactions(&txs, OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_pix_transactions_table_empty() {
        assert!(print_pix_transactions(&[], OutputFormat::Table).is_ok());
    }

    #[test]
    fn test_print_pix_transactions_table_nonempty() {
        let txs = vec![sample_transaction(), sample_transaction_no_optionals()];
        assert!(print_pix_transactions(&txs, OutputFormat::Table).is_ok());
    }

    // ── print_transfer ──

    #[test]
    fn test_print_transfer_json() {
        assert!(print_transfer(&sample_transfer(), OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_print_transfer_human() {
        assert!(print_transfer(&sample_transfer(), OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_print_transfer_table() {
        assert!(print_transfer(&sample_transfer(), OutputFormat::Table).is_ok());
    }

    // ── format_status ──

    #[test]
    fn test_format_status_all_variants() {
        // Just ensure no panics and non-empty output
        for status in [
            ChargeStatus::Active,
            ChargeStatus::Completed,
            ChargeStatus::RemovedByUser,
            ChargeStatus::RemovedByPsp,
            ChargeStatus::Expired,
        ] {
            let formatted = format_status(status);
            assert!(!formatted.is_empty());
        }
    }

    // ── OutputFormat enum ──

    #[test]
    fn test_output_format_debug() {
        assert_eq!(format!("{:?}", OutputFormat::Human), "Human");
        assert_eq!(format!("{:?}", OutputFormat::Json), "Json");
        assert_eq!(format!("{:?}", OutputFormat::Table), "Table");
    }

    #[test]
    fn test_output_format_clone() {
        let f = OutputFormat::Json;
        let c = f;
        assert_eq!(f, c);
    }

    #[test]
    fn test_output_format_eq() {
        assert_eq!(OutputFormat::Human, OutputFormat::Human);
        assert_ne!(OutputFormat::Human, OutputFormat::Json);
    }

    // ── Charges with all statuses ──

    #[test]
    fn test_print_charge_human_all_statuses() {
        for status in [
            ChargeStatus::Active,
            ChargeStatus::Completed,
            ChargeStatus::RemovedByUser,
            ChargeStatus::RemovedByPsp,
            ChargeStatus::Expired,
        ] {
            let mut charge = sample_charge();
            charge.status = status;
            assert!(print_charge(&charge, OutputFormat::Human).is_ok());
        }
    }

    #[test]
    fn test_print_charges_table_all_statuses() {
        let charges: Vec<PixCharge> = [
            ChargeStatus::Active,
            ChargeStatus::Completed,
            ChargeStatus::RemovedByUser,
            ChargeStatus::RemovedByPsp,
            ChargeStatus::Expired,
        ]
        .iter()
        .map(|&status| {
            let mut c = sample_charge();
            c.status = status;
            c
        })
        .collect();
        assert!(print_charges(&charges, OutputFormat::Table).is_ok());
    }
}
