use super::*;
use crate::modules::events::{infrastructure::insert_event_in_tx, user_created_outbox_event};
use chrono::Utc;

pub(crate) async fn list_admin_users(
    pool: Option<Pool<MySql>>,
    query: AdminUserQuery,
) -> AppResult<AdminUsersResponse> {
    let email = query.email.and_then(optional_string);
    let status = query.status.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let (users, total) = list_admin_users_from_store(
        &pool,
        AdminUserListFilter {
            user_id: query.user_id,
            email,
            status,
            include_internal: query.include_internal.unwrap_or(false),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminUsersResponse { users, total })
}

pub(crate) async fn get_admin_user(
    pool: Option<Pool<MySql>>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_user_from_store(&pool, user_id).await
}

pub(crate) async fn create_admin_user(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAdminUserRequest,
) -> AppResult<AdminUserResponse> {
    validate_create_admin_user_request(&request)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let email = request.email.and_then(optional_string);
    let phone = request.phone.and_then(optional_string);
    let status = request
        .status
        .as_deref()
        .map(validate_user_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let kyc_level = request.kyc_level.unwrap_or(0);
    let password_hash = hash_admin_user_password(&request.password)?;
    let pool = admin_mysql_pool(pool)?;

    // 用户创建、邀请码生成和后台审计同事务提交，避免出现无邀请码或无审计的新用户。
    let mut tx = pool.begin().await?;
    let user_id = insert_admin_user_in_tx(
        &mut tx,
        AdminUserInsert {
            email,
            phone,
            password_hash,
            status,
            kyc_level,
        },
    )
    .await?;
    create_user_invite_code_in_tx(&mut tx, user_id).await?;
    insert_event_in_tx(&mut tx, &user_created_outbox_event(user_id, Utc::now())).await?;
    let user = load_admin_user_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user.create",
            target_type: "user",
            target_id: user.id,
            before_json: None,
            after_json: Some(user_audit_json(&user)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(user)
}

pub(crate) async fn update_admin_user_status(
    state: AppState,
    admin_id: u64,
    user_id: u64,
    request: UpdateUserStatusRequest,
) -> AppResult<AdminUserResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let status = validate_user_status(&request.status)?;
    let pool = admin_mysql_pool(state.mysql.clone())?;

    // 状态变更、刷新令牌吊销和审计同事务提交，避免被封禁账号继续用旧令牌续期。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let before = load_admin_user_in_tx(&mut tx, user_id).await?;
    update_admin_user_status_in_tx(&mut tx, user_id, &status).await?;
    let disabled = status != "active";
    if disabled {
        revoke_user_refresh_tokens_in_tx(&mut tx, user_id).await?;
    }
    let after = load_admin_user_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user.status.update",
            target_type: "user",
            target_id: user_id,
            before_json: Some(user_audit_json(&before)),
            after_json: Some(user_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;

    if disabled {
        revoke_actor_auth_sessions(
            &state,
            &AuthActor::new(ActorType::User, user_id, Some(user_id)),
        )
        .await?;
    }
    Ok(after)
}

pub(crate) async fn recharge_admin_user_wallet(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    user_id: u64,
    request: AdminUserRechargeRequest,
) -> AppResult<AdminUserRechargeResponse> {
    validate_admin_user_recharge(&request)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let recharge_id = Uuid::now_v7().to_string();

    // 后台人工充值必须把余额更新、钱包流水和审计写入放在同一事务中。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let asset = load_active_asset_symbol_in_tx(&mut tx, request.asset_id).await?;
    credit_admin_wallet_available_in_tx(
        &mut tx,
        user_id,
        request.asset_id,
        &request.amount,
        "admin_recharge",
        "admin_recharge",
        &recharge_id,
    )
    .await?;
    let wallet = lock_or_create_admin_wallet_row_in_tx(&mut tx, user_id, request.asset_id).await?;
    let response = AdminUserRechargeResponse {
        recharge_id,
        user_id,
        asset_id: request.asset_id,
        asset_symbol: asset.symbol,
        amount: request.amount,
        available: wallet.available,
        frozen: wallet.frozen,
        locked: wallet.locked,
    };
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "wallet.recharge",
            target_type: "wallet_account",
            target_id: user_id,
            before_json: None,
            after_json: Some(recharge_audit_json(&response)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}

pub(crate) async fn get_admin_kyc_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<KycConfigResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_kyc_config_from_kyc(&pool).await
}

pub(crate) async fn save_admin_kyc_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SaveKycConfigRequest,
) -> AppResult<KycConfigResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // KYC 配置变更和后台审计同事务提交，避免审核规则生效后缺少追溯记录。
    let mut tx = pool.begin().await?;
    let change = save_kyc_config_in_tx_from_kyc(&mut tx, admin_id, request).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "kyc.config.update",
            target_type: "kyc_config",
            target_id: change.after.id,
            before_json: Some(kyc_config_audit_json(&change.before)),
            after_json: Some(kyc_config_audit_json(&change.after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(change.after)
}

pub(crate) async fn list_admin_kyc_submissions(
    pool: Option<Pool<MySql>>,
    query: AdminKycSubmissionQuery,
) -> AppResult<KycSubmissionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (submissions, total) = list_kyc_submissions_from_kyc(
        &pool,
        ListKycSubmissionsFilter {
            user_id: query.user_id,
            email: query.email,
            status: query.status,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(KycSubmissionsResponse { submissions, total })
}

pub(crate) async fn get_admin_kyc_submission(
    pool: Option<Pool<MySql>>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_kyc_submission_from_kyc(&pool, submission_id).await
}

pub(crate) async fn review_admin_kyc_submission(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    submission_id: u64,
    request: ReviewKycSubmissionRequest,
) -> AppResult<KycSubmissionResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 审核结果、用户 KYC 等级和后台审计必须同事务完成，避免审批状态与用户等级不一致。
    let mut tx = pool.begin().await?;
    let change =
        review_kyc_submission_in_tx_from_kyc(&mut tx, submission_id, admin_id, request).await?;
    let action = if change.after.status == "approved" {
        "kyc.submission.approve"
    } else {
        "kyc.submission.reject"
    };
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "user_kyc_submission",
            target_id: submission_id,
            before_json: Some(kyc_submission_audit_json(&change.before)),
            after_json: Some(kyc_submission_audit_json(&change.after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(change.after)
}

pub(crate) async fn reset_admin_user_two_factor(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    user_id: u64,
    request: ResetUserTwoFactorRequest,
) -> AppResult<AdminUserTwoFactorResetResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 用户存在校验、2FA 重置和后台审计同事务完成，避免审计记录与实际重置状态不一致。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let before = load_admin_user_two_factor_in_tx(&mut tx, user_id).await?;
    let after = reset_admin_user_two_factor_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user_2fa.reset",
            target_type: "user_two_factor",
            target_id: user_id,
            before_json: Some(two_factor_audit_json(&before)),
            after_json: Some(two_factor_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(AdminUserTwoFactorResetResponse {
        user_id,
        totp_enabled: after.totp_enabled,
        login_2fa_enabled: after.login_2fa_enabled,
    })
}
