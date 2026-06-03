use crate::error::{AppError, AppResult};
use axum::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::{authentication::Credentials, client::Tls},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub text_body: String,
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
        let email = Message::builder()
            .from(from)
            .to(to)
            .subject(message.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(message.text_body)
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

#[cfg(test)]
mod tests {
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
}
