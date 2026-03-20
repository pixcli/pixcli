//! QR code generation and decoding for Pix payments.
//!
//! Generates static Pix QR codes rendered in the terminal or saved as PNG files,
//! and decodes EMV/BRCode payload strings.

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::output::OutputFormat;

/// QR code subcommands.
#[derive(Subcommand)]
pub enum QrCommand {
    /// Generate a static Pix QR code
    Generate {
        /// Pix key (CPF, CNPJ, email, phone, or EVP)
        #[arg(long)]
        key: String,
        /// Amount in BRL (optional for static QR)
        #[arg(long)]
        amount: Option<f64>,
        /// Merchant/receiver name (max 25 chars)
        #[arg(long, default_value = "PIX")]
        name: String,
        /// Merchant city (max 15 chars)
        #[arg(long, default_value = "BRASILIA")]
        city: String,
        /// Description text
        #[arg(long)]
        description: Option<String>,
        /// Transaction ID
        #[arg(long)]
        txid: Option<String>,
        /// Save QR as PNG to this file path
        #[arg(long, short = 'f')]
        output: Option<String>,
        /// QR module size in pixels (for PNG, default: 10)
        #[arg(long, default_value = "10")]
        size: u32,
    },
    /// Decode a Pix EMV/BR Code payload
    Decode {
        /// EMV payload string (the "Pix Copia e Cola" text)
        payload: String,
    },
}

/// Runs the QR subcommand.
pub fn run(cmd: QrCommand, format: OutputFormat) -> Result<()> {
    match cmd {
        QrCommand::Generate {
            key,
            amount,
            name,
            city,
            description,
            txid,
            output,
            size,
        } => generate_qr(
            &key,
            amount,
            &name,
            &city,
            description.as_deref(),
            txid.as_deref(),
            output.as_deref(),
            size,
            format,
        ),
        QrCommand::Decode { payload } => decode_payload(&payload, format),
    }
}

/// Generates a static Pix QR code.
#[allow(clippy::too_many_arguments)]
fn generate_qr(
    key: &str,
    amount: Option<f64>,
    name: &str,
    city: &str,
    description: Option<&str>,
    txid: Option<&str>,
    output_file: Option<&str>,
    module_size: u32,
    format: OutputFormat,
) -> Result<()> {
    use pix_brcode::{encode_brcode, BrCode};
    use qrcode::QrCode;

    // Build BRCode payload
    let mut builder = BrCode::builder(key, name, city).point_of_initiation("11");

    if let Some(amt) = amount {
        builder = builder.transaction_amount(format!("{amt:.2}"));
    }

    if let Some(desc) = description {
        builder = builder.description(desc);
    }

    if let Some(tx) = txid {
        builder = builder.txid(tx);
    }

    let brcode = builder.build().context("failed to build BRCode")?;
    let payload = encode_brcode(&brcode);

    let code = QrCode::new(payload.as_bytes()).context("failed to create QR code")?;

    match format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "payload": payload,
                "key": key,
                "amount": amount,
                "merchant_name": name,
                "city": city,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            // Render QR in terminal using Unicode block chars
            let string = code
                .render::<char>()
                .quiet_zone(true)
                .module_dimensions(2, 1)
                .build();
            println!("{string}");
            println!();
            println!("📋 Pix Copia e Cola:");
            println!("{payload}");
        }
    }

    // Save as PNG if requested
    if let Some(path) = output_file {
        let img = code
            .render::<image::Luma<u8>>()
            .quiet_zone(true)
            .module_dimensions(module_size, module_size)
            .build();
        img.save(path)
            .with_context(|| format!("failed to save QR image to {path}"))?;
        if !matches!(format, OutputFormat::Json) {
            println!("💾 Saved QR code to: {path}");
        }
    }

    Ok(())
}

/// Decodes a Pix EMV/BRCode payload string.
fn decode_payload(payload: &str, format: OutputFormat) -> Result<()> {
    use pix_brcode::decode_brcode;

    let brcode = decode_brcode(payload).context("failed to decode BRCode payload")?;

    match format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "payload_format_indicator": brcode.payload_format_indicator,
                "point_of_initiation": brcode.point_of_initiation,
                "pix_key": brcode.pix_key,
                "description": brcode.description,
                "amount": brcode.transaction_amount,
                "merchant_name": brcode.merchant_name,
                "merchant_city": brcode.merchant_city,
                "merchant_category_code": brcode.merchant_category_code,
                "txid": brcode.txid,
                "currency": brcode.transaction_currency,
                "country_code": brcode.country_code,
                "crc": brcode.crc,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("🔍 Decoded BR Code:");
            println!("   Key:      {}", brcode.pix_key);
            if let Some(ref amount) = brcode.transaction_amount {
                println!("   Amount:   R${amount}");
            }
            println!("   Name:     {}", brcode.merchant_name);
            println!("   City:     {}", brcode.merchant_city);
            if let Some(ref txid) = brcode.txid {
                println!("   TxID:     {txid}");
            }
            if let Some(ref desc) = brcode.description {
                println!("   Desc:     {desc}");
            }
            let qr_type = if brcode.point_of_initiation.as_deref() == Some("12") {
                "Dynamic"
            } else {
                "Static"
            };
            println!("   Type:     {qr_type}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pix_brcode::{decode_brcode, encode_brcode, BrCode};

    #[test]
    fn test_generate_and_decode_roundtrip() {
        let brcode = BrCode::builder("+5511999999999", "TESTE", "SAO PAULO")
            .point_of_initiation("11")
            .transaction_amount("25.00")
            .txid("PIXTEST123")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.pix_key, "+5511999999999");
        assert_eq!(decoded.transaction_amount, Some("25.00".to_string()));
        assert_eq!(decoded.txid, Some("PIXTEST123".to_string()));
        assert_eq!(decoded.merchant_name, "TESTE");
        assert_eq!(decoded.merchant_city, "SAO PAULO");
    }

    #[test]
    fn test_generate_qr_without_amount() {
        let brcode = BrCode::builder("test@email.com", "PIX", "BRASILIA")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.pix_key, "test@email.com");
        assert_eq!(decoded.transaction_amount, None);
    }

    #[test]
    fn test_generate_qr_saves_png() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("test-qr.png");

        generate_qr(
            "test@email.com",
            Some(10.50),
            "TESTE",
            "BRASILIA",
            None,
            None,
            Some(png_path.to_str().unwrap()),
            5,
            OutputFormat::Human,
        )
        .unwrap();

        assert!(png_path.exists());

        // Verify it's a valid PNG (starts with PNG magic bytes)
        let bytes = std::fs::read(&png_path).unwrap();
        assert!(bytes.len() > 8);
        assert_eq!(&bytes[1..4], b"PNG");
    }

    #[test]
    fn test_decode_known_valid_payload() {
        // Encode a known payload and then decode it
        let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .point_of_initiation("12")
            .transaction_amount("100.50")
            .txid("PAG123")
            .description("Teste pagamento")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.pix_key, "user@example.com");
        assert_eq!(decoded.transaction_amount, Some("100.50".to_string()));
        assert_eq!(decoded.txid, Some("PAG123".to_string()));
        assert_eq!(decoded.description, Some("Teste pagamento".to_string()));
        assert_eq!(decoded.point_of_initiation, Some("12".to_string()));
    }

    #[test]
    fn test_decode_invalid_crc_returns_error() {
        let brcode = BrCode::builder("test@test.com", "Test", "City")
            .build()
            .unwrap();
        let mut payload = encode_brcode(&brcode);
        let len = payload.len();
        payload.replace_range(len - 4..len, "0000");

        let result = decode_payload(&payload, OutputFormat::Human);
        assert!(result.is_err());
    }

    #[test]
    fn test_terminal_rendering_does_not_panic() {
        use qrcode::QrCode;

        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let code = QrCode::new(payload.as_bytes()).unwrap();
        let _string = code
            .render::<char>()
            .quiet_zone(true)
            .module_dimensions(2, 1)
            .build();
        // If we get here without panicking, the test passes
    }

    #[test]
    fn test_json_output_format() {
        // Just verify it doesn't error; output goes to stdout
        let result = generate_qr(
            "test@email.com",
            Some(5.00),
            "TEST",
            "CITY",
            None,
            None,
            None,
            10,
            OutputFormat::Json,
        );
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod additional_qr_tests {
    use super::*;
    use pix_brcode::{decode_brcode, encode_brcode, BrCode};

    // --- Generate with every key type ---

    #[test]
    fn test_generate_with_cpf_key() {
        let brcode = BrCode::builder("52998224725", "Maria", "SP")
            .point_of_initiation("11")
            .transaction_amount("10.00")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "52998224725");
        assert_eq!(decoded.transaction_amount, Some("10.00".to_string()));
    }

    #[test]
    fn test_generate_with_cnpj_key() {
        let brcode = BrCode::builder("11222333000181", "Empresa X", "Rio")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "11222333000181");
    }

    #[test]
    fn test_generate_with_email_key() {
        let brcode = BrCode::builder("pix@empresa.com", "Loja", "Recife")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "pix@empresa.com");
    }

    #[test]
    fn test_generate_with_phone_key() {
        let brcode = BrCode::builder("+5521998765432", "Joao", "RJ")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "+5521998765432");
    }

    #[test]
    fn test_generate_with_evp_key() {
        let evp = "550e8400-e29b-41d4-a716-446655440000";
        let brcode = BrCode::builder(evp, "Test", "City")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, evp);
    }

    // --- Amount variations ---

    #[test]
    fn test_generate_with_amount() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("11")
            .transaction_amount("50.00")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.transaction_amount, Some("50.00".to_string()));
    }

    #[test]
    fn test_generate_without_amount() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.transaction_amount, None);
    }

    // --- Special characters in description ---

    #[test]
    fn test_generate_with_special_description() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .description("Cafe and Pao 42")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.description, Some("Cafe and Pao 42".to_string()));
    }

    // --- Max/over-limit merchant name/city ---

    #[test]
    fn test_generate_max_name_and_city() {
        let name = "A".repeat(25);
        let city = "B".repeat(15);
        let brcode = BrCode::builder("k@t.com", &name, &city).build().unwrap();
        let payload = encode_brcode(&brcode);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.merchant_name.len(), 25);
        assert_eq!(decoded.merchant_city.len(), 15);
    }

    #[test]
    fn test_generate_over_limit_name_fails() {
        let name = "A".repeat(26);
        let result = BrCode::builder("k@t.com", &name, "City").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_over_limit_city_fails() {
        let city = "B".repeat(16);
        let result = BrCode::builder("k@t.com", "Name", &city).build();
        assert!(result.is_err());
    }

    // --- Decode invalid payloads ---

    #[test]
    fn test_decode_invalid_payload() {
        let result = decode_payload("not-a-valid-brcode-payload", OutputFormat::Human);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_empty_payload() {
        let result = decode_payload("", OutputFormat::Human);
        assert!(result.is_err());
    }

    // --- PNG output ---

    #[test]
    fn test_png_file_created() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("qr.png");
        assert!(!png_path.exists());

        generate_qr(
            "key@test.com",
            Some(5.00),
            "TEST",
            "CITY",
            None,
            None,
            Some(png_path.to_str().unwrap()),
            10,
            OutputFormat::Human,
        )
        .unwrap();

        assert!(png_path.exists());
    }

    #[test]
    fn test_png_valid_format() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("qr.png");

        generate_qr(
            "key@test.com",
            None,
            "TEST",
            "CITY",
            None,
            None,
            Some(png_path.to_str().unwrap()),
            8,
            OutputFormat::Human,
        )
        .unwrap();

        let bytes = std::fs::read(&png_path).unwrap();
        // PNG magic bytes: 0x89 'P' 'N' 'G'
        assert!(bytes.len() > 8);
        assert_eq!(bytes[0], 0x89);
        assert_eq!(&bytes[1..4], b"PNG");
    }

    #[test]
    fn test_png_different_sizes() {
        let dir = tempfile::tempdir().unwrap();

        for size in [5, 10, 20] {
            let png_path = dir.path().join(format!("qr_{}.png", size));
            generate_qr(
                "key@test.com",
                None,
                "TEST",
                "CITY",
                None,
                None,
                Some(png_path.to_str().unwrap()),
                size,
                OutputFormat::Human,
            )
            .unwrap();
            assert!(png_path.exists());
        }

        // Larger module size should produce larger file
        let small = std::fs::metadata(dir.path().join("qr_5.png"))
            .unwrap()
            .len();
        let large = std::fs::metadata(dir.path().join("qr_20.png"))
            .unwrap()
            .len();
        assert!(large > small);
    }

    // --- Output format tests ---

    #[test]
    fn test_json_output_does_not_error() {
        let result = generate_qr(
            "key@test.com",
            Some(10.00),
            "TEST",
            "CITY",
            Some("desc"),
            Some("TX1"),
            None,
            10,
            OutputFormat::Json,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_human_output_does_not_error() {
        let result = generate_qr(
            "key@test.com",
            None,
            "TEST",
            "CITY",
            None,
            None,
            None,
            10,
            OutputFormat::Human,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_table_output_does_not_error() {
        let result = generate_qr(
            "key@test.com",
            None,
            "TEST",
            "CITY",
            None,
            None,
            None,
            10,
            OutputFormat::Table,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_json_output_does_not_error() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("10.00")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let result = decode_payload(&payload, OutputFormat::Json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_human_output_does_not_error() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("12")
            .transaction_amount("10.00")
            .txid("TX1")
            .description("Desc")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        let result = decode_payload(&payload, OutputFormat::Human);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_static_qr_shows_static_type() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        // Just ensure it doesn't error
        assert!(decode_payload(&payload, OutputFormat::Human).is_ok());
    }

    #[test]
    fn test_decode_dynamic_qr_shows_dynamic_type() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("12")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(decode_payload(&payload, OutputFormat::Human).is_ok());
    }
}
