use super::*;
use utoipa::{PartialSchema, Path};

#[test]
fn login_two_factor_setup_paths_document_public_request_and_response_contracts() {
    assert_eq!(
        <__path_user_login_two_factor_setup as Path>::path(),
        "/api/v1/auth/login/2fa/setup"
    );
    assert_eq!(
        <__path_user_login_two_factor_setup_confirm as Path>::path(),
        "/api/v1/auth/login/2fa/setup/confirm"
    );

    let setup_operation =
        serde_json::to_value(<__path_user_login_two_factor_setup as Path>::operation()).unwrap();
    assert_eq!(
        setup_operation["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/LoginTwoFactorSetupRequest"
    );
    assert_eq!(
        setup_operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/LoginTwoFactorSetupResponse"
    );

    let confirm_operation =
        serde_json::to_value(<__path_user_login_two_factor_setup_confirm as Path>::operation())
            .unwrap();
    assert_eq!(
        confirm_operation["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/LoginTwoFactorSetupConfirmRequest"
    );
    assert_eq!(
        confirm_operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/TokenResponse"
    );

    let setup_response_schema =
        serde_json::to_value(LoginTwoFactorSetupResponse::schema()).unwrap();
    let properties = &setup_response_schema["properties"];
    for field in ["secret", "otpauth_uri", "expires_in_seconds"] {
        assert!(
            properties.get(field).is_some(),
            "missing LoginTwoFactorSetupResponse.{field}"
        );
    }
}
