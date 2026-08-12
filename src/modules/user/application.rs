//! user bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    infra::email::verification_code_email_message,
    infra::secrets::{decrypt_secret, encrypt_secret},
    modules::{
        admin::{
            application::{load_enabled_admin_smtp_config, upload_image_for_owner},
            presentation::UploadFileInput,
            repository::UploadObjectOwner,
        },
        auth::{
            ActorType, AuthActor, AuthService, MySqlAuthRepository, hash_password,
            infrastructure::RedisProjectRefreshTokenRepository, normalize_username,
            revoke_actor_auth_sessions, verify_password,
        },
        kyc::{
            KycStatusResponse, KycSubmissionResponse, SubmitKycRequest,
            create_user_kyc_submission_in_tx, kyc_submission_audit_json, latest_kyc_submission,
            load_kyc_config,
        },
        security::{
            LoginTwoFactorMode, confirm_user_totp, generate_totp_secret, load_security_policy,
            load_user_two_factor, reset_user_two_factor, save_pending_totp_secret,
            set_user_login_two_factor, totp_otpauth_uri, verify_totp_code,
        },
        user::{
            domain::email_verification_is_expired,
            infrastructure::{
                ensure_active_agent_in_tx, ensure_active_user_in_tx, ensure_email_available_in_tx,
                ensure_email_verification_not_cooling_down_in_tx,
                ensure_fund_password_exists_in_tx, ensure_user_exists, ensure_user_exists_in_tx,
                increment_email_verification_attempt_count_in_tx,
                increment_invite_code_used_count_in_tx, insert_pending_email_verification_in_tx,
                insert_user_audit_event_in_tx, insert_user_referral_in_tx,
                list_direct_invited_users, list_user_third_party_bindings,
                load_referral_link_in_tx, load_user_account_label, load_user_invite_code,
                load_user_profile, load_user_referral_in_tx, lock_active_invite_code_in_tx,
                lock_active_user_username_in_tx, lock_fund_password_hash_in_tx,
                lock_latest_pending_email_verification_in_tx, lock_user_password_in_tx,
                lock_user_referral_in_tx, lock_verified_user_email_in_tx,
                mark_email_verification_verified_in_tx, revoke_user_refresh_tokens_in_tx,
                supersede_pending_email_verifications_in_tx, update_fund_password_hash_in_tx,
                update_user_avatar_url, update_user_bound_email_in_tx,
                update_user_password_hash_in_tx, update_user_username_in_tx,
                upsert_fund_password_hash_in_tx, upsert_user_third_party_binding_in_tx,
                write_user_invite_code,
            },
            presentation::{
                BindEmailCodeResponse, BindEmailResponse, FundPasswordResponse, MyInvitesResponse,
                ReferralBindingResponse, ReferralCodeResponse, SetupTwoFactorResponse,
                ThirdPartyBindingStatusResponse, TokenResponse, UpdateUsernameResponse,
                UserAvatarResponse, UserProfileResponse, UserTwoFactorStatusResponse,
            },
            service::{
                EMAIL_BIND_PURPOSE, EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS,
                EMAIL_VERIFICATION_CODE_TTL_MINUTES, FUND_PASSWORD_RESET_PURPOSE,
                TWO_FACTOR_RESET_PURPOSE, USER_INVITE_CODE_CREATE_ATTEMPTS, generate_email_code,
                generate_user_invite_code, is_third_party_binding_enabled,
                is_valid_user_invite_code, normalize_invite_code,
                normalize_third_party_display_name, normalize_third_party_provider, validate_email,
                validate_email_code, validate_fund_password, validate_login_password,
                validate_third_party_identifier,
            },
        },
    },
    state::AppState,
};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::{MySql, Pool};
use std::sync::Arc;

/// 按认证用户 ID 查询个人资料、国家信息与资金密码设置状态，未命中返回未授权。
/// 响应含邮箱、手机号等本人资料，只可返回当前认证用户，不得跨账号缓存或写普通日志。
pub(crate) async fn get_user_profile(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserProfileResponse> {
    load_user_profile(pool, user_id).await
}

/// 规范化登录用户名后锁定活跃用户，同事务更新唯一值并记录审计。
/// 重复用户名映射为冲突；任一写入失败整体回滚，成功后返回已转小写的权威值。
pub(crate) async fn update_user_username(
    pool: &Pool<MySql>,
    user_id: u64,
    raw_username: String,
) -> AppResult<UpdateUsernameResponse> {
    let username = normalize_username(&raw_username)?;
    let mut tx = pool.begin().await?;
    let before_username = lock_active_user_username_in_tx(&mut tx, user_id).await?;

    update_user_username_in_tx(&mut tx, user_id, &username).await?;
    // 用户名是登录标识，不是昵称；审计记录必须保留修改前后值，方便追踪账号归属变化。
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.username.update",
        "user",
        user_id.to_string(),
        Some(json!({ "username": before_username })),
        Some(json!({ "username": username.clone() })),
    )
    .await?;
    tx.commit().await?;

    Ok(UpdateUsernameResponse { username })
}

/// 先经上传服务写入文件/对象元数据，再将返回 URL 更新到用户资料；本函数未预查用户是否存在。
/// 用户无效或资料更新失败时，外部对象及上传记录可能已经存在且不会补偿删除；重试可能再次上传。
pub(crate) async fn upload_user_avatar(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
    input: UploadFileInput,
) -> AppResult<UserAvatarResponse> {
    let upload = upload_image_for_owner(
        pool,
        UploadObjectOwner::User(user_id),
        state.settings.exposed_credential_encryption_key(),
        input,
    )
    .await?;
    let avatar_url = upload.download_url.clone();
    update_user_avatar_url(pool, user_id, &avatar_url).await?;

    Ok(UserAvatarResponse { avatar_url, upload })
}

/// 确认用户存在后读取当前 KYC 配置与最新申请，无申请时返回未提交状态。
/// 最新申请包含本人未掩码证件号和材料地址，仅可返回给当前认证用户；调用方不得写日志或跨用户复用。
/// 配置缺失时读取流程会先写入默认配置，因此该状态用例并非严格只读。
pub(crate) async fn get_user_kyc_status(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<KycStatusResponse> {
    ensure_user_exists(pool, user_id).await?;
    let config = load_kyc_config(pool).await?;
    let latest_submission = latest_kyc_submission(pool, user_id).await?;
    Ok(KycStatusResponse {
        config,
        latest_submission,
    })
}

/// 在单一事务中校验并创建用户 KYC 申请，同时写入不含证件原文的审计事件。
/// 用户状态、既有待审申请或材料校验失败不提交；用户行与查得的待审行会锁至提交，
/// 但申请表未由本函数声明“每用户唯一待审”约束，防重依赖当前事务隔离和锁查询。
pub(crate) async fn submit_user_kyc_submission(
    pool: &Pool<MySql>,
    user_id: u64,
    request: SubmitKycRequest,
) -> AppResult<KycSubmissionResponse> {
    let mut tx = pool.begin().await?;
    let submission = create_user_kyc_submission_in_tx(&mut tx, user_id, request).await?;
    // KYC 材料包含敏感身份信息，审计只记录脱敏摘要，避免日志和审计表扩散证件号。
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.kyc.submit",
        "user_kyc_submission",
        submission.id.to_string(),
        None,
        Some(kyc_submission_audit_json(&submission)),
    )
    .await?;
    tx.commit().await?;
    Ok(submission)
}

/// 读取用户最早的自有邀请码；缺失或格式失效时在限定次数内生成全局唯一 code 并回读。
/// 写入与回读不是事务，数据库只以 code 冲突触发重试；并发首次调用可能各自插入记录，
/// 本函数最终仍返回排序最早的一条。随机冲突重试用尽返回内部错误。
pub(crate) async fn get_user_referral_code(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<ReferralCodeResponse> {
    ensure_user_exists(pool, user_id).await?;

    if let Some(code) = load_user_invite_code(pool, user_id).await? {
        if is_valid_user_invite_code(&code.code) {
            return Ok(code);
        }
        write_unique_user_invite_code(pool, user_id, Some(code.id)).await?;
    } else {
        write_unique_user_invite_code(pool, user_id, None).await?;
    }

    load_user_invite_code(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Internal("failed to create user invite code".to_owned()))
}

async fn write_unique_user_invite_code(
    pool: &Pool<MySql>,
    user_id: u64,
    existing_code_id: Option<u64>,
) -> AppResult<()> {
    for _ in 0..USER_INVITE_CODE_CREATE_ATTEMPTS {
        let code = generate_user_invite_code()?;
        if write_user_invite_code(pool, user_id, existing_code_id, &code).await? {
            return Ok(());
        }
    }

    Err(AppError::Internal(
        "failed to create unique user invite code".to_owned(),
    ))
}

/// 将当前用户一次性绑定到有效邀请码，建立直接邀请人与根代理归属关系。
/// 用户和邀请码持有人须有效；禁止自邀，且邀请码未达到使用上限，代理链上的根代理须启用。
/// 事务先锁定用户既有绑定和邀请码，再插入邀请链并递增使用次数，二者必须原子提交。
/// 代理树归属与用户邀请路径同时保存，路径深度必须在邀请人链上递增，避免返佣归属漂移。
/// 已绑定用户直接返回原绑定且不重复计数；并发首绑由锁和唯一约束保证，失败整体回滚。
pub(crate) async fn bind_user_referral_code(
    pool: &Pool<MySql>,
    user_id: u64,
    raw_code: String,
) -> AppResult<ReferralBindingResponse> {
    let code = normalize_invite_code(&raw_code)?;
    let mut tx = pool.begin().await?;

    ensure_user_exists_in_tx(&mut tx, user_id).await?;
    if let Some(existing) = lock_user_referral_in_tx(&mut tx, user_id).await? {
        tx.commit().await?;
        return Ok(existing);
    }

    let invite = lock_active_invite_code_in_tx(&mut tx, &code).await?;
    if invite
        .usage_limit
        .is_some_and(|usage_limit| invite.used_count >= usage_limit)
    {
        return Err(AppError::Validation("invite code is exhausted".to_owned()));
    }

    // 代理树决定公司归属，用户邀请链只记录具体介绍人；两条关系必须同时保留。
    let (direct_inviter_type, direct_inviter_id, root_agent_id, depth, path) =
        match invite.owner_type.as_str() {
            "agent" => {
                ensure_active_agent_in_tx(&mut tx, invite.owner_id).await?;
                (
                    "agent".to_owned(),
                    invite.owner_id,
                    Some(invite.owner_id),
                    1,
                    format!("/agent:{}/user:{}", invite.owner_id, user_id),
                )
            }
            "user" => {
                if invite.owner_id == user_id {
                    return Err(AppError::Validation(
                        "user cannot bind own invite code".to_owned(),
                    ));
                }
                ensure_active_user_in_tx(&mut tx, invite.owner_id).await?;
                let inviter = load_referral_link_in_tx(&mut tx, invite.owner_id).await?;
                if let Some(owner_agent_id) = inviter.root_agent_id {
                    ensure_active_agent_in_tx(&mut tx, owner_agent_id).await?;
                }
                (
                    "user".to_owned(),
                    invite.owner_id,
                    inviter.root_agent_id,
                    inviter.depth + 1,
                    format!("{}/user:{}", inviter.path, user_id),
                )
            }
            _ => {
                return Err(AppError::Validation(
                    "unsupported invite code owner".to_owned(),
                ));
            }
        };

    insert_user_referral_in_tx(
        &mut tx,
        user_id,
        direct_inviter_id,
        &direct_inviter_type,
        root_agent_id,
        depth,
        &path,
    )
    .await?;
    increment_invite_code_used_count_in_tx(&mut tx, invite.id).await?;

    let binding = load_user_referral_in_tx(&mut tx, user_id).await?;
    tx.commit().await?;

    Ok(binding)
}

/// 列出由当前用户直接邀请的用户，不包含间接下级或代理公司其他成员。
pub(crate) async fn list_user_invites(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<MyInvitesResponse> {
    let users = list_direct_invited_users(pool, user_id).await?;
    Ok(MyInvitesResponse { users })
}

/// 为待绑定邮箱生成验证码，同事务检查唯一占用、冷却并取代旧码。
/// 哈希记录提交后才发信；SMTP 失败不回滚已存记录，明文验证码始终不入库。
pub(crate) async fn send_user_email_bind_code(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
    raw_email: String,
) -> AppResult<BindEmailCodeResponse> {
    let email = validate_email(&raw_email, "email")?;
    let now = Utc::now();
    let expires_at = now + Duration::minutes(i64::from(EMAIL_VERIFICATION_CODE_TTL_MINUTES));
    let code = generate_email_code()?;
    let code_hash = hash_password(&code)?;
    let sender = state
        .email_sender
        .clone()
        .ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    let smtp_config =
        load_enabled_admin_smtp_config(pool, state.settings.exposed_credential_encryption_key())
            .await?
            .ok_or_else(|| {
                AppError::Internal("enabled smtp config is not configured".to_owned())
            })?;

    let mut tx = pool.begin().await?;
    ensure_active_user_in_tx(&mut tx, user_id).await?;
    ensure_email_available_in_tx(&mut tx, user_id, &email).await?;
    ensure_email_verification_not_cooling_down_in_tx(
        &mut tx,
        user_id,
        &email,
        EMAIL_BIND_PURPOSE,
        now,
        EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS,
    )
    .await?;
    supersede_pending_email_verifications_in_tx(&mut tx, user_id, EMAIL_BIND_PURPOSE).await?;
    insert_pending_email_verification_in_tx(
        &mut tx,
        user_id,
        &email,
        EMAIL_BIND_PURPOSE,
        &code_hash,
        expires_at,
        now,
    )
    .await?;
    tx.commit().await?;

    // 验证码落库后再发信，避免邮件已经发送但数据库没有可验证记录。
    let message = verification_code_email_message(
        email,
        "绑定邮箱验证码",
        &code,
        EMAIL_VERIFICATION_CODE_TTL_MINUTES,
        smtp_config.verification_code_template_html_for_purpose(EMAIL_BIND_PURPOSE),
    );
    sender.send(smtp_config, message).await?;

    Ok(BindEmailCodeResponse {
        sent: true,
        expires_at,
    })
}

/// 锁定最新邮箱绑定码，成功时同事务更新用户邮箱、消费码并写审计。
/// 错码只累加尝试次数并提交；邮箱冲突、过期或 SQL 失败不留半绑定。
pub(crate) async fn bind_user_email(
    pool: &Pool<MySql>,
    user_id: u64,
    raw_email: String,
    raw_code: String,
) -> AppResult<BindEmailResponse> {
    let email = validate_email(&raw_email, "email")?;
    let code = validate_email_code(&raw_code)?;
    let verified_at = Utc::now();
    let mut tx = pool.begin().await?;

    ensure_active_user_in_tx(&mut tx, user_id).await?;
    ensure_email_available_in_tx(&mut tx, user_id, &email).await?;
    let verification =
        lock_latest_pending_email_verification_in_tx(&mut tx, user_id, &email, EMAIL_BIND_PURPOSE)
            .await?
            .ok_or_else(|| AppError::Validation("email verification code is invalid".to_owned()))?;
    if email_verification_is_expired(
        verification.expires_at,
        verification.attempt_count,
        verified_at,
    ) {
        return Err(AppError::Validation(
            "email verification code is expired".to_owned(),
        ));
    }
    if !verify_password(&verification.code_hash, &code)? {
        increment_email_verification_attempt_count_in_tx(&mut tx, verification.id).await?;
        tx.commit().await?;
        return Err(AppError::Validation(
            "email verification code is invalid".to_owned(),
        ));
    }

    update_user_bound_email_in_tx(&mut tx, user_id, &email, verified_at).await?;
    mark_email_verification_verified_in_tx(&mut tx, verification.id, verified_at).await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.email.bind",
        "user",
        user_id.to_string(),
        None,
        Some(json!({ "email": email.clone() })),
    )
    .await?;
    tx.commit().await?;

    Ok(BindEmailResponse {
        email,
        email_verified_at: verified_at,
    })
}

/// 校验旧口令后更新用户登录密码，并撤销既有会话再签发新的用户令牌对。
/// 用户须处于启用状态、旧口令正确，且新口令满足策略并与旧口令不同。
/// 事务锁定凭证行，原子写入口令哈希、撤销刷新令牌并记录用户审计，任何数据库失败均回滚。
/// 提交后再撤销外部访问会话和签发令牌；该阶段失败时新密码已生效，调用方应重新登录而非重放改密。
/// 会话撤销依赖 Sa-Token/Redis 外部后端；令牌枚举失败可能被会话助手按空集合处理，
/// 因而本用例不能单凭成功响应证明所有旧访问令牌已删除。返回值只包含新会话凭据。
pub(crate) async fn change_user_password(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
    old_password: String,
    raw_new_password: String,
) -> AppResult<TokenResponse> {
    let old_password =
        crate::modules::user::domain::required_string(Some(old_password), "old_password")?;
    let new_password = validate_login_password(&raw_new_password, "new_password")?;
    if old_password == new_password {
        return Err(AppError::Validation(
            "new_password must be different from old_password".to_owned(),
        ));
    }

    let mut tx = pool.begin().await?;
    let user = lock_user_password_in_tx(&mut tx, user_id).await?;
    if user.status != "active" || !verify_password(&user.password_hash, &old_password)? {
        return Err(AppError::Unauthorized);
    }
    let password_hash = hash_password(&new_password)?;
    update_user_password_hash_in_tx(&mut tx, user.id, &password_hash).await?;
    revoke_user_refresh_tokens_in_tx(&mut tx, user.id).await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user.id,
        "user.password.change",
        "user",
        user.id.to_string(),
        None,
        Some(json!({ "changed": true })),
    )
    .await?;
    tx.commit().await?;

    // 密码变更后必须撤销旧会话并签发新 token，避免旧凭证继续访问用户资产相关接口。
    let actor = AuthActor::new(ActorType::User, user.id, Some(user.id));
    revoke_actor_auth_sessions(state, &actor).await?;
    let project_refresh_tokens = state.redis.clone().map(|manager| {
        Arc::new(RedisProjectRefreshTokenRepository::new(manager))
            as Arc<dyn crate::modules::auth::ProjectRefreshTokenRepository>
    });
    let tokens = AuthService::new(
        MySqlAuthRepository::new(pool.clone()),
        state.settings.clone(),
        state.auth_manager.clone(),
        project_refresh_tokens,
    )
    .issue_tokens_for_actor(actor)
    .await?;

    Ok(TokenResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: tokens.token_type,
        scope: tokens.scope,
    })
}

/// 为尚未设置资金密码的活跃用户创建六位数字密码哈希并写审计。
/// 凭证行在事务中锁定；已存在时拒绝覆盖，任一写入失败整体回滚。
pub(crate) async fn create_user_fund_password(
    pool: &Pool<MySql>,
    user_id: u64,
    login_password: String,
    raw_fund_password: String,
) -> AppResult<FundPasswordResponse> {
    let login_password =
        crate::modules::user::domain::required_string(Some(login_password), "login_password")?;
    let fund_password = validate_fund_password(&raw_fund_password, "fund_password")?;
    if login_password == fund_password {
        return Err(AppError::Validation(
            "fund_password must be different from login_password".to_owned(),
        ));
    }

    let mut tx = pool.begin().await?;
    let user = lock_user_password_in_tx(&mut tx, user_id).await?;
    if user.status != "active" || !verify_password(&user.password_hash, &login_password)? {
        return Err(AppError::Unauthorized);
    }
    if lock_fund_password_hash_in_tx(&mut tx, user.id)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(
            "fund password already exists".to_owned(),
        ));
    }

    let fund_password_hash = hash_password(&fund_password)?;
    upsert_fund_password_hash_in_tx(&mut tx, user.id, &fund_password_hash).await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user.id,
        "user.fund_password.create",
        "user_security",
        user.id.to_string(),
        None,
        Some(json!({ "fund_password_set": true })),
    )
    .await?;
    tx.commit().await?;

    Ok(FundPasswordResponse {
        fund_password_set: true,
    })
}

/// 锁定资金密码哈希并校验旧值，同事务更新新哈希与审计事件。
/// 旧密码错误、新旧相同或未设置均不修改状态，密码明文不进入审计。
pub(crate) async fn change_user_fund_password(
    pool: &Pool<MySql>,
    user_id: u64,
    old_fund_password: String,
    new_fund_password: String,
) -> AppResult<FundPasswordResponse> {
    let old_fund_password = validate_fund_password(&old_fund_password, "old_fund_password")?;
    let new_fund_password = validate_fund_password(&new_fund_password, "new_fund_password")?;
    if old_fund_password == new_fund_password {
        return Err(AppError::Validation(
            "new_fund_password must be different from old_fund_password".to_owned(),
        ));
    }

    let mut tx = pool.begin().await?;
    ensure_active_user_in_tx(&mut tx, user_id).await?;
    let existing_hash = lock_fund_password_hash_in_tx(&mut tx, user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if !verify_password(&existing_hash, &old_fund_password)? {
        return Err(AppError::Unauthorized);
    }
    let new_hash = hash_password(&new_fund_password)?;
    update_fund_password_hash_in_tx(&mut tx, user_id, &new_hash).await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.fund_password.change",
        "user_security",
        user_id.to_string(),
        None,
        Some(json!({ "fund_password_set": true })),
    )
    .await?;
    tx.commit().await?;

    Ok(FundPasswordResponse {
        fund_password_set: true,
    })
}

/// 向用户已验证邮箱发送二次验证重置专用码，与其他用途的冷却和消费隔离。
pub(crate) async fn send_user_two_factor_reset_code(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<BindEmailCodeResponse> {
    send_verified_email_code_for_purpose(
        state,
        pool,
        user_id,
        TWO_FACTOR_RESET_PURPOSE,
        "重置 2FA 验证码",
        false,
    )
    .await
}

/// 在独立事务消费二次验证重置邮件码后，依次清除 TOTP、写审计并回读状态。
/// 后三步不与验证码事务原子提交；重置或审计失败时验证码已消费，审计失败时 TOTP 也可能已清除。
pub(crate) async fn reset_user_two_factor_with_email_code(
    pool: &Pool<MySql>,
    user_id: u64,
    code: String,
) -> AppResult<UserTwoFactorStatusResponse> {
    verify_verified_email_code_for_purpose(pool, user_id, &code, TWO_FACTOR_RESET_PURPOSE).await?;
    reset_user_two_factor(pool, user_id).await?;
    insert_user_audit_event(
        pool,
        user_id,
        "user.2fa.reset",
        "user_two_factor_settings",
        user_id.to_string(),
        Some(json!({ "totp_enabled": false, "login_2fa_enabled": false })),
    )
    .await?;

    get_user_two_factor_status(pool, user_id).await
}

/// 向用户已验证邮箱发送资金密码重置专用码，不与登录密码码混用。
/// 未设置资金密码时不发送；验证码哈希先提交再发信，SMTP 失败不回滚冷却记录。
pub(crate) async fn send_user_fund_password_reset_code(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<BindEmailCodeResponse> {
    send_verified_email_code_for_purpose(
        state,
        pool,
        user_id,
        FUND_PASSWORD_RESET_PURPOSE,
        "重置资金密码验证码",
        true,
    )
    .await
}

/// 使用已验证邮箱的专用验证码重置既有资金密码，并记录安全审计。
/// 用户须已绑定验证邮箱且已设置资金密码；新密码满足策略，验证码须用途匹配、未过期且未超尝试次数。
/// 事务锁定邮箱和最新待验证记录，成功时原子更新哈希、消费验证码并写用户审计。
/// 错误验证码只递增尝试次数并提交该计数，不修改资金密码；过期或缺失验证码不产生密码变更。
/// 已消费验证码不能再次使用；数据库失败由事务回滚，成功响应不返回任何密码或哈希内容。
pub(crate) async fn reset_user_fund_password(
    pool: &Pool<MySql>,
    user_id: u64,
    raw_code: String,
    raw_new_fund_password: String,
) -> AppResult<FundPasswordResponse> {
    let new_fund_password = validate_fund_password(&raw_new_fund_password, "new_fund_password")?;
    let now = Utc::now();
    let code = validate_email_code(&raw_code)?;
    let mut tx = pool.begin().await?;
    let email = lock_verified_user_email_in_tx(&mut tx, user_id).await?;
    ensure_fund_password_exists_in_tx(&mut tx, user_id).await?;
    let verification = lock_latest_pending_email_verification_in_tx(
        &mut tx,
        user_id,
        &email,
        FUND_PASSWORD_RESET_PURPOSE,
    )
    .await?
    .ok_or_else(|| AppError::Validation("email verification code is invalid".to_owned()))?;
    if email_verification_is_expired(verification.expires_at, verification.attempt_count, now) {
        return Err(AppError::Validation(
            "email verification code is expired".to_owned(),
        ));
    }
    if !verify_password(&verification.code_hash, &code)? {
        increment_email_verification_attempt_count_in_tx(&mut tx, verification.id).await?;
        tx.commit().await?;
        return Err(AppError::Validation(
            "email verification code is invalid".to_owned(),
        ));
    }

    let new_hash = hash_password(&new_fund_password)?;
    update_fund_password_hash_in_tx(&mut tx, user_id, &new_hash).await?;
    mark_email_verification_verified_in_tx(&mut tx, verification.id, now).await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.fund_password.reset",
        "user_security",
        user_id.to_string(),
        None,
        Some(json!({ "fund_password_set": true })),
    )
    .await?;
    tx.commit().await?;

    Ok(FundPasswordResponse {
        fund_password_set: true,
    })
}

/// 按用户读取当前第三方账号绑定列表，返回前同时附带安全策略的入口开关。
pub(crate) async fn get_user_third_party_bindings(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<ThirdPartyBindingStatusResponse> {
    crate::modules::user::infrastructure::ensure_user_exists(pool, user_id).await?;
    let policy = load_security_policy(pool).await?;
    let bindings = list_user_third_party_bindings(pool, user_id).await?;
    Ok(ThirdPartyBindingStatusResponse {
        policy: policy.third_party_bindings,
        bindings,
    })
}

/// 校验第三方类型、策略开关、账号标识与显示名后，同事务 upsert 绑定并写审计。
/// 重复绑定同一 provider 更新快照而不新增多条；策略关闭或事务写入失败无部分数据库副作用。
/// 本用例只保存调用方提交的本地标识，不联系 Coinbase、Telegram 或验证远端账号所有权。
pub(crate) async fn bind_user_third_party_account(
    pool: &Pool<MySql>,
    user_id: u64,
    raw_provider: String,
    raw_account_identifier: String,
    raw_display_name: Option<String>,
) -> AppResult<ThirdPartyBindingStatusResponse> {
    let provider = normalize_third_party_provider(&raw_provider)?;
    let account_identifier = validate_third_party_identifier(provider, &raw_account_identifier)?;
    let display_name = normalize_third_party_display_name(raw_display_name)?;
    let policy = load_security_policy(pool).await?;
    if !is_third_party_binding_enabled(&policy.third_party_bindings, provider) {
        return Err(AppError::security_forbidden(
            "third_party_binding_disabled",
            "当前后台未开启该第三方账号绑定",
        ));
    }

    let mut tx = pool.begin().await?;
    ensure_active_user_in_tx(&mut tx, user_id).await?;
    upsert_user_third_party_binding_in_tx(
        &mut tx,
        user_id,
        provider,
        &account_identifier,
        &display_name,
    )
    .await?;
    // 第三方账号可能成为后续安全动作的辅助凭证，绑定和覆盖都要写审计。
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        "user.third_party_binding.bind",
        "user_third_party_binding",
        provider.to_owned(),
        None,
        Some(json!({
            "provider": provider,
            "account_identifier": account_identifier,
            "display_name": display_name,
            "status": "bound"
        })),
    )
    .await?;
    tx.commit().await?;

    Ok(ThirdPartyBindingStatusResponse {
        policy: policy.third_party_bindings,
        bindings: list_user_third_party_bindings(pool, user_id).await?,
    })
}

/// 组装用户 TOTP 绑定、登录开关与平台强制策略状态，不返回加密密钥。
pub(crate) async fn get_user_two_factor_status(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorStatusResponse> {
    let settings = load_user_two_factor(pool, user_id).await?;
    let policy = load_security_policy(pool).await?;
    Ok(UserTwoFactorStatusResponse {
        totp_enabled: settings.totp_enabled,
        login_2fa_enabled: settings.login_2fa_enabled,
        login_2fa_mode: policy.login_2fa_mode,
        can_toggle_login_2fa: policy.login_2fa_mode == LoginTwoFactorMode::UserEnabled,
        payment_policies: policy.payment_policies,
        third_party_bindings: policy.third_party_bindings,
    })
}

/// 读取用户与未绑定状态后生成 TOTP 密钥，加密保存待确认快照，并返回明文 secret/导入 URI。
/// 状态检查与 upsert 分离，并发启用可能被后写待确认值关闭；响应只可交付当前用户且不得记录。
pub(crate) async fn setup_user_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<SetupTwoFactorResponse> {
    ensure_user_exists(pool, user_id).await?;
    let existing = load_user_two_factor(pool, user_id).await?;
    if existing.totp_enabled {
        return Err(AppError::security_validation(
            "2fa_already_enabled",
            "2FA 已绑定",
        ));
    }
    let key = state
        .settings
        .exposed_credential_encryption_key()
        .ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
    let secret = generate_totp_secret()?;
    let encrypted_secret = encrypt_secret(&secret, key)?;
    save_pending_totp_secret(pool, user_id, &encrypted_secret).await?;
    let account = load_user_account_label(pool, user_id)
        .await?
        .unwrap_or_else(|| format!("user:{user_id}"));

    Ok(SetupTwoFactorResponse {
        otpauth_uri: totp_otpauth_uri("Exchange", &account, &secret),
        secret,
    })
}

/// 解密读取到的待确认 TOTP 密钥并校验动态码，随后以该密文启用绑定并另写审计。
/// 确认 SQL 不比较读取后的并发替换；确认与审计也不是同一事务，审计失败时绑定可能已启用。
pub(crate) async fn confirm_user_two_factor(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
    totp_code: String,
) -> AppResult<UserTwoFactorStatusResponse> {
    let code = validate_totp_code(&totp_code)?;
    let settings = load_user_two_factor(pool, user_id).await?;
    if settings.totp_enabled {
        return Err(AppError::security_validation(
            "2fa_already_enabled",
            "2FA 已绑定",
        ));
    }
    let encrypted_secret = settings.totp_secret_encrypted.ok_or_else(|| {
        AppError::security_validation("security_verification_required", "请先生成 2FA 密钥")
    })?;
    let key = state
        .settings
        .exposed_credential_encryption_key()
        .ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
    let secret = decrypt_secret(&encrypted_secret, key)?;
    if !verify_totp_code(&secret, &code, Utc::now())? {
        return Err(AppError::security_validation(
            "invalid_2fa_code",
            "2FA 验证码错误",
        ));
    }
    confirm_user_totp(pool, user_id, &encrypted_secret).await?;
    insert_user_audit_event(
        pool,
        user_id,
        "user.2fa.confirm",
        "user_two_factor_settings",
        user_id.to_string(),
        Some(json!({ "totp_enabled": true })),
    )
    .await?;

    get_user_two_factor_status(pool, user_id).await
}

/// 切换用户自选登录二次验证开关，启用前必须已完成 TOTP 绑定。
/// 平台非自选模式拒绝修改；状态读取、开关更新和审计为独立 SQL，确认后的并发解绑可使更新零行
/// 但仍继续审计。审计失败不会回滚已修改的开关，本函数不替换密钥或撤销会话。
pub(crate) async fn update_user_login_two_factor(
    pool: &Pool<MySql>,
    user_id: u64,
    enabled: bool,
) -> AppResult<UserTwoFactorStatusResponse> {
    let policy = load_security_policy(pool).await?;
    if policy.login_2fa_mode != LoginTwoFactorMode::UserEnabled {
        return Err(AppError::security_forbidden(
            "login_2fa_policy_locked",
            "当前登录 2FA 策略不允许用户修改",
        ));
    }
    let settings = load_user_two_factor(pool, user_id).await?;
    if enabled && !settings.totp_enabled {
        return Err(AppError::security_validation(
            "2fa_not_enabled",
            "请先绑定 2FA",
        ));
    }
    set_user_login_two_factor(pool, user_id, enabled).await?;
    // 登录 2FA 开关受后台策略约束；每次用户主动切换都要写审计，方便安全追踪。
    insert_user_audit_event(
        pool,
        user_id,
        "user.2fa.login.update",
        "user_two_factor_settings",
        user_id.to_string(),
        Some(json!({ "login_2fa_enabled": enabled })),
    )
    .await?;

    get_user_two_factor_status(pool, user_id).await
}

async fn send_verified_email_code_for_purpose(
    state: &AppState,
    pool: &Pool<MySql>,
    user_id: u64,
    purpose: &'static str,
    subject: &'static str,
    require_fund_password: bool,
) -> AppResult<BindEmailCodeResponse> {
    let now = Utc::now();
    let expires_at = now + Duration::minutes(i64::from(EMAIL_VERIFICATION_CODE_TTL_MINUTES));
    let code = generate_email_code()?;
    let code_hash = hash_password(&code)?;
    let sender = state
        .email_sender
        .clone()
        .ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    let smtp_config =
        load_enabled_admin_smtp_config(pool, state.settings.exposed_credential_encryption_key())
            .await?
            .ok_or_else(|| {
                AppError::Internal("enabled smtp config is not configured".to_owned())
            })?;

    let mut tx = pool.begin().await?;
    let email = lock_verified_user_email_in_tx(&mut tx, user_id).await?;
    if require_fund_password {
        ensure_fund_password_exists_in_tx(&mut tx, user_id).await?;
    }
    ensure_email_verification_not_cooling_down_in_tx(
        &mut tx,
        user_id,
        &email,
        purpose,
        now,
        EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS,
    )
    .await?;
    supersede_pending_email_verifications_in_tx(&mut tx, user_id, purpose).await?;
    insert_pending_email_verification_in_tx(
        &mut tx, user_id, &email, purpose, &code_hash, expires_at, now,
    )
    .await?;
    tx.commit().await?;

    let message = verification_code_email_message(
        email,
        subject,
        &code,
        EMAIL_VERIFICATION_CODE_TTL_MINUTES,
        smtp_config.verification_code_template_html_for_purpose(purpose),
    );
    sender.send(smtp_config, message).await?;

    Ok(BindEmailCodeResponse {
        sent: true,
        expires_at,
    })
}

async fn verify_verified_email_code_for_purpose(
    pool: &Pool<MySql>,
    user_id: u64,
    raw_code: &str,
    purpose: &'static str,
) -> AppResult<()> {
    let code = validate_email_code(raw_code)?;
    let now = Utc::now();
    let mut tx = pool.begin().await?;
    let email = lock_verified_user_email_in_tx(&mut tx, user_id).await?;
    let verification =
        lock_latest_pending_email_verification_in_tx(&mut tx, user_id, &email, purpose)
            .await?
            .ok_or_else(|| AppError::Validation("email verification code is invalid".to_owned()))?;
    if email_verification_is_expired(verification.expires_at, verification.attempt_count, now) {
        return Err(AppError::Validation(
            "email verification code is expired".to_owned(),
        ));
    }
    if !verify_password(&verification.code_hash, &code)? {
        increment_email_verification_attempt_count_in_tx(&mut tx, verification.id).await?;
        tx.commit().await?;
        return Err(AppError::Validation(
            "email verification code is invalid".to_owned(),
        ));
    }

    mark_email_verification_verified_in_tx(&mut tx, verification.id, now).await?;
    tx.commit().await?;
    Ok(())
}

async fn insert_user_audit_event(
    pool: &Pool<MySql>,
    user_id: u64,
    action: &'static str,
    target_type: &'static str,
    target_id: String,
    after_json: Option<serde_json::Value>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    insert_user_audit_event_in_tx(
        &mut tx,
        user_id,
        action,
        target_type,
        target_id,
        None,
        after_json,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

fn validate_totp_code(value: &str) -> AppResult<String> {
    let code = value.trim().to_owned();
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::security_validation(
            "invalid_2fa_code",
            "2FA 验证码错误",
        ));
    }
    Ok(code)
}
