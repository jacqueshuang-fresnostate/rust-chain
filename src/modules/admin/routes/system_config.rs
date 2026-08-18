//! 承载运营仪表盘、后台审计日志、SMTP 发信配置、对象存储上传配置与平台品牌的 HTTP 传输入口。
//!
//! 本组入口的共同点是都涉及带密文的系统级配置：SMTP 与上传配置的凭据在库中以密文保存，
//! 路由从运行配置取出凭据加密密钥并向下传递，响应侧一律只回掩码和「是否已设置」标记，不回显明文。
//! SMTP 同时存在「兼容默认配置」和「具名多配置」两套路径，前者维持历史单例语义，后者支持按策略选择发信通道。
//! 图片上传是唯一使用 multipart 的入口，并单独放宽了请求体大小上限；仪表盘则会额外汇集行情监督器运行快照。

use super::*;

/// 构建仪表盘、审计、SMTP、上传及平台品牌配置的后台传输路由。
///
/// 管理员 subject 继续作为审计主体传入应用层；SMTP sender 与凭据加密键只做依赖转发。
/// 图片上传维持独立的请求体大小上限，并由表现层解析 multipart 的精确 `file` 字段；
/// 解析或应用错误均沿原有 `AppError` 到 HTTP 的映射返回。
/// 注意 `/smtp/config` 与 `/smtp/configs` 是两套并存的资源：前者操作兼容用途的默认单例配置，
/// 后者管理具名配置集合，二者的写入口分别落到不同用例，不要相互替代。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(get_admin_dashboard))
        .route(
            "/smtp/config",
            get(get_smtp_config).patch(save_smtp_config_route),
        )
        .route(
            "/smtp/configs",
            get(list_smtp_config_route).post(create_smtp_config_route),
        )
        .route("/smtp/configs/:id", patch(update_smtp_config_route))
        .route(
            "/smtp/delivery-settings",
            patch(save_smtp_delivery_settings_route),
        )
        .route("/smtp/test", post(send_smtp_test))
        .route(
            "/upload/config",
            get(get_upload_config).patch(save_upload_config_route),
        )
        .route(
            "/platform/brand",
            get(get_platform_brand).patch(save_platform_brand),
        )
        .route(
            "/uploads/images",
            post(upload_image_route).layer(DefaultBodyLimit::max(MAX_UPLOAD_BODY_SIZE_BYTES)),
        )
        .route("/audit-logs", get(list_admin_audit_logs))
}

/// 处理 GET /smtp/config，读取兼容用途的默认 SMTP 单例配置。
/// 响应类型为 Option，从未配置过时返回 JSON null 而非 404；口令等凭据不会被解密，只以掩码形式出现。
async fn get_smtp_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<SmtpConfigResponse>>> {
    Ok(Json(get_smtp_config_use_case(state.mysql.clone()).await?))
}

/// 处理 GET /smtp/configs，列出全部具名 SMTP 配置并附带当前发信选择策略。
/// 配置列表与投递策略来自两次独立只读查询，不共享事务快照；本入口不解密凭据也不试发邮件。
async fn list_smtp_config_route(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<SmtpConfigListResponse>> {
    Ok(Json(list_smtp_configs_use_case(state.mysql.clone()).await?))
}

/// 处理 POST /smtp/configs，新建一条具名 SMTP 发信配置。
/// 路由把配置里的凭据加密密钥一并传入，应用层据此加密口令后与审计同事务写入；密钥未配置时直接失败。
/// 配置名称必须唯一，重名会返回冲突；创建成功只落库，不会立即用该配置投递任何邮件。
async fn create_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = create_smtp_config_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

/// 处理 PATCH /smtp/configs/:id，修改指定具名 SMTP 配置。
/// 与创建入口共用同一个请求 DTO，但应用层会先锁定目标记录，且凭据字段留空表示沿用已有密文，
/// 因此可以只改主机端口而不必重新提交口令；改名撞上其他配置的名称会返回冲突。
async fn update_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(config_id): Path<u64>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = update_smtp_config_use_case(
        state.mysql.clone(),
        admin_id,
        config_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

/// 处理 PATCH /smtp/config，创建或覆盖兼容用途的默认 SMTP 单例配置。
/// 该入口不接受配置编号，始终作用于同一条默认记录，属于 upsert 而非新增，因此重复提交不会产生多条配置。
/// 凭据字段留空同样表示保留已有密文，配置写入与审计同事务提交，本入口不发送任何邮件。
async fn save_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_smtp_config_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

/// 处理 PATCH /smtp/delivery-settings，保存多个 SMTP 配置之间的发信选择策略。
/// 该入口不涉及任何凭据，因此无需传入加密密钥；应用层锁定策略行后按新策略决定保留还是重置轮询游标。
/// 策略只影响后续发信时挑选哪条配置，不会改动各配置自身的主机、端口或账号。
async fn save_smtp_delivery_settings_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpDeliverySettingsRequest>,
) -> AppResult<Json<SmtpDeliverySettingsResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let settings =
        save_smtp_delivery_settings_use_case(state.mysql.clone(), admin_id, request).await?;
    Ok(Json(settings))
}

/// 处理 POST /smtp/test，用真实 SMTP 通道向指定收件人投递一封测试邮件。
/// 这是本文件唯一会产生外部网络副作用的入口，需要同时注入凭据加密密钥和运行时邮件发送器。
/// 请求可指定配置编号，未指定时按当前发信策略挑选并可能推进轮询游标；投递不可撤回也不具幂等性，重复调用会重复发信。
async fn send_smtp_test(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SendSmtpTestRequest>,
) -> AppResult<Json<SendSmtpTestResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let response = send_smtp_test_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        state.email_sender.clone(),
        request,
    )
    .await?;
    Ok(Json(response))
}

/// 处理 GET /upload/config，读取当前对象存储上传配置。
/// 响应为 Option，未配置时返回 JSON null；访问密钥只以掩码和「是否已设置」标记呈现，不会解密回传。
async fn get_upload_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<UploadConfigResponse>>> {
    Ok(Json(get_upload_config_use_case(state.mysql.clone()).await?))
}

/// 处理 PATCH /upload/config，保存对象存储提供商、端点、桶、容量与允许的 MIME 白名单。
/// 路由传入凭据加密密钥供应用层加密访问密钥；当存储目标未变化时可以留空密钥字段以沿用旧密文，
/// 但一旦改变了目标却没有提交新密钥就会被拒绝，避免出现指向新桶却仍用旧凭据的配置。
async fn save_upload_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveUploadConfigRequest>,
) -> AppResult<Json<UploadConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_upload_config_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

/// 处理 GET /platform/brand，读取平台上下文维护的品牌名称、Logo 与主题配置。
/// 后台在此只做跨上下文只读转发，不缓存也不写审计；返回的是与前台公开接口同源的那份权威配置。
async fn get_platform_brand(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PlatformBrandResponse>> {
    Ok(Json(
        get_platform_brand_use_case(state.mysql.clone()).await?,
    ))
}

/// 处理 PATCH /platform/brand，保存品牌名称、Logo 与主题配置。
/// 校验、持久化与审计边界都由平台配置用例承担，后台侧不额外加锁；返回结果即前台公开接口将读到的规范化配置。
/// 本入口只写数据库配置，不会上传或删除 Logo 图片对象，图片需先通过图片上传入口取得地址。
async fn save_platform_brand(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SavePlatformBrandRequest>,
) -> AppResult<Json<PlatformBrandResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        save_platform_brand_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

/// 在路由级请求体上限内解析唯一 `file` multipart 字段，并把文件内容、管理员审计主体
/// 与加密配置转发给上传用例；字段缺失、媒体解析及存储失败保持既有统一错误映射。
/// 该入口是本文件唯一放宽了请求体大小上限的路由，且上传对象归属固定记为管理员；
/// 每次调用都会在对象存储中生成新对象，因此重复提交同一文件不会去重。
async fn upload_image_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    multipart: Multipart,
) -> AppResult<Json<UploadImageResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let input = multipart_file_input(multipart).await?;
    let response = upload_image_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        input,
    )
    .await?;
    Ok(Json(response))
}

/// 处理 GET /dashboard，返回跨用户、钱包、行情、交易、产品、风险与审计的运营总览。
/// 与其他只读入口不同，这里会先取行情监督器运行快照再交给应用层，与库中保存的行情配置合并出订阅状态。
/// 路由只向用例传递非敏感的 APP_ENV 文本，不透传完整 Settings；
/// 各分项摘要由多条独立查询拼装且不共享事务快照。
/// 任一查询失败会导致整份仪表盘报错，但响应不会包含配置密钥、审计快照或错误堆栈。
async fn get_admin_dashboard(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AdminDashboardResponse>> {
    let runtime = load_market_feed_runtime(&state).await;
    Ok(Json(
        get_admin_dashboard_use_case(state.mysql.clone(), runtime, &state.settings.app_env).await?,
    ))
}

/// 处理 GET /audit-logs，按管理员、动作、目标类型、目标编号和审计时间范围检索后台操作审计。
/// 这是查看其余各入口所写 reason 与 before/after 快照的统一出口，结果按时间倒序分页返回；
/// created_from/created_to 接收 Unix 毫秒并采用包含边界；读取审计本身不会再写一条审计，
/// 因此查询动作不会污染留痕数据。
async fn list_admin_audit_logs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAuditLogsQuery>,
) -> AppResult<Json<AdminAuditLogsResponse>> {
    Ok(Json(
        list_admin_audit_logs_use_case(state.mysql.clone(), query).await?,
    ))
}
