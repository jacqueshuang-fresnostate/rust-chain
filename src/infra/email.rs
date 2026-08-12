use crate::error::{AppError, AppResult};
use axum::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart, header::ContentType},
    transport::smtp::{authentication::Credentials, client::Tls},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpEmailConfig {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: Option<String>,
    pub verification_code_template_html: Option<String>,
    pub verification_code_templates: Vec<VerificationCodeTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationCodeTemplate {
    pub key: String,
    pub name: String,
    pub purpose: Option<String>,
    pub html: String,
    pub enabled: bool,
}

impl SmtpEmailConfig {
    /// 按业务 purpose 选择启用且非空的验证码模板；优先精确用途，其次无用途默认项，最后兼容历史单模板。
    /// 该选择只返回模板原文，不渲染验证码，也不触发 SMTP 外发。
    pub fn verification_code_template_html_for_purpose(&self, purpose: &str) -> Option<&str> {
        verification_code_template_html_for_purpose(
            &self.verification_code_templates,
            self.verification_code_template_html.as_deref(),
            purpose,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpSecurity {
    None,
    StartTls,
    Tls,
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    /// 通过给定配置发送一封邮件；实现必须在返回成功前完成 SMTP 交付请求，失败不得记录密码或正文中的验证码。
    async fn send(&self, config: SmtpEmailConfig, message: EmailMessage) -> AppResult<()>;
}

#[derive(Debug, Default)]
pub struct SmtpEmailSender;

#[async_trait]
impl EmailSender for SmtpEmailSender {
    /// 构建纯文本或 multipart 邮件并调用 SMTP；收发地址在建连前校验，传输错误统一映射为内部错误。
    /// 凭据只用于本次 transport 构造，不写入日志或持久化；调用方负责业务级限频、幂等与重试策略。
    async fn send(&self, config: SmtpEmailConfig, message: EmailMessage) -> AppResult<()> {
        let from = mailbox(
            config.from_email.as_str(),
            config.from_name.as_deref(),
            "smtp from_email is invalid",
        )?;
        let to = mailbox(message.to.as_str(), None, "email recipient is invalid")?;
        let builder = Message::builder()
            .from(from)
            .to(to)
            .subject(message.subject);
        let email = match message.html_body {
            Some(html_body) => builder.multipart(MultiPart::alternative_plain_html(
                message.text_body,
                html_body,
            )),
            None => builder
                .header(ContentType::TEXT_PLAIN)
                .body(message.text_body),
        }
        .map_err(|error| AppError::Internal(format!("smtp email build failed: {error}")))?;
        let mailer = smtp_transport(&config)?;
        mailer
            .send(email)
            .await
            .map_err(|error| AppError::Internal(format!("smtp email send failed: {error}")))?;
        Ok(())
    }
}

/// 将持久化的 SMTP 安全模式解析为封闭枚举，未知值必须拒绝，避免配置错误意外降级到明文连接。
pub fn parse_smtp_security(value: &str) -> AppResult<SmtpSecurity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(SmtpSecurity::None),
        "starttls" => Ok(SmtpSecurity::StartTls),
        "tls" => Ok(SmtpSecurity::Tls),
        _ => Err(AppError::Validation("smtp security is invalid".to_owned())),
    }
}

/// 将 SMTP 安全枚举映射回稳定存储码，保证管理端读写配置时不改变既有字符串契约。
pub fn smtp_security_code(security: SmtpSecurity) -> &'static str {
    match security {
        SmtpSecurity::None => "none",
        SmtpSecurity::StartTls => "starttls",
        SmtpSecurity::Tls => "tls",
    }
}

fn smtp_transport(config: &SmtpEmailConfig) -> AppResult<AsyncSmtpTransport<Tokio1Executor>> {
    let mut builder = match config.security {
        SmtpSecurity::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
        SmtpSecurity::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host).map_err(|error| {
                AppError::Internal(format!("smtp transport build failed: {error}"))
            })?
        }
        SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|error| AppError::Internal(format!("smtp transport build failed: {error}")))?,
    }
    .port(config.port);

    if config.security == SmtpSecurity::None {
        builder = builder.tls(Tls::None);
    }
    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }
    Ok(builder.build())
}

fn mailbox(email: &str, name: Option<&str>, error: &'static str) -> AppResult<Mailbox> {
    let address = email
        .trim()
        .parse()
        .map_err(|_| AppError::Validation(error.to_owned()))?;
    let name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    Ok(Mailbox::new(name, address))
}

/// 生成验证码邮件载荷；纯文本始终存在，HTML 仅在模板非空时渲染并对动态字段做转义。
/// 本函数不发送邮件、不持久化验证码，调用方仍负责用途、有效期、频率与敏感信息日志边界。
pub fn verification_code_email_message(
    to: String,
    subject: &str,
    code: &str,
    expires_minutes: u32,
    template_html: Option<&str>,
) -> EmailMessage {
    EmailMessage {
        to,
        subject: subject.to_owned(),
        text_body: format!("您的{subject}是 {code}，{expires_minutes} 分钟内有效。"),
        html_body: template_html.and_then(|template| {
            let template = template.trim();
            (!template.is_empty()).then(|| {
                render_verification_code_html_template(template, subject, code, expires_minutes)
            })
        }),
    }
}

/// 从模板集合中按“精确用途→默认模板→历史模板”选择 HTML，禁用或空模板不得参与回退。
/// 返回值借用原配置且未经渲染，调用方必须在插值时保持 HTML 转义。
pub fn verification_code_template_html_for_purpose<'a>(
    templates: &'a [VerificationCodeTemplate],
    legacy_template_html: Option<&'a str>,
    purpose: &str,
) -> Option<&'a str> {
    let purpose = purpose.trim();
    templates
        .iter()
        .find(|template| {
            template.enabled
                && template
                    .purpose
                    .as_deref()
                    .is_some_and(|template_purpose| template_purpose == purpose)
                && !template.html.trim().is_empty()
        })
        .or_else(|| {
            templates.iter().find(|template| {
                template.enabled && template.purpose.is_none() && !template.html.trim().is_empty()
            })
        })
        .map(|template| template.html.as_str())
        .or(legacy_template_html)
}

fn render_verification_code_html_template(
    template: &str,
    subject: &str,
    code: &str,
    expires_minutes: u32,
) -> String {
    template
        .replace("{{subject}}", &escape_html(subject))
        .replace("{{code}}", &escape_html(code))
        .replace("{{expires_minutes}}", &expires_minutes.to_string())
}

fn escape_html(value: &str) -> String {
    value.chars().fold(String::new(), |mut escaped, character| {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
        escaped
    })
}

#[cfg(test)]
#[path = "../../tests/unit_src/src_infra_email_tests.rs"]
mod tests;
