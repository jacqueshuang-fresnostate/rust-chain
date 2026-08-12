use super::*;

const DEFAULT_UPLOAD_CONFIG_NAME: &str = "default";

#[derive(Debug)]
pub(crate) struct AdminCountryListFilter {
    pub(crate) country_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) registration_enabled: Option<bool>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminCountryInsert {
    pub(crate) country_code: String,
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
}

#[derive(Debug)]
pub(crate) struct AdminCountryUpdate {
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) sort_order: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminSmtpConfigRow {
    id: u64,
    name: String,
    host: String,
    port: u16,
    security: String,
    username_ciphertext: Option<String>,
    password_ciphertext: Option<String>,
    username_mask: Option<String>,
    from_email: String,
    from_name: Option<String>,
    verification_code_template_html: Option<String>,
    verification_code_templates_json: Option<SqlxJson<Vec<VerificationCodeTemplate>>>,
    enabled: bool,
    priority: u32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminSmtpDeliverySettingsRow {
    strategy: String,
    round_robin_cursor: Option<u64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AdminUploadConfigRow {
    id: u64,
    name: String,
    provider: String,
    endpoint: Option<String>,
    file_field: Option<String>,
    bearer_token_ciphertext: Option<String>,
    bearer_token_mask: Option<String>,
    access_key_ciphertext: Option<String>,
    access_key_mask: Option<String>,
    secret_key_ciphertext: Option<String>,
    bucket: Option<String>,
    region: Option<String>,
    public_base_url: Option<String>,
    local_root: Option<String>,
    key_prefix: Option<String>,
    max_file_size_bytes: u64,
    allowed_mime_types_json: SqlxJson<Vec<String>>,
    enabled: bool,
}

pub(crate) async fn list_admin_countries(
    pool: &Pool<MySql>,
    filter: AdminCountryListFilter,
) -> AppResult<(Vec<AdminCountryResponse>, i64)> {
    let mut rows = admin_country_query();
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM country_configs");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(country_code) = filter.country_code.clone() {
            builder.push(" AND country_code = ");
            builder.push_bind(country_code);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(registration_enabled) = filter.registration_enabled {
            builder.push(" AND registration_enabled = ");
            builder.push_bind(registration_enabled);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY sort_order ASC, country_code ASC",
        filter.limit,
        filter.offset,
    )
    .await
}

pub(crate) async fn insert_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminCountryInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO country_configs
           (country_code, country_name, remark, default_locale, supported_locales, registration_enabled, status, sort_order)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.country_code)
    .bind(&input.country_name)
    .bind(&input.remark)
    .bind(&input.default_locale)
    .bind(SqlxJson(input.supported_locales))
    .bind(input.registration_enabled)
    .bind(&input.status)
    .bind(input.sort_order)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_country_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
    input: AdminCountryUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE country_configs
           SET country_name = ?, remark = ?, default_locale = ?, supported_locales = ?, registration_enabled = ?, sort_order = ?
           WHERE id = ?"#,
    )
    .bind(&input.country_name)
    .bind(&input.remark)
    .bind(&input.default_locale)
    .bind(SqlxJson(input.supported_locales))
    .bind(input.registration_enabled)
    .bind(input.sort_order)
    .bind(country_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_country_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE country_configs SET status = ? WHERE id = ?")
        .bind(status)
        .bind(country_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
) -> AppResult<AdminCountryResponse> {
    let mut builder = admin_country_query();
    builder.push(" WHERE id = ");
    builder.push_bind(country_id);
    builder
        .build_query_as::<AdminCountryResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_admin_country_in_tx(
    tx: &mut Transaction<'_, MySql>,
    country_id: u64,
) -> AppResult<AdminCountryResponse> {
    let mut builder = admin_country_query();
    builder.push(" WHERE id = ");
    builder.push_bind(country_id);
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<AdminCountryResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_admin_smtp_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    let row = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "WHERE name = ?",
        false,
    ))
    .bind(DEFAULT_SMTP_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(admin_smtp_config_record))
}

pub(crate) async fn list_admin_smtp_configs(
    pool: &Pool<MySql>,
) -> AppResult<Vec<AdminSmtpConfigRecord>> {
    let rows = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "ORDER BY priority ASC, id ASC",
        false,
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(admin_smtp_config_record).collect())
}

pub(crate) async fn load_admin_smtp_delivery_settings(
    pool: &Pool<MySql>,
) -> AppResult<AdminSmtpDeliverySettingsRecord> {
    let row = sqlx::query_as::<_, AdminSmtpDeliverySettingsRow>(
        "SELECT strategy, round_robin_cursor FROM smtp_delivery_settings WHERE id = ?",
    )
    .bind(SMTP_DELIVERY_SETTINGS_ID)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(admin_smtp_delivery_settings_record)
        .unwrap_or_else(default_smtp_delivery_settings_record))
}

pub(crate) async fn lock_admin_smtp_delivery_settings_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<AdminSmtpDeliverySettingsRecord> {
    let row = sqlx::query_as::<_, AdminSmtpDeliverySettingsRow>(
        "SELECT strategy, round_robin_cursor FROM smtp_delivery_settings WHERE id = ? FOR UPDATE",
    )
    .bind(SMTP_DELIVERY_SETTINGS_ID)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row
        .map(admin_smtp_delivery_settings_record)
        .unwrap_or_else(default_smtp_delivery_settings_record))
}

pub(crate) async fn upsert_admin_smtp_delivery_settings_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy: &str,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO smtp_delivery_settings (id, strategy, updated_by)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE strategy = VALUES(strategy), updated_by = VALUES(updated_by)"#,
    )
    .bind(SMTP_DELIVERY_SETTINGS_ID)
    .bind(strategy)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn lock_admin_smtp_config_by_name_in_tx(
    tx: &mut Transaction<'_, MySql>,
    name: &str,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE name = ?", true))
        .bind(name)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn lock_admin_smtp_config_by_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE id = ?", true))
        .bind(config_id)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_smtp_config_by_name_in_tx(
    tx: &mut Transaction<'_, MySql>,
    name: &str,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE name = ?", false))
        .bind(name)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_smtp_config_by_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql("WHERE id = ?", false))
        .bind(config_id)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_smtp_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_smtp_config_by_id(
    pool: &Pool<MySql>,
    config_id: u64,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    let row = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "WHERE id = ?",
        false,
    ))
    .bind(config_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(admin_smtp_config_record))
}

pub(crate) async fn admin_smtp_config_name_exists_except(
    tx: &mut Transaction<'_, MySql>,
    name: &str,
    config_id: u64,
) -> AppResult<bool> {
    let id = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM smtp_configs WHERE name = ? AND id <> ? LIMIT 1",
    )
    .bind(name)
    .bind(config_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(id.is_some())
}

pub(crate) async fn insert_admin_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminSmtpConfigWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO smtp_configs
           (name, host, port, security, username_ciphertext, password_ciphertext,
            username_mask, from_email, from_name, verification_code_template_html,
            verification_code_templates_json, enabled, priority, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.security)
    .bind(&input.username_ciphertext)
    .bind(&input.password_ciphertext)
    .bind(&input.username_mask)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(&input.verification_code_template_html)
    .bind(SqlxJson(input.verification_code_templates))
    .bind(input.enabled)
    .bind(input.priority)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn upsert_default_admin_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminSmtpConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO smtp_configs
           (name, host, port, security, username_ciphertext, password_ciphertext,
            username_mask, from_email, from_name, verification_code_template_html,
            verification_code_templates_json, enabled, priority, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE host = VALUES(host),
                                   port = VALUES(port),
                                   security = VALUES(security),
                                   username_ciphertext = VALUES(username_ciphertext),
                                   password_ciphertext = VALUES(password_ciphertext),
                                   username_mask = VALUES(username_mask),
                                   from_email = VALUES(from_email),
                                   from_name = VALUES(from_name),
                                   verification_code_template_html = VALUES(verification_code_template_html),
                                   verification_code_templates_json = VALUES(verification_code_templates_json),
                                   enabled = VALUES(enabled),
                                   priority = VALUES(priority),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.security)
    .bind(&input.username_ciphertext)
    .bind(&input.password_ciphertext)
    .bind(&input.username_mask)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(&input.verification_code_template_html)
    .bind(SqlxJson(input.verification_code_templates))
    .bind(input.enabled)
    .bind(input.priority)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_admin_smtp_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    config_id: u64,
    input: AdminSmtpConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE smtp_configs
           SET name = ?, host = ?, port = ?, security = ?,
               username_ciphertext = ?, password_ciphertext = ?, username_mask = ?,
               from_email = ?, from_name = ?, verification_code_template_html = ?,
               verification_code_templates_json = ?, enabled = ?, priority = ?, updated_by = ?
           WHERE id = ?"#,
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.security)
    .bind(&input.username_ciphertext)
    .bind(&input.password_ciphertext)
    .bind(&input.username_mask)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(&input.verification_code_template_html)
    .bind(SqlxJson(input.verification_code_templates))
    .bind(input.enabled)
    .bind(input.priority)
    .bind(input.updated_by)
    .bind(config_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_enabled_admin_smtp_config_records_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Vec<AdminSmtpConfigRecord>> {
    let rows = sqlx::query_as::<_, AdminSmtpConfigRow>(&select_admin_smtp_config_sql(
        "WHERE enabled = TRUE ORDER BY priority ASC, id ASC",
        false,
    ))
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().map(admin_smtp_config_record).collect())
}

pub(crate) async fn load_admin_smtp_config_for_delivery(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminSmtpConfigRecord>> {
    let mut tx = pool.begin().await?;
    let settings = lock_admin_smtp_delivery_settings_in_tx(&mut tx).await?;
    let records = load_enabled_admin_smtp_config_records_in_tx(&mut tx).await?;
    let Some(record) = select_smtp_delivery_config(&settings, &records) else {
        tx.commit().await?;
        return Ok(None);
    };
    if settings.strategy == SMTP_DELIVERY_STRATEGY_ROUND_ROBIN {
        sqlx::query(
            r#"INSERT INTO smtp_delivery_settings (id, strategy, round_robin_cursor)
               VALUES (?, ?, ?)
               ON DUPLICATE KEY UPDATE round_robin_cursor = VALUES(round_robin_cursor)"#,
        )
        .bind(SMTP_DELIVERY_SETTINGS_ID)
        .bind(SMTP_DELIVERY_STRATEGY_ROUND_ROBIN)
        .bind(record.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Some(record))
}

pub(crate) fn admin_smtp_email_config(
    record: &AdminSmtpConfigRecord,
    key: Option<&str>,
) -> AppResult<SmtpEmailConfig> {
    let key = if record.username_ciphertext.is_some() || record.password_ciphertext.is_some() {
        Some(key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?)
    } else {
        None
    };
    let username = match key {
        Some(key) => decrypt_optional_secret(record.username_ciphertext.as_deref(), key)?,
        None => None,
    };
    let password = match key {
        Some(key) => decrypt_optional_secret(record.password_ciphertext.as_deref(), key)?,
        None => None,
    };
    Ok(SmtpEmailConfig {
        host: record.host.clone(),
        port: record.port,
        security: parse_smtp_security(&record.security)?,
        username,
        password,
        from_email: record.from_email.clone(),
        from_name: record.from_name.clone(),
        verification_code_template_html: record.verification_code_template_html.clone(),
        verification_code_templates: smtp_templates_from_record(record),
    })
}

pub(crate) async fn load_enabled_admin_smtp_email_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_admin_smtp_config_for_delivery(pool)
        .await?
        .map(|record| admin_smtp_email_config(&record, key))
        .transpose()
}

pub(crate) async fn load_admin_upload_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminUploadConfigRecord>> {
    let row = sqlx::query_as::<_, AdminUploadConfigRow>(&select_admin_upload_config_sql(false))
        .bind(DEFAULT_UPLOAD_CONFIG_NAME)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(admin_upload_config_record))
}

pub(crate) async fn lock_admin_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<AdminUploadConfigRecord>> {
    sqlx::query_as::<_, AdminUploadConfigRow>(&select_admin_upload_config_sql(true))
        .bind(DEFAULT_UPLOAD_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(admin_upload_config_record))
        .map_err(AppError::Database)
}

pub(crate) async fn load_admin_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<AdminUploadConfigRecord> {
    sqlx::query_as::<_, AdminUploadConfigRow>(&select_admin_upload_config_sql(false))
        .bind(DEFAULT_UPLOAD_CONFIG_NAME)
        .fetch_one(&mut **tx)
        .await
        .map(admin_upload_config_record)
        .map_err(AppError::Database)
}

/// 在调用方事务中写入唯一的默认上传存储配置，覆盖其完整运行参数和加密凭证字段。
/// 调用方须先完成管理员权限、字段合法性及启用所需凭证校验，并只传入已加密的敏感值。
/// 该函数不自行提交也不写审计；配置写入必须与调用方的前后快照审计共用事务。
/// 默认名称唯一键使相同输入可重复覆盖；SQL 失败向上返回并由调用方回滚，不暴露明文凭证。
pub(crate) async fn upsert_admin_upload_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminUploadConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO upload_storage_configs
           (name, provider, endpoint, file_field, bearer_token_ciphertext, bearer_token_mask,
            access_key_ciphertext, access_key_mask, secret_key_ciphertext, bucket, region,
            public_base_url, local_root, key_prefix, max_file_size_bytes, allowed_mime_types_json,
            enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE provider = VALUES(provider),
                                   endpoint = VALUES(endpoint),
                                   file_field = VALUES(file_field),
                                   bearer_token_ciphertext = VALUES(bearer_token_ciphertext),
                                   bearer_token_mask = VALUES(bearer_token_mask),
                                   access_key_ciphertext = VALUES(access_key_ciphertext),
                                   access_key_mask = VALUES(access_key_mask),
                                   secret_key_ciphertext = VALUES(secret_key_ciphertext),
                                   bucket = VALUES(bucket),
                                   region = VALUES(region),
                                   public_base_url = VALUES(public_base_url),
                                   local_root = VALUES(local_root),
                                   key_prefix = VALUES(key_prefix),
                                   max_file_size_bytes = VALUES(max_file_size_bytes),
                                   allowed_mime_types_json = VALUES(allowed_mime_types_json),
                                   enabled = VALUES(enabled),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_UPLOAD_CONFIG_NAME)
    .bind(&input.provider)
    .bind(&input.endpoint)
    .bind(&input.file_field)
    .bind(&input.bearer_token_ciphertext)
    .bind(&input.bearer_token_mask)
    .bind(&input.access_key_ciphertext)
    .bind(&input.access_key_mask)
    .bind(&input.secret_key_ciphertext)
    .bind(&input.bucket)
    .bind(&input.region)
    .bind(&input.public_base_url)
    .bind(&input.local_root)
    .bind(&input.key_prefix)
    .bind(input.max_file_size_bytes)
    .bind(SqlxJson(input.allowed_mime_types))
    .bind(input.enabled)
    .bind(input.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_enabled_admin_upload_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<AdminUploadConfigRecord>> {
    let row = sqlx::query_as::<_, AdminUploadConfigRow>(
        r#"SELECT id, name, provider, endpoint, file_field, bearer_token_ciphertext,
                  bearer_token_mask, access_key_ciphertext, access_key_mask, secret_key_ciphertext,
                  bucket, region, public_base_url, local_root, key_prefix, max_file_size_bytes,
                  allowed_mime_types_json, enabled
           FROM upload_storage_configs
           WHERE name = ? AND enabled = TRUE"#,
    )
    .bind(DEFAULT_UPLOAD_CONFIG_NAME)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(admin_upload_config_record))
}

pub(crate) fn admin_upload_config_response(
    record: AdminUploadConfigRecord,
) -> UploadConfigResponse {
    UploadConfigResponse {
        id: record.id,
        name: record.name,
        provider: record.provider,
        endpoint: record.endpoint,
        file_field: record.file_field,
        bearer_token_mask: record.bearer_token_mask,
        bearer_token_set: record.bearer_token_ciphertext.is_some(),
        access_key_mask: record.access_key_mask,
        access_key_set: record.access_key_ciphertext.is_some(),
        secret_key_set: record.secret_key_ciphertext.is_some(),
        bucket: record.bucket,
        region: record.region,
        public_base_url: record.public_base_url,
        local_root: record.local_root,
        key_prefix: record.key_prefix,
        max_file_size_bytes: record.max_file_size_bytes,
        allowed_mime_types: record.allowed_mime_types,
        enabled: record.enabled,
    }
}

pub(crate) async fn upload_admin_file_to_storage(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    validate_upload_file(
        record.max_file_size_bytes,
        &record.allowed_mime_types,
        input,
    )?;
    let provider = UploadProvider::parse(&record.provider)?;
    match provider {
        UploadProvider::ImageBed => upload_to_image_bed(record, key, input).await,
        UploadProvider::Local => upload_to_local(record, input).await,
        UploadProvider::S3 => upload_to_s3(record, key, input).await,
        UploadProvider::Oss => upload_to_oss(record, key, input).await,
    }
}

pub(crate) async fn insert_admin_upload_object(
    pool: &Pool<MySql>,
    input: AdminUploadObjectWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO upload_objects
           (provider, object_key, public_url, share_url, delete_url, mime_type, size_bytes,
            original_filename, uploaded_by, uploaded_by_user)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.provider)
    .bind(&input.object_key)
    .bind(&input.public_url)
    .bind(&input.share_url)
    .bind(&input.delete_url)
    .bind(&input.mime_type)
    .bind(input.size_bytes)
    .bind(&input.original_filename)
    .bind(input.owner.admin_id())
    .bind(input.owner.user_id())
    .execute(pool)
    .await?;
    Ok(())
}

fn admin_country_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT id, country_code, country_name, remark, default_locale, supported_locales,
                  registration_enabled, status, sort_order, created_at, updated_at
           FROM country_configs"#,
    )
}

fn map_duplicate_country_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("country already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

fn select_admin_upload_config_sql(for_update: bool) -> String {
    let mut sql = r#"SELECT id, name, provider, endpoint, file_field, bearer_token_ciphertext,
              bearer_token_mask, access_key_ciphertext, access_key_mask, secret_key_ciphertext,
              bucket, region, public_base_url, local_root, key_prefix, max_file_size_bytes,
              allowed_mime_types_json, enabled
       FROM upload_storage_configs
       WHERE name = ?"#
        .to_owned();
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn select_admin_smtp_config_sql(clause: &str, for_update: bool) -> String {
    let mut sql = format!(
        r#"SELECT id, name, host, port, security, username_ciphertext, password_ciphertext,
                  username_mask, from_email, from_name, verification_code_template_html,
                  verification_code_templates_json, enabled, priority
           FROM smtp_configs
           {clause}"#
    );
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn admin_smtp_config_record(row: AdminSmtpConfigRow) -> AdminSmtpConfigRecord {
    AdminSmtpConfigRecord {
        id: row.id,
        name: row.name,
        host: row.host,
        port: row.port,
        security: row.security,
        username_ciphertext: row.username_ciphertext,
        password_ciphertext: row.password_ciphertext,
        username_mask: row.username_mask,
        from_email: row.from_email,
        from_name: row.from_name,
        verification_code_template_html: row.verification_code_template_html,
        verification_code_templates: row
            .verification_code_templates_json
            .map(|templates| templates.0)
            .unwrap_or_default(),
        enabled: row.enabled,
        priority: row.priority,
    }
}

fn admin_smtp_delivery_settings_record(
    row: AdminSmtpDeliverySettingsRow,
) -> AdminSmtpDeliverySettingsRecord {
    AdminSmtpDeliverySettingsRecord {
        strategy: row.strategy,
        round_robin_cursor: row.round_robin_cursor,
    }
}

fn admin_upload_config_record(row: AdminUploadConfigRow) -> AdminUploadConfigRecord {
    AdminUploadConfigRecord {
        id: row.id,
        name: row.name,
        provider: row.provider,
        endpoint: row.endpoint,
        file_field: row.file_field,
        bearer_token_ciphertext: row.bearer_token_ciphertext,
        bearer_token_mask: row.bearer_token_mask,
        access_key_ciphertext: row.access_key_ciphertext,
        access_key_mask: row.access_key_mask,
        secret_key_ciphertext: row.secret_key_ciphertext,
        bucket: row.bucket,
        region: row.region,
        public_base_url: row.public_base_url,
        local_root: row.local_root,
        key_prefix: row.key_prefix,
        max_file_size_bytes: row.max_file_size_bytes,
        allowed_mime_types: row.allowed_mime_types_json.0,
        enabled: row.enabled,
    }
}

async fn upload_to_image_bed(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let endpoint = record
        .endpoint
        .as_deref()
        .ok_or_else(|| AppError::Validation("image bed endpoint is not configured".to_owned()))?;
    let token = decrypt_required_upload_secret(
        record.bearer_token_ciphertext.as_deref(),
        key,
        "bearer token",
    )?;
    let field = record
        .file_field
        .as_deref()
        .unwrap_or(DEFAULT_UPLOAD_FILE_FIELD);
    let filename = safe_upload_filename(input.original_filename.as_deref(), &input.mime_type);
    let part = Part::bytes(input.bytes.clone())
        .file_name(filename)
        .mime_str(&input.mime_type)
        .map_err(|_| AppError::Validation("upload file mime type is invalid".to_owned()))?;
    let form = Form::new().part(field.to_owned(), part);
    let response = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|_| AppError::Validation("image bed upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "image bed upload failed with status {}",
            response.status().as_u16()
        )));
    }
    let payload = response
        .json::<ImageBedUploadResponse>()
        .await
        .map_err(|_| AppError::Validation("image bed upload response is invalid".to_owned()))?;
    if payload.success == Some(false) {
        return Err(AppError::Validation("image bed upload failed".to_owned()));
    }
    let download_url = safe_upload_response_url(
        payload.links.download.as_deref(),
        "image bed download url",
        true,
    )?
    .ok_or_else(|| AppError::Validation("image bed download url is missing".to_owned()))?;
    let share_url =
        safe_upload_response_url(payload.links.share.as_deref(), "image bed share url", false)?;
    let delete_url = safe_upload_response_url(
        payload.links.delete.as_deref(),
        "image bed delete url",
        false,
    )?;
    let object_key = payload
        .file
        .as_ref()
        .and_then(|file| file.id.as_deref())
        .map(safe_upload_key_segment)
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let size_bytes = payload
        .file
        .as_ref()
        .and_then(|file| file.size)
        .unwrap_or(input.bytes.len() as u64);
    let mime_type = payload
        .file
        .and_then(|file| file.file_type)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| UPLOAD_IMAGE_MIME_TYPES.contains(&value.as_str()))
        .unwrap_or_else(|| input.mime_type.clone());
    Ok(UploadImageResponse {
        provider: UploadProvider::ImageBed.code().to_owned(),
        object_key,
        download_url,
        share_url,
        delete_url,
        mime_type,
        size_bytes,
    })
}

async fn upload_to_local(
    record: &AdminUploadConfigRecord,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let root = record
        .local_root
        .as_deref()
        .ok_or_else(|| AppError::Validation("local_root is not configured".to_owned()))?;
    let base_url = record
        .public_base_url
        .as_deref()
        .ok_or_else(|| AppError::Validation("public_base_url is not configured".to_owned()))?;
    let object_key = generated_upload_object_key(record.key_prefix.as_deref(), &input.mime_type);
    let path = PathBuf::from(root).join(&object_key);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| AppError::Internal("failed to create upload directory".to_owned()))?;
    }
    tokio::fs::write(&path, &input.bytes)
        .await
        .map_err(|_| AppError::Internal("failed to write upload file".to_owned()))?;
    Ok(UploadImageResponse {
        provider: UploadProvider::Local.code().to_owned(),
        download_url: join_upload_public_url(base_url, &object_key),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn upload_to_s3(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let access_key =
        decrypt_required_upload_secret(record.access_key_ciphertext.as_deref(), key, "access key")?;
    let secret_key =
        decrypt_required_upload_secret(record.secret_key_ciphertext.as_deref(), key, "secret key")?;
    let bucket = record
        .bucket
        .as_deref()
        .ok_or_else(|| AppError::Validation("bucket is not configured".to_owned()))?;
    let region = record
        .region
        .as_deref()
        .ok_or_else(|| AppError::Validation("region is not configured".to_owned()))?;
    let endpoint = record
        .endpoint
        .clone()
        .unwrap_or_else(|| format!("https://s3.{region}.amazonaws.com"));
    let object_key = generated_upload_object_key(record.key_prefix.as_deref(), &input.mime_type);
    let url = join_upload_endpoint_path(&endpoint, &[bucket, &object_key])?;
    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|_| AppError::Validation("s3 endpoint is invalid".to_owned()))?;
    let host = upload_url_host(&parsed_url)?;
    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let payload_hash = sha256_hex(&input.bytes);
    let canonical_uri = parsed_url.path();
    let canonical_request = format!(
        "PUT\n{canonical_uri}\n\ncontent-type:{}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n\ncontent-type;host;x-amz-content-sha256;x-amz-date\n{payload_hash}",
        input.mime_type
    );
    let scope = format!("{date}/{region}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = s3_upload_signature(&secret_key, &date, region, &string_to_sign);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature={signature}"
    );
    let response = reqwest::Client::new()
        .put(&url)
        .header("content-type", &input.mime_type)
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-date", amz_date)
        .header("authorization", authorization)
        .body(input.bytes.clone())
        .send()
        .await
        .map_err(|_| AppError::Validation("s3 upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "s3 upload failed with status {}",
            response.status().as_u16()
        )));
    }
    Ok(UploadImageResponse {
        provider: UploadProvider::S3.code().to_owned(),
        download_url: record
            .public_base_url
            .as_deref()
            .map(|base| join_upload_public_url(base, &object_key))
            .unwrap_or(url),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

async fn upload_to_oss(
    record: &AdminUploadConfigRecord,
    key: Option<&str>,
    input: &UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let access_key =
        decrypt_required_upload_secret(record.access_key_ciphertext.as_deref(), key, "access key")?;
    let secret_key =
        decrypt_required_upload_secret(record.secret_key_ciphertext.as_deref(), key, "secret key")?;
    let endpoint = record
        .endpoint
        .as_deref()
        .ok_or_else(|| AppError::Validation("oss endpoint is not configured".to_owned()))?;
    let bucket = record
        .bucket
        .as_deref()
        .ok_or_else(|| AppError::Validation("bucket is not configured".to_owned()))?;
    let object_key = generated_upload_object_key(record.key_prefix.as_deref(), &input.mime_type);
    let url = join_upload_endpoint_path(endpoint, &[bucket, &object_key])?;
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let canonical_resource = format!("/{bucket}/{object_key}");
    let string_to_sign = format!("PUT\n\n{}\n{date}\n{canonical_resource}", input.mime_type);
    let signature = hmac_sha1_base64(secret_key.as_bytes(), &string_to_sign);
    let authorization = format!("OSS {access_key}:{signature}");
    let response = reqwest::Client::new()
        .put(&url)
        .header("date", date)
        .header("content-type", &input.mime_type)
        .header("authorization", authorization)
        .body(input.bytes.clone())
        .send()
        .await
        .map_err(|_| AppError::Validation("oss upload failed".to_owned()))?;
    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "oss upload failed with status {}",
            response.status().as_u16()
        )));
    }
    Ok(UploadImageResponse {
        provider: UploadProvider::Oss.code().to_owned(),
        download_url: record
            .public_base_url
            .as_deref()
            .map(|base| join_upload_public_url(base, &object_key))
            .unwrap_or(url),
        share_url: None,
        delete_url: None,
        object_key,
        mime_type: input.mime_type.clone(),
        size_bytes: input.bytes.len() as u64,
    })
}

fn decrypt_required_upload_secret(
    ciphertext: Option<&str>,
    key: Option<&str>,
    field: &str,
) -> AppResult<String> {
    let ciphertext =
        ciphertext.ok_or_else(|| AppError::Validation(format!("{field} is not configured")))?;
    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    decrypt_optional_secret(Some(ciphertext), key)?
        .ok_or_else(|| AppError::Validation(format!("{field} is not configured")))
}

#[derive(Debug, Deserialize)]
struct ImageBedUploadResponse {
    success: Option<bool>,
    file: Option<ImageBedFile>,
    links: ImageBedLinks,
}

#[derive(Debug, Deserialize)]
struct ImageBedFile {
    id: Option<String>,
    size: Option<u64>,
    #[serde(rename = "type")]
    file_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageBedLinks {
    download: Option<String>,
    share: Option<String>,
    delete: Option<String>,
}
