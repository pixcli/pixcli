//! HTTP request handlers for the Pix webhook server.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::AppState;

/// Top-level webhook payload sent by Efí.
#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookPayload {
    /// List of Pix events in this notification.
    pub pix: Vec<PixEvent>,
}

/// A single Pix event from a webhook notification.
#[derive(Debug, Deserialize, Serialize)]
pub struct PixEvent {
    /// End-to-end identifier for the Pix transfer.
    #[serde(rename = "endToEndId")]
    pub end_to_end_id: String,
    /// Transaction ID (may be absent for some events).
    pub txid: Option<String>,
    /// Amount as a string (e.g. "10.00").
    pub valor: String,
    /// Timestamp of the event.
    pub horario: String,
    /// Payer information text.
    #[serde(rename = "infoPagador")]
    pub info_pagador: Option<String>,
    /// Pix key that received the payment.
    pub chave: Option<String>,
    /// Refund details, if any.
    pub devolucoes: Option<Vec<serde_json::Value>>,
}

/// Validates the Bearer token from the Authorization header against the configured auth token.
///
/// Returns `Ok(())` if no auth token is configured or if the token matches.
/// Returns `Err(StatusCode::UNAUTHORIZED)` if the token is missing or invalid.
fn check_auth(headers: &HeaderMap, state: &AppState) -> Result<(), StatusCode> {
    let expected = match state.auth_token {
        Some(ref token) => token,
        None => return Ok(()),
    };

    let header_value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let provided = header_value.strip_prefix("Bearer ").unwrap_or("");

    // Use constant-time comparison to prevent timing attacks on the auth token.
    let equal = provided.len() == expected.len()
        && provided
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;

    if provided.is_empty() || !equal {
        warn!("Webhook request rejected: invalid or missing auth token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(())
}

/// Handles incoming webhook POST requests at `/pix`.
///
/// If an auth token is configured, validates the `Authorization: Bearer <token>` header
/// before processing. Parses the Efí webhook payload, prints events to stdout
/// (unless `--quiet`), optionally appends them to a JSONL file, and optionally
/// forwards them via HTTP POST.
pub async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<WebhookPayload>,
) -> StatusCode {
    if let Err(status) = check_auth(&headers, &state) {
        return status;
    }

    info!("Received webhook with {} event(s)", payload.pix.len());

    for event in &payload.pix {
        let event_json = match serde_json::to_string(event) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize event: {e}");
                continue;
            }
        };

        // Print to stdout (intentional user-facing CLI output)
        if !state.quiet {
            if let Ok(pretty) = serde_json::to_string_pretty(event) {
                #[allow(clippy::print_stdout)]
                {
                    println!("{pretty}");
                }
            }
        }

        // Append to file
        if let Some(ref path) = state.output_file {
            use tokio::io::AsyncWriteExt;
            match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await
            {
                Ok(mut file) => {
                    let mut line = event_json.clone();
                    line.push('\n');
                    if let Err(e) = file.write_all(line.as_bytes()).await {
                        error!("Failed to write to {path}: {e}");
                    }
                    if let Err(e) = file.flush().await {
                        error!("Failed to flush {path}: {e}");
                    }
                }
                Err(e) => error!("Failed to open {path}: {e}"),
            }
        }

        // Forward to URL
        if let Some(ref url) = state.forward_url {
            let client = state.http_client.clone();
            let url = url.clone();
            let body = event_json.clone();
            tokio::spawn(async move {
                match client
                    .post(&url)
                    .body(body)
                    .header("Content-Type", "application/json")
                    .send()
                    .await
                {
                    Ok(resp) => info!("Forwarded to {url}: {}", resp.status()),
                    Err(e) => warn!("Failed to forward to {url}: {e}"),
                }
            });
        }
    }

    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, post};
    use axum::Router;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            forward_url: None,
            output_file: None,
            quiet: true,
            http_client: reqwest::Client::new(),
            auth_token: None,
        })
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/pix", post(handle_webhook))
            .route("/health", get(|| async { "OK" }))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_valid_webhook_returns_200() {
        let app = test_app(test_state());

        let payload = r#"{"pix":[{"endToEndId":"E123","txid":"abc","valor":"10.00","horario":"2026-03-19T05:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_empty_pix_array_returns_200() {
        let app = test_app(test_state());

        let payload = r#"{"pix":[]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_malformed_json_returns_422() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Axum returns 400 Bad Request for malformed JSON bodies
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_events_written_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("events.jsonl");

        let state = Arc::new(AppState {
            forward_url: None,
            output_file: Some(output_path.to_str().unwrap().to_string()),
            quiet: true,
            http_client: reqwest::Client::new(),
            auth_token: None,
        });

        let app = test_app(state);

        let payload = r#"{"pix":[{"endToEndId":"E456","txid":"def","valor":"25.50","horario":"2026-03-19T10:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(output_path.exists());

        let content = std::fs::read_to_string(&output_path).unwrap();
        let event: PixEvent = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(event.end_to_end_id, "E456");
        assert_eq!(event.valor, "25.50");
    }

    #[tokio::test]
    async fn test_multiple_events_in_payload() {
        let app = test_app(test_state());

        let payload = r#"{"pix":[
            {"endToEndId":"E1","valor":"10.00","horario":"2026-03-19T05:00:00Z"},
            {"endToEndId":"E2","valor":"20.00","horario":"2026-03-19T06:00:00Z"}
        ]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_pix_event_deserialization() {
        let json = r#"{
            "endToEndId": "E12345678202603191234",
            "txid": "abc123",
            "valor": "10.00",
            "horario": "2026-03-19T05:21:00.000Z",
            "infoPagador": "Payment note",
            "chave": "+5511999999999"
        }"#;

        let event: PixEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.end_to_end_id, "E12345678202603191234");
        assert_eq!(event.txid, Some("abc123".to_string()));
        assert_eq!(event.valor, "10.00");
        assert_eq!(event.chave, Some("+5511999999999".to_string()));
    }

    #[test]
    fn test_pix_event_serialization_roundtrip() {
        let event = PixEvent {
            end_to_end_id: "E999".to_string(),
            txid: Some("TX1".to_string()),
            valor: "42.00".to_string(),
            horario: "2026-03-19T12:00:00Z".to_string(),
            info_pagador: Some("test".to_string()),
            chave: Some("key@test.com".to_string()),
            devolucoes: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: PixEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.end_to_end_id, event.end_to_end_id);
        assert_eq!(decoded.valor, event.valor);
    }
}

#[cfg(test)]
mod additional_webhook_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, post};
    use axum::Router;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            forward_url: None,
            output_file: None,
            quiet: true,
            http_client: reqwest::Client::new(),
            auth_token: None,
        })
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/pix", post(handle_webhook))
            .route("/health", get(|| async { "OK" }))
            .with_state(state)
    }

    // --- Valid payload tests ---

    #[tokio::test]
    async fn test_valid_payload_all_fields() {
        let app = test_app(test_state());
        let payload = r#"{"pix":[{
            "endToEndId":"E12345678202603191234567890123456",
            "txid":"pix550e8400e29b41d4a716446655440000",
            "valor":"100.50",
            "horario":"2026-03-19T12:00:00.000Z",
            "infoPagador":"Pagamento do pedido #123",
            "chave":"+5511999999999",
            "devolucoes":[{"id":"D1","rtrId":"R1","valor":"10.00","horario":{"solicitacao":"2026-03-19T13:00:00Z"},"status":"DEVOLVIDO"}]
        }]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_valid_payload_minimal_fields() {
        let app = test_app(test_state());
        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_multiple_pix_events() {
        let app = test_app(test_state());
        let payload = r#"{"pix":[
            {"endToEndId":"E1","valor":"10.00","horario":"2026-03-19T01:00:00Z"},
            {"endToEndId":"E2","valor":"20.00","horario":"2026-03-19T02:00:00Z"},
            {"endToEndId":"E3","valor":"30.00","horario":"2026-03-19T03:00:00Z"},
            {"endToEndId":"E4","valor":"40.00","horario":"2026-03-19T04:00:00Z"},
            {"endToEndId":"E5","valor":"50.00","horario":"2026-03-19T05:00:00Z"}
        ]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_empty_pix_array() {
        let app = test_app(test_state());
        let payload = r#"{"pix":[]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- Error cases ---

    #[tokio::test]
    async fn test_malformed_json_returns_400() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from("{invalid json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_empty_body_returns_400() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_wrong_json_structure_returns_422() {
        let app = test_app(test_state());
        // Valid JSON but wrong structure (missing "pix" key)
        let payload = r#"{"events":[{"id":"1"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Axum returns 422 for valid JSON that doesn't match the expected type
        assert!(
            response.status() == StatusCode::UNPROCESSABLE_ENTITY
                || response.status() == StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_large_payload() {
        let app = test_app(test_state());
        // Generate a large but valid payload
        let mut events = Vec::new();
        for i in 0..100 {
            events.push(format!(
                r#"{{"endToEndId":"E{:06}","valor":"1.00","horario":"2026-03-19T00:00:00Z"}}"#,
                i
            ));
        }
        let payload = format!(r#"{{"pix":[{}]}}"#, events.join(","));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- Health endpoint ---

    #[tokio::test]
    async fn test_health_returns_ok_body() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"OK");
    }

    // --- File output tests ---

    #[tokio::test]
    async fn test_multiple_events_create_multiple_lines() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("events.jsonl");

        let state = Arc::new(AppState {
            forward_url: None,
            output_file: Some(output_path.to_str().unwrap().to_string()),
            quiet: true,
            http_client: reqwest::Client::new(),
            auth_token: None,
        });
        let app = test_app(state);

        let payload = r#"{"pix":[
            {"endToEndId":"E1","valor":"10.00","horario":"2026-03-19T01:00:00Z"},
            {"endToEndId":"E2","valor":"20.00","horario":"2026-03-19T02:00:00Z"},
            {"endToEndId":"E3","valor":"30.00","horario":"2026-03-19T03:00:00Z"}
        ]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let content = std::fs::read_to_string(&output_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        // Verify each line is valid JSON
        for line in &lines {
            let event: PixEvent = serde_json::from_str(line).unwrap();
            assert!(!event.end_to_end_id.is_empty());
        }
    }

    #[tokio::test]
    async fn test_file_created_on_first_event() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("new_events.jsonl");
        assert!(!output_path.exists());

        let state = Arc::new(AppState {
            forward_url: None,
            output_file: Some(output_path.to_str().unwrap().to_string()),
            quiet: true,
            http_client: reqwest::Client::new(),
            auth_token: None,
        });
        let app = test_app(state);

        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"5.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/pix")
                .header("Content-Type", "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();

        assert!(output_path.exists());
    }

    // --- Serialization tests ---

    #[test]
    fn test_pix_event_minimal_deserialization() {
        let json = r#"{"endToEndId":"E1","valor":"1.00","horario":"2026-01-01T00:00:00Z"}"#;
        let event: PixEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.end_to_end_id, "E1");
        assert!(event.txid.is_none());
        assert!(event.info_pagador.is_none());
        assert!(event.chave.is_none());
        assert!(event.devolucoes.is_none());
    }

    #[test]
    fn test_pix_event_with_devolucoes() {
        let json = r#"{
            "endToEndId":"E1",
            "valor":"100.00",
            "horario":"2026-01-01T00:00:00Z",
            "devolucoes":[
                {"id":"D1","rtrId":"R1","valor":"50.00","status":"DEVOLVIDO"},
                {"id":"D2","rtrId":"R2","valor":"50.00","status":"EM_PROCESSAMENTO"}
            ]
        }"#;
        let event: PixEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.devolucoes.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_pix_event_serialization_preserves_field_names() {
        let event = PixEvent {
            end_to_end_id: "E999".to_string(),
            txid: Some("TX1".to_string()),
            valor: "42.00".to_string(),
            horario: "2026-03-19T12:00:00Z".to_string(),
            info_pagador: Some("test info".to_string()),
            chave: Some("key@test.com".to_string()),
            devolucoes: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        // Verify camelCase field names
        assert!(json.contains("\"endToEndId\""));
        assert!(json.contains("\"infoPagador\""));
    }

    #[test]
    fn test_webhook_payload_deserialization() {
        let json = r#"{"pix":[
            {"endToEndId":"E1","valor":"10.00","horario":"2026-01-01T00:00:00Z"},
            {"endToEndId":"E2","valor":"20.00","horario":"2026-01-02T00:00:00Z"}
        ]}"#;
        let payload: WebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.pix.len(), 2);
    }

    #[test]
    fn test_webhook_payload_empty_pix() {
        let json = r#"{"pix":[]}"#;
        let payload: WebhookPayload = serde_json::from_str(json).unwrap();
        assert!(payload.pix.is_empty());
    }

    #[test]
    fn test_webhook_payload_serialization_roundtrip() {
        let payload = WebhookPayload {
            pix: vec![
                PixEvent {
                    end_to_end_id: "E1".to_string(),
                    txid: None,
                    valor: "5.00".to_string(),
                    horario: "2026-01-01T00:00:00Z".to_string(),
                    info_pagador: None,
                    chave: None,
                    devolucoes: None,
                },
                PixEvent {
                    end_to_end_id: "E2".to_string(),
                    txid: Some("TX2".to_string()),
                    valor: "15.00".to_string(),
                    horario: "2026-01-02T00:00:00Z".to_string(),
                    info_pagador: Some("Info".to_string()),
                    chave: Some("key@test.com".to_string()),
                    devolucoes: None,
                },
            ],
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: WebhookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pix.len(), 2);
        assert_eq!(back.pix[0].end_to_end_id, "E1");
        assert_eq!(back.pix[1].end_to_end_id, "E2");
        assert_eq!(back.pix[1].txid, Some("TX2".to_string()));
    }

    #[test]
    fn test_pix_event_with_all_optional_fields() {
        let event = PixEvent {
            end_to_end_id: "E12345".to_string(),
            txid: Some("TX123".to_string()),
            valor: "99.99".to_string(),
            horario: "2026-03-19T15:30:00Z".to_string(),
            info_pagador: Some("Payment for invoice #456".to_string()),
            chave: Some("+5511999999999".to_string()),
            devolucoes: Some(vec![serde_json::json!({"id": "D1", "valor": "10.00"})]),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: PixEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.info_pagador,
            Some("Payment for invoice #456".to_string())
        );
        assert_eq!(back.chave, Some("+5511999999999".to_string()));
        assert!(back.devolucoes.is_some());
    }

    // --- GET on /pix should return 405 ---

    #[tokio::test]
    async fn test_get_on_pix_returns_405() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/pix")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    // --- Unknown route ---

    #[tokio::test]
    async fn test_unknown_route_returns_404() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/unknown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // --- Auth token tests ---

    fn auth_state(token: &str) -> Arc<AppState> {
        Arc::new(AppState {
            forward_url: None,
            output_file: None,
            quiet: true,
            http_client: reqwest::Client::new(),
            auth_token: Some(token.to_string()),
        })
    }

    #[tokio::test]
    async fn test_auth_valid_token_accepted() {
        let app = test_app(auth_state("secret123"));
        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer secret123")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_missing_token_rejected() {
        let app = test_app(auth_state("secret123"));
        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_wrong_token_rejected() {
        let app = test_app(auth_state("secret123"));
        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer wrong_token")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_no_bearer_prefix_rejected() {
        let app = test_app(auth_state("secret123"));
        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "secret123")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_no_auth_configured_allows_all() {
        let app = test_app(test_state()); // auth_token is None
        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z"}]}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- Body size limit test ---

    #[tokio::test]
    async fn test_oversized_body_rejected() {
        use axum::extract::DefaultBodyLimit;

        let state = test_state();
        let app = Router::new()
            .route("/pix", post(handle_webhook))
            .layer(DefaultBodyLimit::max(1024)) // 1 KB limit for test
            .with_state(state);

        // Build a payload larger than 1 KB
        let large_payload = format!(
            r#"{{"pix":[{{"endToEndId":"E1","valor":"1.00","horario":"2026-03-19T00:00:00Z","infoPagador":"{}"}}]}}"#,
            "x".repeat(2048)
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(large_payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
