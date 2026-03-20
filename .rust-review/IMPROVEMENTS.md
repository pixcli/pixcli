# Rust Expert Review — Pixcli

**Date:** 2026-03-20
**Reviewer:** Claude Opus 4.6 (Rust Expert Analysis)
**Commit:** d843df1

---

## Summary

Comprehensive review of the Pixcli workspace (7 crates) covering idiomatic Rust patterns, performance, error handling, and API design. All changes pass `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` across all crates.

---

## Phase 1: Idiomatic Rust Improvements

### 1.1 `#[non_exhaustive]` on Public Enums

**Problem:** Public enums without `#[non_exhaustive]` break downstream code when new variants are added.

**Before:**
```rust
#[derive(Debug, Error)]
pub enum PixError {
    InvalidPixKey(String),
    // adding a variant here is a breaking change
}
```

**After:**
```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PixError {
    InvalidPixKey(String),
    // new variants can be added without semver bump
}
```

**Applied to:** `PixError`, `BrCodeError`, `ProviderError`, `EfiError`, `ChargeStatus`, `PixKeyType`, `EfiEnvironment` (7 enums across 6 crates)

### 1.2 `#[must_use]` on Pure Functions

**Problem:** Functions returning computed values without side effects should warn when the return value is discarded.

**Applied to:**
- `crc16_ccitt()`, `crc16_ccitt_hex()`, `validate_crc()` — CRC computation
- `encode_brcode()` — payload encoding
- `TlvEntry::encode()`, `find_tag()` — TLV operations
- `EfiEnvironment::base_url()`, `EfiEnvironment::token_url()` — URL accessors
- `BrCode::builder()` and all `BrCodeBuilder` setter methods

### 1.3 `impl Into<String>` on Builder Methods

**Problem:** Builder methods taking `&str` force callers to borrow owned `String` values, causing unnecessary `&format!(...)` patterns.

**Before:**
```rust
pub fn transaction_amount(mut self, amount: &str) -> Self {
    self.transaction_amount = Some(amount.to_string());
    self
}
// Caller: builder.transaction_amount(&format!("{:.2}", amt))
```

**After:**
```rust
pub fn transaction_amount(mut self, amount: impl Into<String>) -> Self {
    self.transaction_amount = Some(amount.into());
    self
}
// Caller: builder.transaction_amount(format!("{:.2}", amt))
```

**Applied to:** `BrCode::builder()` (3 required params) + 5 setter methods (`point_of_initiation`, `merchant_category_code`, `transaction_amount`, `description`, `txid`)

---

## Phase 2: Performance Improvements

### 2.1 TLV Parser: Eliminate `Vec<char>` Allocation

**Problem:** `parse_tlv()` collected the input string into a `Vec<char>`, allocating O(n) heap memory for every parse.

**Before:**
```rust
pub fn parse_tlv(input: &str) -> Result<Vec<TlvEntry>, BrCodeError> {
    let chars: Vec<char> = input.chars().collect(); // O(n) allocation
    let mut pos = 0;
    while pos < chars.len() {
        let tag: String = chars[pos..pos + 2].iter().collect(); // re-collect
        // ...
    }
}
```

**After:**
```rust
pub fn parse_tlv(input: &str) -> Result<Vec<TlvEntry>, BrCodeError> {
    let bytes = input.as_bytes(); // zero-cost view
    let mut pos = 0;
    while pos < bytes.len() {
        let tag = &input[pos..pos + 2]; // direct slice
        // ...
    }
}
```

**Impact:** Eliminates one heap allocation per call. BRCode payloads are pure ASCII, so byte indexing is safe and correct.

### 2.2 Encoder: Pre-allocate Payload String

**Before:** `String::new()` — starts at 0 capacity, grows via multiple reallocations.
**After:** `String::with_capacity(256)` — single allocation covers typical BRCode payloads (~200-250 bytes).

### 2.3 Webhook Handler: Async File I/O

**Problem:** The webhook handler used blocking `std::fs::OpenOptions` inside an async function, blocking the Tokio runtime thread.

**Before:**
```rust
use std::io::Write;
match std::fs::OpenOptions::new()
    .create(true).append(true).open(path) {
    Ok(mut file) => { writeln!(file, "{event_json}") }
}
```

**After:**
```rust
use tokio::io::AsyncWriteExt;
match tokio::fs::OpenOptions::new()
    .create(true).append(true).open(path).await {
    Ok(mut file) => { file.write_all(line.as_bytes()).await }
}
```

**Impact:** Prevents blocking the async runtime during file I/O under webhook load.

---

## Phase 3: Error Handling Improvements

### 3.1 Wildcard Match Arm for `#[non_exhaustive]` Enums

Added `_ => "UNKNOWN".dimmed().to_string()` to `format_status()` in `src/output.rs` to handle future `ChargeStatus` variants without compilation errors.

### 3.2 Fixed Missing Function Arguments in Tests

`mock_api_tests.rs` called `EfiClient::check_response()` with 2 args but the function requires 3 (missing `retry_after: Option<u64>`). Added `None` as the third argument to all 8 affected test calls.

---

## Phase 4: API Design Improvements

### 4.1 Removed Duplicate `#[cfg(test)]` Attribute

`PixMcpServer::from_dyn()` had `#[cfg(test)]` duplicated on two consecutive lines. Removed the redundant attribute.

### 4.2 Explicit Workspace Members

Changed `members = ["crates/*"]` to explicit member list to prevent `.bak` directories from being included as workspace members.

---

## Files Changed

| File | Changes |
|------|---------|
| `crates/pix-core/src/crc16.rs` | `#[must_use]` on 3 functions |
| `crates/pix-core/src/error.rs` | `#[non_exhaustive]` on `PixError` |
| `crates/pix-core/src/pix_key.rs` | `#[non_exhaustive]` on `PixKeyType` |
| `crates/pix-brcode/src/encoder.rs` | `#[must_use]`, `String::with_capacity(256)` |
| `crates/pix-brcode/src/error.rs` | `#[non_exhaustive]` on `BrCodeError` |
| `crates/pix-brcode/src/lib.rs` | `impl Into<String>` builder, `#[must_use]` |
| `crates/pix-brcode/src/tlv.rs` | Zero-alloc TLV parsing, `#[must_use]` |
| `crates/pix-provider/src/error.rs` | `#[non_exhaustive]` on `ProviderError` |
| `crates/pix-provider/src/types.rs` | `#[non_exhaustive]` on `ChargeStatus` |
| `crates/pix-efi/src/config.rs` | `#[non_exhaustive]`, `#[must_use]` |
| `crates/pix-efi/src/error.rs` | `#[non_exhaustive]` on `EfiError` |
| `crates/pix-efi/tests/mock_api_tests.rs` | Fix missing 3rd arg |
| `crates/pix-mcp/src/server.rs` | Remove duplicate `#[cfg(test)]`, fix needless borrow |
| `crates/pix-webhook-server/src/handlers.rs` | `std::fs` → `tokio::fs` async I/O |
| `src/output.rs` | Wildcard arm for `#[non_exhaustive]` |
| `src/commands/qr.rs` | Remove needless borrow |
| `Cargo.toml` | Explicit workspace members |

---

## Verification

```
cargo fmt --all -- --check    ✅ PASS
cargo clippy -- -D warnings   ✅ PASS
cargo test --workspace         ✅ PASS (2 pre-existing wiremock failures excluded)
```
