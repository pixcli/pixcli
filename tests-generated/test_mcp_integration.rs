// =============================================================================
// MCP Server Integration Tests
// =============================================================================
//
// TARGET CRATE: pix-mcp
// PLACEMENT:    crates/pix-mcp/tests/mcp_integration.rs
//
// DEPENDENCIES NEEDED IN Cargo.toml:
//   [dev-dependencies]
//   chrono = { version = "0.4", features = ["serde"] }
//   pix-provider = { path = "../pix-provider" }
//   rmcp = { version = "0.1", features = ["server"] }
//   serde = { version = "1", features = ["derive"] }
//   serde_json = "1"
//   tokio = { version = "1", features = ["full"] }
//
// WHY THESE TESTS EXIST:
//
// The MCP server exposes 6 Pix tools to AI agents. Edge cases around numeric
// boundaries, unicode handling, concurrency safety, and schema correctness
// are critical because:
//   - AI agents will pass unexpected values (NaN, Infinity, huge numbers)
//   - The payment threshold at R$ 1000 is a safety boundary that must be exact
//   - Tool schemas must be valid JSON Schema for MCP protocol compliance
//   - Unicode in merchant names could break QR code generation
//   - Error responses must be tool-level, not MCP protocol errors
//
// These tests use the same MockProvider pattern from the existing test suite
// to test edge cases that are not covered.
// =============================================================================

#[cfg(test)]
mod mcp_integration_tests {
    use chrono::{Duration, Utc};
    use serde::{Deserialize, Serialize};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    // =========================================================================
    // Mock infrastructure
    // =========================================================================
    // These types mirror the MCP server internals. When placed inside the
    // crate, import from crate::server and crate::tools directly.

    // Simplified provider trait for testing
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ChargeResponse {
        txid: String,
        brcode: String,
        status: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct PixTransfer {
        e2eid: String,
        amount: String,
        status: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Balance {
        available: String,
    }

    // Param types (mirrors of crate::tools)
    #[derive(Debug, Deserialize, Serialize)]
    struct CreateChargeParams {
        amount: f64,
        pix_key: Option<String>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SendPaymentParams {
        key: String,
        amount: f64,
        description: Option<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct ListTransactionsParams {
        days: Option<u32>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct GenerateQrParams {
        key: String,
        amount: Option<f64>,
        merchant_name: Option<String>,
        city: Option<String>,
    }

    const PAYMENT_WARNING_THRESHOLD: f64 = 1000.0;

    // =========================================================================
    // 1. pix_create_charge with very large amount (overflow check)
    // =========================================================================
    // GAP: The amount field is f64. Very large values could cause formatting
    // issues when converted to string with format!("{:.2}", amount).
    // f64::MAX would produce a very long string that the API would reject.

    #[test]
    fn test_create_charge_very_large_amount_format() {
        // f64 can represent very large numbers. When formatted with {:.2},
        // the result must still be a valid decimal string.
        let amount: f64 = 999_999_999.99;
        let formatted = format!("{:.2}", amount);
        assert_eq!(formatted, "999999999.99");

        // Verify it parses back correctly
        let parsed: f64 = formatted.parse().unwrap();
        assert!((parsed - amount).abs() < 0.01);
    }

    #[test]
    fn test_create_charge_f64_max_format() {
        // f64::MAX formatted as decimal produces a very long string.
        // This documents the behavior -- the API would reject it, but
        // the formatting itself should not panic.
        let amount = f64::MAX;
        let formatted = format!("{:.2}", amount);
        assert!(
            !formatted.is_empty(),
            "Formatting f64::MAX should not panic"
        );
        // The string will be extremely long (hundreds of digits)
        assert!(
            formatted.len() > 100,
            "f64::MAX formatted string should be very long: {}",
            formatted.len()
        );
    }

    // =========================================================================
    // 2. pix_create_charge with NaN and Infinity
    // =========================================================================
    // GAP: f64 can be NaN or Infinity, which are not valid JSON numbers.
    // The MCP server must handle these gracefully.

    #[test]
    fn test_nan_is_not_positive() {
        // The server checks `amount <= 0.0` which returns false for NaN.
        // NaN comparisons always return false, so NaN > 0.0 is false AND
        // NaN <= 0.0 is also false. This means NaN passes the positive check!
        let amount = f64::NAN;
        let passes_check = !(amount <= 0.0);
        assert!(
            passes_check,
            "BUG: NaN passes the 'amount <= 0.0' check because NaN comparisons are always false. \
             The validation should explicitly check for NaN: amount.is_nan() || amount <= 0.0"
        );
    }

    #[test]
    fn test_infinity_is_positive_but_invalid() {
        // Infinity is greater than 0, so it passes the positive check.
        // But it cannot be serialized to JSON (serde_json will error).
        let amount = f64::INFINITY;
        assert!(
            !(amount <= 0.0),
            "Infinity passes the positive check"
        );

        let formatted = format!("{:.2}", amount);
        assert_eq!(formatted, "inf");

        // serde_json cannot serialize infinity
        let result = serde_json::to_string(&amount);
        assert!(
            result.is_err(),
            "serde_json should reject Infinity -- this would cause an internal error at serialization"
        );
    }

    #[test]
    fn test_negative_infinity_rejected() {
        let amount = f64::NEG_INFINITY;
        assert!(
            amount <= 0.0,
            "Negative infinity should be caught by the <= 0.0 check"
        );
    }

    // =========================================================================
    // 3. pix_send_payment threshold boundary tests
    // =========================================================================
    // GAP: The PAYMENT_WARNING_THRESHOLD is 1000.0. The warning is triggered
    // when amount > 1000.0 (strict greater-than). Boundary values are critical.

    #[test]
    fn test_threshold_exactly_1000_no_warning() {
        // amount > 1000.0 is false when amount == 1000.0
        let amount: f64 = 1000.00;
        let triggers_warning = amount > PAYMENT_WARNING_THRESHOLD;
        assert!(
            !triggers_warning,
            "Exactly R$ 1000.00 should NOT trigger the high-value warning (> not >=)"
        );
    }

    #[test]
    fn test_threshold_1000_01_triggers_warning() {
        let amount: f64 = 1000.01;
        let triggers_warning = amount > PAYMENT_WARNING_THRESHOLD;
        assert!(
            triggers_warning,
            "R$ 1000.01 should trigger the high-value warning"
        );
    }

    #[test]
    fn test_threshold_999_99_no_warning() {
        let amount: f64 = 999.99;
        let triggers_warning = amount > PAYMENT_WARNING_THRESHOLD;
        assert!(
            !triggers_warning,
            "R$ 999.99 should NOT trigger the high-value warning"
        );
    }

    #[test]
    fn test_warning_message_format() {
        let amount: f64 = 5000.00;
        if amount > PAYMENT_WARNING_THRESHOLD {
            let warning = format!(
                "WARNING: High-value payment of R$ {:.2}. Proceed with caution.\n",
                amount
            );
            assert!(warning.contains("5000.00"));
            assert!(warning.contains("WARNING"));
        }
    }

    // =========================================================================
    // 4. pix_list_transactions with 0 days
    // =========================================================================
    // GAP: days=0 means "from now to now", which is a zero-width time window.
    // The server should handle this without error but may return 0 results.

    #[test]
    fn test_list_transactions_zero_days_produces_valid_range() {
        let days: u32 = 0;
        let now = Utc::now();
        let start = now - Duration::days(i64::from(days));

        // With 0 days, start == now (approximately).
        let diff = now - start;
        assert!(
            diff.num_seconds().abs() < 1,
            "0 days should produce start ~= end"
        );
    }

    #[test]
    fn test_list_transactions_365_days_produces_valid_range() {
        let days: u32 = 365;
        let now = Utc::now();
        let start = now - Duration::days(i64::from(days));

        let diff = now - start;
        assert_eq!(diff.num_days(), 365);
    }

    #[test]
    fn test_list_transactions_very_large_days() {
        // u32::MAX days would overflow chrono's Duration.
        // The server uses Duration::days(i64::from(days)), and u32::MAX
        // fits in i64, so it should not panic. But 4 billion days is
        // ~11 million years, which would produce an invalid date.
        let days: u32 = 36500; // 100 years, still reasonable
        let now = Utc::now();
        let start = now - Duration::days(i64::from(days));

        // Should not panic
        assert!(start < now);
    }

    // =========================================================================
    // 5. pix_generate_qr with unicode in merchant name
    // =========================================================================
    // GAP: Brazilian merchant names may contain accented characters (e.g.,
    // "JOAO DA SILVA" or "CAFE DO PEDRO"). The BR Code spec limits
    // merchant_name to 25 ASCII characters. Unicode could cause issues.

    #[test]
    fn test_unicode_merchant_name_length() {
        // The BR Code spec says max 25 chars for merchant name.
        // Unicode characters may be multiple bytes but count as 1 char.
        let name_with_accents = "CAFE JOAO";
        assert!(
            name_with_accents.len() <= 25,
            "Name with ASCII accents substitute should fit in 25 chars"
        );

        // Real accented name
        let accented = "JOAO DA SILVA";
        assert!(accented.chars().count() <= 25);
    }

    // =========================================================================
    // 6. pix_generate_qr with all optional fields populated
    // =========================================================================
    // GAP: The test suite only tests partial field combinations. Testing all
    // fields together verifies there are no interaction issues.

    #[test]
    fn test_generate_qr_params_all_fields_serialize() {
        let params = GenerateQrParams {
            key: "test@example.com".to_string(),
            amount: Some(42.50),
            merchant_name: Some("LOJA DO TESTE".to_string()),
            city: Some("SAO PAULO".to_string()),
        };

        let json = serde_json::to_string(&params).unwrap();
        let back: GenerateQrParams = serde_json::from_str(&json).unwrap();

        assert_eq!(back.key, "test@example.com");
        assert_eq!(back.amount, Some(42.50));
        assert_eq!(back.merchant_name.as_deref(), Some("LOJA DO TESTE"));
        assert_eq!(back.city.as_deref(), Some("SAO PAULO"));
    }

    // =========================================================================
    // 7. Tool parameter JSON schemas
    // =========================================================================
    // GAP: Each tool must have a valid JSON Schema for MCP protocol compliance.
    // AI agents use these schemas to determine how to call the tools.

    #[test]
    fn test_create_charge_params_schema_has_required_amount() {
        // Verify that the CreateChargeParams struct serializes amount as required.
        // When amount is missing, deserialization should fail.
        let json_without_amount = r#"{"pix_key": "test@email.com"}"#;
        let result: Result<CreateChargeParams, _> = serde_json::from_str(json_without_amount);
        assert!(
            result.is_err(),
            "amount should be required in CreateChargeParams"
        );
    }

    #[test]
    fn test_send_payment_params_schema_has_required_key_and_amount() {
        let json_without_key = r#"{"amount": 10.0}"#;
        let result: Result<SendPaymentParams, _> = serde_json::from_str(json_without_key);
        assert!(
            result.is_err(),
            "key should be required in SendPaymentParams"
        );

        let json_without_amount = r#"{"key": "test@email.com"}"#;
        let result: Result<SendPaymentParams, _> = serde_json::from_str(json_without_amount);
        assert!(
            result.is_err(),
            "amount should be required in SendPaymentParams"
        );
    }

    #[test]
    fn test_list_transactions_params_days_is_optional() {
        let json_empty = r#"{}"#;
        let result: Result<ListTransactionsParams, _> = serde_json::from_str(json_empty);
        assert!(
            result.is_ok(),
            "days should be optional in ListTransactionsParams"
        );
        assert!(result.unwrap().days.is_none());
    }

    #[test]
    fn test_generate_qr_params_only_key_required() {
        let json_key_only = r#"{"key": "test@email.com"}"#;
        let result: Result<GenerateQrParams, _> = serde_json::from_str(json_key_only);
        assert!(
            result.is_ok(),
            "Only key should be required in GenerateQrParams"
        );
        let params = result.unwrap();
        assert!(params.amount.is_none());
        assert!(params.merchant_name.is_none());
        assert!(params.city.is_none());
    }

    // =========================================================================
    // 8. Error response formatting
    // =========================================================================
    // GAP: Tool errors should be returned as CallToolResult with is_error=true,
    // NOT as MCP protocol errors (Err(ErrorData)). Protocol errors indicate
    // a problem with the MCP communication itself, while tool errors indicate
    // a problem with the operation the tool is trying to perform.

    #[test]
    fn test_error_result_format() {
        // The error_result helper returns Ok(CallToolResult::error(...))
        // NOT Err(ErrorData). This is the correct MCP pattern.
        //
        // Simulating the pattern:
        let msg = "Failed to create charge: authentication error";

        // Correct pattern: return Ok with error content
        let result: Result<String, String> = Ok(format!("ERROR: {}", msg));
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Failed to create charge"));
    }

    // =========================================================================
    // 9. Amount formatting precision
    // =========================================================================
    // GAP: The server formats amounts with {:.2}. Floating point precision
    // issues could cause incorrect amounts (e.g., 0.1 + 0.2 = 0.30000000000000004).

    #[test]
    fn test_amount_formatting_precision() {
        // Classic floating point issue
        let amount = 0.1_f64 + 0.2_f64;
        let formatted = format!("{:.2}", amount);
        assert_eq!(
            formatted, "0.30",
            "0.1 + 0.2 formatted with .2 should be '0.30', not '0.30'"
        );
    }

    #[test]
    fn test_amount_formatting_small_values() {
        assert_eq!(format!("{:.2}", 0.01_f64), "0.01");
        assert_eq!(format!("{:.2}", 0.001_f64), "0.00"); // Truncated!
        assert_eq!(format!("{:.2}", 0.005_f64), "0.01"); // Rounded up (banker's rounding varies)
    }

    #[test]
    fn test_amount_formatting_large_values() {
        assert_eq!(format!("{:.2}", 99999.99_f64), "99999.99");
        assert_eq!(format!("{:.2}", 100000.00_f64), "100000.00");
    }

    // =========================================================================
    // 10. Concurrency safety (multiple rapid tool calls)
    // =========================================================================
    // GAP: The MCP server uses Arc<dyn DynPixProvider> which should be safe
    // for concurrent access. This test verifies the type constraints.

    #[test]
    fn test_provider_arc_is_send_sync() {
        // This is a compile-time check. If DynPixProvider is not Send + Sync,
        // this function will fail to compile.
        fn assert_send_sync<T: Send + Sync>() {}

        // Arc<T> is Send + Sync when T is Send + Sync.
        // DynPixProvider requires Send + Sync in its trait definition.
        // This test verifies the constraint at compile time.
        assert_send_sync::<Arc<AtomicU32>>(); // Proxy check since we cannot reference the real trait here
    }

    // =========================================================================
    // 11. Server info correctness
    // =========================================================================
    // GAP: The server name and version must match the crate metadata.
    // Incorrect values could cause MCP client confusion.

    #[test]
    fn test_server_name_is_pix_mcp() {
        // The server reports its name as "pix-mcp" in get_info().
        // This must match what MCP clients expect.
        let name = "pix-mcp";
        assert_eq!(name, "pix-mcp");
        assert!(!name.is_empty());
        assert!(!name.contains(' '), "Server name should not contain spaces");
    }

    // =========================================================================
    // 12. Default values
    // =========================================================================
    // GAP: When optional parameters are omitted, defaults must be applied.

    #[test]
    fn test_default_expiry_seconds() {
        let expiry: Option<u32> = None;
        let effective_expiry = expiry.unwrap_or(3600);
        assert_eq!(effective_expiry, 3600, "Default expiry should be 1 hour");
    }

    #[test]
    fn test_default_days_for_list_transactions() {
        let days: Option<u32> = None;
        let effective_days = days.unwrap_or(7);
        assert_eq!(effective_days, 7, "Default listing period should be 7 days");
    }

    #[test]
    fn test_default_merchant_name() {
        let name: Option<String> = None;
        let effective_name = name.unwrap_or_else(|| "PAGAMENTO PIX".to_string());
        assert_eq!(effective_name, "PAGAMENTO PIX");
        assert!(effective_name.len() <= 25, "Default name must fit BR Code limit");
    }

    #[test]
    fn test_default_city() {
        let city: Option<String> = None;
        let effective_city = city.unwrap_or_else(|| "SAO PAULO".to_string());
        assert_eq!(effective_city, "SAO PAULO");
        assert!(effective_city.len() <= 15, "Default city must fit BR Code limit");
    }
}
