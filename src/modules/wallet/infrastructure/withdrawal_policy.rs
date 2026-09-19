//! 提现策略、冷静期与独立复核的持久化适配器。

use crate::{
    error::{AppError, AppResult},
    modules::{
        admin::infrastructure::{AdminAuditLogEntry, insert_admin_audit_log_entry_in_tx},
        wallet::{
            domain::withdrawal_policy::{
                WithdrawalAmountBasis, WithdrawalPendingMode, WithdrawalPolicy,
            },
            presentation::withdrawal_policy::{
                WithdrawalAddressResponse, WithdrawalPolicyResponse,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{MySql, Pool, Transaction, types::Json};

/// 后台读取资产策略；尚未创建的行返回停用且版本为零，不产生配置写入。
pub(crate) async fn load_policy(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<WithdrawalPolicyResponse> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?)")
        .bind(asset_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let stored = sqlx::query_as::<_, (u64, Json<WithdrawalPolicy>)>(
        "SELECT revision, config_json FROM wallet_withdrawal_policies WHERE asset_id = ?",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?;
    Ok(policy_response(asset_id, stored))
}

fn policy_response(
    asset_id: u64,
    stored: Option<(u64, Json<WithdrawalPolicy>)>,
) -> WithdrawalPolicyResponse {
    let (revision, policy) = stored
        .map(|(revision, Json(policy))| (revision, policy))
        .unwrap_or_default();
    WithdrawalPolicyResponse {
        asset_id,
        revision,
        policy,
    }
}

/// 配置事务先锁资产，再锁策略，与创建提现一致；资产精度校验和版本 CAS 都在审计写入之前执行。
/// 缺省版本零只允许首次创建，过期编辑失败而不覆盖别人配置；审计失败则整次保存回滚。
pub(crate) async fn save_policy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    asset_id: u64,
    expected_revision: u64,
    policy: &WithdrawalPolicy,
    reason: &str,
) -> AppResult<WithdrawalPolicyResponse> {
    let precision: i32 =
        sqlx::query_scalar("SELECT precision_scale FROM assets WHERE id = ? FOR UPDATE")
            .bind(asset_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(AppError::NotFound)?;
    policy.validate(precision).map_err(AppError::Validation)?;
    let before = load_policy_in_tx(tx, asset_id).await?;
    if before.revision != expected_revision {
        return Err(AppError::Conflict(
            "withdrawal policy revision changed".into(),
        ));
    }
    let revision = expected_revision
        .checked_add(1)
        .ok_or_else(|| AppError::Conflict("withdrawal policy revision exhausted".into()))?;
    sqlx::query(
        "INSERT INTO wallet_withdrawal_policies (asset_id, revision, config_json, updated_by) VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE revision = VALUES(revision), config_json = VALUES(config_json), updated_by = VALUES(updated_by)",
    )
    .bind(asset_id).bind(revision).bind(Json(policy)).bind(admin_id)
    .execute(&mut **tx).await?;
    let after = WithdrawalPolicyResponse {
        asset_id,
        revision,
        policy: policy.clone(),
    };
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action: "wallet.withdrawal_policy.update",
            target_type: "asset",
            target_id: asset_id,
            before_json: Some(json!(before)),
            after_json: Some(json!(after)),
            reason: Some(reason.to_owned()),
        },
    )
    .await?;
    Ok(after)
}

async fn load_policy_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<WithdrawalPolicyResponse> {
    let row = sqlx::query_as::<_, (u64, Json<WithdrawalPolicy>)>(
        "SELECT revision, config_json FROM wallet_withdrawal_policies WHERE asset_id = ? FOR SHARE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(policy_response(asset_id, row))
}

/// 创建提现时调用，调用方已持有资产排他锁且尚未建立一致性读快照。
/// 匹配的每条累计规则都使用本资产、本用户、精确 Decimal 和明确滚动窗口，拒绝/确定失败不占额。
/// 资产锁串行化并发新单；状态释放的并发只可能保守高估占额，不会多放额度。所有查询失败均拒绝。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn check_creation_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
    precision: i32,
    user_id: u64,
    network: &str,
    address: &str,
    amount: &BigDecimal,
    total_reserved: &BigDecimal,
) -> AppResult<(WithdrawalPolicyResponse, u32)> {
    let config = load_policy_in_tx(tx, asset_id).await?;
    let policy = &config.policy;
    let measured = policy.measured_amount(amount, total_reserved);
    let required = policy.required_approvals(measured);
    if !policy.enabled {
        return Ok((config, required));
    }
    policy.validate(precision).map_err(AppError::Validation)?;
    let (kyc_level, now) = check_cooling_in_tx(tx, policy, user_id, network, address).await?;
    // No earlier consistent read is allowed in this create transaction:
    // the first snapshot must follow the authoritative asset serialization lock.
    for allowance in &policy.allowances {
        if allowance.user_id.is_some_and(|id| id != user_id)
            || allowance.kyc_level.is_some_and(|level| level != kyc_level)
        {
            continue;
        }
        let column = match policy.amount_basis {
            WithdrawalAmountBasis::Principal => "amount",
            WithdrawalAmountBasis::TotalReserved => "total_reserved",
        };
        let used: BigDecimal = sqlx::query_scalar(&format!(
            "SELECT COALESCE(SUM({column}), 0) FROM wallet_withdrawal_requests \
             WHERE user_id = ? AND asset_id = ? AND status NOT IN ('rejected', 'failed') \
             AND (created_at >= ? OR (? AND status <> 'confirmed'))"
        ))
        .bind(user_id)
        .bind(asset_id)
        .bind(now - Duration::seconds(i64::from(allowance.window_seconds)))
        .bind(allowance.pending_mode == WithdrawalPendingMode::AllOutstanding)
        .fetch_one(&mut **tx)
        .await?;
        if used + measured > allowance.max_amount {
            return Err(AppError::security_forbidden(
                "withdrawal_allowance_exceeded",
                "累计提现额度不足",
            ));
        }
    }
    Ok((config, required))
}

/// 新单冻结前保存审核人数及完整规则快照；无独立提交，钱包或单据失败时不会留下孤立凭据。
pub(crate) async fn insert_receipt_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    config: &WithdrawalPolicyResponse,
    required: u32,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO wallet_withdrawal_policy_receipts (withdrawal_id, policy_revision, config_json, required_approvals) VALUES (?, ?, ?, ?)",
    ).bind(withdrawal_id).bind(config.revision).bind(Json(&config.policy)).bind(required)
        .execute(&mut **tx).await?;
    Ok(())
}

/// 在提现单行锁保护下追加不同管理员复核票；同人重放不增加票数，审计和票据同事务提交。
/// 旧单无快照时沿用历史一次审核，新增启用策略的单必须填写原因且再次检查安全冷静期。
/// 返回 false 代表票数未齐，调用方必须保留 pending_review，不能触发广播。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_review_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    user_id: u64,
    network: &str,
    address: &str,
    admin_id: u64,
    reason: Option<&str>,
) -> AppResult<bool> {
    let receipt = sqlx::query_as::<_, (u32, Json<WithdrawalPolicy>)>(
        "SELECT required_approvals, config_json FROM wallet_withdrawal_policy_receipts WHERE withdrawal_id = ?",
    ).bind(withdrawal_id).fetch_optional(&mut **tx).await?;
    let Some((required, Json(policy))) = receipt else {
        return Ok(true);
    };
    if !policy.enabled {
        return Ok(true);
    }
    let reason = reason
        .filter(|v| !v.trim().is_empty() && v.chars().count() <= 512)
        .ok_or_else(|| {
            AppError::Validation(
                "withdrawal policy review requires a reason of 1 to 512 characters".into(),
            )
        })?;
    check_cooling_in_tx(tx, &policy, user_id, network, address).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM wallet_withdrawal_reviews WHERE withdrawal_id = ? AND admin_id = ?)",
    ).bind(withdrawal_id).bind(admin_id).fetch_one(&mut **tx).await?;
    if !exists {
        sqlx::query("INSERT INTO wallet_withdrawal_reviews (withdrawal_id, admin_id, reason) VALUES (?, ?, ?)")
            .bind(withdrawal_id).bind(admin_id).bind(reason).execute(&mut **tx).await?;
        insert_admin_audit_log_entry_in_tx(
            tx,
            admin_id,
            AdminAuditLogEntry {
                action: "wallet.withdrawal.review",
                target_type: "wallet_withdrawal_request",
                target_id: withdrawal_id,
                before_json: None,
                after_json: Some(json!({"required_approvals": required})),
                reason: Some(reason.into()),
            },
        )
        .await?;
    }
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_withdrawal_reviews WHERE withdrawal_id = ?",
    )
    .bind(withdrawal_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(votes >= i64::from(required))
}

async fn check_cooling_in_tx(
    tx: &mut Transaction<'_, MySql>,
    policy: &WithdrawalPolicy,
    user_id: u64,
    network: &str,
    address: &str,
) -> AppResult<(i32, DateTime<Utc>)> {
    let (kyc, user_changed): (i32, DateTime<Utc>) = sqlx::query_as(
        "SELECT kyc_level, COALESCE(withdrawal_security_changed_at, created_at) FROM users WHERE id = ? FOR SHARE",
    ).bind(user_id).fetch_optional(&mut **tx).await?.ok_or(AppError::NotFound)?;
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT CURRENT_TIMESTAMP(6)")
        .fetch_one(&mut **tx)
        .await?;
    if let Some(seconds) = policy.security_cooling_seconds {
        let fund: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT COALESCE(withdrawal_security_changed_at, created_at) FROM user_security WHERE user_id = ? FOR SHARE",
        ).bind(user_id).fetch_optional(&mut **tx).await?;
        let totp: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT COALESCE(withdrawal_security_changed_at, created_at) FROM user_two_factor_settings WHERE user_id = ? FOR SHARE",
        ).bind(user_id).fetch_optional(&mut **tx).await?;
        let latest = [Some(user_changed), fund, totp]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(user_changed);
        if latest + Duration::seconds(i64::from(seconds)) > now {
            return Err(AppError::security_forbidden(
                "withdrawal_security_cooling",
                "安全信息变更后处于提现冷静期",
            ));
        }
    }
    if let Some(seconds) = policy.address_cooling_seconds {
        let registered: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT created_at FROM wallet_withdrawal_addresses WHERE user_id = ? AND identity_hash = ? FOR SHARE",
        ).bind(user_id).bind(address_identity(network, address)).fetch_optional(&mut **tx).await?;
        if registered.is_none_or(|at| at + Duration::seconds(i64::from(seconds)) > now) {
            return Err(AppError::security_forbidden(
                "withdrawal_address_cooling",
                "请先登记提现地址并等待冷静期结束",
            ));
        }
    }
    Ok((kyc, now))
}

fn address_identity(network: &str, address: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(network.as_bytes());
    digest.update([0]);
    digest.update(address.as_bytes());
    format!("{:x}", digest.finalize())
}

/// 经应用层验证用户后登记网络与地址的精确身份；重复登记不刷新首次登记时间，不允许回填历史。
pub(crate) async fn register_address(
    pool: &Pool<MySql>,
    user_id: u64,
    network: &str,
    address: &str,
) -> AppResult<WithdrawalAddressResponse> {
    let identity = address_identity(network, address);
    sqlx::query(
        "INSERT INTO wallet_withdrawal_addresses (user_id, network, address, identity_hash) VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE id = id",
    ).bind(user_id).bind(network).bind(address).bind(&identity).execute(pool).await?;
    sqlx::query_as("SELECT id, network, address, created_at FROM wallet_withdrawal_addresses WHERE user_id = ? AND identity_hash = ?")
        .bind(user_id).bind(identity).fetch_one(pool).await.map_err(Into::into)
}

/// 只读取本人最新登记地址，不能通过请求体读取或变更其他用户的允许列表。
pub(crate) async fn list_addresses(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<WithdrawalAddressResponse>> {
    sqlx::query_as("SELECT id, network, address, created_at FROM wallet_withdrawal_addresses WHERE user_id = ? ORDER BY id DESC LIMIT 100")
        .bind(user_id).fetch_all(pool).await.map_err(Into::into)
}

/// 广播认领与创建时规则的冷静期复核共用事务；安全信息已变更时只延期，不广播、不退冻。
/// 仅处理 approved，结果不明/广播中仍由原查询状态机管理；请求锁后按用户、安全行顺序加锁。
/// 配置/数据库失败中止认领，成功才增加尝试次数；旧单无快照仍沿用原合同。
pub(crate) async fn claim_broadcast(pool: &Pool<MySql>, withdrawal_id: u64) -> AppResult<bool> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, (u64, Option<String>, String)>(
        "SELECT user_id, network, address FROM wallet_withdrawal_requests WHERE id = ? AND status = 'approved' AND (next_attempt_at IS NULL OR next_attempt_at <= CURRENT_TIMESTAMP(6)) FOR UPDATE",
    ).bind(withdrawal_id).fetch_optional(&mut *tx).await?;
    let Some((user_id, network, address)) = row else {
        return Ok(false);
    };
    let policy: Option<Json<WithdrawalPolicy>> = sqlx::query_scalar(
        "SELECT config_json FROM wallet_withdrawal_policy_receipts WHERE withdrawal_id = ?",
    )
    .bind(withdrawal_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(Json(policy)) = policy
        && policy.enabled
        && let Err(error) = check_cooling_in_tx(
            &mut tx,
            &policy,
            user_id,
            network.as_deref().unwrap_or(""),
            &address,
        )
        .await
    {
        if !matches!(
            &error,
            AppError::Api {
                code: "withdrawal_security_cooling" | "withdrawal_address_cooling",
                ..
            }
        ) {
            return Err(error);
        }
        sqlx::query("UPDATE wallet_withdrawal_requests SET next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 30 SECOND) WHERE id = ?")
            .bind(withdrawal_id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(false);
    }
    let result = sqlx::query(
        "UPDATE wallet_withdrawal_requests SET status = 'broadcasting', broadcasting_at = CURRENT_TIMESTAMP(6), retry_count = retry_count + 1, next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL 30 SECOND) WHERE id = ? AND status = 'approved'",
    ).bind(withdrawal_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}
