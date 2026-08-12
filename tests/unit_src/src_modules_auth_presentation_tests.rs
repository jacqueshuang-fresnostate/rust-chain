use super::*;
use axum::http::HeaderValue;

#[test]
fn login_transport_prefers_cloudflare_ip_and_detects_clearance_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.7"));
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.8, 10.0.0.1"),
    );
    headers.insert(
        "cookie",
        HeaderValue::from_static("session=one; cf_clearance=clearance-token; theme=dark"),
    );

    assert_eq!(
        LoginTransportContext::from_headers(&headers),
        LoginTransportContext {
            remote_ip: Some("203.0.113.7".to_owned()),
            has_cf_clearance: true,
        }
    );
}

#[test]
fn login_transport_uses_first_forwarded_ip_without_false_clearance_match() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.8, 10.0.0.1"),
    );
    headers.insert(
        "cookie",
        HeaderValue::from_static("prefix_cf_clearance=not-clearance"),
    );

    assert_eq!(
        LoginTransportContext::from_headers(&headers),
        LoginTransportContext {
            remote_ip: Some("198.51.100.8".to_owned()),
            has_cf_clearance: false,
        }
    );
}
