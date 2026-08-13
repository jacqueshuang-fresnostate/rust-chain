//! 邮件发送基础设施：向各业务上下文提供统一的 SMTP 外发能力和验证码邮件的模板渲染。
//! 发送不使用长连接，每封邮件都按调用方传入的配置快照临时建立传输，因此后台改完 SMTP 配置可立即生效。
//! 配置来自数据库中的后台设置而非环境变量，凭据只在本次传输构造期间存在，不缓存也不写日志。
//! 验证码模板按业务用途选择，渲染时对主题与验证码做 HTML 转义，避免后台录入的模板变成注入点。
//! 本模块只关心「这一封怎么发出去」，验证码的生成、有效期、发送频率与幂等控制全部由调用方负责。

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
    /// 使用调用方快照配置构建并提交一封 SMTP 邮件；返回成功表示服务器接受本次发送请求，不代表收件箱最终投递。
    /// 实现不得持久化或记录密码、正文和验证码；地址、连接、认证或发送错误必须返回，由上层决定业务幂等与重试。
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

/// 把持久化的 SMTP 安全模式文本解析成封闭枚举，比较前先去空白并转小写，因此大小写写法都能接受。
/// 未知取值必须拒绝而不是回落到任一默认模式，避免配置录错时悄悄降级成明文连接把账号口令暴露在链路上。
pub fn parse_smtp_security(value: &str) -> AppResult<SmtpSecurity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(SmtpSecurity::None),
        "starttls" => Ok(SmtpSecurity::StartTls),
        "tls" => Ok(SmtpSecurity::Tls),
        _ => Err(AppError::Validation("smtp security is invalid".to_owned())),
    }
}

/// 把 SMTP 安全模式枚举映射回入库用的稳定字符串码，与解析函数构成一对可往返的转换。
/// 这些字面量属于持久化契约，管理端读写配置时依赖它们保持不变，改名会让历史配置无法再被解析。
pub fn smtp_security_code(security: SmtpSecurity) -> &'static str {
    match security {
        SmtpSecurity::None => "none",
        SmtpSecurity::StartTls => "starttls",
        SmtpSecurity::Tls => "tls",
    }
}

/// 依据安全模式构造异步 SMTP 传输：明文走不校验证书的裸连接，另外两种分别按 STARTTLS 升级和直接 TLS 建连。
/// 端口始终采用配置值而不用协议默认端口；明文模式还会显式关闭 TLS，防止底层实现自行尝试加密协商。
/// 只有用户名和口令同时存在时才附加认证信息，缺任一项就按匿名投递处理，适配内网免认证的中继服务器。
/// 传输对象在这里只是构建出来，尚未发起任何网络连接，域名解析与握手错误要到实际发送时才会暴露。
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

/// 把地址文本与可选显示名组装成邮箱对象，地址先去首尾空白再解析，解析失败返回调用方指定的校验错误。
/// 之所以由调用方传入错误文案，是为了让发件人和收件人两种非法情况能给出可区分的提示。
/// 显示名同样去空白，裁剪后为空则视作未提供，避免生成带空名字的邮件头。
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

/// 用简单的字面量替换渲染验证码 HTML 模板，只认主题、验证码和有效分钟数三个占位符，不支持条件或循环。
/// 主题与验证码在插入前会做 HTML 转义，分钟数按整数格式化，因此动态内容不会破坏模板本身的标签结构。
/// 模板中未出现的占位符会被忽略，多余的占位符也会原样保留，渲染不会因为模板不匹配而失败。
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

/// 逐字符转义 HTML 中具有语法含义的五个符号，覆盖尖括号、和号以及双引号与单引号两种属性定界符。
/// 单引号也一并转义，是为了让渲染结果放进用单引号包裹的属性值里同样安全，而不只适用于标签正文。
/// 其余字符原样保留，因此中文与空白不受影响；本函数不做 URL 或 JavaScript 上下文的转义。
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
