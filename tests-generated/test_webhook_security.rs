// =============================================================================
// Webhook Security Tests
// =============================================================================
//
// TARGET CRATE: pix-webhook-server
// PLACEMENT:    crates/pix-webhook-server/tests/webhook_security.rs
//
// DEPENDENCIES NEEDED IN Cargo.toml:
//   [dev-dependencies]
//   axum = "0.8"
//   reqwest = { version = "0.12", features = ["json"] }
//   serde = { version = "1", features = ["derive"] }
//   serde_json = "1"
//   tempfile = "3"
//   tokio = { version = "1", features = ["full", "test-util"] }
//   tower = { version = "0.5", features = ["util"] }
//   http-body-util = "0.1"
//
// WHY THESE TESTS EXIST:
//
// The webhook server at /pix is a publicly reachable endpoint that receives
// payment notifications from the Efi PSP. It currently lacks:
//   - Body size limits (an attacker can send multi-GB payloads to exhaust memory)
//   - Content-Type enforcement (only JSON should be accepted)
//   - HTTP method enforcement beyond GET already tested (PUT/DELETE/PATCH)
//   - Input sanitization on user-controlled fields like infoPagador
//   - Concurrent write safety for JSONL file output
//   - Rate limiting or abuse mitigation
//
// These tests document the CURRENT behavior and serve as a specification for
// future hardening. Tests that verify current (potentially insecure) behavior
// are marked with comments indicating what SHOULD change.
// =============================================================================

#[cfg(test)]
mod webhook_security_tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;
    use std::sync::Arc;
    use tower::ServiceExt;

    // These types come from pix-webhook-server's internal modules.
    // When placed in the crate's tests/ directory, use:
    //   use pix_webhook_server::{AppState, handlers::*};
    // Since the handler module is private, these tests must go inside
    // the crate (e.g., as an additional #[cfg(test)] module in handlers.rs)
    // or the types must be re-exported.
    //
    // For this reference file, we inline the necessary types.
    use axum::extract::State;
    use axum::Json;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    pub struct WebhookPayload {
        pub pix: Vec<PixEvent>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct PixEvent {
        #[serde(rename = "endToEndId")]
        pub end_to_end_id: String,
        pub txid: Option<String>,
        pub valor: String,
        pub horario: String,
        #[serde(rename = "infoPagador")]
        pub info_pagador: Option<String>,
        pub chave: Option<String>,
        pub devolucoes: Option<Vec<serde_json::Value>>,
    }

    pub struct AppState {
        pub forward_url: Option<String>,
        pub output_file: Option<String>,
        pub quiet: bool,
        pub http_client: reqwest::Client,
    }

    // Re-implement the handler for test isolation.
    // In the real crate, import it directly.
    async fn handle_webhook(
        State(state): State<Arc<AppState>>,
        Json(payload): Json<WebhookPayload>,
    ) -> StatusCode {
        for event in &payload.pix {
            let event_json = match serde_json::to_string(event) {
                Ok(j) => j,
                Err(_) => continue,
            };

            if !state.quiet {
                if let Ok(pretty) = serde_json::to_string_pretty(event) {
                    println!("{pretty}");
                }
            }

            if let Some(ref path) = state.output_file {
                use std::io::Write;
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    Ok(mut file) => {
                        let _ = writeln!(file, "{event_json}");
                    }
                    Err(_) => {}
                }
            }

            if let Some(ref url) = state.forward_url {
                let client = state.http_client.clone();
                let url = url.clone();
                let body = event_json.clone();
                tokio::spawn(async move {
                    let _ = client
                        .post(&url)
                        .body(body)
                        .header("Content-Type", "application/json")
                        .send()
                        .await;
                });
            }
        }

        StatusCode::OK
    }

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            forward_url: None,
            output_file: None,
            quiet: true,
            http_client: reqwest::Client::new(),
        })
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/pix", post(handle_webhook))
            .route("/health", get(|| async { "OK" }))
            .with_state(state)
    }

    fn valid_payload() -> String {
        r#"{"pix":[{"endToEndId":"E12345","valor":"10.00","horario":"2026-03-19T05:00:00Z"}]}"#
            .to_string()
    }

    // =========================================================================
    // 1. Oversized payload handling
    // =========================================================================
    // GAP: The server has no body size limit. An attacker can POST a multi-megabyte
    // (or multi-gigabyte) JSON body to exhaust server memory. Axum's default
    // limit for Json<T> is 2MB, but this is not explicitly configured and could
    // change between versions. This test verifies current behavior.
    // RECOMMENDATION: Add explicit `DefaultBodyLimit::max(1_048_576)` (1MB).

    #[tokio::test]
    async fn test_oversized_payload_10mb_should_be_rejected() {
        let app = test_app(test_state());

        // Build a ~10MB payload with many repeated events
        let single_event =
            r#"{"endToEndId":"E12345678901234567890","valor":"1.00","horario":"2026-01-01T00:00:00Z"}"#;
        let events: Vec<&str> = std::iter::repeat(single_event).take(100_000).collect();
        let payload = format!(r#"{{"pix":[{}]}}"#, events.join(","));

        assert!(
            payload.len() > 10_000_000,
            "Payload should be >10MB, got {} bytes",
            payload.len()
        );

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

        // CURRENT BEHAVIOR: Axum's default Json extractor has a 2MB limit,
        // so this should return 413 Payload Too Large.
        // If this test fails with 200, it means the server accepted a 10MB body,
        // which is a security concern.
        assert!(
            response.status() == StatusCode::PAYLOAD_TOO_LARGE
                || response.status() == StatusCode::BAD_REQUEST,
            "Expected 413 or 400 for oversized payload, got {}",
            response.status()
        );
    }

    // =========================================================================
    // 2. Deeply nested JSON
    // =========================================================================
    // GAP: Deeply nested JSON can cause stack overflow during deserialization.
    // serde_json has a recursion limit of 128 by default, but the devolucoes
    // field accepts arbitrary serde_json::Value, which could contain deep nesting.

    #[tokio::test]
    async fn test_deeply_nested_json_in_devolucoes_field() {
        let app = test_app(test_state());

        // Build JSON with 200 levels of nesting inside devolucoes
        let mut nested = "\"leaf\"".to_string();
        for _ in 0..200 {
            nested = format!(r#"{{"inner":{}}}"#, nested);
        }
        let payload = format!(
            r#"{{"pix":[{{"endToEndId":"E1","valor":"1.00","horario":"2026-01-01T00:00:00Z","devolucoes":[{}]}}]}}"#,
            nested
        );

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

        // serde_json should reject this with a recursion limit error (400)
        // or the server should handle it gracefully.
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
                || response.status() == StatusCode::OK, // If serde_json handles it
            "Deeply nested JSON should not crash the server, got {}",
            response.status()
        );
    }

    // =========================================================================
    // 3. XSS / injection in infoPagador field
    // =========================================================================
    // GAP: The infoPagador field is user-controlled payer text. The server
    // writes it to stdout and to a JSONL file without sanitization. If this
    // data is later rendered in a web UI, it could enable XSS.
    // This test verifies the data passes through to file output unsanitized,
    // documenting the risk.

    #[tokio::test]
    async fn test_xss_in_info_pagador_passes_through_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("events.jsonl");

        let state = Arc::new(AppState {
            forward_url: None,
            output_file: Some(output_path.to_str().unwrap().to_string()),
            quiet: true,
            http_client: reqwest::Client::new(),
        });
        let app = test_app(state);

        let xss_payload = "<script>alert('xss')</script>";
        let payload = format!(
            r#"{{"pix":[{{"endToEndId":"E1","valor":"1.00","horario":"2026-01-01T00:00:00Z","infoPagador":"{}"}}]}}"#,
            xss_payload
        );

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

        // SECURITY CONCERN: The XSS payload is written to the file as-is.
        // Any downstream consumer must sanitize before rendering in HTML.
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(
            content.contains("<script>"),
            "XSS payload should be present in file output (unsanitized). \
             If this changes, the server has gained input sanitization."
        );
    }

    // =========================================================================
    // 4. Special characters and injection attempts in fields
    // =========================================================================
    // GAP: Fields like endToEndId, valor, and horario accept arbitrary strings.
    // SQL injection, path traversal, and null byte injection should be tested.

    #[tokio::test]
    async fn test_special_characters_in_fields() {
        let app = test_app(test_state());

        let payload = r#"{"pix":[{
            "endToEndId": "E1'; DROP TABLE pix; --",
            "valor": "10.00",
            "horario": "2026-01-01T00:00:00Z",
            "infoPagador": "../../etc/passwd",
            "chave": "key\u0000injected"
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

        // The server currently accepts any string values without validation.
        // This documents the behavior -- ideally endToEndId should be validated.
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Server currently accepts arbitrary string values without validation"
        );
    }

    // =========================================================================
    // 5. Request without Content-Type header
    // =========================================================================
    // GAP: Axum's Json extractor requires Content-Type: application/json.
    // Requests without it should be rejected, not silently accepted.

    #[tokio::test]
    async fn test_request_without_content_type_header() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    // No Content-Type header
                    .body(Body::from(valid_payload()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Axum's Json extractor should reject requests without Content-Type.
        assert!(
            response.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
                || response.status() == StatusCode::BAD_REQUEST,
            "Request without Content-Type should be rejected, got {}",
            response.status()
        );
    }

    // =========================================================================
    // 6. Request with wrong Content-Type
    // =========================================================================
    // GAP: Only application/json should be accepted. Other media types
    // (text/plain, application/xml, multipart/form-data) must be rejected.

    #[tokio::test]
    async fn test_request_with_wrong_content_type_text_plain() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "text/plain")
                    .body(Body::from(valid_payload()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
                || response.status() == StatusCode::BAD_REQUEST,
            "text/plain Content-Type should be rejected, got {}",
            response.status()
        );
    }

    #[tokio::test]
    async fn test_request_with_wrong_content_type_xml() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/xml")
                    .body(Body::from("<pix/>"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
                || response.status() == StatusCode::BAD_REQUEST,
            "application/xml Content-Type should be rejected, got {}",
            response.status()
        );
    }

    // =========================================================================
    // 7. PUT/DELETE/PATCH on /pix should return 405 Method Not Allowed
    // =========================================================================
    // GAP: Only POST is routed for /pix. Other methods should return 405,
    // not 404 (which would leak information about endpoint existence).

    #[tokio::test]
    async fn test_put_on_pix_returns_405() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(valid_payload()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "PUT on /pix should return 405"
        );
    }

    #[tokio::test]
    async fn test_delete_on_pix_returns_405() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/pix")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "DELETE on /pix should return 405"
        );
    }

    #[tokio::test]
    async fn test_patch_on_pix_returns_405() {
        let app = test_app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(valid_payload()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "PATCH on /pix should return 405"
        );
    }

    // =========================================================================
    // 8. Empty string values in required fields
    // =========================================================================
    // GAP: The PixEvent struct requires endToEndId and valor as String, but
    // does not validate they are non-empty. Empty values pass deserialization
    // but are semantically invalid.

    #[tokio::test]
    async fn test_empty_end_to_end_id_accepted() {
        let app = test_app(test_state());

        let payload = r#"{"pix":[{"endToEndId":"","valor":"10.00","horario":"2026-01-01T00:00:00Z"}]}"#;

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

        // CURRENT BEHAVIOR: Server accepts empty endToEndId.
        // RECOMMENDATION: Validate that endToEndId is non-empty.
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Server currently accepts empty endToEndId without validation"
        );
    }

    #[tokio::test]
    async fn test_empty_valor_accepted() {
        let app = test_app(test_state());

        let payload = r#"{"pix":[{"endToEndId":"E1","valor":"","horario":"2026-01-01T00:00:00Z"}]}"#;

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

        // CURRENT BEHAVIOR: Server accepts empty valor.
        // RECOMMENDATION: Validate that valor is a valid decimal > 0.
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Server currently accepts empty valor without validation"
        );
    }

    // =========================================================================
    // 9. Concurrent webhook requests -- file output race conditions
    // =========================================================================
    // GAP: Multiple simultaneous webhook requests writing to the same output
    // file use std::fs::OpenOptions::append(true), which is NOT atomic on
    // all platforms. Lines from different requests could interleave within
    // the same write, producing corrupt JSONL.

    #[tokio::test]
    async fn test_concurrent_writes_produce_valid_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("concurrent.jsonl");

        let state = Arc::new(AppState {
            forward_url: None,
            output_file: Some(output_path.to_str().unwrap().to_string()),
            quiet: true,
            http_client: reqwest::Client::new(),
        });

        // We need to start a real server since oneshot consumes the router.
        let app = test_app(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let mut handles = Vec::new();

        // Send 20 concurrent requests, each with 5 events
        for batch in 0..20 {
            let client = client.clone();
            let url = format!("http://{}/pix", addr);
            handles.push(tokio::spawn(async move {
                let mut events = Vec::new();
                for i in 0..5 {
                    events.push(format!(
                        r#"{{"endToEndId":"E{:03}{:03}","valor":"{}.00","horario":"2026-01-01T00:00:00Z"}}"#,
                        batch, i, batch * 5 + i + 1
                    ));
                }
                let payload = format!(r#"{{"pix":[{}]}}"#, events.join(","));
                let resp = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await;
                assert!(resp.is_ok(), "Request {batch} failed");
                assert_eq!(resp.unwrap().status(), 200);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // Give a moment for any in-flight file writes to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let content = std::fs::read_to_string(&output_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // We sent 20 * 5 = 100 events total
        assert_eq!(
            lines.len(),
            100,
            "Expected 100 JSONL lines, got {}. \
             Missing lines may indicate a race condition in file writes.",
            lines.len()
        );

        // Every line must be valid JSON
        for (i, line) in lines.iter().enumerate() {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(
                parsed.is_ok(),
                "Line {} is not valid JSON (possible interleaved write): {}",
                i + 1,
                &line[..line.len().min(200)]
            );
        }

        server_handle.abort();
    }

    // =========================================================================
    // 10. Forward URL error handling (unreachable URL)
    // =========================================================================
    // GAP: When forward_url points to an unreachable host, the forwarding
    // runs in a spawned task. The main request should still return 200.
    // Errors should be logged but not propagated.

    #[tokio::test]
    async fn test_forward_url_unreachable_does_not_affect_response() {
        let state = Arc::new(AppState {
            forward_url: Some("http://192.0.2.1:9999/unreachable".to_string()), // RFC 5737 TEST-NET
            output_file: None,
            quiet: true,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(100))
                .build()
                .unwrap(),
        });
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pix")
                    .header("Content-Type", "application/json")
                    .body(Body::from(valid_payload()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // The handler should return 200 even if forwarding fails,
        // because forwarding is fire-and-forget via tokio::spawn.
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Unreachable forward URL should not block the response"
        );
    }

    // =========================================================================
    // 11. Multiple rapid requests (basic rate limiting behavior)
    // =========================================================================
    // GAP: The server has no rate limiting. An attacker can send thousands of
    // requests per second. This test documents the lack of rate limiting.

    #[tokio::test]
    async fn test_rapid_requests_all_accepted_no_rate_limiting() {
        let app = test_app(test_state());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let url = format!("http://{}/pix", addr);

        let mut success_count = 0u32;
        let mut reject_count = 0u32;

        // Send 50 rapid-fire requests
        for _ in 0..50 {
            let resp = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(valid_payload())
                .send()
                .await
                .unwrap();

            if resp.status() == reqwest::StatusCode::OK {
                success_count += 1;
            } else if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                reject_count += 1;
            }
        }

        // CURRENT BEHAVIOR: All 50 requests succeed (no rate limiting).
        // RECOMMENDATION: Implement rate limiting (e.g., tower::limit::RateLimitLayer).
        assert_eq!(
            success_count, 50,
            "All requests accepted -- no rate limiting in place"
        );
        assert_eq!(
            reject_count, 0,
            "No requests rate-limited. If this changes, rate limiting has been added."
        );

        server_handle.abort();
    }

    // =========================================================================
    // 12. Valid structure but semantically invalid data
    // =========================================================================
    // GAP: The server accepts any string for valor, including non-numeric
    // values, negative amounts, and absurdly large numbers.

    #[tokio::test]
    async fn test_non_numeric_valor_accepted() {
        let app = test_app(test_state());

        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"not-a-number","horario":"2026-01-01T00:00:00Z"}]}"#;

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

        // CURRENT BEHAVIOR: Server accepts non-numeric valor since it is typed as String.
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Non-numeric valor is accepted because it is typed as String"
        );
    }

    #[tokio::test]
    async fn test_negative_valor_accepted() {
        let app = test_app(test_state());

        let payload =
            r#"{"pix":[{"endToEndId":"E1","valor":"-999.99","horario":"2026-01-01T00:00:00Z"}]}"#;

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

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Negative valor is accepted because no validation exists"
        );
    }
}
