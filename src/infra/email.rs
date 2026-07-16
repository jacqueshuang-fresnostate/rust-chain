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
    async fn send(&self, config: SmtpEmailConfig, message: EmailMessage) -> AppResult<()>;
}

#[derive(Debug, Default)]
pub struct SmtpEmailSender;

#[async_trait]
impl EmailSender for SmtpEmailSender {
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

pub fn parse_smtp_security(value: &str) -> AppResult<SmtpSecurity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(SmtpSecurity::None),
        "starttls" => Ok(SmtpSecurity::StartTls),
        "tls" => Ok(SmtpSecurity::Tls),
        _ => Err(AppError::Validation("smtp security is invalid".to_owned())),
    }
}

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
