//! Integration tests for EfiClient's PixProvider methods using wiremock.
//!
//! These tests spin up a mock HTTP server and verify the full request/response
//! cycle for each PixProvider method.

use chrono::Utc;
use pix_efi::auth::EfiAuth;
use pix_efi::client::EfiClient;
use pix_efi::config::{EfiConfig, EfiEnvironment};
use pix_efi::EfiError;
use pix_provider::{
    ChargeRequest, ChargeStatus, Debtor, DueDateChargeRequest, PixProvider, TransactionFilter,
};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> EfiConfig {
    EfiConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_client_secret".to_string(),
        certificate_path: "/nonexistent/cert.p12".into(),
        certificate_password: String::new(),
        environment: EfiEnvironment::Sandbox,
    }
}

/// Helper: create an EfiClient backed by a wiremock server with a pre-populated token.
fn mock_client(server_url: &str) -> EfiClient {
    let config = test_config();
    let http_client = reqwest::Client::new();
    let auth = EfiAuth::with_client_and_url(config, http_client, server_url.to_string());
    EfiClient::with_auth(auth)
}

fn mock_client_with_key(server_url: &str, pix_key: &str) -> EfiClient {
    let config = test_config();
    let http_client = reqwest::Client::new();
    let auth = EfiAuth::with_client_and_url(config, http_client, server_url.to_string());
    EfiClient::with_auth_and_key(auth, pix_key.to_string())
}

fn oauth_token_json() -> serde_json::Value {
    serde_json::json!({
        "access_token": "mock_token_12345",
        "token_type": "Bearer",
        "expires_in": 3600,
        "scope": "pix.read pix.write"
    })
}

fn charge_response_json(txid: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "txid": txid,
        "status": status,
        "calendario": {
            "criacao": "2026-03-19T12:00:00Z",
            "expiracao": 3600
        },
        "valor": {
            "original": "100.00"
        },
        "chave": "user@example.com",
        "solicitacaoPagador": "Test charge",
        "pixCopiaECola": "00020126580014br.gov.bcb.pix...",
        "devedor": null,
        "pix": null
    })
}

fn charge_with_debtor_json(txid: &str) -> serde_json::Value {
    serde_json::json!({
        "txid": txid,
        "status": "ATIVA",
        "calendario": {
            "criacao": "2026-03-19T12:00:00Z",
            "expiracao": 3600
        },
        "valor": {
            "original": "50.00"
        },
        "chave": "user@example.com",
        "solicitacaoPagador": "Payment",
        "pixCopiaECola": "brcode_here",
        "devedor": {
            "nome": "João da Silva",
            "cpf": "52998224725"
        },
        "pix": [{"endToEndId": "E12345678901234567890123456789AB"}]
    })
}

async fn setup_token_mock(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(oauth_token_json()))
        .expect(1..)
        .mount(server)
        .await;
}

/// Helper to generate a self-signed PKCS#12 certificate for testing.
/// Uses openssl command-line tool.
fn generate_test_cert() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("key.pem");
    let cert_path = dir.path().join("cert.pem");
    let p12_path = dir.path().join("cert.p12");

    let status = std::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            key_path.to_str().unwrap(),
            "-out",
            cert_path.to_str().unwrap(),
            "-days",
            "1",
            "-nodes",
            "-subj",
            "/CN=test",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "openssl req failed");

    let status = std::process::Command::new("openssl")
        .args([
            "pkcs12",
            "-export",
            "-out",
            p12_path.to_str().unwrap(),
            "-inkey",
            key_path.to_str().unwrap(),
            "-in",
            cert_path.to_str().unwrap(),
            "-passout",
            "pass:",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "openssl pkcs12 failed");

    (dir, p12_path)
}

// ── Auth Constructor Tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_auth_new_with_valid_certificate() {
    let (_dir, cert_path) = generate_test_cert();

    let config = EfiConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        certificate_path: cert_path,
        certificate_password: String::new(),
        environment: EfiEnvironment::Sandbox,
    };

    let auth = EfiAuth::new(config);
    assert!(auth.is_ok());

    let auth = auth.unwrap();
    assert_eq!(auth.base_url(), "https://pix-h.api.efipay.com.br");

    // Test Debug impl hides credentials
    let debug = format!("{:?}", auth);
    assert!(debug.contains("***"));
    assert!(!debug.contains("test_id"));
}

#[tokio::test]
async fn test_auth_new_with_invalid_cert_path() {
    let config = EfiConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        certificate_path: "/nonexistent/cert.p12".into(),
        certificate_password: String::new(),
        environment: EfiEnvironment::Sandbox,
    };

    let result = EfiAuth::new(config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("certificate"));
}

#[tokio::test]
async fn test_auth_new_with_invalid_cert_content() {
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("bad_cert.p12");
    std::fs::write(&cert_path, b"not a real certificate").unwrap();

    let config = EfiConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        certificate_path: cert_path,
        certificate_password: String::new(),
        environment: EfiEnvironment::Sandbox,
    };

    let result = EfiAuth::new(config);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("PKCS#12") || err.to_string().contains("certificate"));
}

#[tokio::test]
async fn test_efi_client_new_with_valid_cert() {
    let (_dir, cert_path) = generate_test_cert();

    let config = EfiConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        certificate_path: cert_path,
        certificate_password: String::new(),
        environment: EfiEnvironment::Sandbox,
    };

    let client = EfiClient::new(config);
    assert!(client.is_ok());
    assert_eq!(client.unwrap().provider_name(), "efi");
}

#[tokio::test]
async fn test_efi_client_with_pix_key() {
    let (_dir, cert_path) = generate_test_cert();

    let config = EfiConfig {
        client_id: "test_id".to_string(),
        client_secret: "test_secret".to_string(),
        certificate_path: cert_path,
        certificate_password: String::new(),
        environment: EfiEnvironment::Sandbox,
    };

    let client = EfiClient::with_pix_key(config, "test@example.com".to_string());
    assert!(client.is_ok());
}

// ── Auth Tests ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_auth_get_token_from_server() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    let config = test_config();
    let http_client = reqwest::Client::new();
    let auth = EfiAuth::with_client_and_url(config, http_client, server.uri());

    let token = auth.get_token().await.unwrap();
    assert_eq!(token, "mock_token_12345");
}

#[tokio::test]
async fn test_auth_token_cached_after_first_call() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(oauth_token_json()))
        .expect(1) // Should only be called once
        .mount(&server)
        .await;

    let config = test_config();
    let http_client = reqwest::Client::new();
    let auth = EfiAuth::with_client_and_url(config, http_client, server.uri());

    let token1 = auth.get_token().await.unwrap();
    let token2 = auth.get_token().await.unwrap();
    assert_eq!(token1, token2);
    assert_eq!(token1, "mock_token_12345");
}

#[tokio::test]
async fn test_auth_token_failure_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid credentials"))
        .mount(&server)
        .await;

    let config = test_config();
    let http_client = reqwest::Client::new();
    let auth = EfiAuth::with_client_and_url(config, http_client, server.uri());

    let result: Result<String, EfiError> = auth.get_token().await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("401") || err_msg.contains("invalid"));
}

#[tokio::test]
async fn test_auth_token_parse_failure() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let config = test_config();
    let http_client = reqwest::Client::new();
    let auth = EfiAuth::with_client_and_url(config, http_client, server.uri());

    let result: Result<String, EfiError> = auth.get_token().await;
    assert!(result.is_err());
}

// ── Create Charge Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_charge_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cob/.+"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(charge_response_json(
                "pixABCDEF1234567890abcdef12345",
                "ATIVA",
            )),
        )
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "100.00".to_string(),
        description: Some("Test charge".to_string()),
        debtor: None,
        expiration_secs: 3600,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let resp = client.create_charge(req).await.unwrap();
    assert_eq!(resp.status, ChargeStatus::Active);
    assert!(!resp.brcode.is_empty());
}

#[tokio::test]
async fn test_create_charge_with_debtor() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cob/.+"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(charge_with_debtor_json("pixABCDEF1234567890abcdef12345")),
        )
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "50.00".to_string(),
        description: Some("Payment".to_string()),
        debtor: Some(Debtor {
            name: "João da Silva".to_string(),
            document: "52998224725".to_string(),
        }),
        expiration_secs: 3600,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let resp = client.create_charge(req).await.unwrap();
    assert_eq!(resp.status, ChargeStatus::Active);
}

#[tokio::test]
async fn test_create_charge_with_cnpj_debtor() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cob/.+"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(charge_response_json(
                "pixABCDEF1234567890abcdef12345",
                "ATIVA",
            )),
        )
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "100.00".to_string(),
        description: None,
        debtor: Some(Debtor {
            name: "Empresa LTDA".to_string(),
            document: "11222333000181".to_string(), // CNPJ (14 chars)
        }),
        expiration_secs: 3600,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let resp = client.create_charge(req).await.unwrap();
    assert_eq!(resp.status, ChargeStatus::Active);
}

#[tokio::test]
async fn test_create_charge_invalid_txid() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri());

    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "100.00".to_string(),
        description: None,
        debtor: None,
        expiration_secs: 3600,
        txid: Some("short".to_string()), // Too short
    };

    let result = client.create_charge(req).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_charge_invalid_amount() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri());

    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "0.00".to_string(), // Zero is invalid
        description: None,
        debtor: None,
        expiration_secs: 3600,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let result = client.create_charge(req).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_charge_generates_txid_if_none() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cob/.+"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(charge_response_json(
                "pixABCDEF1234567890abcdef12345",
                "ATIVA",
            )),
        )
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "10.50".to_string(),
        description: None,
        debtor: None,
        expiration_secs: 3600,
        txid: None, // Should auto-generate
    };

    let resp = client.create_charge(req).await.unwrap();
    assert_eq!(resp.status, ChargeStatus::Active);
}

#[tokio::test]
async fn test_create_charge_api_error() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cob/.+"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "10.00".to_string(),
        description: None,
        debtor: None,
        expiration_secs: 3600,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let result = client.create_charge(req).await;
    assert!(result.is_err());
}

// ── Create Due Date Charge Tests ────────────────────────────────────────────

#[tokio::test]
async fn test_create_due_date_charge_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cobv/.+"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(charge_response_json(
                "pixABCDEF1234567890abcdef12345",
                "ATIVA",
            )),
        )
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = DueDateChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "200.00".to_string(),
        description: Some("Due date charge".to_string()),
        debtor: None,
        due_date: "2026-04-01".to_string(),
        days_after_due: Some(30),
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let resp = client.create_due_date_charge(req).await.unwrap();
    assert_eq!(resp.status, ChargeStatus::Active);
}

#[tokio::test]
async fn test_create_due_date_charge_no_days_after() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cobv/.+"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(charge_response_json(
                "pixABCDEF1234567890abcdef12345",
                "ATIVA",
            )),
        )
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = DueDateChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "50.00".to_string(),
        description: None,
        debtor: Some(Debtor {
            name: "Test".to_string(),
            document: "52998224725".to_string(),
        }),
        due_date: "2026-05-01".to_string(),
        days_after_due: None,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let resp = client.create_due_date_charge(req).await.unwrap();
    assert_eq!(resp.status, ChargeStatus::Active);
}

#[tokio::test]
async fn test_create_due_date_charge_invalid_amount() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri());

    let req = DueDateChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "abc".to_string(),
        description: None,
        debtor: None,
        due_date: "2026-04-01".to_string(),
        days_after_due: None,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let result = client.create_due_date_charge(req).await;
    assert!(result.is_err());
}

// ── Get Charge Tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_charge_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    let txid = "pixABCDEF1234567890abcdef12345";
    Mock::given(method("GET"))
        .and(path(format!("/v2/cob/{txid}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(charge_with_debtor_json(txid)))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let charge = client.get_charge(txid).await.unwrap();
    assert_eq!(charge.txid, txid);
    assert_eq!(charge.status, ChargeStatus::Active);
    assert!(charge.debtor.is_some());
    assert_eq!(charge.e2eids.len(), 1);
}

#[tokio::test]
async fn test_get_charge_not_found() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    let txid = "pixABCDEF1234567890abcdef12345";
    Mock::given(method("GET"))
        .and(path(format!("/v2/cob/{txid}")))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.get_charge(txid).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_charge_invalid_txid() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri());

    let result = client.get_charge("bad").await;
    assert!(result.is_err());
}

// ── List Charges Tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_charges_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/cob"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cobs": [
                {
                    "txid": "tx1aaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "status": "ATIVA",
                    "calendario": { "criacao": "2026-03-19T12:00:00Z", "expiracao": 3600 },
                    "valor": { "original": "10.00" },
                    "chave": "key@test.com",
                    "solicitacaoPagador": null,
                    "devedor": null,
                    "pixCopiaECola": null,
                    "pix": null
                },
                {
                    "txid": "tx2bbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "status": "CONCLUIDA",
                    "calendario": { "criacao": "2026-03-18T12:00:00Z", "expiracao": 3600 },
                    "valor": { "original": "25.00" },
                    "chave": "key@test.com",
                    "solicitacaoPagador": null,
                    "devedor": null,
                    "pixCopiaECola": null,
                    "pix": null
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let filter = TransactionFilter::default();
    let charges = client.list_charges(filter).await.unwrap();
    assert_eq!(charges.len(), 2);
    assert_eq!(charges[0].status, ChargeStatus::Active);
    assert_eq!(charges[1].status, ChargeStatus::Completed);
}

#[tokio::test]
async fn test_list_charges_empty() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/cob"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cobs": []
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let filter = TransactionFilter {
        start: Some(Utc::now() - chrono::Duration::days(1)),
        end: Some(Utc::now()),
        ..Default::default()
    };
    let charges = client.list_charges(filter).await.unwrap();
    assert!(charges.is_empty());
}

// ── Send Pix Tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_send_pix_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v3/gn/pix/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "idEnvio": "envio123",
            "e2eId": "E12345678901234567890123456789AB",
            "valor": "50.00",
            "status": "EM_PROCESSAMENTO"
        })))
        .mount(&server)
        .await;

    let client = mock_client_with_key(&server.uri(), "sender@example.com");
    let transfer = client
        .send_pix("recipient@example.com", "50.00", Some("Payment"))
        .await
        .unwrap();
    assert_eq!(transfer.amount, "50.00");
    assert_eq!(transfer.id_envio, "envio123");
    assert_eq!(transfer.e2eid, "E12345678901234567890123456789AB");
}

#[tokio::test]
async fn test_send_pix_no_default_key() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri()); // No pix key set

    let result = client
        .send_pix("recipient@example.com", "50.00", None)
        .await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("default_pix_key"));
}

#[tokio::test]
async fn test_send_pix_invalid_amount() {
    let server = MockServer::start().await;
    let client = mock_client_with_key(&server.uri(), "sender@example.com");

    let result = client.send_pix("recipient@example.com", "0.00", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_pix_api_error() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v3/gn/pix/.+"))
        .respond_with(ResponseTemplate::new(403).set_body_string("insufficient funds"))
        .mount(&server)
        .await;

    let client = mock_client_with_key(&server.uri(), "sender@example.com");
    let result = client
        .send_pix("recipient@example.com", "50.00", None)
        .await;
    assert!(result.is_err());
}

// ── Get Pix Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_pix_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    let e2eid = "E12345678901234567890123456789AB";
    Mock::given(method("GET"))
        .and(path(format!("/v2/pix/{e2eid}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "endToEndId": e2eid,
            "txid": "tx1aaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "valor": "42.00",
            "horario": "2026-03-19T12:00:00Z",
            "infoPagador": "Payment description",
            "pagador": {
                "cpf": "52998224725",
                "nome": "João"
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let tx = client.get_pix(e2eid).await.unwrap();
    assert_eq!(tx.e2eid, e2eid);
    assert_eq!(tx.amount, "42.00");
    assert_eq!(tx.payer_name, Some("João".to_string()));
    assert_eq!(tx.payer_document, Some("52998224725".to_string()));
}

#[tokio::test]
async fn test_get_pix_invalid_e2eid() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri());

    let result = client.get_pix("invalid").await;
    assert!(result.is_err());
}

// ── List Received Pix Tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_list_received_pix_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/pix"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "pix": [
                {
                    "endToEndId": "E12345678901234567890123456789AB",
                    "txid": "tx1aaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "valor": "10.00",
                    "horario": "2026-03-19T12:00:00Z",
                    "infoPagador": null,
                    "pagador": null
                },
                {
                    "endToEndId": "E12345678901234567890123456789CD",
                    "valor": "20.00",
                    "horario": "2026-03-19T13:00:00Z",
                    "infoPagador": "test",
                    "pagador": {
                        "cnpj": "11222333000181",
                        "nome": "Empresa"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let filter = TransactionFilter::default();
    let txs = client.list_received_pix(filter).await.unwrap();
    assert_eq!(txs.len(), 2);
    assert_eq!(txs[0].amount, "10.00");
    assert_eq!(txs[1].payer_document, Some("11222333000181".to_string()));
}

#[tokio::test]
async fn test_list_received_pix_empty() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/pix"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "pix": []
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let filter = TransactionFilter::default();
    let txs = client.list_received_pix(filter).await.unwrap();
    assert!(txs.is_empty());
}

// ── Get Balance Tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_balance_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/gn/saldo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "saldo": "1234.56"
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let balance = client.get_balance().await.unwrap();
    assert_eq!(balance.available, "1234.56");
}

#[tokio::test]
async fn test_get_balance_api_error() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/gn/saldo"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.get_balance().await;
    assert!(result.is_err());
}

// ── Webhook Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_register_webhook_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/webhook/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client
        .register_webhook("user@example.com", "https://example.com/webhook")
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_webhook_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path_regex(r"/v2/webhook/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "webhookUrl": "https://example.com/webhook",
            "chave": "user@example.com",
            "criacao": "2026-03-19T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let info = client.get_webhook("user@example.com").await.unwrap();
    assert_eq!(info.webhook_url, "https://example.com/webhook");
    assert_eq!(info.chave, Some("user@example.com".to_string()));
}

#[tokio::test]
async fn test_remove_webhook_success() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("DELETE"))
        .and(path_regex(r"/v2/webhook/.+"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.remove_webhook("user@example.com").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_remove_webhook_not_found() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("DELETE"))
        .and(path_regex(r"/v2/webhook/.+"))
        .respond_with(ResponseTemplate::new(404).set_body_string("no webhook"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.remove_webhook("user@example.com").await;
    assert!(result.is_err());
}

// ── Retry Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_with_retry_recovers_from_503() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    // First call returns 503, second returns 200
    Mock::given(method("GET"))
        .and(path("/v2/gn/saldo"))
        .respond_with(ResponseTemplate::new(503).set_body_string("maintenance"))
        .up_to_n_times(2)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v2/gn/saldo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "saldo": "100.00"
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let balance = client.get_balance().await.unwrap();
    assert_eq!(balance.available, "100.00");
}

// ── Invalid Response Tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_create_charge_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cob/.+"))
        .respond_with(ResponseTemplate::new(201).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = ChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "10.00".to_string(),
        description: None,
        debtor: None,
        expiration_secs: 3600,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let result = client.create_charge(req).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("parse"));
}

#[tokio::test]
async fn test_create_due_date_charge_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v2/cobv/.+"))
        .respond_with(ResponseTemplate::new(201).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let req = DueDateChargeRequest {
        pix_key: "user@example.com".to_string(),
        amount: "10.00".to_string(),
        description: None,
        debtor: None,
        due_date: "2026-04-01".to_string(),
        days_after_due: None,
        txid: Some("pixABCDEF1234567890abcdef12345".to_string()),
    };

    let result = client.create_due_date_charge(req).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_charge_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    let txid = "pixABCDEF1234567890abcdef12345";
    Mock::given(method("GET"))
        .and(path(format!("/v2/cob/{txid}")))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.get_charge(txid).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_charges_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/cob"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let filter = TransactionFilter::default();
    let result = client.list_charges(filter).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_pix_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    let e2eid = "E12345678901234567890123456789AB";
    Mock::given(method("GET"))
        .and(path(format!("/v2/pix/{e2eid}")))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.get_pix(e2eid).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_received_pix_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/pix"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let filter = TransactionFilter::default();
    let result = client.list_received_pix(filter).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_pix_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v3/gn/pix/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client_with_key(&server.uri(), "sender@example.com");
    let result = client
        .send_pix("recipient@example.com", "50.00", None)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_balance_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/v2/gn/saldo"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.get_balance().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_webhook_invalid_json_response() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path_regex(r"/v2/webhook/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = mock_client(&server.uri());
    let result = client.get_webhook("user@example.com").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_pix_without_description() {
    let server = MockServer::start().await;
    setup_token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path_regex(r"/v3/gn/pix/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "idEnvio": "envio456",
            "e2eId": null,
            "valor": "25.00",
            "status": "EM_PROCESSAMENTO"
        })))
        .mount(&server)
        .await;

    let client = mock_client_with_key(&server.uri(), "sender@example.com");
    let transfer = client
        .send_pix("recipient@example.com", "25.00", None)
        .await
        .unwrap();
    assert_eq!(transfer.amount, "25.00");
    assert_eq!(transfer.e2eid, ""); // null e2eId becomes empty string
}

// ── Provider Name Test ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_provider_name_is_efi() {
    let server = MockServer::start().await;
    let client = mock_client(&server.uri());
    assert_eq!(client.provider_name(), "efi");
}
