use crate::error::AppError;
use axum::http::StatusCode;
use bigdecimal::BigDecimal;
use std::{collections::BTreeMap, str::FromStr};

use super::*;
use crate::modules::quick_recharge::{
    infrastructure::{GMPAY_REQUEST_FAILED_CODE, create_gmpay_order_with_name},
    service::{
        QuickRechargeRuntimeConfig, decimal_to_gmpay_string, redirect_url_for_target,
        validate_optional_return_url,
    },
};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string_contains, method, path},
};

#[test]
fn gmpay_signature_matches_official_example() {
    let mut params = BTreeMap::new();
    params.insert("pid".to_owned(), "1000".to_owned());
    params.insert("order_id".to_owned(), "ORD202605230001".to_owned());
    params.insert("currency".to_owned(), "cny".to_owned());
    params.insert("token".to_owned(), "usdt".to_owned());
    params.insert("network".to_owned(), "tron".to_owned());
    params.insert("amount".to_owned(), "100".to_owned());
    params.insert(
        "notify_url".to_owned(),
        "https://merchant.example/notify".to_owned(),
    );
    params.insert(
        "redirect_url".to_owned(),
        "https://merchant.example/return".to_owned(),
    );
    params.insert("name".to_owned(), "VIP".to_owned());

    assert_eq!(
        gmpay_signature(&params, "epusdt_secret_key"),
        "476412c422f4dd75c3d533f5c47a9cac"
    );
}

#[test]
fn gmpay_signature_ignores_empty_and_signature_fields() {
    let mut params = BTreeMap::new();
    params.insert("signature".to_owned(), "bad".to_owned());
    params.insert("pid".to_owned(), "1000".to_owned());
    params.insert("amount".to_owned(), "100".to_owned());
    params.insert("empty".to_owned(), "   ".to_owned());

    let mut expected = BTreeMap::new();
    expected.insert("pid".to_owned(), "1000".to_owned());
    expected.insert("amount".to_owned(), "100".to_owned());

    assert_eq!(
        gmpay_signature(&params, "secret"),
        gmpay_signature(&expected, "secret")
    );
}

#[test]
fn decimal_to_gmpay_string_uses_plain_trimmed_decimal() {
    assert_eq!(
        decimal_to_gmpay_string(&BigDecimal::from_str("100.000000000000000000").unwrap()),
        "100"
    );
    assert_eq!(
        decimal_to_gmpay_string(&BigDecimal::from_str("14.290000000000000000").unwrap()),
        "14.29"
    );
}

#[test]
fn quick_recharge_return_target_uses_specific_url_with_default_fallback() {
    let mut config = quick_recharge_runtime_config("https://pay.example".to_owned());

    assert_eq!(
        redirect_url_for_target(&config, Some(QuickRechargeReturnTarget::PcApp)).as_deref(),
        Some("rustchain://quick-recharge/return")
    );
    assert_eq!(
        redirect_url_for_target(&config, Some(QuickRechargeReturnTarget::MobileWeb)).as_deref(),
        Some("https://m.merchant.example/return")
    );

    config.android_app_redirect_url = None;
    assert_eq!(
        redirect_url_for_target(&config, Some(QuickRechargeReturnTarget::AndroidApp)).as_deref(),
        Some("https://merchant.example/return")
    );
    assert_eq!(
        redirect_url_for_target(&config, None).as_deref(),
        Some("https://merchant.example/return")
    );
}

#[test]
fn quick_recharge_app_return_url_allows_deep_links() {
    assert_eq!(
        validate_optional_return_url(
            Some("rustchain-ios://quick-recharge/return".to_owned()),
            "ios_app_redirect_url",
        )
        .unwrap()
        .as_deref(),
        Some("rustchain-ios://quick-recharge/return")
    );
    assert!(
        validate_optional_return_url(
            Some("javascript:alert(1)".to_owned()),
            "ios_app_redirect_url",
        )
        .is_err()
    );
}

#[tokio::test]
async fn create_gmpay_order_posts_signed_custom_order_name() {
    let server = MockServer::start().await;
    let amount = BigDecimal::from_str("18.500000000000000000").unwrap();
    let order_id = "ORD202606130001";
    let mut expected_params = BTreeMap::new();
    expected_params.insert("pid".to_owned(), "1000".to_owned());
    expected_params.insert("order_id".to_owned(), order_id.to_owned());
    expected_params.insert("currency".to_owned(), "cny".to_owned());
    expected_params.insert("token".to_owned(), "usdt".to_owned());
    expected_params.insert("network".to_owned(), "tron".to_owned());
    expected_params.insert("amount".to_owned(), "18.5".to_owned());
    expected_params.insert(
        "notify_url".to_owned(),
        "https://merchant.example/notify".to_owned(),
    );
    expected_params.insert(
        "redirect_url".to_owned(),
        "https://merchant.example/return".to_owned(),
    );
    expected_params.insert("name".to_owned(), "Admin Quick Recharge Test".to_owned());
    let expected_signature = gmpay_signature(&expected_params, "secret");

    Mock::given(method("POST"))
        .and(path("/payments/gmpay/v1/order/create-transaction"))
        .and(body_string_contains("pid=1000"))
        .and(body_string_contains(format!("order_id={order_id}")))
        .and(body_string_contains("amount=18.5"))
        .and(body_string_contains("name=Admin+Quick+Recharge+Test"))
        .and(body_string_contains(format!(
            "signature={expected_signature}"
        )))
        .respond_with(move |request: &wiremock::Request| {
            let body = String::from_utf8(request.body.clone()).unwrap();
            let order_id = form_value(&body, "order_id").unwrap();
            let amount = form_value(&body, "amount").unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "status_code": 200,
                "message": "ok",
                "data": {
                    "trade_id": "GM202606130001",
                    "order_id": order_id,
                    "amount": amount,
                    "currency": "cny",
                    "actual_amount": "2.500000000000000000",
                    "receive_address": "TTestReceiveAddress",
                    "token": "usdt",
                    "expiration_time": 1_775_100_000,
                    "payment_url": "https://cashier.example/GM202606130001"
                }
            }))
        })
        .mount(&server)
        .await;

    let config = quick_recharge_runtime_config(server.uri());

    let response = create_gmpay_order_with_name(
        &config,
        order_id,
        &amount,
        "Admin Quick Recharge Test",
        None,
    )
    .await
    .unwrap();

    assert_eq!(response.trade_id, "GM202606130001");
    assert_eq!(response.order_id, order_id);
    assert_eq!(response.amount, amount);
    assert_eq!(
        response.payment_url,
        "https://cashier.example/GM202606130001"
    );
}

#[tokio::test]
async fn create_gmpay_order_sanitizes_cloudflare_challenge_error() {
    let server = MockServer::start().await;
    let amount = BigDecimal::from_str("18.500000000000000000").unwrap();
    let order_id = "ORD202606130002";

    Mock::given(method("POST"))
        .and(path("/payments/gmpay/v1/order/create-transaction"))
        .respond_with(ResponseTemplate::new(403).insert_header(
            "content-type",
            "text/html; charset=UTF-8",
        ).set_body_string(
            r#"<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</title></head><body><script src="/cdn-cgi/challenge-platform/h/g/orchestrate/chl_page/v1"></script><iframe src="https://challenges.cloudflare.com"></iframe></body></html>"#,
        ))
        .mount(&server)
        .await;

    let error = create_gmpay_order_with_name(
        &quick_recharge_runtime_config(server.uri()),
        order_id,
        &amount,
        "Admin Quick Recharge Test",
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
        panic!("expected GMPay api error");
    };
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(code, GMPAY_REQUEST_FAILED_CODE);
    assert!(message.contains("Cloudflare 防护拦截"));
    assert!(message.contains("放行名单"));
    assert!(!message.contains("<!DOCTYPE html>"));
    assert!(!message.contains("challenge-platform"));
}

fn quick_recharge_runtime_config(api_base_url: String) -> QuickRechargeRuntimeConfig {
    QuickRechargeRuntimeConfig {
        api_base_url,
        merchant_pid: "1000".to_owned(),
        merchant_secret: "secret".to_owned(),
        currency: "cny".to_owned(),
        token: "usdt".to_owned(),
        network: "tron".to_owned(),
        notify_url: "https://merchant.example/notify".to_owned(),
        redirect_url: Some("https://merchant.example/return".to_owned()),
        pc_app_redirect_url: Some("rustchain://quick-recharge/return".to_owned()),
        mac_app_redirect_url: Some("rustchain-mac://quick-recharge/return".to_owned()),
        ios_app_redirect_url: Some("rustchain-ios://quick-recharge/return".to_owned()),
        android_app_redirect_url: Some("rustchain-android://quick-recharge/return".to_owned()),
        mobile_web_redirect_url: Some("https://m.merchant.example/return".to_owned()),
        desktop_web_redirect_url: Some("https://merchant.example/return".to_owned()),
        min_amount: BigDecimal::from_str("1").unwrap(),
        max_amount: None,
    }
}

fn form_value(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    body.split('&')
        .find_map(|part| part.strip_prefix(&prefix))
        .map(|value| value.replace('+', " "))
}
