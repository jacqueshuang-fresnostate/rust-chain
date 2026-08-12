use super::*;
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::post,
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[test]
fn presentation_facade_keeps_public_config_dto_paths() {
    fn assert_type<T>() {}

    assert_type::<SaveMarketFeedConfigRequest>();
    assert_type::<MarketFeedConfigResponse>();
    assert_type::<SaveSmtpConfigRequest>();
    assert_type::<UploadConfigResponse>();
    assert_type::<UploadFileInput>();
    assert_type::<UploadImageResponse>();
}

#[test]
fn presentation_facade_keeps_crate_visible_subdomain_dto_paths() {
    fn assert_type<T>() {}

    assert_type::<AdminUserQuery>();
    assert_type::<AdminWalletAccountQuery>();
    assert_type::<AdminAssetQuery>();
    assert_type::<AdminTradingPairQuery>();
    assert_type::<AdminRiskRuleQuery>();
    assert_type::<AdminNewCoinProjectQuery>();
    assert_type::<AdminAgentQuery>();
    assert_type::<AdminConvertPairQuery>();
}

async fn inspect_multipart(multipart: Multipart) -> AppResult<Json<Value>> {
    let input = multipart_file_input(multipart).await?;
    Ok(Json(json!({
        "original_filename": input.original_filename,
        "mime_type": input.mime_type,
        "bytes": input.bytes,
    })))
}

fn multipart_request(boundary: &str, body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body.into())
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn multipart_file_input_keeps_filename_mime_and_file_bytes() {
    let boundary = "presentation-upload-boundary";
    let expected_bytes = b"\0GIF89a\xff\r\npayload";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"metadata\"\r\n\r\nignored\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"avatar.GIF\"\r\nContent-Type: image/gif\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(expected_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = Router::new()
        .route("/upload", post(inspect_multipart))
        .oneshot(multipart_request(boundary, body))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["original_filename"], "avatar.GIF");
    assert_eq!(payload["mime_type"], "image/gif");
    assert_eq!(payload["bytes"], json!(expected_bytes));
}

#[tokio::test]
async fn multipart_file_input_requires_exact_file_field() {
    let boundary = "missing-file-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"avatar.gif\"\r\nContent-Type: image/gif\r\n\r\nGIF89a\r\n--{boundary}--\r\n"
    );

    let response = Router::new()
        .route("/upload", post(inspect_multipart))
        .oneshot(multipart_request(boundary, body))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({
            "code": "VALIDATION_ERROR",
            "message": "validation error: upload file is required",
        })
    );
}

#[tokio::test]
async fn multipart_file_input_requires_file_content_type() {
    let boundary = "missing-content-type-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"avatar.gif\"\r\n\r\nGIF89a\r\n--{boundary}--\r\n"
    );

    let response = Router::new()
        .route("/upload", post(inspect_multipart))
        .oneshot(multipart_request(boundary, body))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({
            "code": "VALIDATION_ERROR",
            "message": "validation error: upload file content type is required",
        })
    );
}

#[tokio::test]
async fn multipart_file_input_maps_malformed_fields_to_existing_error() {
    let boundary = "invalid-multipart-boundary";
    let body =
        format!("--{boundary}\r\nContent-Disposition broken\r\n\r\nGIF89a\r\n--{boundary}--\r\n");

    let response = Router::new()
        .route("/upload", post(inspect_multipart))
        .oneshot(multipart_request(boundary, body))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({
            "code": "VALIDATION_ERROR",
            "message": "validation error: upload multipart body is invalid",
        })
    );
}
