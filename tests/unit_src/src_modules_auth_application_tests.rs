use super::*;
use axum::http::StatusCode;

fn runtime_config(
    secret: Option<&str>,
    site_key: Option<&str>,
    enforce_token: bool,
) -> TurnstileRuntimeConfig {
    TurnstileRuntimeConfig {
        secret: secret.map(str::to_owned),
        site_key: site_key.map(str::to_owned),
        enforce_token,
        siteverify_url: "http://127.0.0.1:1/unused".to_owned(),
    }
}

fn transport(has_cf_clearance: bool) -> LoginTransportContext {
    LoginTransportContext {
        remote_ip: Some("203.0.113.7".to_owned()),
        has_cf_clearance,
    }
}

#[test]
fn login_config_preserves_enabled_and_site_key_contract() {
    assert_eq!(
        turnstile_login_config(&runtime_config(None, None, false)),
        (false, None)
    );
    assert_eq!(
        turnstile_login_config(&runtime_config(Some("secret"), None, false)),
        (false, None)
    );
    assert_eq!(
        turnstile_login_config(&runtime_config(None, Some("site-key"), false)),
        (false, Some("site-key".to_owned()))
    );
    assert_eq!(
        turnstile_login_config(&runtime_config(Some("secret"), Some("site-key"), false,)),
        (true, Some("site-key".to_owned()))
    );
}

#[tokio::test]
async fn login_turnstile_use_case_skips_when_disabled_or_clearance_is_compatible() {
    verify_login_turnstile_with_runtime(
        None,
        transport(false),
        &runtime_config(Some("secret"), None, true),
    )
    .await
    .unwrap();

    verify_login_turnstile_with_runtime(
        None,
        transport(true),
        &runtime_config(Some("secret"), Some("site-key"), false),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn login_turnstile_use_case_preserves_missing_token_error() {
    let error = verify_login_turnstile_with_runtime(
        Some("   ".to_owned()),
        transport(true),
        &runtime_config(Some("secret"), Some("site-key"), true),
    )
    .await
    .unwrap_err();

    let AppError::Api {
        status,
        code,
        message,
    } = error
    else {
        panic!("expected Turnstile validation error");
    };
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code, "CF_TURNSTILE_TOKEN_MISSING");
    assert_eq!(message, "cf_turnstile_token is required");
}
