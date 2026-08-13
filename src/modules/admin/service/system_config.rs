//! 系统配置域的纯业务规则层，集中 SMTP 发信、对象存储上传与国家配置三类设置的校验、映射与签名计算。
//!
//! 本文件不持有连接池、不开事务、不发起网络请求，所有函数都是可单独测试的纯函数或纯内存转换：
//! 校验函数负责把请求 DTO 收敛成已规范化的中间结构，响应映射函数负责剥离密文只输出掩码，
//! 审计 JSON 函数负责生成写入审计表的前后值快照，签名与摘要函数则为对象存储协议提供确定性计算。
//! 凭据在这一层只被读取形状而不会被解密或加密，密钥落库与旧密文保留策略统一由 application 层的事务决定。

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
/// 长度上限分别是主机 255 字节、发件人显示名 128 字节、单模板 HTML 两万字节，端口为 0 直接判非法。
/// 优先级取请求值、回退值、常量 100 三级兜底并要求不超过 9999，数值越小越优先由发信选择算法解释。
/// 注意本函数完全不读取 username 与 password 字段，凭据是否替换由 `smtp_request_has_new_secret` 单独判断。
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
/// 具体拒绝条件是总长超过 255 字节、出现第二个 `@`、本地部分或域名为空、以及含任意空白字符。
/// field 参数只用于拼接错误文案以区分是发件人还是收件人出错，不影响判定规则本身。
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
/// 密码字段被压缩成 password_set 布尔量，只表明是否存在密文，因此前端无法据此区分口令内容是否变化。
/// 验证码模板走 `smtp_templates_from_record` 做新旧字段合并，所以响应里看到的模板集合可能来自旧版单模板字段。
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
/// 与对外响应的差别在于这里额外保留了旧版单模板 HTML 原文，便于回溯模板迁移过程中的实际内容。
/// 由于密码只记 password_set 布尔量，仅换口令而不改其他字段的操作在审计前后值上看不出差异，需结合操作原因判断。
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
/// 合并规则是新版模板数组只要非空就完全胜出，旧版单模板字段被忽略而不做追加，避免同一用途出现两份模板。
/// 只有新版数组为空且旧版 HTML 去空后非空时，才把它包装成 key 与 name 固定、purpose 为空且启用的兼容模板。
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
    /// 该代码同时是落库值和分派键，因此必须与 `parse` 接受的主名称保持一致，改动会让历史配置无法解析。
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ImageBed => "image_bed",
            Self::Oss => "oss",
            Self::S3 => "s3",
            Self::Local => "local",
        }
    }

    /// 判断该上传提供商是否需要 Bearer 凭据。
    /// 当前只有图床走 Bearer 令牌，本地存储不需要凭据，对象存储走访问密钥对，因此三者互不重叠。
    pub(crate) const fn uses_bearer(self) -> bool {
        matches!(self, Self::ImageBed)
    }

    /// 判断该上传提供商是否需要访问密钥与密钥对。
    /// 仅 OSS 与 S3 两类对象存储成立，应用层据此决定保存配置时是否必须校验访问密钥与私钥同时存在。
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
/// 四种提供商的必填组合各不相同：图床只要求可用的凭据类端点；本地存储要求根目录与公开基础地址；
/// S3 要求桶名与区域而端点可选；OSS 要求端点与桶名但不要求区域。凭据类端点强制 HTTPS，仅回环地址放行 HTTP。
/// 文件上限缺省 10 MiB 且必须落在 1 字节到 100 MiB 之间，MIME 白名单缺省为四种图片类型且去重后不得为空。
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
/// 校验顺序是先判空、再核对魔数与声明 MIME 是否匹配、然后比大小、最后比白名单，
/// 因此伪造扩展名或 MIME 的非图片内容会在魔数环节被拒；白名单比对区分大小写，取值由配置校验时统一转小写保证。
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
/// 三类凭据的记录粒度并不一致：Bearer 与访问密钥同时给出掩码和存在标记，私钥只给出存在标记，
/// 因此仅轮换私钥的操作在审计前后值上不可见，需要结合本次操作原因来还原意图。
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
/// 处理链是先把反斜杠统一成正斜杠再取最后一段，从而同时挡住 Windows 与 POSIX 两种路径穿越写法；
/// 清洗后若结果为空则退回按 MIME 推导的默认名，最终再按 255 字节截断并尽量保住扩展名后缀。
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

/// 按声明的 MIME 逐一比对文件头魔数，确认上传内容确实是对应格式的图片。
/// PNG 比对八字节签名，JPEG 比对起始三字节，GIF 接受 87a 与 89a 两种版本，WebP 需要 RIFF 头且第 8 到 12 字节为 WEBP。
/// 声明为白名单以外的 MIME 一律判为非法，因此该函数同时充当「只允许图片」的兜底闸门；
/// 它只看文件头而不解码整幅图像，所以能挡住改扩展名的可执行文件，但不保证图片本身没有损坏。
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

/// 规范化允许上传的 MIME 白名单：缺省填入四种内置图片类型，显式提供时逐项去空、转小写并去重。
/// 任何一项在去空后为空或不属于内置图片类型集合都会整体判为校验错误，避免通过配置绕开图片限制。
/// 去重保留首次出现的顺序，结果为空同样报错，因此调用方拿到的一定是非空且各项均受支持的列表。
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

/// 把对象键前缀规范成安全的多段路径：统一分隔符后逐段清洗，丢弃空段并用单斜杠重新拼接。
/// 出现 `.` 或 `..` 段直接判非法以阻断目录穿越，其余字符按 ASCII 字母数字与点、横线、下划线过滤。
/// 清洗后总长超过 128 字节报错；输入为空或清洗后无有效段时返回 None，表示对象键不加任何前缀。
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

/// 把文件名截断到字节上限，并在原名本就以该扩展名结尾时优先保住后缀而只裁剪主干部分。
/// 长度未超限时原样返回；无法保住后缀的情况直接硬截断，因此结果可能不再带扩展名。
/// 按字节而非字符切分，调用前的清洗已把内容限制为 ASCII，故不会在此切出非法 UTF-8。
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

/// 把受支持的图片 MIME 映射成生成对象键和文件名时使用的扩展名，JPEG 统一取 jpg 而非 jpeg。
/// 其余取值一律回落到 bin；由于上传前已做过白名单与魔数校验，实际不应出现该兜底分支。
fn upload_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

/// 校验必填上传配置字段的字节长度并原样返回值，超限时用字段名拼出统一的校验错误文案。
/// 按字节而非字符计数，因此含中文的值会更早触顶；本函数只管长度，不做去空白或格式判断。
fn validate_upload_len(value: String, field: &str, max_len: usize) -> AppResult<String> {
    if value.len() > max_len {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(value)
    }
}

/// 校验可选上传配置字段的字节长度：值缺省时视为通过，存在且超限时返回带字段名的校验错误。
/// 与必填版本的区别只在于 None 被接受且不回传值，因此适用于 endpoint、公开地址这类允许留空的项。
fn validate_upload_optional_len(value: Option<&str>, field: &str, max_len: usize) -> AppResult<()> {
    if value.is_some_and(|value| value.len() > max_len) {
        Err(AppError::Validation(format!("{field} is invalid")))
    } else {
        Ok(())
    }
}

/// 校验面向终端用户展示的公开地址：必须提供，且 HTTP 与 HTTPS 均可接受。
/// 用于 public_base_url 这类只用来拼接下载链接、不承载凭据的地址，因此不强制加密传输。
fn validate_upload_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_upload_value(value, field)?;
    validate_upload_safe_url(value, field, false).map(|_| ())
}

/// 校验会携带凭据发起请求的端点地址：必须提供，且原则上只接受 HTTPS。
/// 与公开地址校验的唯一差别是这里开启了强制加密开关，仅回环主机被特别放行以便本地联调。
fn validate_upload_credential_url(value: Option<&str>, field: &str) -> AppResult<()> {
    let value = require_upload_value(value, field)?;
    validate_upload_safe_url(value, field, true).map(|_| ())
}

/// 解析并收紧上传相关地址的形状，是公开地址与凭据端点两个入口共用的底层判定。
/// 除协议限制外还统一拒绝超过 2048 字节、内嵌用户名或口令、带查询串或片段的地址，
/// 目的是避免把凭据写进地址、也避免签名时因多余组件导致规范化请求与服务端不一致。
/// require_https 为真时只放行 HTTPS 及回环主机上的 HTTP，为假时 HTTP 与 HTTPS 同等接受。
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

/// 判断地址主机是否为本机回环，用于放行本地联调时的明文 HTTP 端点。
/// 仅按字面匹配 localhost 与 IPv4、IPv6 两种回环字面量，不做 DNS 解析，
/// 因此解析到 127.0.0.1 的自定义域名不会被视为回环，仍需使用 HTTPS。
fn is_loopback_upload_url(url: &url::Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

/// 校验对象存储桶名必填且长度在 3 到 255 字节之间，字符仅限 ASCII 字母数字与点、横线、下划线。
/// 这是一套同时兼容 S3 与 OSS 的宽松规则，不校验各家更细的首尾字符或点号连用限制，
/// 因此通过校验的桶名仍可能被具体服务商拒绝，最终以对象存储返回的错误为准。
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

/// 校验对象存储区域必填、长度不超过 128 字节，且只含 ASCII 字母数字与横线。
/// 该值会直接参与 AWS V4 签名密钥派生，含空格或其他字符会导致签名与服务端计算结果不符，故在此提前收紧。
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

/// 断言某个上传配置字段已提供且去空后非空，返回去空后的借用值供后续格式校验复用。
/// 纯空白与 None 被同等视为缺失并报出带字段名的必填错误，因此下游拿到的一定是非空片段。
fn require_upload_value<'a>(value: Option<&'a str>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(optional_str)
        .ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

/// 计算 HMAC-SHA256 原始字节，是 AWS V4 签名逐级派生密钥时反复调用的底层原语。
/// 输出保持二进制而非十六进制，以便直接作为下一级派生的密钥输入；只有最终一级才转成十六进制。
/// 由于 HMAC 接受任意长度密钥，构造失败在此被断言为不可能，密钥本身不会被记录或复制到日志。
fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// 复用国家域规则，将国家代码规范化为后台与注册接口共用的稳定格式。
/// 后台不另立一套代码规则，直接委托国家上下文的规范化实现，以保证配置写入值与注册时的匹配值完全同源。
pub(crate) fn validate_country_code(value: &str) -> AppResult<String> {
    normalize_country_code(value)
}

/// 去除国家名称首尾空白并限制 128 个字符；多语言显示名由 locale 配置另行维护。
/// 纯空白等同于缺失并报必填错误，长度按字符数而非字节数统计，因此中文名称的可用字数与英文一致。
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
/// 与国家名称的规则完全一致但字段独立：备注面向运营说明用途，不参与注册判定，也不会展示给终端用户。
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
/// 仅接受 active 与 disabled 两个取值，比对在去空之后进行且区分大小写，其余输入一律返回不支持的状态错误。
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
/// 快照同时覆盖内容修改与状态切换两个入口所能改动的全部字段，因此两类操作可以用同一份结构做前后对比。
/// 支持语言以数组原样展开而非拼接字符串，便于在审计中直接看出增删了哪些语言。
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

/// 解析 SMTP 配置名称：优先取请求值，请求为空时回退到调用方给出的旧名称。
/// 更新场景正是靠这个回退实现「不提交名称即保持原名」，创建场景没有回退值因而名称成为必填。
/// 请求值与回退值都会先做去空白判定，纯空白视为未提供；最终名称超过 64 字节返回校验错误。
/// 该函数只管形状，不查询是否与其他配置重名，唯一性由数据库约束和应用层冲突处理负责。
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

/// 逐条校验并规范化验证码邮件模板集合，返回可直接落库的模板数组。
/// 请求未提供模板字段时返回空数组，代表本次不配置新版模板而由旧版单模板字段兜底，这与「显式提交空数组」等价。
/// 单次最多 20 条模板；每条要求 key、name、html 去空后非空，长度上限依次为 64、128 与两万字节。
/// key 在集合内必须唯一，重复即整体报错；purpose 为可选且字面量 default 会被归一成空，
/// 以免默认用途同时以两种写法存在，其余 purpose 取值限长 64 字节。
/// 任何一条不合法都会让整批校验失败，不存在部分模板被接受的情况。
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
