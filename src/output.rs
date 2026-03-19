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
    }
}
