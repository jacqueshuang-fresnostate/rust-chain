use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use exchange_api::{build_router, config::Settings, state::AppState};
use secrecy::SecretString;
use serde_json::Value;
use tower::ServiceExt;

fn test_state() -> AppState {
    AppState::new(Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
    })
}

async fn request_json(uri: &str) -> Value {
    let response = build_router(test_state())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn openapi_json() -> Value {
    request_json("/openapi.json").await
}

fn operation_has_bearer_security(openapi: &Value, path: &str, method: &str) -> bool {
    openapi["paths"][path][method]["security"]
        .as_array()
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.get("bearerAuth").is_some())
        })
}

fn schema_is_unix_millis(value: &Value) -> bool {
    let has_integer_type = value.get("type").is_some_and(|schema_type| {
        schema_type == "integer"
            || schema_type
                .as_array()
                .is_some_and(|types| types.iter().any(|value| value == "integer"))
    });
    if has_integer_type && value.get("format") == Some(&Value::String("int64".to_owned())) {
        return true;
    }

    value
        .get("anyOf")
        .or_else(|| value.get("oneOf"))
        .and_then(Value::as_array)
        .is_some_and(|schemas| schemas.iter().any(schema_is_unix_millis))
}

#[tokio::test]
async fn openapi_json_exposes_first_batch_contract() {
    let openapi = openapi_json().await;

    assert_eq!(openapi["openapi"].as_str(), Some("3.1.0"));
    assert!(openapi["info"]["title"].as_str().is_some());
    assert_eq!(
        openapi["components"]["securitySchemes"]["bearerAuth"]["scheme"].as_str(),
        Some("bearer")
    );

    for path in [
        "/health",
        "/api/v1/auth/login/config",
        "/api/v1/auth/register",
        "/api/v1/auth/login",
        "/api/v1/auth/refresh",
        "/api/v1/countries",
        "/api/v1/platform/brand",
        "/admin/api/v1/auth/register",
        "/admin/api/v1/auth/login",
        "/admin/api/v1/auth/refresh",
        "/admin/api/v1/platform/brand",
        "/admin/api/v1/countries",
        "/admin/api/v1/countries/{id}",
        "/admin/api/v1/countries/{id}/status",
        "/agent/api/v1/auth/register",
        "/agent/api/v1/auth/login",
        "/agent/api/v1/auth/refresh",
        "/api/v1/user/profile",
        "/api/v1/user/username",
        "/api/v1/user/email/bind-code",
        "/api/v1/user/email/bind",
        "/api/v1/user/password",
        "/api/v1/user/fund-password",
        "/api/v1/user/fund-password/reset-code",
        "/api/v1/user/fund-password/reset",
        "/api/v1/wallet/deposit-assets",
        "/api/v1/wallet/withdraw-assets",
        "/api/v1/wallet/deposit-address",
        "/admin/api/v1/smtp/config",
        "/admin/api/v1/smtp/configs",
        "/admin/api/v1/smtp/configs/{id}",
        "/admin/api/v1/smtp/delivery-settings",
        "/admin/api/v1/smtp/test",
        "/admin/api/v1/deposit-address-pool",
        "/admin/api/v1/deposit-address-pool/{id}",
        "/admin/api/v1/deposit-address-pool/{id}/reclaim",
        "/admin/api/v1/quick-recharge/config",
        "/admin/api/v1/quick-recharge/config/test",
        "/admin/api/v1/quick-recharge/orders",
    ] {
        assert!(openapi["paths"].get(path).is_some(), "missing path {path}");
    }

    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/user/profile",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/user/email/bind-code",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/user/fund-password/reset-code",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/wallet/deposit-assets",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/wallet/withdraw-assets",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/wallet/deposit-address",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/deposit-address-pool",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/api/v1/user/fund-password/reset",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/smtp/config",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/smtp/configs",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/smtp/configs",
        "post"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/smtp/configs/{id}",
        "patch"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/smtp/delivery-settings",
        "patch"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/countries",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/countries/{id}",
        "patch"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/countries/{id}/status",
        "patch"
    ));
    assert!(!operation_has_bearer_security(
        &openapi,
        "/api/v1/countries",
        "get"
    ));
    assert!(!operation_has_bearer_security(
        &openapi,
        "/api/v1/platform/brand",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/platform/brand",
        "get"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/platform/brand",
        "patch"
    ));
    assert!(operation_has_bearer_security(
        &openapi,
        "/admin/api/v1/quick-recharge/config/test",
        "post"
    ));

    let error_properties = &openapi["components"]["schemas"]["ErrorResponse"]["properties"];
    assert!(error_properties.get("code").is_some());
    assert!(error_properties.get("message").is_some());

    let profile_properties = &openapi["components"]["schemas"]["UserProfileResponse"]["properties"];
    assert!(schema_is_unix_millis(
        &profile_properties["email_verified_at"]
    ));

    let smtp_response_properties =
        &openapi["components"]["schemas"]["SmtpConfigResponse"]["properties"];
    assert!(smtp_response_properties.get("username_mask").is_some());
    assert!(smtp_response_properties.get("password_set").is_some());
    assert!(smtp_response_properties.get("priority").is_some());
    assert!(
        smtp_response_properties
            .get("verification_code_template_html")
            .is_some()
    );
    assert!(
        smtp_response_properties
            .get("verification_code_templates")
            .is_some()
    );
    assert!(smtp_response_properties.get("password").is_none());
    assert!(
        smtp_response_properties
            .get("password_ciphertext")
            .is_none()
    );
    assert!(
        smtp_response_properties
            .get("username_ciphertext")
            .is_none()
    );
    let smtp_list_properties =
        &openapi["components"]["schemas"]["SmtpConfigListResponse"]["properties"];
    assert!(smtp_list_properties.get("configs").is_some());
    assert!(smtp_list_properties.get("delivery_settings").is_some());
    let smtp_delivery_settings_properties =
        &openapi["components"]["schemas"]["SmtpDeliverySettingsResponse"]["properties"];
    assert!(smtp_delivery_settings_properties.get("strategy").is_some());

    let platform_brand_properties =
        &openapi["components"]["schemas"]["PlatformBrandResponse"]["properties"];
    assert!(platform_brand_properties.get("platform_name").is_some());
    assert!(platform_brand_properties.get("logo_url").is_some());
    assert!(platform_brand_properties.get("chart_provider").is_some());
    assert!(schema_is_unix_millis(
        &platform_brand_properties["updated_at"]
    ));

    let deposit_asset_properties =
        &openapi["components"]["schemas"]["DepositAssetResponse"]["properties"];
    assert!(deposit_asset_properties.get("deposit_enabled").is_some());
    assert!(deposit_asset_properties.get("withdraw_enabled").is_some());
    assert!(deposit_asset_properties.get("withdraw_fee_tiers").is_some());

    let quick_recharge_config_properties =
        &openapi["components"]["schemas"]["QuickRechargeConfigResponse"]["properties"];
    assert!(
        quick_recharge_config_properties
            .get("pc_app_redirect_url")
            .is_some()
    );
    assert!(
        quick_recharge_config_properties
            .get("desktop_web_redirect_url")
            .is_some()
    );
    let quick_recharge_create_properties =
        &openapi["components"]["schemas"]["CreateQuickRechargeOrderRequest"]["properties"];
    assert!(
        quick_recharge_create_properties
            .get("return_target")
            .is_some()
    );
    let quick_recharge_order_properties =
        &openapi["components"]["schemas"]["QuickRechargeOrderResponse"]["properties"];
    assert!(
        quick_recharge_order_properties
            .get("return_target")
            .is_some()
    );
    assert!(
        quick_recharge_order_properties
            .get("redirect_url")
            .is_some()
    );

    let quick_recharge_test_properties =
        &openapi["components"]["schemas"]["TestQuickRechargeConfigResponse"]["properties"];
    assert!(quick_recharge_test_properties.get("order_id").is_some());
    assert!(
        quick_recharge_test_properties
            .get("provider_trade_id")
            .is_some()
    );
    assert!(quick_recharge_test_properties.get("payment_url").is_some());
    assert!(schema_is_unix_millis(
        &quick_recharge_test_properties["tested_at"]
    ));
}

#[tokio::test]
async fn openapi_json_documents_agent_management_contract() {
    let openapi = openapi_json().await;

    for (path, methods) in [
        ("/admin/api/v1/agents", ["get", "post"].as_slice()),
        ("/admin/api/v1/agents/{id}", ["get"].as_slice()),
        ("/admin/api/v1/agents/{id}/status", ["patch"].as_slice()),
        ("/admin/api/v1/agents/{id}/users", ["get"].as_slice()),
        ("/admin/api/v1/users/{id}/agent", ["patch"].as_slice()),
        ("/admin/api/v1/agent-commissions", ["get"].as_slice()),
        (
            "/admin/api/v1/agent-commissions/{id}/status",
            ["patch"].as_slice(),
        ),
        (
            "/admin/api/v1/agent-commission-rules",
            ["get", "post"].as_slice(),
        ),
        (
            "/admin/api/v1/agent-commission-rules/{id}",
            ["patch"].as_slice(),
        ),
    ] {
        for method in methods {
            assert!(
                openapi["paths"][path].get(*method).is_some(),
                "missing {method} {path}"
            );
            assert!(
                operation_has_bearer_security(&openapi, path, method),
                "missing bearer security on {method} {path}"
            );
        }
    }

    assert!(
        openapi["paths"]["/agent/api/v1/auth/register"]["post"]["responses"]
            .get("403")
            .is_some()
    );
    assert!(
        openapi["paths"]["/agent/api/v1/auth/register"]["post"]["responses"]
            .get("200")
            .is_none()
    );

    let agent_auth_properties = &openapi["components"]["schemas"]["AgentAuthRequest"]["properties"];
    assert!(agent_auth_properties.get("agent_id").is_none());

    let create_agent_properties =
        &openapi["components"]["schemas"]["CreateAdminAgentRequest"]["properties"];
    assert!(create_agent_properties.get("admin_password").is_some());
    assert!(create_agent_properties.get("parent_agent_id").is_some());
    assert!(create_agent_properties.get("admin_password_hash").is_none());
    assert!(create_agent_properties.get("password_hash").is_none());

    let agent_response_properties =
        &openapi["components"]["schemas"]["AdminAgentResponse"]["properties"];
    assert!(agent_response_properties.get("email").is_some());
    for field in [
        "parent_agent_id",
        "root_agent_id",
        "path",
        "direct_user_count",
        "team_user_count",
        "child_agent_count",
    ] {
        assert!(
            agent_response_properties.get(field).is_some(),
            "missing AdminAgentResponse.{field}"
        );
    }
    assert!(agent_response_properties.get("admin_status").is_some());
    assert!(agent_response_properties.get("password_hash").is_none());

    let agent_user_properties =
        &openapi["components"]["schemas"]["AdminAgentUserResponse"]["properties"];
    for field in [
        "owner_agent_id",
        "root_agent_id",
        "direct_inviter_id",
        "direct_inviter_type",
    ] {
        assert!(
            agent_user_properties.get(field).is_some(),
            "missing AdminAgentUserResponse.{field}"
        );
    }

    let commission_status_properties =
        &openapi["components"]["schemas"]["UpdateAdminAgentCommissionStatusRequest"]["properties"];
    assert_eq!(
        commission_status_properties["status"]["pattern"].as_str(),
        Some("^(settled|rejected)$")
    );

    let commission_rule_properties =
        &openapi["components"]["schemas"]["AdminAgentCommissionRuleResponse"]["properties"];
    assert!(commission_rule_properties.get("updated_at").is_some());
    assert!(commission_rule_properties.get("commission_rate").is_some());
    assert_eq!(
        commission_rule_properties["product_type"]["pattern"].as_str(),
        Some("^(convert|prediction|spot|margin|seconds_contract)$")
    );

    let create_rule_properties =
        &openapi["components"]["schemas"]["CreateAdminAgentCommissionRuleRequest"]["properties"];
    assert_eq!(
        create_rule_properties["product_type"]["pattern"].as_str(),
        Some("^(convert|prediction|spot|margin|seconds_contract)$")
    );

    let commission_properties =
        &openapi["components"]["schemas"]["AdminAgentCommissionResponse"]["properties"];
    assert!(commission_properties.get("commission_rate").is_some());
}

#[tokio::test]
async fn openapi_json_documents_agent_portal_contract() {
    let openapi = openapi_json().await;

    for (path, methods) in [
        ("/agent/api/v1/me", ["get"].as_slice()),
        ("/agent/api/v1/dashboard", ["get"].as_slice()),
        ("/agent/api/v1/users", ["get"].as_slice()),
        ("/agent/api/v1/invite-codes", ["get", "post"].as_slice()),
        (
            "/agent/api/v1/invite-codes/{id}/status",
            ["patch"].as_slice(),
        ),
        ("/agent/api/v1/commissions", ["get"].as_slice()),
        ("/agent/api/v1/convert/stats", ["get"].as_slice()),
        ("/agent/api/v1/sub-agents", ["get"].as_slice()),
        ("/agent/api/v1/team-tree", ["get"].as_slice()),
    ] {
        for method in methods {
            assert!(
                openapi["paths"][path].get(*method).is_some(),
                "missing {method} {path}"
            );
            assert!(
                operation_has_bearer_security(&openapi, path, method),
                "missing bearer security on {method} {path}"
            );
        }
    }

    for schema_name in [
        "AgentMeResponse",
        "AgentDashboardResponse",
        "AgentTeamUserResponse",
        "AgentUsersResponse",
        "CreateAgentInviteCodeRequest",
        "UpdateAgentInviteCodeStatusRequest",
        "AgentInviteCodeResponse",
        "AgentInviteCodesResponse",
        "AgentCommissionResponse",
        "AgentCommissionsResponse",
        "AgentConvertStatsResponse",
        "AgentSubAgentResponse",
        "AgentSubAgentsResponse",
        "AgentTeamTreeNodeResponse",
        "AgentTeamTreeResponse",
    ] {
        let schema = &openapi["components"]["schemas"][schema_name];
        assert!(
            schema.get("properties").is_some(),
            "missing schema {schema_name}"
        );
        let schema_json = serde_json::to_string(schema).unwrap();
        assert!(
            !schema_json.contains("password_hash"),
            "schema {schema_name} leaks password_hash"
        );
        assert!(
            !schema_json.contains("access_token"),
            "schema {schema_name} leaks access_token"
        );
        assert!(
            !schema_json.contains("refresh_token"),
            "schema {schema_name} leaks refresh_token"
        );
    }

    let me_properties = &openapi["components"]["schemas"]["AgentMeResponse"]["properties"];
    for field in [
        "agent_admin_id",
        "agent_id",
        "username",
        "agent_code",
        "parent_agent_id",
        "root_agent_id",
        "level",
        "path",
        "agent_status",
        "admin_status",
        "last_login_at",
    ] {
        assert!(
            me_properties.get(field).is_some(),
            "missing AgentMeResponse.{field}"
        );
    }
    assert!(schema_is_unix_millis(&me_properties["last_login_at"]));

    let invite_code_properties =
        &openapi["components"]["schemas"]["AgentInviteCodeResponse"]["properties"];
    assert!(schema_is_unix_millis(&invite_code_properties["created_at"]));
    assert_eq!(
        invite_code_properties["status"]["pattern"].as_str(),
        Some("^(active|disabled)$")
    );

    let commission_properties =
        &openapi["components"]["schemas"]["AgentCommissionResponse"]["properties"];
    assert!(commission_properties.get("commission_rate").is_some());
    assert!(schema_is_unix_millis(&commission_properties["created_at"]));
    assert!(schema_is_unix_millis(
        &commission_properties["payout_created_at"]
    ));

    let team_user_properties =
        &openapi["components"]["schemas"]["AgentTeamUserResponse"]["properties"];
    assert!(schema_is_unix_millis(&team_user_properties["referred_at"]));
    for field in [
        "owner_agent_id",
        "root_agent_id",
        "direct_inviter_id",
        "direct_inviter_type",
    ] {
        assert!(
            team_user_properties.get(field).is_some(),
            "missing AgentTeamUserResponse.{field}"
        );
    }

    let team_tree_properties =
        &openapi["components"]["schemas"]["AgentTeamTreeNodeResponse"]["properties"];
    assert!(schema_is_unix_millis(&team_tree_properties["referred_at"]));
}

#[tokio::test]
async fn openapi_json_documents_admin_news_contract() {
    let openapi = openapi_json().await;

    for (path, methods) in [
        ("/admin/api/v1/news", ["get", "post"].as_slice()),
        ("/admin/api/v1/news/{id}", ["get", "patch"].as_slice()),
        ("/admin/api/v1/news/{id}/status", ["patch"].as_slice()),
    ] {
        for method in methods {
            assert!(
                openapi["paths"][path].get(*method).is_some(),
                "missing {method} {path}"
            );
            assert!(
                operation_has_bearer_security(&openapi, path, method),
                "missing bearer security on {method} {path}"
            );
        }
    }

    let tags = openapi["tags"].as_array().unwrap();
    assert!(tags.iter().any(|tag| tag["name"] == "admin-news"));

    for schema_name in [
        "NewsContentDocument",
        "NewsContentTranslation",
        "NewsRichTextBlock",
        "NewsRichTextLeaf",
        "AdminNewsItemResponse",
        "AdminNewsItemsResponse",
        "CreateAdminNewsItemRequest",
        "UpdateAdminNewsItemRequest",
        "UpdateAdminNewsStatusRequest",
    ] {
        let schema = &openapi["components"]["schemas"][schema_name];
        assert!(
            schema.get("properties").is_some(),
            "missing schema {schema_name}"
        );
        let schema_json = serde_json::to_string(schema).unwrap().to_lowercase();
        for sensitive in ["password", "token", "secret", "ciphertext"] {
            assert!(
                !schema_json.contains(sensitive),
                "schema {schema_name} leaks {sensitive}"
            );
        }
    }

    let news_properties = &openapi["components"]["schemas"]["AdminNewsItemResponse"]["properties"];
    for field in [
        "id",
        "title",
        "category",
        "status",
        "country_code",
        "default_locale",
        "content_json",
        "published_at",
        "created_by_admin_id",
        "updated_by_admin_id",
        "created_at",
        "updated_at",
    ] {
        assert!(
            news_properties.get(field).is_some(),
            "missing AdminNewsItemResponse.{field}"
        );
    }
    assert!(schema_is_unix_millis(&news_properties["published_at"]));
    assert!(schema_is_unix_millis(&news_properties["created_at"]));
    assert!(schema_is_unix_millis(&news_properties["updated_at"]));
    assert_eq!(
        news_properties["category"]["pattern"].as_str(),
        Some("^(general|market|product|system|promotion)$")
    );
    assert_eq!(
        news_properties["status"]["pattern"].as_str(),
        Some("^(draft|published|archived)$")
    );

    let create_properties =
        &openapi["components"]["schemas"]["CreateAdminNewsItemRequest"]["properties"];
    assert!(create_properties.get("content_json").is_some());
    assert!(create_properties.get("reason").is_some());

    let status_properties =
        &openapi["components"]["schemas"]["UpdateAdminNewsStatusRequest"]["properties"];
    assert_eq!(
        status_properties["status"]["pattern"].as_str(),
        Some("^(draft|published|archived)$")
    );
}

#[tokio::test]
async fn openapi_json_documents_public_news_contract() {
    let openapi = openapi_json().await;

    for (path, methods) in [
        ("/api/v1/news", ["get"].as_slice()),
        ("/api/v1/news/{id}", ["get"].as_slice()),
    ] {
        for method in methods {
            assert!(
                openapi["paths"][path].get(*method).is_some(),
                "missing {method} {path}"
            );
            assert!(
                !operation_has_bearer_security(&openapi, path, method),
                "public news must not require bearer security on {method} {path}"
            );
        }
    }

    let tags = openapi["tags"].as_array().unwrap();
    assert!(tags.iter().any(|tag| tag["name"] == "news"));

    for schema_name in ["PublicNewsItemResponse", "PublicNewsItemsResponse"] {
        let schema = &openapi["components"]["schemas"][schema_name];
        assert!(
            schema.get("properties").is_some(),
            "missing schema {schema_name}"
        );
        let schema_json = serde_json::to_string(schema).unwrap().to_lowercase();
        for forbidden in [
            "password",
            "token",
            "secret",
            "ciphertext",
            "created_by_admin_id",
            "updated_by_admin_id",
        ] {
            assert!(
                !schema_json.contains(forbidden),
                "schema {schema_name} leaks {forbidden}"
            );
        }
    }

    let news_properties = &openapi["components"]["schemas"]["PublicNewsItemResponse"]["properties"];
    for field in [
        "id",
        "title",
        "category",
        "status",
        "country_code",
        "default_locale",
        "content_json",
        "published_at",
        "created_at",
        "updated_at",
    ] {
        assert!(
            news_properties.get(field).is_some(),
            "missing PublicNewsItemResponse.{field}"
        );
    }
    assert!(schema_is_unix_millis(&news_properties["published_at"]));
    assert!(schema_is_unix_millis(&news_properties["created_at"]));
    assert!(schema_is_unix_millis(&news_properties["updated_at"]));
}

#[tokio::test]
async fn openapi_json_documents_user_2fa_security_policy_contract() {
    let openapi = openapi_json().await;

    for (path, methods, requires_bearer) in [
        ("/api/v1/auth/login/2fa", ["post"].as_slice(), false),
        (
            "/api/v1/auth/login/2fa/reset-code",
            ["post"].as_slice(),
            false,
        ),
        ("/api/v1/auth/login/2fa/reset", ["post"].as_slice(), false),
        ("/api/v1/user/2fa", ["get"].as_slice(), true),
        ("/api/v1/user/2fa/setup", ["post"].as_slice(), true),
        ("/api/v1/user/2fa/confirm", ["post"].as_slice(), true),
        ("/api/v1/user/2fa/login", ["patch"].as_slice(), true),
        ("/api/v1/user/2fa/reset-code", ["post"].as_slice(), true),
        ("/api/v1/user/2fa/reset", ["post"].as_slice(), true),
        ("/api/v1/wallet/withdrawals", ["post"].as_slice(), true),
        (
            "/admin/api/v1/security-policy",
            ["get", "patch"].as_slice(),
            true,
        ),
        (
            "/admin/api/v1/users/{id}/2fa/reset",
            ["post"].as_slice(),
            true,
        ),
    ] {
        for method in methods {
            assert!(
                openapi["paths"][path].get(*method).is_some(),
                "missing {method} {path}"
            );
            assert_eq!(
                operation_has_bearer_security(&openapi, path, method),
                requires_bearer,
                "unexpected bearer security on {method} {path}"
            );
        }
    }

    for schema_name in [
        "UserTwoFactorStatusResponse",
        "SetupTwoFactorResponse",
        "ConfirmTwoFactorRequest",
        "UpdateLoginTwoFactorRequest",
        "ResetTwoFactorRequest",
        "LoginTwoFactorRequest",
        "LoginTwoFactorResetCodeRequest",
        "LoginTwoFactorResetRequest",
        "LoginTwoFactorChallengeResponse",
        "LoginTwoFactorSetupChallengeResponse",
        "LoginConfigResponse",
        "UpdateUsernameRequest",
        "UpdateUsernameResponse",
        "LoginTwoFactorCodeResponse",
        "LoginTwoFactorResetResponse",
        "PaymentPolicy",
        "PaymentPolicies",
        "UserSecurityPolicy",
        "UpdateSecurityPolicyRequest",
        "ResetUserTwoFactorRequest",
        "AdminUserTwoFactorResetResponse",
        "CreateWithdrawalRequest",
        "WithdrawalRequestResponse",
    ] {
        let schema = &openapi["components"]["schemas"][schema_name];
        assert!(
            schema.get("properties").is_some(),
            "missing schema {schema_name}"
        );
    }

    let policy_properties = &openapi["components"]["schemas"]["PaymentPolicies"]["properties"];
    for field in ["withdraw", "spot_order", "convert", "earn_subscribe"] {
        assert!(
            policy_properties.get(field).is_some(),
            "missing PaymentPolicies.{field}"
        );
    }
    assert!(policy_properties.get("deposit").is_none());

    let status_schema_json =
        serde_json::to_string(&openapi["components"]["schemas"]["UserTwoFactorStatusResponse"])
            .unwrap()
            .to_lowercase();
    for forbidden in ["secret", "ciphertext", "totp_secret"] {
        assert!(
            !status_schema_json.contains(forbidden),
            "2FA status schema leaks {forbidden}"
        );
    }

    let user_policy_properties =
        &openapi["components"]["schemas"]["UserSecurityPolicy"]["properties"];
    assert!(
        user_policy_properties
            .get("username_login_enabled")
            .is_some()
    );
    let user_profile_properties =
        &openapi["components"]["schemas"]["UserProfileResponse"]["properties"];
    assert!(user_profile_properties.get("username").is_some());
}

#[tokio::test]
async fn openapi_json_alias_is_registered() {
    let openapi = request_json("/api/openapi.json").await;

    assert_eq!(openapi["openapi"].as_str(), Some("3.1.0"));
    assert!(openapi["paths"].get("/api/v1/user/profile").is_some());
}

#[tokio::test]
async fn swagger_ui_route_is_registered() {
    for uri in ["/docs", "/api/docs"] {
        let response = build_router(test_state())
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(
            matches!(
                response.status(),
                StatusCode::OK
                    | StatusCode::MOVED_PERMANENTLY
                    | StatusCode::SEE_OTHER
                    | StatusCode::TEMPORARY_REDIRECT
            ),
            "unexpected Swagger UI status for {uri}: {}",
            response.status()
        );
    }
}
