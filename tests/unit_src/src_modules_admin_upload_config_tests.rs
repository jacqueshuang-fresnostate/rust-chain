use super::*;
use crate::modules::admin::service::validate_upload_config;

#[test]
fn validates_upload_provider_config() {
    let config = validate_upload_config(&SaveUploadConfigRequest {
        provider: "image-bed".to_owned(),
        endpoint: Some(" https://oss.example.test/api/v1/upload ".to_owned()),
        file_field: None,
        bearer_token: None,
        access_key: None,
        secret_key: None,
        bucket: None,
        region: None,
        public_base_url: None,
        local_root: None,
        key_prefix: Some(" /images//2026 ".to_owned()),
        max_file_size_bytes: None,
        allowed_mime_types: Some(vec![" image/png ".to_owned(), "image/png".to_owned()]),
        enabled: true,
        reason: Some("configure upload".to_owned()),
    })
    .unwrap();

    assert_eq!(config.provider.code(), "image_bed");
    assert_eq!(config.file_field.as_deref(), Some("file"));
    assert_eq!(config.key_prefix.as_deref(), Some("images/2026"));
    assert_eq!(config.allowed_mime_types, ["image/png"]);
}

#[test]
fn rejects_invalid_upload_provider_config() {
    let request = SaveUploadConfigRequest {
        provider: "local".to_owned(),
        endpoint: None,
        file_field: None,
        bearer_token: None,
        access_key: None,
        secret_key: None,
        bucket: None,
        region: None,
        public_base_url: None,
        local_root: None,
        key_prefix: None,
        max_file_size_bytes: Some(0),
        allowed_mime_types: Some(vec!["text/plain".to_owned()]),
        enabled: true,
        reason: Some("configure upload".to_owned()),
    };

    assert!(validate_upload_config(&request).is_err());
}
