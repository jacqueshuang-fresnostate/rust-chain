//! 提现网关适配与提现申请状态机持久化。
//!
//! 资金不变量：申请金额与手续费统一冻结为 total_reserved；拒绝/失败等额释放，链上确认仅从 frozen 永久扣除，所有状态与流水同事务推进。

use super::shared::{
    fetch_admin_page, insert_wallet_ledger_in_tx, lock_wallet_balance, update_wallet_balance,
};
use crate::{
    error::{AppError, AppResult},
    modules::wallet::{
        WithdrawFeeTier, calculate_withdraw_fee, normalize_withdraw_fee_tiers,
        presentation::WalletWithdrawalResponse,
        repository::{
            WalletChainBroadcastCommand, WalletChainBroadcastResult, WalletChainGateway,
            WalletChainPollPage,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::time::Duration;

/// 分页排序必须带唯一列 id，否则同一时间戳的行会在页间重复或丢失。
const WALLET_WITHDRAWAL_ORDER_BY: &str = " ORDER BY requests.id DESC";

#[derive(Debug, Clone)]
pub struct HttpWalletChainGateway {
    client: reqwest::Client,
}

impl Default for HttpWalletChainGateway {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl WalletChainGateway for HttpWalletChainGateway {
    async fn broadcast_withdrawal(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        command: &WalletChainBroadcastCommand,
    ) -> AppResult<WalletChainBroadcastResult> {
        let mut request = self
            .client
            .post(endpoint)
            .timeout(Duration::from_secs(15))
            .json(command);
        if let Some(token) = bearer_token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("wallet gateway request failed: {error}")))?
            .error_for_status()
            .map_err(|error| {
                AppError::Internal(format!("wallet gateway rejected broadcast: {error}"))
            })?;
        response.json().await.map_err(|error| {
            AppError::Internal(format!(
                "wallet gateway broadcast response is invalid: {error}"
            ))
        })
    }

    async fn poll_chain_events(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        cursor: Option<&str>,
        limit: u32,
    ) -> AppResult<WalletChainPollPage> {
        let limit = limit.to_string();
        let mut request = self
            .client
            .get(endpoint)
            .timeout(Duration::from_secs(15))
            .query(&[("cursor", cursor.unwrap_or("")), ("limit", limit.as_str())]);
        if let Some(token) = bearer_token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("wallet gateway poll failed: {error}")))?
            .error_for_status()
            .map_err(|error| {
                AppError::Internal(format!("wallet gateway poll rejected: {error}"))
            })?;
        response.json().await.map_err(|error| {
            AppError::Internal(format!("wallet gateway poll response is invalid: {error}"))
        })
    }
}
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct WithdrawalAssetRule {
    pub(crate) id: u64,
    pub(crate) precision_scale: i32,
    pub(crate) fee: BigDecimal,
}
pub(crate) async fn load_withdrawal_asset_rule(
    pool: &Pool<MySql>,
    asset_symbol: &str,
    amount: &BigDecimal,
) -> AppResult<WithdrawalAssetRule> {
    let row = sqlx::query_as::<_, (u64, bool, BigDecimal, i32, SqlxJson<Vec<WithdrawFeeTier>>)>(
        r#"SELECT id, withdraw_enabled, withdraw_fee, precision_scale,
                  COALESCE(withdraw_fee_tiers_json, JSON_ARRAY())
           FROM assets
           WHERE symbol = ? AND status = 'active'
           LIMIT 1"#,
    )
    .bind(asset_symbol)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((id, true, fixed_fee, precision_scale, SqlxJson(tiers))) => {
            let tiers = normalize_withdraw_fee_tiers(tiers).map_err(AppError::Validation)?;
            Ok(WithdrawalAssetRule {
                id,
                precision_scale,
                fee: calculate_withdraw_fee(amount, &fixed_fee, &tiers, precision_scale),
            })
        }
        Some((_, false, _, _, _)) => Err(AppError::Validation(
            "asset does not support withdraw".to_owned(),
        )),
        None => Err(AppError::NotFound),
    }
}

pub(crate) async fn load_withdrawal_by_user_key(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<WalletWithdrawalResponse>> {
    sqlx::query_as::<_, WalletWithdrawalResponse>(&format!(
        "{} WHERE requests.user_id = ? AND requests.idempotency_key = ? LIMIT 1",
        wallet_withdrawal_select_sql()
    ))
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

#[allow(clippy::too_many_arguments)]
/// 创建提现申请并把金额与手续费从 available 等额冻结到 frozen。
/// 资产规则、安全校验和幂等重放由应用层先行处理；本函数以钱包行锁复核余额并写入冻结流水。
/// 提现记录、钱包三桶和账本共用同一事务，任一步失败都不会保留部分冻结结果。
pub(crate) async fn reserve_withdrawal_request(
    pool: &Pool<MySql>,
    user_id: u64,
    asset: &WithdrawalAssetRule,
    asset_symbol: &str,
    network: Option<&str>,
    address: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
    security_method: &str,
) -> AppResult<WalletWithdrawalResponse> {
    let total_reserved = (amount.clone() + asset.fee.clone()).with_scale(18);
    let gateway_request_id = uuid::Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
              (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
               status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending_review', ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset.id)
    .bind(asset_symbol)
    .bind(network)
    .bind(address)
    .bind(amount)
    .bind(&asset.fee)
    .bind(&total_reserved)
    .bind(security_method)
    .bind(idempotency_key)
    .bind(&gateway_request_id)
    .execute(&mut *tx)
    .await;
    let withdrawal_id = match result {
        Ok(result) => result.last_insert_id(),
        Err(error) => {
            tx.rollback().await?;
            return Err(AppError::Database(error));
        }
    };

    let wallet = lock_wallet_balance(&mut tx, user_id, asset.id).await?;
    if wallet.available < total_reserved {
        return Err(AppError::Validation(format!(
            "insufficient available balance for withdrawal: requested {}, available {}",
            total_reserved, wallet.available
        )));
    }
    let available_after = (wallet.available.clone() - total_reserved.clone()).with_scale(18);
    let frozen_after = (wallet.frozen.clone() + total_reserved.clone()).with_scale(18);
    update_wallet_balance(
        &mut tx,
        user_id,
        asset.id,
        &available_after,
        &frozen_after,
        &wallet.locked,
    )
    .await?;
    insert_wallet_ledger_in_tx(
        &mut tx,
        user_id,
        asset.id,
        "withdrawal_reserve",
        &(-total_reserved),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        "wallet_withdrawal_request",
        &withdrawal_id.to_string(),
    )
    .await?;
    let withdrawal = load_withdrawal_by_id_in_tx(&mut tx, withdrawal_id).await?;
    tx.commit().await?;
    Ok(withdrawal)
}

pub(crate) async fn list_wallet_withdrawals(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<WalletWithdrawalResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(wallet_withdrawal_select_sql());
    push_wallet_withdrawal_filters(&mut builder, user_id, status);
    builder.push(WALLET_WITHDRAWAL_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(limit.clamp(1, 200) as i64);
    builder
        .build_query_as::<WalletWithdrawalResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 后台提现列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
pub(crate) async fn list_admin_wallet_withdrawals_page(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<WalletWithdrawalResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(wallet_withdrawal_select_sql());
    let mut total =
        QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM wallet_withdrawal_requests requests");
    for builder in [&mut rows, &mut total] {
        push_wallet_withdrawal_filters(builder, user_id, status);
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        WALLET_WITHDRAWAL_ORDER_BY,
        limit.clamp(1, 200),
        offset,
    )
    .await
}

fn push_wallet_withdrawal_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id: Option<u64>,
    status: Option<&str>,
) {
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = user_id {
        builder.push(" AND requests.user_id = ");
        builder.push_bind(user_id);
    }
    if let Some(status) = status {
        builder.push(" AND requests.status = ");
        builder.push_bind(status.to_owned());
    }
}

pub(crate) async fn approve_withdrawal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    admin_id: u64,
    reason: Option<&str>,
) -> AppResult<WalletWithdrawalResponse> {
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "approved" {
        return Ok(withdrawal);
    }
    if withdrawal.status != "pending_review" {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot be approved from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'approved', reviewed_by = ?, reviewed_at = CURRENT_TIMESTAMP(6),
               review_reason = ?, failure_reason = NULL, next_attempt_at = CURRENT_TIMESTAMP(6)
           WHERE id = ? AND status = 'pending_review'"#,
    )
    .bind(admin_id)
    .bind(reason)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

/// 在拒绝或可安全失败的提现状态下释放 frozen，并把完整预留额退回 available。
/// 已产生链上交易哈希的请求不得通过该路径自动解冻；调用方持有事务并负责同时提交审核状态。
/// 钱包更新与释放流水必须保持三桶总额守恒，目标状态重放直接返回且不重复退款。
pub(crate) async fn release_withdrawal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    admin_id: Option<u64>,
    target_status: &str,
    reason: &str,
) -> AppResult<WalletWithdrawalResponse> {
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == target_status {
        return Ok(withdrawal);
    }
    let release_allowed = match target_status {
        "rejected" => matches!(withdrawal.status.as_str(), "pending_review" | "approved"),
        // 已经取得交易哈希的请求不得自动解冻，必须等待链上确认或进入人工处置。
        "failed" => matches!(withdrawal.status.as_str(), "approved" | "broadcasting"),
        _ => false,
    };
    if !release_allowed {
        return Err(AppError::Conflict(format!(
            "withdrawal reservation cannot be released from status {}",
            withdrawal.status
        )));
    }
    let wallet = lock_wallet_balance(tx, withdrawal.user_id, withdrawal.asset_id).await?;
    if wallet.frozen < withdrawal.total_reserved {
        return Err(AppError::Conflict(
            "withdrawal frozen balance is lower than reserved amount".to_owned(),
        ));
    }
    let available_after =
        (wallet.available.clone() + withdrawal.total_reserved.clone()).with_scale(18);
    let frozen_after = (wallet.frozen.clone() - withdrawal.total_reserved.clone()).with_scale(18);
    update_wallet_balance(
        tx,
        withdrawal.user_id,
        withdrawal.asset_id,
        &available_after,
        &frozen_after,
        &wallet.locked,
    )
    .await?;
    insert_wallet_ledger_in_tx(
        tx,
        withdrawal.user_id,
        withdrawal.asset_id,
        "withdrawal_release",
        &withdrawal.total_reserved,
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        "wallet_withdrawal_request",
        &withdrawal.id.to_string(),
    )
    .await?;
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = ?, failure_reason = ?,
               review_reason = CASE WHEN ? = 'rejected' THEN ? ELSE review_reason END,
               reviewed_by = COALESCE(?, reviewed_by),
               reviewed_at = COALESCE(reviewed_at, CURRENT_TIMESTAMP(6)),
               failed_at = CASE WHEN ? = 'failed' THEN CURRENT_TIMESTAMP(6) ELSE failed_at END,
               failed_by = CASE WHEN ? = 'failed' THEN COALESCE(?, failed_by) ELSE failed_by END,
               released_at = CURRENT_TIMESTAMP(6), next_attempt_at = NULL
           WHERE id = ?"#,
    )
    .bind(target_status)
    .bind(reason)
    .bind(target_status)
    .bind(reason)
    .bind(admin_id)
    .bind(target_status)
    .bind(target_status)
    .bind(admin_id)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

pub(crate) async fn mark_withdrawal_broadcasted_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    admin_id: Option<u64>,
    tx_hash: &str,
    block_height: Option<u64>,
    confirmations: u32,
) -> AppResult<WalletWithdrawalResponse> {
    let tx_hash = normalize_chain_value(tx_hash, "tx_hash", 255)?;
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "broadcasted" && withdrawal.tx_hash.as_deref() == Some(&tx_hash) {
        return update_withdrawal_chain_progress_in_tx(
            tx,
            withdrawal_id,
            &tx_hash,
            block_height,
            confirmations,
        )
        .await;
    }
    if !matches!(withdrawal.status.as_str(), "approved" | "broadcasting") {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot be broadcast from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'broadcasted', tx_hash = ?, block_height = ?,
               confirmations = ?, broadcast_at = CURRENT_TIMESTAMP(6),
               broadcasted_by = COALESCE(?, broadcasted_by), next_attempt_at = NULL
           WHERE id = ? AND status IN ('approved', 'broadcasting')"#,
    )
    .bind(&tx_hash)
    .bind(block_height)
    .bind(confirmations)
    .bind(admin_id)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

/// 在链上广播已确认后核销提现 frozen 预留额，并写入最终确认流水。
/// 仅接受 broadcasted 或人工审核状态；冻结额不足会中止事务，防止账本确认超过真实预留。
/// 已确认请求幂等返回，钱包扣减、确认流水及提现状态由调用方事务原子提交。
pub(crate) async fn confirm_withdrawal_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    admin_id: Option<u64>,
    block_height: Option<u64>,
    confirmations: u32,
) -> AppResult<WalletWithdrawalResponse> {
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "confirmed" {
        return Ok(withdrawal);
    }
    if !matches!(withdrawal.status.as_str(), "broadcasted" | "manual_review") {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot be confirmed from status {}",
            withdrawal.status
        )));
    }
    let wallet = lock_wallet_balance(tx, withdrawal.user_id, withdrawal.asset_id).await?;
    if wallet.frozen < withdrawal.total_reserved {
        return Err(AppError::Conflict(
            "withdrawal frozen balance is lower than reserved amount".to_owned(),
        ));
    }
    let frozen_after = (wallet.frozen.clone() - withdrawal.total_reserved.clone()).with_scale(18);
    update_wallet_balance(
        tx,
        withdrawal.user_id,
        withdrawal.asset_id,
        &wallet.available,
        &frozen_after,
        &wallet.locked,
    )
    .await?;
    insert_wallet_ledger_in_tx(
        tx,
        withdrawal.user_id,
        withdrawal.asset_id,
        "withdrawal_confirm",
        &(-withdrawal.total_reserved.clone()),
        "frozen",
        &frozen_after,
        &wallet.available,
        &frozen_after,
        &wallet.locked,
        "wallet_withdrawal_request",
        &withdrawal.id.to_string(),
    )
    .await?;
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'confirmed', block_height = COALESCE(?, block_height),
               confirmations = GREATEST(confirmations, ?),
               confirmed_at = CURRENT_TIMESTAMP(6),
               confirmed_by = COALESCE(?, confirmed_by), next_attempt_at = NULL
           WHERE id = ? AND status IN ('broadcasted', 'manual_review')"#,
    )
    .bind(block_height)
    .bind(confirmations)
    .bind(admin_id)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

pub(crate) async fn load_withdrawal_by_gateway_request_for_update(
    tx: &mut Transaction<'_, MySql>,
    gateway_request_id: &str,
) -> AppResult<WalletWithdrawalResponse> {
    let gateway_request_id = normalize_chain_value(gateway_request_id, "gateway_request_id", 128)?;
    sqlx::query_as::<_, WalletWithdrawalResponse>(&format!(
        "{} WHERE requests.gateway_request_id = ? FOR UPDATE",
        wallet_withdrawal_select_sql()
    ))
    .bind(gateway_request_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn update_withdrawal_chain_progress_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    tx_hash: &str,
    block_height: Option<u64>,
    confirmations: u32,
) -> AppResult<WalletWithdrawalResponse> {
    let tx_hash = normalize_chain_value(tx_hash, "tx_hash", 255)?;
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if !matches!(
        withdrawal.status.as_str(),
        "broadcasted" | "manual_review" | "confirmed"
    ) {
        return Err(AppError::Conflict(format!(
            "withdrawal chain progress cannot update status {}",
            withdrawal.status
        )));
    }
    if withdrawal.tx_hash.as_deref() != Some(&tx_hash) {
        return Err(AppError::Conflict(
            "withdrawal chain transaction hash does not match".to_owned(),
        ));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET block_height = COALESCE(?, block_height),
               confirmations = GREATEST(confirmations, ?)
           WHERE id = ?"#,
    )
    .bind(block_height)
    .bind(confirmations)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

pub(crate) async fn mark_withdrawal_manual_review_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    reason: &str,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = reason.chars().take(500).collect::<String>();
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "manual_review" {
        return Ok(withdrawal);
    }
    if withdrawal.status != "broadcasted" {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot enter manual review from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'manual_review', failure_reason = ?, next_attempt_at = NULL
           WHERE id = ? AND status = 'broadcasted'"#,
    )
    .bind(reason)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

fn wallet_withdrawal_select_sql() -> &'static str {
    r#"SELECT requests.id, requests.user_id, requests.asset_id, requests.asset_symbol,
              requests.network, requests.address, requests.amount, requests.fee,
              requests.total_reserved, requests.status, requests.security_method,
              requests.idempotency_key, requests.gateway_request_id, requests.tx_hash,
              requests.block_height, requests.confirmations, requests.failure_reason,
              requests.review_reason,
              requests.reviewed_by, requests.broadcasted_by, requests.confirmed_by,
              requests.failed_by, requests.reviewed_at, requests.broadcast_at,
              requests.confirmed_at, requests.failed_at, requests.released_at, requests.created_at
       FROM wallet_withdrawal_requests requests"#
}

fn normalize_chain_value(value: &str, label: &str, max_length: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_length || value.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!("{label} format is invalid")));
    }
    Ok(value.to_owned())
}

async fn load_withdrawal_by_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
) -> AppResult<WalletWithdrawalResponse> {
    sqlx::query_as::<_, WalletWithdrawalResponse>(&format!(
        "{} WHERE requests.id = ? LIMIT 1",
        wallet_withdrawal_select_sql()
    ))
    .bind(withdrawal_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_withdrawal_by_id_for_update(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
) -> AppResult<WalletWithdrawalResponse> {
    sqlx::query_as::<_, WalletWithdrawalResponse>(&format!(
        "{} WHERE requests.id = ? LIMIT 1 FOR UPDATE",
        wallet_withdrawal_select_sql()
    ))
    .bind(withdrawal_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}
