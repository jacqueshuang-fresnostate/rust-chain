//! user bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 本文件承载用户自服务的全部用例编排：个人资料与头像、用户名变更、邀请码与推荐关系绑定、
//! 邮箱绑定与验证码收发、登录密码与资金密码的创建/修改/重置、第三方账号绑定，以及 TOTP 二次验证的
//! 生成、确认、登录开关与邮箱重置。
//! 权限边界统一为用户自服务：所有用例都以路由层从访问令牌解析出的 `user_id` 为唯一操作对象，
//! 不接受调用方指定他人 ID，也不提供任何管理员越权入口；KYC 审核等管理动作在 admin 上下文实现。
//! 隐私边界：资料、邮箱、证件材料只回给本人；密码、资金密码、验证码明文与 TOTP 密钥一律不写入
//! 审计表与日志，审计只记录布尔标志或脱敏摘要。
//! 事务口径：需要原子性的写入统一在单个 MySQL 事务内完成，跨外部系统的动作（发信、撤销会话、
//! 签发令牌、对象存储上传）一律放在事务提交之后，因此这些步骤失败时数据库侧的变更不会回滚，
//! 各用例注释中会分别说明由此产生的补偿责任。

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

/// 修改当前用户的登录用户名：先按认证域统一规则规范化（含转小写与字符集校验），
/// 再在事务内锁定该用户的活跃行并读出改前用户名，随后更新为新值。
/// 用户名是登录标识而非展示昵称，改名会直接影响后续登录凭据，因此同事务写入
/// `user.username.update` 审计并完整保留前后两个值，便于事后追踪账号归属变化。
/// 用户不存在或非活跃状态在锁行时即失败；与他人重名由数据库唯一索引拦截并映射为 `AppError::Conflict`。
/// 更新与审计要么一起提交要么整体回滚，不会出现改了名却没有审计记录的中间态。
/// 本用例不撤销既有会话，也不重新签发令牌，改名后旧访问令牌仍然有效。
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

/// 为用户写入一个全局唯一的邀请码，通过「随机生成加冲突重试」而非查询后插入来保证唯一性。
/// 传入 `existing_code_id` 表示改写该条已存在但码值失效的记录，传 `None` 表示新建一条。
/// 每轮生成一个新码交给仓储写入，写入方以数据库唯一键冲突返回 `false` 表示撞码，此时继续下一轮；
/// 最多重试 `USER_INVITE_CODE_CREATE_ATTEMPTS` 次，全部撞码返回 `AppError::Internal`，
/// 而不是退让到可预测的码值，避免邀请码空间被猜测。
/// 每轮写入都是独立自治的语句，本函数不持有事务，因此失败时不存在需要回滚的中间状态。
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

/// 列出把当前用户记录为「直接邀请人」的下级用户，只取推荐关系的第一层。
/// 邀请链上更深层级的下级和同代理公司的其他成员都不在结果内，因此该列表不能用于计算多级返佣。
/// 返回的下级信息由仓储侧按对外可见口径裁剪，不含下级的邮箱、手机号等联系方式。
/// 只读用例，不校验用户是否存在：无邀请记录或用户不存在都返回空列表而非报错。
pub(crate) async fn list_user_invites(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<MyInvitesResponse> {
    let users = list_direct_invited_users(pool, user_id).await?;
    Ok(MyInvitesResponse { users })
}

/// 向用户填写的待绑定邮箱发送一次性验证码，用于首次绑定或更换邮箱。
/// 与重置类验证码不同，这里的目标地址由调用方提交而非从已验证邮箱读取，所以必须先确认该地址未被他人占用。
/// 执行顺序是先在事务外准备好随机验证码、其哈希、发信通道和启用中的 SMTP 配置，
/// 缺少发信器或没有启用的 SMTP 配置直接返回 `AppError::Internal`，避免写了记录却发不出信。
/// 事务内依次校验用户处于活跃状态、目标邮箱未被占用、距上次同用途发送已过
/// `EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS` 秒的冷却窗口，然后把同用户同用途的旧待验证码整体作废，
/// 再插入新的一条，有效期为 `EMAIL_VERIFICATION_CODE_TTL_MINUTES` 分钟。
/// 作废旧码保证任一时刻只有最新一条可用，杜绝多码并存被重放。
/// 只有哈希入库并提交成功后才真正发信，因此不会出现邮件已到而库中无可验证记录的情况；
/// 反过来 SMTP 失败不回滚已提交的记录，用户需等冷却期结束后重新发起。
/// 验证码明文只存在于本次调用的内存与邮件正文中，绝不落库也绝不写日志。
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

/// 校验绑定用途的邮件验证码并把该邮箱正式落到用户账号上，完成邮箱绑定或更换。
/// 邮箱与验证码都先做格式校验，随后在单个事务内确认用户活跃、该地址仍未被他人占用，
/// 再锁定同用户同地址同用途下最新的一条待验证记录；没有可用记录时统一返回「验证码无效」，
/// 不区分「从未发送」与「已被消费」，避免据此探测他人邮箱是否注册过。
/// 失效判定同时看有效期和累计失败次数，任一越界都返回「验证码已过期」。
/// 验证码比对走密码哈希校验而非明文相等；比对失败时递增该记录的尝试次数并提交这次计数，
/// 因此错码同样消耗尝试配额，达到上限后即使有效期未到该码也不再可用。
/// 比对成功则在同事务内写入用户邮箱与验证时间、把验证码标记为已消费、追加 `user.email.bind` 审计，
/// 三者原子提交，不会出现邮箱已改但验证码仍可复用的中间态。
/// 审计记录邮箱地址本身，用于安全追溯账号联系方式变更；本用例不撤销既有会话也不重新签发令牌。
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
/// 会话撤销依赖 Sa-Token/Redis 外部后端；枚举或登出失败会让本用例报错，不会在旧访问令牌状态未知时返回成功。
/// 返回值只包含撤销完成后签发的新会话凭据。
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

/// 为尚未设置资金密码的活跃用户首次创建六位数字资金密码，需同时提交登录密码作为身份确认。
/// 先要求资金密码与登录密码不同，防止用户把两道口令设成同一串从而让资金密码失去二次确认意义。
/// 事务内锁定用户凭证行，要求状态为 `active` 且登录密码校验通过，否则一律返回 `AppError::Unauthorized`，
/// 不区分「账号被禁用」与「登录密码错误」以免泄露账号状态。
/// 随后锁定资金密码行，若已存在则返回 `AppError::Conflict` 而不是静默覆盖，
/// 修改既有资金密码必须走独立的修改或重置用例。
/// 只写入哈希，登录密码与资金密码明文都不入库；审计仅记录 `fund_password_set` 布尔标志，不含任何口令内容。
/// 哈希写入与审计同事务提交，任一失败整体回滚，不会留下有密码却无审计的痕迹。
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

/// 由用户凭旧资金密码修改为新资金密码，与创建用例的区别是这里不需要登录密码，只凭旧资金密码本身鉴权。
/// 新旧密码都必须满足六位数字规则，且两者不得相同，避免用户提交无实质变更的请求。
/// 事务内先确认用户处于活跃状态，再锁定资金密码哈希行；从未设置过资金密码时返回 `AppError::NotFound`，
/// 提示调用方改走创建用例，而不是在此隐式创建。
/// 旧密码比对失败返回 `AppError::Unauthorized` 且不做任何写入，本用例不累计失败次数也不锁定账户，
/// 暴力尝试的限制依赖接口层限流。
/// 校验通过后同事务写入新哈希并追加 `user.fund_password.change` 审计，两者原子提交；
/// 审计只落 `fund_password_set` 标志，新旧口令明文与哈希都不进入审计和日志。
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

/// 向用户已绑定并验证过的邮箱发送用于解绑 TOTP 的专用验证码，是用户丢失验证器后的自助恢复入口。
/// 目标地址不接受调用方指定，而是在事务内从用户记录中读取已验证邮箱，因此未绑定邮箱的用户无法走此流程。
/// 用途标记为 `TWO_FACTOR_RESET_PURPOSE`，与邮箱绑定、资金密码重置各自独立计算冷却窗口并独立消费，
/// 任一用途的验证码都不能跨用途使用。
/// 这里传入 `require_fund_password` 为 `false`：重置二次验证不以已设置资金密码为前提。
/// 具体的冷却、旧码作废、落库后发信等语义由 `send_verified_email_code_for_purpose` 统一实现。
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

/// 用户凭邮箱验证码解除 TOTP 绑定，用于验证器丢失后的自助恢复，全过程不需要提供当前动态码。
/// 只接受 `TWO_FACTOR_RESET_PURPOSE` 用途的验证码，其他用途的有效码在此不被接受。
/// 分四步执行且刻意不共享事务：先在独立事务内校验并消费验证码，再清除 TOTP 密钥与登录二次验证开关，
/// 接着单独写入 `user.2fa.reset` 审计，最后回读并返回最新的二次验证状态。
/// 因为不是原子的，中途失败会留下部分生效的结果：重置步骤失败时验证码已被消费，用户需重新发码；
/// 审计步骤失败时 TOTP 实际已被清除但缺少审计记录。返回成功即表示前三步均已完成。
/// 重置后用户处于完全未绑定状态，需要重新走生成与确认流程；本用例不撤销既有登录会话。
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

/// 向用户已验证邮箱发送用于重置资金密码的专用验证码，是用户遗忘六位资金密码时的自助入口。
/// 用途标记为 `FUND_PASSWORD_RESET_PURPOSE`，冷却窗口和消费状态与邮箱绑定码、二次验证重置码彼此隔离。
/// 与二次验证重置的关键差异是这里传入 `require_fund_password` 为 `true`：
/// 事务内会额外确认用户确实已设置过资金密码，未设置时不发送验证码，
/// 以免把「先创建后重置」的路径混淆，也避免向无此凭证的账号发送无意义邮件。
/// 落库提交后才发信，SMTP 失败不回滚已写入的冷却与待验证记录，用户须等冷却结束再试。
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

/// 读取当前用户已绑定的第三方账号清单，并附带后台安全策略中各提供方的入口开关。
/// 两者一起返回是为了让前端据此决定按钮的可用状态：策略关闭时即使已有绑定记录也不应再提供绑定入口。
/// 先确认用户存在，不存在直接失败，避免为无效 ID 返回一份看似合法的空绑定视图。
/// 只读用例，不创建默认策略也不补写绑定记录；返回的账号标识是绑定时用户自填的本地值，不含第三方令牌。
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

/// 把一个第三方账号标识登记到当前用户名下，目前支持 Coinbase 钱包与 Telegram 账号两类。
/// 依次做四道校验：提供方必须落在白名单内，账号标识需满足该提供方的长度与空白约束，
/// 展示名可缺省但不得超长，最后读取后台安全策略确认该提供方的绑定入口处于开启状态。
/// 策略关闭时返回带 `third_party_binding_disabled` 码的安全禁止错误，且此前没有任何写入发生。
/// 通过后在事务内确认用户活跃并对绑定表做 upsert：同一用户同一提供方只保留一条记录，
/// 重复绑定是覆盖既有快照而非追加，因此不存在同类型多绑并存的情况。
/// 第三方账号可能成为后续安全动作的辅助凭证，所以绑定和覆盖都同事务写入
/// `user.third_party_binding.bind` 审计，其中包含提供方、账号标识与展示名快照。
/// 关键限制：本用例只登记用户自报的标识，既不调用 Coinbase 或 Telegram 接口，
/// 也不校验该账号是否真实存在或确实属于本人，所有权确认需由外部授权流程另行完成。
/// 提交后重新查询并返回完整绑定列表与策略开关；事务内任一步失败整体回滚，不留部分写入。
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

/// 汇总当前用户的二次验证视图，把用户侧设置与平台侧策略合并成一份可直接驱动界面的状态。
/// 用户侧提供 TOTP 是否已绑定和登录二次验证是否开启；平台侧提供全局登录二次验证模式、
/// 各支付动作要求的验证方式，以及第三方绑定入口开关。
/// `can_toggle_login_2fa` 由策略模式推导：只有处于「由用户自行决定」模式时用户才被允许改动登录开关，
/// 强制开启或强制关闭模式下前端应把开关置灰，服务端也会在修改用例中再次拒绝。
/// 只读用例，绝不返回 TOTP 密钥明文或其密文，密钥只在生成阶段一次性回传给本人。
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

/// 生成一枚新的 TOTP 密钥并以待确认状态保存，返回明文密钥与可供验证器扫码导入的 otpauth URI。
/// 这是二次验证绑定流程的第一步，必须由 `confirm_user_two_factor` 输入正确动态码后才真正生效。
/// 前置条件是用户存在且当前尚未绑定 TOTP，已绑定时返回 `2fa_already_enabled` 安全校验错误，
/// 换绑需先走重置流程，本用例不会覆盖已生效的密钥。
/// 密钥用凭证加密密钥加密后才写库，未配置该密钥时返回 `AppError::Internal` 并且不生成任何密钥。
/// otpauth URI 中的账号标签取用户可读标识，取不到时回落为 `user:<id>` 形式，仅影响验证器中的显示名称。
/// 并发风险：状态检查与写入不在同一事务内，两次并发调用会各自生成密钥，后写入的待确认值覆盖先写入的，
/// 此时只有后一枚密钥能通过确认。
/// 响应中的明文密钥等同于长期凭证，只能直接交付当前认证用户，不得写日志、不得缓存、不得转发。
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

/// 校验用户从验证器读出的六位动态码，通过后把待确认的 TOTP 密钥正式启用，完成绑定流程第二步。
/// 依次要求：动态码为六位数字、当前尚未绑定、已经存在由生成步骤写入的待确认密钥。
/// 缺少待确认密钥时返回 `security_verification_required`，提示调用方先执行生成步骤。
/// 密钥从库中取出后用凭证加密密钥解密再参与校验，未配置该密钥返回 `AppError::Internal`。
/// 动态码校验以当前 UTC 时间为基准，容许前后各一个时间步的漂移以适配设备时钟偏差，
/// 校验失败返回 `invalid_2fa_code` 且不做任何写入。
/// 启用时把参与校验的那份密文原样写回，保证生效的密钥与刚刚验证通过的是同一枚。
/// 并发限制：确认语句不比较读取之后是否被并发替换，与生成步骤竞态时可能启用后写入的密钥。
/// 启用与 `user.2fa.confirm` 审计不在同一事务，审计失败时绑定实际已生效；审计只记布尔标志不含密钥。
/// 成功后回读并返回最新的二次验证状态，其中不包含密钥内容。
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

/// 向用户已验证邮箱发送指定用途验证码的公共实现，被二次验证重置与资金密码重置两条入口复用。
/// 与邮箱绑定发码的本质区别在于目标地址不可由调用方指定：地址是在事务内从用户记录中锁定读出的
/// 已验证邮箱，因此攻击者无法把重置码引到自己控制的邮箱。
/// `purpose` 决定冷却窗口与后续消费的隔离域，不同用途互不干扰，一个用途的码不能用于另一个用途。
/// `require_fund_password` 为真时额外要求用户已设置资金密码，用于资金密码重置这类有前置凭证的场景。
/// `subject` 只影响邮件标题，同时按用途选取对应的邮件模板。
/// 事务外先备好验证码、哈希、发信器与启用中的 SMTP 配置，任一缺失直接返回 `AppError::Internal`，
/// 保证不会出现记录已写但无法发信的情况。
/// 事务内按锁邮箱、校验前置条件、冷却检查、作废同用途旧码、插入新待验证记录的顺序执行；
/// 作废旧码确保同一用途下同时只有最新一条可用，防止旧码被重放。
/// 提交成功后才发信，SMTP 失败不回滚已提交的记录与冷却状态。验证码明文不入库也不写日志。
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

/// 校验并一次性消费发往用户已验证邮箱的指定用途验证码，成功返回空值表示调用方可继续后续动作。
/// 邮箱同样从用户记录锁定读出而非由调用方传入，`purpose` 限定只匹配同用途的记录。
/// 在单个事务内锁定该用户该地址该用途下最新的待验证记录，缺失时统一返回「验证码无效」，
/// 不透露究竟是未发送过还是已被消费。
/// 失效判定同时看有效期与累计失败次数，越界返回「验证码已过期」且不递增计数。
/// 哈希比对失败时递增尝试次数并提交该计数后再返回错误，所以错码会消耗有限的尝试配额；
/// 这次提交是有意为之，若回滚则失败次数无法累积，暴力枚举将不受限制。
/// 比对成功则把记录标记为已消费并提交，同一枚验证码不能被第二次使用。
/// 本函数只负责验证码本身，不执行任何业务动作，调用方需在其返回成功后自行完成后续变更。
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

/// 用一个仅包含审计写入的独立事务记录一条用户安全事件，供那些主业务动作没有开启事务的用例调用。
/// 与事务内版本的区别是这里自带 begin 与 commit，因此审计的成败与主业务动作彼此独立：
/// 审计失败不会回滚此前已生效的业务变更，调用方需按各自注释承担这一后果。
/// 变更前快照固定传空，只写变更后的 `after_json`，适用于开关切换、绑定确认这类无需对比原值的事件。
/// 传入的 JSON 只能是布尔标志或已脱敏摘要，禁止携带密码、验证码、TOTP 密钥或证件号等敏感原文。
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

/// 校验用户提交的 TOTP 动态码必须是恰好六位 ASCII 数字，返回去空白后的规范值。
/// 与邮件验证码校验的差异在于错误类型：这里返回带 `invalid_2fa_code` 码的安全校验错误，
/// 与动态码比对失败时的错误完全一致，使调用方无法据错误码区分「格式不对」和「码值不对」。
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
