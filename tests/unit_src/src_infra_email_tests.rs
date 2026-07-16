use super::*;

#[test]
fn parses_smtp_security_codes() {
    assert_eq!(parse_smtp_security("none").unwrap(), SmtpSecurity::None);
    assert_eq!(
        parse_smtp_security("STARTTLS").unwrap(),
        SmtpSecurity::StartTls
    );
    assert_eq!(parse_smtp_security("tls").unwrap(), SmtpSecurity::Tls);
    assert!(parse_smtp_security("ssl").is_err());
}

#[test]
fn maps_smtp_security_to_storage_code() {
    assert_eq!(smtp_security_code(SmtpSecurity::None), "none");
    assert_eq!(smtp_security_code(SmtpSecurity::StartTls), "starttls");
    assert_eq!(smtp_security_code(SmtpSecurity::Tls), "tls");
}

#[test]
fn renders_verification_code_html_template_with_escaped_variables() {
    let message = verification_code_email_message(
        "user@example.test".to_owned(),
        "绑定邮箱<验证码>",
        "123456",
        10,
        Some("<p>{{subject}}</p><strong>{{code}}</strong><em>{{expires_minutes}}</em>"),
    );

    assert_eq!(message.subject, "绑定邮箱<验证码>");
    assert_eq!(
        message.text_body,
        "您的绑定邮箱<验证码>是 123456，10 分钟内有效。"
    );
    assert_eq!(
        message.html_body.as_deref(),
        Some("<p>绑定邮箱&lt;验证码&gt;</p><strong>123456</strong><em>10</em>")
    );
}

#[test]
fn selects_purpose_specific_template_before_default_and_legacy_template() {
    let templates = vec![
        VerificationCodeTemplate {
            key: "default".to_owned(),
            name: "通用模板".to_owned(),
            purpose: None,
            html: "<p>default {{code}}</p>".to_owned(),
            enabled: true,
        },
        VerificationCodeTemplate {
            key: "fund".to_owned(),
            name: "资金密码".to_owned(),
            purpose: Some("fund_password_reset".to_owned()),
            html: "<p>fund {{code}}</p>".to_owned(),
            enabled: true,
        },
    ];

    assert_eq!(
        verification_code_template_html_for_purpose(
            &templates,
            Some("<p>legacy</p>"),
            "fund_password_reset"
        ),
        Some("<p>fund {{code}}</p>")
    );
    assert_eq!(
        verification_code_template_html_for_purpose(&templates, Some("<p>legacy</p>"), "bind"),
        Some("<p>default {{code}}</p>")
    );
    assert_eq!(
        verification_code_template_html_for_purpose(&[], Some("<p>legacy</p>"), "bind"),
        Some("<p>legacy</p>")
    );
}
