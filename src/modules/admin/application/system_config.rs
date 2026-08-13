//! SMTP 发信、对象存储上传、国家配置与平台品牌四组系统级设置的应用用例层。
//!
//! 这里集中处理带密文配置的三条共性规则：凭据留空表示沿用旧密文而非清空；
//! 只有请求确实携带新凭据时才强制要求加密密钥存在；对外响应与审计快照一律只输出掩码和存在标记。
//! SMTP 同时维护「兼容默认单例」与「具名多配置」两套写入口，二者共用同一张表并共同参与发信策略选择。
//! 上传配置额外引入「目标未变才可沿用旧密钥」的判定，切换提供商或换桶换区域都会要求重新提交凭据。
//! 测试发信是本文件唯一会发起外部投递的用例，它刻意先提交审计再发送，因此审计只记录尝试而不记录成败。

use super::*;

/// 读取兼容默认 SMTP 配置并映射为不含密文的可选后台响应。
/// 无配置返回 None 而非未找到；查询不加锁或解密凭据，连接池、模板 JSON 或 SQL 失败返回错误。
pub(crate) async fn get_admin_smtp_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<SmtpConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_smtp_config_from_store(&pool)
        .await?
        .map(smtp_config_response))
}

/// 读取全部 SMTP 配置及当前投递策略，映射为掩码化配置列表和策略响应。
/// 配置与策略通过独立只读查询获取，不共享事务快照；不会解密凭据或发送测试邮件。
/// 策略记录可能尚未初始化，此时回落到按优先级选择且轮询游标为空的默认值，因此响应里的策略字段恒有值。
/// 列表不分页，全量返回所有具名配置，调用方无需处理翻页。
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

/// 校验新的命名 SMTP 配置并加密凭据，在确认名称未占用后创建配置及管理员审计。
/// 配置与审计共用事务；名称冲突、密钥缺失或加密/数据库失败会回滚且不发送邮件。
/// 创建路径没有旧记录可继承，因此账号与口令必须同时提供或同时留空，只给一项会被判为校验错误。
/// 请求确实携带新凭据时才要求加密密钥存在，密钥缺失归为内部错误而非调用方输入问题。
/// 名称唯一性依赖事务内查重与数据库约束共同保证，并发同名创建由后者兜底。
/// 本用例只落库，不会用新配置试发邮件，连通性需另行调用测试入口验证。
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

/// 锁定指定 SMTP 配置，校验名称与字段并替换新凭据或保留空缺字段对应的已有密文。
/// 配置和审计共用事务；记录缺失、名称冲突、加密或 SQL 失败整体回滚且不发送邮件。
/// 与创建路径的关键差异是这里有旧记录可继承：名称和优先级未提交时沿用锁定后的旧值，
/// 账号与口令留空同样表示保留既有密文，因此可以只调整主机端口而无需重新提交凭据。
/// 账号与口令必须同时存在或同时为空的约束在合并旧值之后再判定，避免误伤只改一项的合法更新。
/// 改名撞上其他配置返回冲突，本用例不会顺带调整发信策略或轮询游标。
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

/// 创建或更新兼容的默认 SMTP 配置，空凭据字段保留已有密文并返回脱敏配置响应。
/// 默认配置写入和审计共用事务；密钥缺失、加密或 SQL 失败整体回滚且不发送邮件。
/// 这是为兼容历史单例语义保留的入口：它不接受配置编号，始终作用于固定名称的那一条默认配置，
/// 存在则更新、不存在则新建，因此重复提交不会像具名创建那样堆出多条记录。
/// 审计动作据本次是新建还是更新分别记录，但目标始终是同一条默认配置。
/// 与具名配置共用同一张表，因此默认配置也会出现在具名列表里并参与发信策略选择。
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

/// 校验并保存 SMTP 发信选择策略，在锁定策略行后保留或重置轮询游标并记录审计。
/// 策略与审计在同一事务提交；非法策略或数据库失败整体回滚，不会在此入口发送邮件。
/// 轮询游标的处置取决于目标策略：切到轮询时沿用旧游标以免打断轮转节奏，切到优先级时游标失去意义而被清空。
/// 本入口只决定「下次发信挑哪条配置」，不触碰任何配置自身的主机、账号与口令。
/// 策略行可能尚不存在，此时以默认记录作为审计的前值，写入即完成初始化。
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

/// 校验测试收件人，并按可选配置编号选择 SMTP 配置后发送一封真实测试邮件。
/// 可选连接池和邮件发送器必须已配置；本包装函数不持有数据库事务，重复调用会重复投递，具体配置选择、审计和失败结果由内部发送流程返回。
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

/// 使用调用方提供的邮件发送器校验收件人、加载指定或当前 SMTP 配置并发送真实测试邮件。
/// 指定 config_id 时读取该配置，否则按发信策略选择并可能在内部事务推进轮询游标；随后解密凭据并执行不可幂等的外部投递，配置缺失、解密或发送失败返回错误。
/// 审计刻意在投递之前提交，且结果字段固定写为「已尝试」而非最终成败，
/// 这样即便投递环节抛错或进程中断，也一定留下「谁在什么时候向哪个地址试发过」的痕迹。
/// 代价是审计无法反映投递是否成功，真实结果只体现在本次调用的返回值上。
/// 该入口是本文件唯一发起外部网络请求的用例，投递不可撤回也不具幂等性，重复调用即重复发信。
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

/// 选择并解密一个已启用 SMTP 配置，返回邮件基础设施可直接使用的可选发信参数。
/// 无可用配置返回 None；读取不加锁，但可能使用密钥解密敏感字段，失败不发送邮件也不推进轮询游标。
pub(crate) async fn load_enabled_admin_smtp_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_enabled_admin_smtp_email_config(pool, key).await
}

/// 读取默认上传配置并映射为仅含凭据掩码和“是否已设置”标记的可选后台响应。
/// 无配置返回 None；查询不加锁、不解密密钥或访问存储端，SQL/JSON 解码失败返回错误。
pub(crate) async fn get_admin_upload_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<UploadConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_upload_config_from_store(&pool)
        .await?
        .map(admin_upload_config_response))
}

/// 校验上传提供商、容量、MIME 和端点约束，并加密或按目标不变规则保留已有访问凭据。
/// 锁定配置后将新配置与审计同事务提交；目标变化却未提供新密钥、校验或加密失败均会回滚。
/// 能否沿用旧密文由「提供商相同且端点、桶、区域三项都未变」共同决定，任一项改变都视为换了目标，
/// 旧凭据不再被继承，此时若请求也没带新密钥，启用态配置会因缺凭据而报校验错误。
/// 凭据要求按提供商分支：图床只需 Bearer 令牌，OSS 与 S3 需要访问密钥与私钥成对存在，本地存储不需要凭据。
/// 缺凭据的校验只在配置处于启用态时触发，因此允许先存一份停用的半成品配置再补齐密钥。
/// 切换提供商时另一分支的凭据字段会被显式写空，避免残留上一种存储的密文被后续误用。
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

/// 使用当前启用上传配置保存后台管理员图片，并记录管理员归属、大小、MIME 和对象地址。
/// 包装函数解析后台连接池后把 owner 固定为管理员；对象存储写入先于元数据落库，重复调用会生成新对象，失败语义沿用通用上传流程。
pub(crate) async fn upload_admin_image(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    input: UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let pool = admin_mysql_pool(pool)?;
    upload_image_for_owner(&pool, UploadObjectOwner::Admin(admin_id), key, input).await
}

/// 校验文件后按指定管理员或用户归属上传图片，并将返回地址与对象元数据持久化。
/// 先无锁读取启用配置并执行事务外、非幂等的存储上传，再独立插入 upload_objects；配置/校验/上传失败不落库，元数据插入失败时已上传对象不会在此补偿删除。
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

/// 按规范化国家代码、状态和注册开关筛选国家配置，并返回语言集合的分页结果与总数。
/// 非空代码和状态使用写入同款校验，分页统一裁剪；查询不锁配置或改变注册策略。
/// 复用写入侧校验意味着非法代码或状态会直接报校验错误，而不是当作查不到记录静默返回空列表。
/// 注册开关作为布尔筛选按原值下推，不做归一，未提供时不参与过滤。
/// 响应中的支持语言以数组返回，反映的是配置当前值而非已注册用户的实际语言分布。
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
    let (countries, total) = list_admin_countries_from_store(
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
    Ok(AdminCountriesResponse { countries, total })
}

/// 创建国家注册与语言配置，并返回数据库生成 ID 和时间戳后的完整国家响应。
/// 国家代码、名称、备注、默认/支持语言及可选状态须合法；状态和排序分别缺省为 active 与 0，权限由调用方保证。
/// 事务不锁其他业务行，依次插入国家、回读和写 after 审计；国家代码唯一冲突或任一步失败整体回滚。
/// 创建无幂等键，且不会迁移既有用户或刷新外部地域服务。
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

/// 更新国家名称、备注、语言集合、注册开关和排序，并返回最终国家配置。
/// 请求须提供审计原因；国家代码和状态不在此用例修改，缺省排序沿用锁定旧值。
/// 事务先锁国家，再覆盖配置、回读并写 before/after 审计；记录缺失或 SQL 失败整体回滚。
/// 相同配置重放仍新增审计，不修改已注册用户的语言选择。
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

/// 单独切换国家 active/disabled 状态，并返回更新后的完整国家配置。
/// 请求须含支持的状态和审计原因；本函数不检查该国家已有用户或进行迁移。
/// 事务先锁国家，再更新状态、回读并写 before/after 审计；缺失或数据库失败整体回滚。
/// 相同状态重放仍产生审计，注册读取端会在后续请求中观察新状态。
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

/// 通过平台上下文读取后台品牌名称、Logo 等权威配置并返回平台响应。
/// 本用例只委托跨上下文只读查询，不加锁或写后台审计；底层缺省/失败语义原样返回。
pub(crate) async fn get_admin_platform_brand(
    pool: Option<Pool<MySql>>,
) -> AppResult<PlatformBrandResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_platform_brand_from_platform(&pool).await
}

/// 校验并保存平台品牌名称、Logo 与主题配置，返回公开品牌配置所使用的同一份规范化结果。
/// 持久化和审计边界由平台配置用例负责；输入或数据库失败不产生部分更新，本入口无外部 I/O。
/// 后台在此开启事务并委托平台上下文完成校验与写入，再把其返回的前后值写进后台审计后统一提交，
/// 因此品牌变更与后台留痕要么同时生效要么同时回滚。
/// 返回值与前台公开接口读到的是同一份规范化配置，保证后台所见即用户所见。
/// 本用例只保存图片地址，不上传也不清理对象存储中的旧 Logo 文件。
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

/// 仅在请求确实带来新密钥时才加密替换，否则原样返回既有密文。
/// 判定以去空白后是否非空为准，因此提交空串等同于不修改而不是清空凭据。
/// 需要加密却没有配置加密密钥时归为内部错误，属于服务配置缺失而非调用方输入问题。
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

/// 计算 SMTP 账号与口令的最终密文及账号掩码，是创建、更新与默认配置三条写路径共用的凭据处理。
/// 只有请求确实带来新账号或新口令时才强制要求加密密钥存在，缺失归为内部错误；
/// 否则即便没有密钥也能继续，此时两项凭据直接沿用旧密文，从而允许在无密钥环境下修改非敏感字段。
/// 账号与口令必须同时有值或同时为空，该校验在合并旧密文之后进行，因此只补交其中一项也能通过。
/// 掩码优先按新账号重算，未提交账号时沿用旧掩码，保证响应里的掩码始终与实际密文对应。
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

/// 把已校验的 SMTP 配置、算好的凭据密文与掩码，以及操作人编号组装成落库写入结构。
/// 纯字段搬运，不做任何校验或加密，因此调用前必须先完成校验与凭据准备两步。
/// 单独抽出这层是为了让创建、更新与默认配置三条路径共用同一套字段映射，避免某条路径漏写字段。
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
