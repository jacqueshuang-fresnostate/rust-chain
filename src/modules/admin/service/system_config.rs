use super::*;

pub(crate) const DEFAULT_SMTP_CONFIG_NAME: &str = "default";

pub(crate) const DEFAULT_SMTP_CONFIG_PRIORITY: u32 = 100;

pub(crate) const SMTP_DELIVERY_SETTINGS_ID: u8 = 1;

pub(crate) const SMTP_DELIVERY_STRATEGY_PRIORITY: &str = "priority";

pub(crate) const SMTP_DELIVERY_STRATEGY_ROUND_ROBIN: &str = "round_robin";

pub(crate) const DEFAULT_UPLOAD_FILE_FIELD: &str = "file";

const DEFAULT_UPLOAD_MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

const MAX_UPLOAD_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;

pub(crate) const MAX_UPLOAD_BODY_SIZE_BYTES: usize =
    (MAX_UPLOAD_FILE_SIZE_BYTES as usize) + 1024 * 1024;

pub(crate) const UPLOAD_IMAGE_MIME_TYPES: &[&str] =
    &["image/png", "image/jpeg", "image/webp", "image/gif"];

type HmacSha256 = Hmac<sha2::Sha256>;

type HmacSha1 = Hmac<sha1::Sha1>;

#[derive(Debug)]
pub(crate) struct SmtpValidatedConfig {
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) security: String,
    pub(crate) from_email: String,
    pub(crate) from_name: Option<String>,
    pub(crate) verification_code_template_html: Option<String>,
    pub(crate) verification_code_templates: Vec<VerificationCodeTemplate>,
    pub(crate) enabled: bool,
    pub(crate) priority: u32,
}

/// 校验 SMTP 名称、主机、端口、安全模式、发件人、优先级及验证码模板组合约束。
/// 更新时可用旧名称和优先级作为回退；该函数不测试网络或凭据，实际连通性由测试发送用例确认。
pub(crate) fn validate_smtp_save_request(
    request: &SaveSmtpConfigRequest,
    fallback_name: Option<&str>,
    fallback_priority: Option<u32>,
) -> AppResult<SmtpValidatedConfig> {
    let name = validate_smtp_config_name(request.name.clone(), fallback_name)?;
    let host = optional_string(Some(request.host.clone()))
        .ok_or_else(|| AppError::Validation("smtp host is required".to_owned()))?;
    if host.len() > 255 {
        return Err(AppError::Validation("smtp host is too long".to_owned()));
    }
    if request.port == 0 {
        return Err(AppError::Validation("smtp port is invalid".to_owned()));
    }
    let security = smtp_security_code(parse_smtp_security(&request.security)?).to_owned();
    let from_email = validate_smtp_email(&request.from_email, "from_email")?;
    let from_name = optional_string(request.from_name.clone());
    if let Some(from_name) = &from_name
        && from_name.len() > 128
    {
        return Err(AppError::Validation("from_name is too long".to_owned()));
    }
    let verification_code_template_html =
        optional_string(request.verification_code_template_html.clone());
    if let Some(template_html) = &verification_code_template_html
        && template_html.len() > 20_000
    {
        return Err(AppError::Validation(
            "verification_code_template_html is too long".to_owned(),
        ));
    }
    let verification_code_templates =
        validate_smtp_verification_code_templates(request.verification_code_templates.clone())?;
    let priority = request.priority.or(fallback_priority).unwrap_or(100);
    if priority > 9999 {
        return Err(AppError::Validation(
            "smtp priority cannot exceed 9999".to_owned(),
        ));
    }

    Ok(SmtpValidatedConfig {
        name,
        host,
        port: request.port,
        security,
        from_email,
        from_name,
        verification_code_template_html,
        verification_code_templates,
        enabled: request.enabled,
        priority,
    })
}

/// 规范化 SMTP 发信策略，仅接受优先级或轮询模式并返回稳定代码。
/// 该值只决定配置选择算法，不启动发信任务，也不变更现有轮询游标。
pub(crate) fn validate_smtp_delivery_strategy(value: &str) -> AppResult<String> {
    match value.trim() {
        "priority" => Ok("priority".to_owned()),
        "round_robin" => Ok("round_robin".to_owned()),
        _ => Err(AppError::Validation(
            "smtp delivery strategy is invalid".to_owned(),
        )),
    }
}

/// 对 SMTP 发件人或测试收件地址做轻量邮箱格式校验，要求单个 `@` 且本地部分与域名均非空。
/// 该检查不验证 DNS/MX 或邮箱可投递性；发送失败由 SMTP 适配器返回。
pub(crate) fn validate_smtp_email(value: &str, field: &str) -> AppResult<String> {
    let email = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation(format!("smtp {field} is required")))?;
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if email.len() > 255
        || local.is_empty()
        || domain.is_empty()
        || parts.next().is_some()
        || email.chars().any(char::is_whitespace)
    {
        return Err(AppError::Validation(format!("smtp {field} is invalid")));
    }
    Ok(email)
}

/// 提取并规范化SMTP 操作审计原因，拒绝空白值及超过审计字段上限的内容。
/// 返回去空后的必填原因，供 SMTP 配置/策略或测试邮件审计使用；缺失或超长返回校验错误，函数不落审计表。
pub(crate) fn required_smtp_audit_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

/// 判断 SMTP 保存请求是否明确携带新用户名或密码，用于决定加密替换还是保留已有密文。
/// 仅当 username 或 password 去空后非空才返回 true；不比较主机等目标字段，也不解密已有凭据。
pub(crate) fn smtp_request_has_new_secret(request: &SaveSmtpConfigRequest) -> bool {
    request.username.as_deref().and_then(optional_str).is_some()
        || request.password.as_deref().and_then(optional_str).is_some()
}

/// 构造默认 SMTP 发信策略记录，在数据库尚无配置时提供确定的优先级选择和空轮询游标。
/// 构造过程无 I/O；返回值仅供应用层初始化或响应，不会自行持久化。
pub(crate) fn default_smtp_delivery_settings_record() -> AdminSmtpDeliverySettingsRecord {
    AdminSmtpDeliverySettingsRecord {
        strategy: SMTP_DELIVERY_STRATEGY_PRIORITY.to_owned(),
        round_robin_cursor: None,
    }
}

/// 将SMTP 发信策略仓储记录映射为后台响应，统一时间、掩码和可选字段的对外表示。
/// 当前响应只暴露 strategy，明确丢弃内部 round_robin_cursor；转换不访问数据库或推进轮询。
pub(crate) fn smtp_delivery_settings_response(
    record: AdminSmtpDeliverySettingsRecord,
) -> SmtpDeliverySettingsResponse {
    SmtpDeliverySettingsResponse {
        strategy: record.strategy,
    }
}

/// 将 SMTP 发信策略和当前轮询游标映射为审计 JSON，供策略保存前后值比对。
/// 本函数不推进游标或写审计；调用方负责在发信策略事务中持久化结果。
pub(crate) fn smtp_delivery_settings_audit_json(record: &AdminSmtpDeliverySettingsRecord) -> Value {
    json!({
        "strategy": record.strategy,
        "round_robin_cursor": record.round_robin_cursor,
    })
}

/// 将SMTP 配置仓储记录映射为后台响应，统一时间、掩码和可选字段的对外表示。
/// 输出包含用户名掩码、密码是否设置和验证码模板，不暴露 password_ciphertext；转换仅消费传入记录且无发信副作用。
pub(crate) fn smtp_config_response(record: AdminSmtpConfigRecord) -> SmtpConfigResponse {
    let verification_code_templates = smtp_templates_from_record(&record);
    SmtpConfigResponse {
        id: record.id,
        name: record.name,
        host: record.host,
        port: record.port,
        security: record.security,
        username_mask: record.username_mask,
        password_set: record.password_ciphertext.is_some(),
        from_email: record.from_email,
        from_name: record.from_name,
        verification_code_template_html: record.verification_code_template_html.clone(),
        verification_code_templates,
        enabled: record.enabled,
        priority: record.priority,
    }
}

/// 将 SMTP 连接、发件人、模板、优先级和启用状态映射为配置审计快照。
/// 密钥仅以用户名掩码和密码是否存在表示；应用层在配置写事务中保存前后值。
pub(crate) fn smtp_config_audit_json(record: &AdminSmtpConfigRecord) -> Value {
    json!({
        "id": record.id,
        "name": record.name,
        "host": record.host,
        "port": record.port,
        "security": record.security,
        "username_mask": record.username_mask,
        "password_set": record.password_ciphertext.is_some(),
        "from_email": record.from_email,
        "from_name": record.from_name,
        "verification_code_template_html": record.verification_code_template_html,
        "verification_code_templates": smtp_templates_from_record(record),
        "enabled": record.enabled,
        "priority": record.priority,
    })
}

/// 从 SMTP 记录合并新版多语言模板与旧版单模板字段，生成邮件发送端可直接使用的模板集合。
/// 转换不读取密钥、不发送邮件；无有效模板时保留默认回退语义，不产生数据库副作用。
pub(crate) fn smtp_templates_from_record(
    record: &AdminSmtpConfigRecord,
) -> Vec<VerificationCodeTemplate> {
    if !record.verification_code_templates.is_empty() {
        return record.verification_code_templates.clone();
    }

    record
        .verification_code_template_html
        .as_deref()
        .and_then(optional_str)
        .map(|html| VerificationCodeTemplate {
            key: "default".to_owned(),
            name: "通用验证码模板".to_owned(),
            purpose: None,
            html: html.to_owned(),
            enabled: true,
        })
        .into_iter()
        .collect()
}

/// 从已按优先级排序的 SMTP 配置快照中选择本次发送项；轮询策略从当前游标的后一项继续。
/// 该纯内存选择不发送邮件或写数据库；空快照返回 `None`，非轮询策略（含未知值）回退到首项，游标推进由基础设施层负责。
pub(crate) fn select_smtp_delivery_config(
    settings: &AdminSmtpDeliverySettingsRecord,
    records: &[AdminSmtpConfigRecord],
) -> Option<AdminSmtpConfigRecord> {
    if records.is_empty() {
        return None;
    }
    if settings.strategy != SMTP_DELIVERY_STRATEGY_ROUND_ROBIN {
        return records.first().cloned();
    }

    let next_index = settings
        .round_robin_cursor
        .and_then(|cursor| records.iter().position(|record| record.id == cursor))
        .map(|index| (index + 1) % records.len())
        .unwrap_or(0);
    records.get(next_index).cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UploadProvider {
    ImageBed,
    Oss,
    S3,
    Local,
}

impl UploadProvider {
    /// 解析上传提供商代码及兼容别名，仅接受图床、OSS、S3 或本地存储。
    /// 解析为纯内存操作；未知值返回校验或内部数据错误，不访问数据库也不产生副作用。
    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "image_bed" | "imagebed" => Ok(Self::ImageBed),
            "oss" => Ok(Self::Oss),
            "s3" => Ok(Self::S3),
            "local" => Ok(Self::Local),
            _ => Err(AppError::Validation(
                "upload provider is invalid".to_owned(),
            )),
        }
    }

    /// 返回上传提供商用于持久化和协议分派的稳定代码。
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ImageBed => "image_bed",
            Self::Oss => "oss",
            Self::S3 => "s3",
            Self::Local => "local",
        }
    }

    /// 判断该上传提供商是否需要 Bearer 凭据。
    pub(crate) const fn uses_bearer(self) -> bool {
        matches!(self, Self::ImageBed)
    }

    /// 判断该上传提供商是否需要访问密钥与密钥对。
    pub(crate) const fn uses_access_secret(self) -> bool {
        matches!(self, Self::Oss | Self::S3)
    }
}

#[derive(Debug)]
pub(crate) struct ValidatedUploadConfig {
    pub(crate) provider: UploadProvider,
    pub(crate) endpoint: Option<String>,
    pub(crate) file_field: Option<String>,
    pub(crate) bucket: Option<String>,
    pub(crate) region: Option<String>,
    pub(crate) public_base_url: Option<String>,
    pub(crate) local_root: Option<String>,
    pub(crate) key_prefix: Option<String>,
    pub(crate) max_file_size_bytes: u64,
    pub(crate) allowed_mime_types: Vec<String>,
    pub(crate) enabled: bool,
}

/// 解析上传 provider，并校验 endpoint、bucket、凭据、文件上限和 MIME 白名单的组合要求。
/// 仅生成规范化配置，不连接对象存储；密钥加密与旧密文保留由应用事务负责。
pub(crate) fn validate_upload_config(
    request: &SaveUploadConfigRequest,
) -> AppResult<ValidatedUploadConfig> {
    let provider = UploadProvider::parse(&request.provider)?;
    let endpoint = optional_string(request.endpoint.clone());
    let public_base_url = optional_string(request.public_base_url.clone());
    let local_root = optional_string(request.local_root.clone());
    let bucket = optional_string(request.bucket.clone());
    let region = optional_string(request.region.clone());
    validate_upload_optional_len(endpoint.as_deref(), "endpoint", 512)?;
    validate_upload_optional_len(public_base_url.as_deref(), "public_base_url", 512)?;
    validate_upload_optional_len(local_root.as_deref(), "local_root", 512)?;
    let key_prefix = normalize_upload_key_prefix(request.key_prefix.clone())?;
    let file_field = Some(validate_upload_len(
        optional_string(request.file_field.clone())
            .unwrap_or_else(|| DEFAULT_UPLOAD_FILE_FIELD.to_owned()),
        "file_field",
        64,
    )?);
    let max_file_size_bytes = request
        .max_file_size_bytes
        .unwrap_or(DEFAULT_UPLOAD_MAX_FILE_SIZE_BYTES);
    if max_file_size_bytes == 0 || max_file_size_bytes > MAX_UPLOAD_FILE_SIZE_BYTES {
        return Err(AppError::Validation(
            "max_file_size_bytes is invalid".to_owned(),
        ));
    }
    let allowed_mime_types = normalize_upload_mime_types(request.allowed_mime_types.clone())?;

    match provider {
        UploadProvider::ImageBed => {
            validate_upload_credential_url(endpoint.as_deref(), "image bed endpoint")?;
        }
        UploadProvider::Local => {
            require_upload_value(local_root.as_deref(), "local_root")?;
            validate_upload_url(public_base_url.as_deref(), "public_base_url")?;
        }
        UploadProvider::S3 => {
            validate_upload_bucket_name(bucket.as_deref())?;
            validate_upload_region(region.as_deref())?;
            if let Some(endpoint) = &endpoint {
                validate_upload_credential_url(Some(endpoint), "s3 endpoint")?;
            }
            if let Some(public_base_url) = &public_base_url {
                validate_upload_url(Some(public_base_url), "public_base_url")?;
            }
        }
        UploadProvider::Oss => {
            validate_upload_credential_url(endpoint.as_deref(), "oss endpoint")?;
            validate_upload_bucket_name(bucket.as_deref())?;
            if let Some(public_base_url) = &public_base_url {
                validate_upload_url(Some(public_base_url), "public_base_url")?;
            }
        }
    }

    Ok(ValidatedUploadConfig {
        provider,
        endpoint,
        file_field,
        bucket,
        region,
        public_base_url,
        local_root,
        key_prefix,
        max_file_size_bytes,
        allowed_mime_types,
        enabled: request.enabled,
    })
}

/// 在发送对象存储请求前校验文件非空、大小不超过配置上限，且 MIME 命中允许列表。
/// 该函数不检查文件内容与声明 MIME 是否一致，也不写临时文件或远端对象。
pub(crate) fn validate_upload_file(
    max_file_size_bytes: u64,
    allowed_mime_types: &[String],
    input: &UploadFileInput,
) -> AppResult<()> {
    if input.bytes.is_empty() {
        return Err(AppError::Validation("upload file is required".to_owned()));
    }
    validate_upload_image_bytes(&input.bytes, &input.mime_type)?;
    let size = input.bytes.len() as u64;
    if size > max_file_size_bytes {
        return Err(AppError::Validation("upload file is too large".to_owned()));
    }
    if !allowed_mime_types
        .iter()
        .any(|mime| mime == &input.mime_type)
    {
        return Err(AppError::Validation(
            "upload file mime type is not allowed".to_owned(),
        ));
    }
    Ok(())
}

/// 提取并规范化上传操作审计原因，拒绝空白值及超过审计字段上限的内容。
/// 返回去空后的上传配置必填审计原因；缺失或超过后台审计上限返回校验错误，成功值仍须由应用事务写入审计表。
pub(crate) fn required_upload_audit_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

/// 比较上传密钥目标位置相关输入，判断本次更新能否沿用现有加密凭据或必须提供新密钥。
/// 仅比较 endpoint、bucket 和 region 是否完全相同；provider 等其他字段不参与，函数不解密旧密钥或判断新密钥是否为空。
pub(crate) fn upload_config_secret_destination_unchanged(
    record: &AdminUploadConfigRecord,
    config: &ValidatedUploadConfig,
) -> bool {
    record.endpoint == config.endpoint
        && record.bucket == config.bucket
        && record.region == config.region
}

/// 将上传提供商、目标位置、公开地址、对象规则、大小/MIME 限制和启用状态映射为审计快照。
/// Bearer、访问密钥和 Secret 只记录掩码或是否已设置；结果不会暴露密文。
pub(crate) fn upload_config_audit_json(record: &AdminUploadConfigRecord) -> Value {
    json!({
        "id": record.id,
        "name": record.name,
        "provider": record.provider,
        "endpoint": record.endpoint,
        "file_field": record.file_field,
        "bearer_token_mask": record.bearer_token_mask,
        "bearer_token_set": record.bearer_token_ciphertext.is_some(),
        "access_key_mask": record.access_key_mask,
        "access_key_set": record.access_key_ciphertext.is_some(),
        "secret_key_set": record.secret_key_ciphertext.is_some(),
        "bucket": record.bucket,
        "region": record.region,
        "public_base_url": record.public_base_url,
        "local_root": record.local_root,
        "key_prefix": record.key_prefix,
        "max_file_size_bytes": record.max_file_size_bytes,
        "allowed_mime_types": record.allowed_mime_types,
        "enabled": record.enabled,
    })
}

/// 按日期、UUID、受支持 MIME 扩展名及可选前缀生成不可预测的上传对象键。
/// 每次调用都会产生新键且不具幂等性；函数不创建文件或远端对象，前缀应先通过配置校验。
pub(crate) fn generated_upload_object_key(prefix: Option<&str>, mime_type: &str) -> String {
    let date = Utc::now().format("%Y/%m/%d");
    let suffix = upload_extension_for_mime(mime_type);
    let key = format!("{date}/{}.{}", Uuid::now_v7().simple(), suffix);
    match prefix.and_then(optional_str) {
        Some(prefix) => format!("{}/{}", prefix.trim_matches('/'), key),
        None => key,
    }
}

/// 从原始文件名提取安全 basename、修正扩展名并限制长度，避免目录穿越和异常响应头。
/// 转换不访问文件系统；空值使用 MIME 对应默认名，不合法字符按既有清洗规则处理。
pub(crate) fn safe_upload_filename(original: Option<&str>, mime_type: &str) -> String {
    let extension = upload_extension_for_mime(mime_type);
    let Some(original) = original.and_then(optional_str) else {
        return format!("upload.{extension}");
    };
    let normalized = original.replace('\\', "/");
    let candidate = normalized.split('/').next_back().unwrap_or("upload");
    let name = safe_upload_key_segment(candidate);
    let name = if name.is_empty() {
        format!("upload.{extension}")
    } else {
        name
    };
    truncate_upload_filename(name, extension, 255)
}

/// 过滤上传路径片段，仅保留 ASCII 字母数字及点、横线、下划线，避免注入目录分隔符。
/// 该纯函数不访问存储；调用方仍需检查过滤后是否为空并决定是否接受。
pub(crate) fn safe_upload_key_segment(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .collect()
}

/// 校验上传提供商返回的公开、分享或删除地址，按必填语义规范空白值并限制协议与主机。
/// 该检查不发起网络请求；缺少必填地址或地址不安全时返回校验错误，避免污染对象记录。
pub(crate) fn safe_upload_response_url(
    value: Option<&str>,
    field: &str,
    required: bool,
) -> AppResult<Option<String>> {
    let Some(value) = value.and_then(optional_str) else {
        return if required {
            Err(AppError::Validation(format!("{field} is missing")))
        } else {
            Ok(None)
        };
    };
    validate_upload_safe_url(value, field, false).map(Some)
}

/// 规范拼接公开基础地址和对象键，确保边界只保留一个斜杠且不改写对象键内容。
/// 函数不验证网络可达性也不执行上传；基础地址和对象键须已分别通过前置校验。
pub(crate) fn join_upload_public_url(base: &str, object_key: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        object_key.trim_start_matches('/')
    )
}

/// 将上传端点与路径组件规范拼接并解析为 URL，避免重复斜杠破坏签名请求路径。
/// 仅接受可解析端点；解析失败返回校验错误，不发起网络请求或存储写入。
pub(crate) fn join_upload_endpoint_path(endpoint: &str, parts: &[&str]) -> AppResult<String> {
    let base = endpoint.trim_end_matches('/');
    let path = parts
        .iter()
        .map(|part| part.trim_matches('/'))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("{base}/{path}");
    url::Url::parse(&url)
        .map_err(|_| AppError::Validation("upload endpoint is invalid".to_owned()))?;
    Ok(url)
}

/// 从已解析上传 URL 生成包含可选端口的 Host 头值，供对象存储签名与请求保持一致。
/// URL 缺少主机时返回校验错误；该函数不解析 DNS、不建立网络连接。
pub(crate) fn upload_url_host(url: &url::Url) -> AppResult<String> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("upload endpoint host is invalid".to_owned()))?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

/// 计算上传请求体的 SHA-256 十六进制摘要，供对象存储内容完整性和签名串使用。
/// 摘要计算是确定性纯函数，不保存原文或密钥，也不执行网络和数据库操作。
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

/// 按 OSS 兼容协议计算 HMAC-SHA1 并输出 Base64，用于请求授权头签名。
/// 输入密钥仅在内存中参与计算；函数不记录密钥、不发起请求，调用方负责保护签名材料。
pub(crate) fn hmac_sha1_base64(key: &[u8], data: &str) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// 按日期、区域和服务名逐级派生 AWS V4 签名密钥，并对规范签名串计算最终十六进制签名。
/// 计算为确定性纯函数；不会校验时钟或发送请求，调用方必须保证规范请求与签名参数完全一致。
pub(crate) fn s3_upload_signature(
    secret: &str,
    date: &str,
    region: &str,
    string_to_sign: &str,
) -> String {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac_sha256(&k_date, region);
    let k_service = hmac_sha256(&k_region, "s3");
    let k_signing = hmac_sha256(&k_service, "aws4_request");
    hex::encode(hmac_sha256(&k_signing, string_to_sign))
}

fn validate_upload_image_bytes(bytes: &[u8], mime_type: &str) -> AppResult<()> {
    let valid = match mime_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation(
            "upload file content is invalid".to_owned(),
        ))
    }
}

fn normalize_upload_mime_types(value: Option<Vec<String>>) -> AppResult<Vec<String>> {
    let values = value.unwrap_or_else(|| {
        UPLOAD_IMAGE_MIME_TYPES
            .iter()
            .map(|item| (*item).to_owned())
            .collect()
    });
    let mut normalized = Vec::new();
    for item in values {
        let mime = optional_string(Some(item))
            .ok_or_else(|| AppError::Validation("allowed mime type is invalid".to_owned()))?
            .to_ascii_lowercase();
        if !UPLOAD_IMAGE_MIME_TYPES.contains(&mime.as_str()) {
            return Err(AppError::Validation(
                "allowed mime type is invalid".to_owned(),
            ));
        }
        if !normalized.contains(&mime) {
            normalized.push(mime);
        }
    }
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "allowed mime types are required".to_owned(),
        ));
    }
    Ok(normalized)
}

fn normalize_upload_key_prefix(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = optional_string(value) else {
        return Ok(None);
    };
    let mut segments = Vec::new();
    for segment in value.replace('\\', "/").split('/').filter_map(optional_str) {
        if matches!(segment, "." | "..") {
            return Err(AppError::Validation("key_prefix is invalid".to_owned()));
        }
        let safe_segment = safe_upload_key_segment(segment);
        if !safe_segment.is_empty() {
            segments.push(safe_segment);
        }
    }
    let prefix = segments.join("/");
    if prefix.len() > 128 {
        return Err(AppError::Validation("key_prefix is invalid".to_owned()));
    }
    Ok((!prefix.is_empty()).then_some(prefix))
}

fn truncate_upload_filename(name: String, extension: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        return name;
    }
    let suffix = format!(".{extension}");
    if name.ends_with(&suffix) && max_len > suffix.len() {
        let stem_len = max_len - suffix.len();
        format!("{}{}", &name[..stem_len], suffix)
    } else {
        name[..max_len].to_owned()
    }
}

fn upload_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

fn validate_upload_len(value: String, field: &str, max_len: usize) -> AppResult<String> {
    if value.len() > max_len {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(value)
    }
}

fn validate_upload_optional_len(value: Option<&str>, field: &str, max_len: usize) -> AppResult<()> {
    if value.is_some_and(|value| value.len() > max_len) {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(())
    }
}

fn validate_upload_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_upload_value(value, field)?;
    validate_upload_safe_url(value, field, false).map(|_| ())
}

fn validate_upload_credential_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_upload_value(value, field)?;
    validate_upload_safe_url(value, field, true).map(|_| ())
}

fn validate_upload_safe_url(value: &str, field: &str, require_https: bool) -> AppResult<String> {
    let url =
        url::Url::parse(value).map_err(|_| AppError::Validation(format!("{field} is invalid")))?;
    let valid_scheme = if require_https {
        url.scheme() == "https" || (url.scheme() == "http" && is_loopback_upload_url(&url))
    } else {
        matches!(url.scheme(), "http" | "https")
    };
    if !valid_scheme
        || value.len() > 2048
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(value.to_owned())
}

fn is_loopback_upload_url(url: &url::Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

fn validate_upload_bucket_name(value: Option<&str>) -> AppResult<()> {
    let value = require_upload_value(value, "bucket")?;
    let valid = (3..=255).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation("bucket is invalid".to_owned()))
    }
}

fn validate_upload_region(value: Option<&str>) -> AppResult<()> {
    let value = require_upload_value(value, "region")?;
    let valid = value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(AppError::Validation("region is invalid".to_owned()))
    }
}

fn require_upload_value<'a>(value: Option<&'a str>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(optional_str)
        .ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// 复用国家域规则，将国家代码规范化为后台与注册接口共用的稳定格式。
pub(crate) fn validate_country_code(value: &str) -> AppResult<String> {
    normalize_country_code(value)
}

/// 去除国家名称首尾空白并限制 128 个字符；多语言显示名由 locale 配置另行维护。
pub(crate) fn validate_country_name(value: &str) -> AppResult<String> {
    let Some(country_name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("country_name is required".to_owned()));
    };
    if country_name.chars().count() > 128 {
        return Err(AppError::Validation("country_name is too long".to_owned()));
    }
    Ok(country_name)
}

/// 去除国家备注首尾空白并限制 128 个字符，空备注按当前后台合同拒绝。
pub(crate) fn validate_country_remark(value: &str) -> AppResult<String> {
    let Some(remark) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("remark is required".to_owned()));
    };
    if remark.chars().count() > 128 {
        return Err(AppError::Validation("remark is too long".to_owned()));
    }
    Ok(remark)
}

/// 规范化国家启停状态；这里只校验目标代码，不判断已有用户或注册流程是否受影响。
pub(crate) fn validate_country_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported country status".to_owned(),
        )),
    }
}

/// 规范化默认 locale 与支持列表，去重后要求默认语言仍包含在列表中。
/// 该纯规则不加载翻译资源；应用事务只持久化经过验证的语言代码集合。
pub(crate) fn validate_country_locale_config(
    default_locale: &str,
    supported_locales: Vec<String>,
) -> AppResult<(String, Vec<String>)> {
    let default_locale = normalize_locale(default_locale)?;
    let supported_locales = normalize_supported_locales(supported_locales)?;
    ensure_default_locale_supported(&default_locale, &supported_locales)?;
    Ok((default_locale, supported_locales))
}

/// 将国家代码、名称、备注、语言集合、注册开关、状态和排序映射为配置审计快照。
/// 时间统一为毫秒值；应用层在国家配置写事务中保存前后值，本函数不修改注册策略。
pub(crate) fn country_config_audit_json(country: &AdminCountryResponse) -> Value {
    json!({
        "id": country.id,
        "country_code": country.country_code,
        "country_name": country.country_name,
        "remark": country.remark,
        "default_locale": country.default_locale,
        "supported_locales": country.supported_locales.0.clone(),
        "registration_enabled": country.registration_enabled,
        "status": country.status,
        "sort_order": country.sort_order,
        "created_at": country.created_at.timestamp_millis(),
        "updated_at": country.updated_at.timestamp_millis(),
    })
}

fn validate_smtp_config_name(value: Option<String>, fallback: Option<&str>) -> AppResult<String> {
    let name =
        optional_string(value).or_else(|| fallback.and_then(optional_str).map(str::to_owned));
    let Some(name) = name else {
        return Err(AppError::Validation(
            "smtp config name is required".to_owned(),
        ));
    };
    if name.len() > 64 {
        return Err(AppError::Validation(
            "smtp config name is too long".to_owned(),
        ));
    }
    Ok(name)
}

fn validate_smtp_verification_code_templates(
    templates: Option<Vec<VerificationCodeTemplate>>,
) -> AppResult<Vec<VerificationCodeTemplate>> {
    let Some(templates) = templates else {
        return Ok(Vec::new());
    };
    if templates.len() > 20 {
        return Err(AppError::Validation(
            "verification_code_templates cannot exceed 20 templates".to_owned(),
        ));
    }

    let mut keys = HashSet::new();
    templates
        .into_iter()
        .map(|template| {
            let key = optional_string(Some(template.key)).ok_or_else(|| {
                AppError::Validation("verification_code_template key is required".to_owned())
            })?;
            if key.len() > 64 {
                return Err(AppError::Validation(
                    "verification_code_template key is too long".to_owned(),
                ));
            }
            if !keys.insert(key.clone()) {
                return Err(AppError::Validation(
                    "verification_code_template key must be unique".to_owned(),
                ));
            }

            let name = optional_string(Some(template.name)).ok_or_else(|| {
                AppError::Validation("verification_code_template name is required".to_owned())
            })?;
            if name.len() > 128 {
                return Err(AppError::Validation(
                    "verification_code_template name is too long".to_owned(),
                ));
            }

            let purpose = optional_string(template.purpose)
                .filter(|purpose| purpose != "default")
                .map(|purpose| {
                    if purpose.len() > 64 {
                        return Err(AppError::Validation(
                            "verification_code_template purpose is too long".to_owned(),
                        ));
                    }
                    Ok(purpose)
                })
                .transpose()?;

            let html = optional_string(Some(template.html)).ok_or_else(|| {
                AppError::Validation("verification_code_template html is required".to_owned())
            })?;
            if html.len() > 20_000 {
                return Err(AppError::Validation(
                    "verification_code_template html is too long".to_owned(),
                ));
            }

            Ok(VerificationCodeTemplate {
                key,
                name,
                purpose,
                html,
                enabled: template.enabled,
            })
        })
        .collect()
}
