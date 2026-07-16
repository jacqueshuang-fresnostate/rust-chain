use super::*;
use crate::infra::email::VerificationCodeTemplate;
use crate::modules::admin::{
    repository::{
        AdminSmtpConfigRecord as SmtpConfigRow,
        AdminSmtpDeliverySettingsRecord as SmtpDeliverySettingsRow,
    },
    service::{
        DEFAULT_SMTP_CONFIG_PRIORITY, SMTP_DELIVERY_STRATEGY_PRIORITY,
        SMTP_DELIVERY_STRATEGY_ROUND_ROBIN, select_smtp_delivery_config as select_delivery_row,
        validate_smtp_save_request as validate_save_request,
    },
};

#[test]
fn validates_smtp_save_request() {
    let request = SaveSmtpConfigRequest {
        name: Some(" 主发信配置 ".to_owned()),
        host: " smtp.example.test ".to_owned(),
        port: 587,
        security: "STARTTLS".to_owned(),
        username: None,
        password: None,
        from_email: " noreply@example.test ".to_owned(),
        from_name: Some(" Exchange ".to_owned()),
        verification_code_template_html: Some(" <p>{{subject}}：{{code}}</p> ".to_owned()),
        verification_code_templates: Some(vec![VerificationCodeTemplate {
            key: " bind ".to_owned(),
            name: " 绑定邮箱 ".to_owned(),
            purpose: Some(" bind ".to_owned()),
            html: " <p>{{code}}</p> ".to_owned(),
            enabled: true,
        }]),
        enabled: true,
        priority: Some(20),
        reason: Some("configure smtp".to_owned()),
    };

    let config = validate_save_request(&request, None, Some(DEFAULT_SMTP_CONFIG_PRIORITY)).unwrap();
    assert_eq!(config.name, "主发信配置");
    assert_eq!(config.host, "smtp.example.test");
    assert_eq!(config.security, "starttls");
    assert_eq!(config.from_email, "noreply@example.test");
    assert_eq!(config.from_name.as_deref(), Some("Exchange"));
    assert_eq!(
        config.verification_code_template_html.as_deref(),
        Some("<p>{{subject}}：{{code}}</p>")
    );
    assert_eq!(config.verification_code_templates[0].key, "bind");
    assert_eq!(
        config.verification_code_templates[0].purpose.as_deref(),
        Some("bind")
    );
    assert_eq!(
        config.verification_code_templates[0].html,
        "<p>{{code}}</p>"
    );
    assert_eq!(config.priority, 20);
}

#[test]
fn rejects_invalid_smtp_values() {
    let mut request = SaveSmtpConfigRequest {
        name: Some("主发信配置".to_owned()),
        host: "smtp.example.test".to_owned(),
        port: 0,
        security: "ssl".to_owned(),
        username: None,
        password: None,
        from_email: "noreply.example.test".to_owned(),
        from_name: None,
        verification_code_template_html: None,
        verification_code_templates: None,
        enabled: true,
        priority: Some(100),
        reason: Some("configure smtp".to_owned()),
    };

    assert!(validate_save_request(&request, None, Some(DEFAULT_SMTP_CONFIG_PRIORITY)).is_err());
    request.port = 587;
    assert!(validate_save_request(&request, None, Some(DEFAULT_SMTP_CONFIG_PRIORITY)).is_err());
    request.security = "tls".to_owned();
    assert!(validate_save_request(&request, None, Some(DEFAULT_SMTP_CONFIG_PRIORITY)).is_err());
}

#[test]
fn selects_delivery_row_by_strategy() {
    let rows = vec![smtp_row(10, 20), smtp_row(11, 30), smtp_row(12, 40)];
    let priority_settings = SmtpDeliverySettingsRow {
        strategy: SMTP_DELIVERY_STRATEGY_PRIORITY.to_owned(),
        round_robin_cursor: Some(12),
    };
    assert_eq!(
        select_delivery_row(&priority_settings, &rows).unwrap().id,
        10
    );

    let round_robin_settings = SmtpDeliverySettingsRow {
        strategy: SMTP_DELIVERY_STRATEGY_ROUND_ROBIN.to_owned(),
        round_robin_cursor: Some(10),
    };
    assert_eq!(
        select_delivery_row(&round_robin_settings, &rows)
            .unwrap()
            .id,
        11
    );

    let wrapped_settings = SmtpDeliverySettingsRow {
        strategy: SMTP_DELIVERY_STRATEGY_ROUND_ROBIN.to_owned(),
        round_robin_cursor: Some(12),
    };
    assert_eq!(
        select_delivery_row(&wrapped_settings, &rows).unwrap().id,
        10
    );
}

fn smtp_row(id: u64, priority: u32) -> SmtpConfigRow {
    SmtpConfigRow {
        id,
        name: format!("smtp-{id}"),
        host: "smtp.example.test".to_owned(),
        port: 587,
        security: "starttls".to_owned(),
        username_ciphertext: None,
        password_ciphertext: None,
        username_mask: None,
        from_email: "noreply@example.test".to_owned(),
        from_name: None,
        verification_code_template_html: None,
        verification_code_templates: Vec::new(),
        enabled: true,
        priority,
    }
}
