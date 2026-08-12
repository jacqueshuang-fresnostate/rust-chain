use super::*;
use axum::http::StatusCode;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string_contains, method, path},
};

#[tokio::test]
async fn turnstile_adapter_posts_token_and_remote_ip() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/turnstile/v0/siteverify"))
        .and(body_string_contains("secret=server-secret"))
        .and(body_string_contains("response=client-token"))
        .and(body_string_contains("remoteip=203.0.113.7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "hostname": "login.example"
        })))
        .mount(&server)
        .await;

    verify_turnstile_site_response(
        &format!("{}/turnstile/v0/siteverify", server.uri()),
        "server-secret",
        "client-token",
        Some("203.0.113.7"),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn turnstile_adapter_preserves_invalid_challenge_error_contract() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/turnstile/v0/siteverify"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": false,
            "error-codes": ["timeout-or-duplicate", "invalid-input-response"]
        })))
        .mount(&server)
        .await;

    let error = verify_turnstile_site_response(
        &format!("{}/turnstile/v0/siteverify", server.uri()),
        "server-secret",
        "client-token",
        None,
    )
    .await
    .unwrap_err();

    let AppError::Api {
        status,
        code,
        message,
    } = error
    else {
        panic!("expected Turnstile api error");
    };
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(code, "CF_TURNSTILE_INVALID");
    assert_eq!(
        message,
        "Cloudflare verification failed: timeout-or-duplicate, invalid-input-response"
    );
}

#[tokio::test]
async fn turnstile_adapter_preserves_bad_and_unparseable_response_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/bad-status"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/invalid-json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&server)
        .await;

    assert_api_error(
        verify_turnstile_site_response(
            &format!("{}/bad-status", server.uri()),
            "server-secret",
            "client-token",
            None,
        )
        .await
        .unwrap_err(),
        "CF_TURNSTILE_BAD_RESPONSE",
        "Cloudflare challenge verification returned 503 Service Unavailable",
    );
    assert_api_error(
        verify_turnstile_site_response(
            &format!("{}/invalid-json", server.uri()),
            "server-secret",
            "client-token",
            None,
        )
        .await
        .unwrap_err(),
        "CF_TURNSTILE_PARSE_FAILED",
        "invalid Cloudflare verification response:",
    );
}

#[tokio::test]
async fn turnstile_adapter_preserves_request_failure_error() {
    assert_api_error(
        verify_turnstile_site_response(
            "://invalid-siteverify-url",
            "server-secret",
            "client-token",
            None,
        )
        .await
        .unwrap_err(),
        "CF_TURNSTILE_REQUEST_FAILED",
        "failed to verify Cloudflare challenge:",
    );
}

#[test]
fn turnstile_adapter_timeout_is_fixed_at_five_seconds() {
    assert_eq!(TURNSTILE_VERIFY_TIMEOUT, StdDuration::from_secs(5));
}

fn assert_api_error(error: AppError, expected_code: &'static str, expected_message: &str) {
    let AppError::Api {
        status,
        code,
        message,
    } = error
    else {
        panic!("expected Turnstile api error");
    };
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(code, expected_code);
    assert!(message.contains(expected_message), "{message}");
}
