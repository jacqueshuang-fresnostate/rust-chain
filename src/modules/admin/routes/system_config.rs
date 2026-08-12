use super::*;

/// 构建仪表盘、审计、SMTP、上传及平台品牌配置的后台传输路由。
///
/// 管理员 subject 继续作为审计主体传入应用层；SMTP sender 与凭据加密键只做依赖转发。
/// 图片上传维持独立的请求体大小上限，并由表现层解析 multipart 的精确 `file` 字段；
/// 解析或应用错误均沿原有 `AppError` 到 HTTP 的映射返回。
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

async fn get_smtp_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<SmtpConfigResponse>>> {
    Ok(Json(get_smtp_config_use_case(state.mysql.clone()).await?))
}

async fn list_smtp_config_route(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<SmtpConfigListResponse>> {
    Ok(Json(list_smtp_configs_use_case(state.mysql.clone()).await?))
}

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

async fn get_upload_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<UploadConfigResponse>>> {
    Ok(Json(get_upload_config_use_case(state.mysql.clone()).await?))
}

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

async fn get_platform_brand(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PlatformBrandResponse>> {
    Ok(Json(
        get_platform_brand_use_case(state.mysql.clone()).await?,
    ))
}

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

async fn get_admin_dashboard(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AdminDashboardResponse>> {
    let runtime = load_market_feed_runtime(&state).await;
    Ok(Json(
        get_admin_dashboard_use_case(state.mysql.clone(), runtime).await?,
    ))
}

async fn list_admin_audit_logs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAuditLogsQuery>,
) -> AppResult<Json<AdminAuditLogsResponse>> {
    Ok(Json(
        list_admin_audit_logs_use_case(state.mysql.clone(), query).await?,
    ))
}
