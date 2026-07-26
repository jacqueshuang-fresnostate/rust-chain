use super::*;

pub(crate) async fn get_admin_smtp_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<SmtpConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_smtp_config_from_store(&pool)
        .await?
        .map(smtp_config_response))
}

pub(crate) async fn list_admin_smtp_configs(
    pool: Option<Pool<MySql>>,
) -> AppResult<SmtpConfigListResponse> {
    let pool = admin_mysql_pool(pool)?;
    let configs = list_admin_smtp_configs_from_store(&pool)
        .await?
        .into_iter()
        .map(smtp_config_response)
        .collect();
    let delivery_settings =
        smtp_delivery_settings_response(load_admin_smtp_delivery_settings(&pool).await?);
    Ok(SmtpConfigListResponse {
        configs,
        delivery_settings,
    })
}

pub(crate) async fn create_admin_smtp_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_smtp_audit_reason(request.reason.clone())?;
    let config = validate_smtp_save_request(&request, None, Some(DEFAULT_SMTP_CONFIG_PRIORITY))?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    if load_admin_smtp_config_by_name_in_tx(&mut tx, &config.name)
        .await?
        .is_some()
    {
        return Err(AppError::Validation(
            "smtp config name already exists".to_owned(),
        ));
    }
    let (username_ciphertext, password_ciphertext, username_mask) =
        prepare_smtp_secret_fields(&request, None, key)?;
    let config_id = insert_admin_smtp_config_in_tx(
        &mut tx,
        smtp_config_write(
            config,
            username_ciphertext,
            password_ciphertext,
            username_mask,
            admin_id,
        ),
    )
    .await?;
    let after = load_admin_smtp_config_by_id_in_tx(&mut tx, config_id)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.create",
            target_type: "smtp_config",
            target_id: after.id,
            before_json: None,
            after_json: Some(smtp_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub(crate) async fn update_admin_smtp_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    config_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_smtp_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_smtp_config_by_id_in_tx(&mut tx, config_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let config = validate_smtp_save_request(&request, Some(&before.name), Some(before.priority))?;
    if config.name != before.name
        && admin_smtp_config_name_exists_except(&mut tx, &config.name, config_id).await?
    {
        return Err(AppError::Validation(
            "smtp config name already exists".to_owned(),
        ));
    }
    let (username_ciphertext, password_ciphertext, username_mask) =
        prepare_smtp_secret_fields(&request, Some(&before), key)?;
    update_admin_smtp_config_in_tx(
        &mut tx,
        config_id,
        smtp_config_write(
            config,
            username_ciphertext,
            password_ciphertext,
            username_mask,
            admin_id,
        ),
    )
    .await?;
    let after = load_admin_smtp_config_by_id_in_tx(&mut tx, config_id)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.update",
            target_type: "smtp_config",
            target_id: after.id,
            before_json: Some(smtp_config_audit_json(&before)),
            after_json: Some(smtp_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub(crate) async fn save_admin_smtp_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_smtp_audit_reason(request.reason.clone())?;
    let config = validate_smtp_save_request(
        &request,
        Some(DEFAULT_SMTP_CONFIG_NAME),
        Some(DEFAULT_SMTP_CONFIG_PRIORITY),
    )?;
    let config_name = config.name.clone();
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_smtp_config_by_name_in_tx(&mut tx, DEFAULT_SMTP_CONFIG_NAME).await?;
    let (username_ciphertext, password_ciphertext, username_mask) =
        prepare_smtp_secret_fields(&request, before.as_ref(), key)?;

    // SMTP 默认配置和审计同事务提交，避免发信凭证已变化但后台没有操作者记录。
    upsert_default_admin_smtp_config_in_tx(
        &mut tx,
        smtp_config_write(
            config,
            username_ciphertext,
            password_ciphertext,
            username_mask,
            admin_id,
        ),
    )
    .await?;
    let after = load_admin_smtp_config_by_name_in_tx(&mut tx, &config_name)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.save",
            target_type: "smtp_config",
            target_id: after.id,
            before_json: before.as_ref().map(smtp_config_audit_json),
            after_json: Some(smtp_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub(crate) async fn save_admin_smtp_delivery_settings(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SaveSmtpDeliverySettingsRequest,
) -> AppResult<SmtpDeliverySettingsResponse> {
    let reason = required_smtp_audit_reason(request.reason)?;
    let strategy = validate_smtp_delivery_strategy(&request.strategy)?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_smtp_delivery_settings_in_tx(&mut tx).await?;
    upsert_admin_smtp_delivery_settings_in_tx(&mut tx, &strategy, admin_id).await?;
    let after = lock_admin_smtp_delivery_settings_in_tx(&mut tx).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_delivery_settings.save",
            target_type: "smtp_delivery_settings",
            target_id: u64::from(SMTP_DELIVERY_SETTINGS_ID),
            before_json: Some(smtp_delivery_settings_audit_json(&before)),
            after_json: Some(smtp_delivery_settings_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_delivery_settings_response(after))
}

pub(crate) async fn send_admin_smtp_test(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    sender: Option<Arc<dyn EmailSender>>,
    request: SendSmtpTestRequest,
) -> AppResult<SendSmtpTestResponse> {
    let pool = admin_mysql_pool(pool)?;
    let sender =
        sender.ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    send_admin_smtp_test_with_sender(&pool, admin_id, key, sender.as_ref(), request).await
}

pub(crate) async fn send_admin_smtp_test_with_sender(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    sender: &dyn EmailSender,
    request: SendSmtpTestRequest,
) -> AppResult<SendSmtpTestResponse> {
    let reason = required_smtp_audit_reason(request.reason)?;
    let recipient = validate_smtp_email(&request.recipient, "recipient")?;
    let record = match request.config_id {
        Some(config_id) => load_admin_smtp_config_by_id(pool, config_id)
            .await?
            .ok_or(AppError::NotFound)?,
        None => load_admin_smtp_config_for_delivery(pool)
            .await?
            .ok_or(AppError::NotFound)?,
    };
    let smtp = admin_smtp_email_config(&record, key)?;
    let mut tx = pool.begin().await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.test",
            target_type: "smtp_config",
            target_id: record.id,
            before_json: Some(smtp_config_audit_json(&record)),
            after_json: Some(json!({
                "status": "attempted",
                "recipient": recipient.clone(),
                "config": smtp_config_audit_json(&record),
            })),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;

    sender
        .send(
            smtp,
            EmailMessage {
                to: recipient.clone(),
                subject: "SMTP test".to_owned(),
                text_body: "SMTP configuration test email.".to_owned(),
                html_body: None,
            },
        )
        .await?;

    Ok(SendSmtpTestResponse {
        sent: true,
        recipient,
        config_id: record.id,
        config_name: record.name,
    })
}

pub(crate) async fn load_enabled_admin_smtp_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_enabled_admin_smtp_email_config(pool, key).await
}

pub(crate) async fn get_admin_upload_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<UploadConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_upload_config_from_store(&pool)
        .await?
        .map(admin_upload_config_response))
}

pub(crate) async fn save_admin_upload_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveUploadConfigRequest,
) -> AppResult<UploadConfigResponse> {
    let reason = required_upload_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_upload_config_in_tx(&mut tx).await?;
    let config = validate_upload_config(&request)?;
    let existing_same_provider = before
        .as_ref()
        .filter(|record| record.provider == config.provider.code())
        .filter(|record| upload_config_secret_destination_unchanged(record, &config));

    let (bearer_token_ciphertext, bearer_token_mask) = if config.provider.uses_bearer() {
        let existing_ciphertext =
            existing_same_provider.and_then(|record| record.bearer_token_ciphertext.clone());
        let existing_mask =
            existing_same_provider.and_then(|record| record.bearer_token_mask.clone());
        let ciphertext = encrypt_optional_upload_secret(
            key,
            request.bearer_token.as_deref(),
            existing_ciphertext,
        )?;
        let mask = request
            .bearer_token
            .as_deref()
            .and_then(optional_str)
            .map(mask_secret)
            .or(existing_mask);
        if config.enabled && ciphertext.is_none() {
            return Err(AppError::Validation(
                "image bed bearer token is required".to_owned(),
            ));
        }
        (ciphertext, mask)
    } else {
        (None, None)
    };

    let (access_key_ciphertext, access_key_mask, secret_key_ciphertext) =
        if config.provider.uses_access_secret() {
            let existing_access_ciphertext =
                existing_same_provider.and_then(|record| record.access_key_ciphertext.clone());
            let existing_secret_ciphertext =
                existing_same_provider.and_then(|record| record.secret_key_ciphertext.clone());
            let existing_access_mask =
                existing_same_provider.and_then(|record| record.access_key_mask.clone());
            let access_ciphertext = encrypt_optional_upload_secret(
                key,
                request.access_key.as_deref(),
                existing_access_ciphertext,
            )?;
            let secret_ciphertext = encrypt_optional_upload_secret(
                key,
                request.secret_key.as_deref(),
                existing_secret_ciphertext,
            )?;
            let access_mask = request
                .access_key
                .as_deref()
                .and_then(optional_str)
                .map(mask_secret)
                .or(existing_access_mask);
            if config.enabled && (access_ciphertext.is_none() || secret_ciphertext.is_none()) {
                return Err(AppError::Validation(
                    "upload access key and secret key are required".to_owned(),
                ));
            }
            (access_ciphertext, access_mask, secret_ciphertext)
        } else {
            (None, None, None)
        };

    // 上传配置和审计必须同事务提交，避免存储凭证已变更但后台无法追踪操作者。
    upsert_admin_upload_config_in_tx(
        &mut tx,
        AdminUploadConfigWrite {
            provider: config.provider.code().to_owned(),
            endpoint: config.endpoint,
            file_field: config.file_field,
            bearer_token_ciphertext,
            bearer_token_mask,
            access_key_ciphertext,
            access_key_mask,
            secret_key_ciphertext,
            bucket: config.bucket,
            region: config.region,
            public_base_url: config.public_base_url,
            local_root: config.local_root,
            key_prefix: config.key_prefix,
            max_file_size_bytes: config.max_file_size_bytes,
            allowed_mime_types: config.allowed_mime_types,
            enabled: config.enabled,
            updated_by: admin_id,
        },
    )
    .await?;
    let after = load_admin_upload_config_in_tx(&mut tx).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "upload_storage_config.save",
            target_type: "upload_storage_config",
            target_id: after.id,
            before_json: before.as_ref().map(upload_config_audit_json),
            after_json: Some(upload_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(admin_upload_config_response(after))
}

pub(crate) async fn upload_admin_image(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    input: UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let pool = admin_mysql_pool(pool)?;
    upload_image_for_owner(&pool, UploadObjectOwner::Admin(admin_id), key, input).await
}

pub(crate) async fn upload_image_for_owner(
    pool: &Pool<MySql>,
    owner: UploadObjectOwner,
    key: Option<&str>,
    input: UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let config = load_enabled_admin_upload_config(pool)
        .await?
        .ok_or_else(|| AppError::Validation("upload storage is not enabled".to_owned()))?;
    let response = upload_admin_file_to_storage(&config, key, &input).await?;
    let original_filename =
        safe_upload_filename(input.original_filename.as_deref(), &input.mime_type);
    insert_admin_upload_object(
        pool,
        AdminUploadObjectWrite {
            provider: response.provider.clone(),
            object_key: response.object_key.clone(),
            public_url: response.download_url.clone(),
            share_url: response.share_url.clone(),
            delete_url: response.delete_url.clone(),
            mime_type: response.mime_type.clone(),
            size_bytes: response.size_bytes,
            original_filename,
            owner,
        },
    )
    .await?;
    Ok(response)
}

pub(crate) async fn list_admin_countries(
    pool: &Pool<MySql>,
    query: AdminCountriesQuery,
) -> AppResult<AdminCountriesResponse> {
    let country_code = query
        .country_code
        .and_then(optional_string)
        .map(|value| validate_country_code(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_country_status(&value))
        .transpose()?;
    let countries = list_admin_countries_from_store(
        pool,
        AdminCountryListFilter {
            country_code,
            status,
            registration_enabled: query.registration_enabled,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminCountriesResponse { countries })
}

pub(crate) async fn create_admin_country(
    pool: &Pool<MySql>,
    admin_id: u64,
    request: CreateAdminCountryRequest,
) -> AppResult<AdminCountryResponse> {
    let country_code = validate_country_code(&request.country_code)?;
    let country_name = validate_country_name(&request.country_name)?;
    let remark = validate_country_remark(&request.remark)?;
    let (default_locale, supported_locales) =
        validate_country_locale_config(&request.default_locale, request.supported_locales)?;
    let status = request
        .status
        .as_deref()
        .map(validate_country_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let sort_order = request.sort_order.unwrap_or(0);

    // 国家配置和后台审计必须同事务提交，避免配置已生效但审计日志缺失。
    let mut tx = pool.begin().await?;
    let country_id = insert_admin_country_in_tx(
        &mut tx,
        AdminCountryInsert {
            country_code,
            country_name,
            remark,
            default_locale,
            supported_locales,
            registration_enabled: request.registration_enabled,
            status,
            sort_order,
        },
    )
    .await?;
    let country = load_admin_country_in_tx(&mut tx, country_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "country_config.create",
            target_type: "country_config",
            target_id: country.id,
            before_json: None,
            after_json: Some(country_config_audit_json(&country)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(country)
}

pub(crate) async fn update_admin_country(
    pool: &Pool<MySql>,
    admin_id: u64,
    country_id: u64,
    request: UpdateAdminCountryRequest,
) -> AppResult<AdminCountryResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let country_name = validate_country_name(&request.country_name)?;
    let remark = validate_country_remark(&request.remark)?;
    let (default_locale, supported_locales) =
        validate_country_locale_config(&request.default_locale, request.supported_locales)?;

    // 先锁定旧值再更新，确保审计 before/after 与本次写入完全对应。
    let mut tx = pool.begin().await?;
    let before = lock_admin_country_in_tx(&mut tx, country_id).await?;
    let sort_order = request.sort_order.unwrap_or(before.sort_order);
    update_admin_country_in_tx(
        &mut tx,
        country_id,
        AdminCountryUpdate {
            country_name,
            remark,
            default_locale,
            supported_locales,
            registration_enabled: request.registration_enabled,
            sort_order,
        },
    )
    .await?;
    let after = load_admin_country_in_tx(&mut tx, country_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "country_config.update",
            target_type: "country_config",
            target_id: after.id,
            before_json: Some(country_config_audit_json(&before)),
            after_json: Some(country_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_country_status(
    pool: &Pool<MySql>,
    admin_id: u64,
    country_id: u64,
    request: UpdateAdminCountryStatusRequest,
) -> AppResult<AdminCountryResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let status = validate_country_status(&request.status)?;

    // 状态切换也写审计，后台可追踪每一次启用/禁用操作。
    let mut tx = pool.begin().await?;
    let before = lock_admin_country_in_tx(&mut tx, country_id).await?;
    update_admin_country_status_in_tx(&mut tx, country_id, &status).await?;
    let after = load_admin_country_in_tx(&mut tx, country_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "country_config.status.update",
            target_type: "country_config",
            target_id: after.id,
            before_json: Some(country_config_audit_json(&before)),
            after_json: Some(country_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn get_admin_platform_brand(
    pool: Option<Pool<MySql>>,
) -> AppResult<PlatformBrandResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_platform_brand_from_platform(&pool).await
}

pub(crate) async fn save_admin_platform_brand(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SavePlatformBrandRequest,
) -> AppResult<PlatformBrandResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 平台品牌配置会影响前后台展示，配置变更和后台审计需要同事务提交。
    let mut tx = pool.begin().await?;
    let change = save_platform_brand_in_tx_from_platform(&mut tx, admin_id, request).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "platform_brand.update",
            target_type: "platform_brand_config",
            target_id: change.after.id,
            before_json: Some(platform_brand_audit_json(&change.before)),
            after_json: Some(platform_brand_audit_json(&change.after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(change.after)
}

fn encrypt_optional_upload_secret(
    key: Option<&str>,
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
) -> AppResult<Option<String>> {
    if new_value.and_then(optional_str).is_some() {
        let key = key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
        encrypt_secret_field(key, new_value, existing_ciphertext)
    } else {
        Ok(existing_ciphertext)
    }
}

fn prepare_smtp_secret_fields(
    request: &SaveSmtpConfigRequest,
    before: Option<&AdminSmtpConfigRecord>,
    key: Option<&str>,
) -> AppResult<(Option<String>, Option<String>, Option<String>)> {
    let needs_key = smtp_request_has_new_secret(request);
    let key = if needs_key {
        Some(key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?)
    } else {
        key
    };

    let username_ciphertext = match key {
        Some(key) => encrypt_secret_field(
            key,
            request.username.as_deref(),
            before.and_then(|record| record.username_ciphertext.clone()),
        )?,
        None => before.and_then(|record| record.username_ciphertext.clone()),
    };
    let password_ciphertext = match key {
        Some(key) => encrypt_secret_field(
            key,
            request.password.as_deref(),
            before.and_then(|record| record.password_ciphertext.clone()),
        )?,
        None => before.and_then(|record| record.password_ciphertext.clone()),
    };
    if username_ciphertext.is_some() != password_ciphertext.is_some() {
        return Err(AppError::Validation(
            "smtp username and password must be configured together".to_owned(),
        ));
    }
    let username_mask = request
        .username
        .as_deref()
        .and_then(optional_str)
        .map(mask_secret)
        .or_else(|| before.and_then(|record| record.username_mask.clone()));

    Ok((username_ciphertext, password_ciphertext, username_mask))
}

fn smtp_config_write(
    config: SmtpValidatedConfig,
    username_ciphertext: Option<String>,
    password_ciphertext: Option<String>,
    username_mask: Option<String>,
    admin_id: u64,
) -> AdminSmtpConfigWrite {
    AdminSmtpConfigWrite {
        name: config.name,
        host: config.host,
        port: config.port,
        security: config.security,
        username_ciphertext,
        password_ciphertext,
        username_mask,
        from_email: config.from_email,
        from_name: config.from_name,
        verification_code_template_html: config.verification_code_template_html,
        verification_code_templates: config.verification_code_templates,
        enabled: config.enabled,
        priority: config.priority,
        updated_by: admin_id,
    }
}
