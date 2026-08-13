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
    /// 使用 Reqwest 默认客户端构造链网关适配器，此时不建立连接、不解析端点，也不发送任何请求。
    /// 客户端内部维护连接池，因此该适配器应长期复用；每次调用重新构造会退化成短连接并放大握手开销。
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl WalletChainGateway for HttpWalletChainGateway {
    /// 以 15 秒超时向 endpoint POST 提现广播 JSON，并按需添加 Bearer token。
    /// 请求体包含请求编号、网络、资产、地址以及以字符串承载的金额和费用，定点数转字符串以避免 JSON 浮点精度损失。
    /// 传输失败、非二百响应和响应体反序列化失败被折叠为三类内部错误，原始错误文本随消息透出便于定位。
    /// HTTP/传输/响应 JSON 失败均返回错误；远端可能已受理，调用方不得据超时释放 frozen，应以 request_id 重试或查询。
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

    /// 以 15 秒超时向 endpoint GET 游标页，发送 cursor 与 limit 并解析充提事件集合。
    /// 游标缺省时按空串发送，表示请求首页；数量上限原样透传，是否被远端裁剪由网关自行决定。
    /// 响应页包含下一游标以及充值与提现两组观测，任一组缺省时按空集合解析，不会因字段缺失整页失败。
    /// 本适配器不保存本地游标、不处理钱包；请求或解析失败时由 worker 保持旧游标重试。
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
/// 加载启用提现资产的精度与费率配置，并就地按本次提现金额算出服务端费用。
/// 阶梯费率读出后先做规范化，重叠区间或开放阶梯位置不合法时直接返回校验错误，不退化成固定费用。
/// 规范化通过后按金额命中的阶梯计百分比费用，无命中阶梯时取资产固定费用，结果按资产精度向零截断。
/// 资产关闭提现返回校验错误，资产缺失或已停用返回未找到，两者区分开以便前端给出不同提示。
/// 服务端规则是费用事实源，客户端传入的费用字段不得覆盖此处结果或资产精度合同。
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

/// 按用户与幂等键读取既有提现请求，用于重复请求安全重放。
/// 查询同时限定用户编号，因此不同用户使用相同幂等键互不干扰，也不会跨用户读到他人申请。
/// 返回空值表示该键尚未使用，调用方可继续走创建流程；返回记录时无论其处于哪个状态都原样给出，本函数不过滤终态。
/// 该查询不锁钱包也不锁申请行；重放仍须核对资产、地址、金额和服务端费用完全一致。
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
/// 实际顺序为先插入 pending_review 请求、再锁钱包；total_reserved=本金+服务端费用，按 18 位写入。
/// 先单据后钱包的锁序与审核、释放、确认三条路径完全一致，是本上下文避免钱包与提现单交叉死锁的统一约定。
/// 申请落库时生成时间有序的网关请求编号，作为后续链上回执定位本申请的外部幂等身份，一经写入不再变更。
/// available 减 total_reserved、frozen 加同额、locked 不变；扣减与增加均按 18 位定点计算，三桶总额守恒。
/// 仅写一条 `withdrawal_reserve` available 负流水，业务引用指向新申请编号，frozen 变化由三桶 after 快照体现。
/// 提现记录、钱包与流水由该函数自有事务提交；插入阶段失败显式回滚并原样抛出数据库错误，供上层识别幂等键冲突。
/// 余额不足时提前返回校验错误，事务随作用域结束隐式回滚，因此申请记录不会以无冻结的状态残留。
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

/// 按用户和状态读取提现请求快照，限制单次返回数量且不锁定资金。
/// 用户与状态均为可选条件，两者缺省时返回全量最新申请，因此调用方必须自行限定用户以免越权读取。
/// 返回条数被钳制在一到二百之间，排序固定按申请编号倒序，该入口只取单页且不返回总数。
/// 返回的金额与预留额字段仅为申请当时的快照，不作为新的扣款依据，也不反映钱包三桶的当前值。
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
/// 使用同一用户与状态谓词查询后台提现行和总数。
/// 与用户侧清单相比多返回匹配总数并支持偏移翻页，排序同样固定按申请编号倒序以保证翻页不重不漏。
/// 每页条数被钳制在一到二百之间；行与总数分两次查询执行，并发写入下可能出现总数与当页内容的短暂不一致。
/// 该入口只读请求与链进度，不变更冻结余额、流水或提现状态。
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

/// 为提现行查询与计数查询追加相同的用户和状态谓词，使两者始终描述同一筛选集合。
/// 以恒真条件起头再逐项以并且关系追加，因此两个可选条件都缺省时退化为无过滤的全量查询。
/// 状态按精确值比较且在此拷贝为持有型字符串以延长生命周期，取值合法性由上层在进入本函数前校验。
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

/// 锁定待审核提现并推进为 approved，重复审核已批准记录时幂等返回。
/// 只允许从待审核迁移，其他状态一律返回带原状态的冲突错误，避免把已广播或已失败的申请重新放行。
/// 同时记录审核人、审核时间与审核意见，清空既有失败原因，并把下次尝试时刻置为当前时间让广播 worker 立即可认领。
/// 调用方拥有事务；审批只改状态，不移动 available 或 frozen，也不追加任何资金流水。
/// 状态写入失败由调用方事务整体回滚，不会产生只写审核人却未改状态的部分结果。
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
/// 目标状态只接受拒绝与失败两种：拒绝允许从待审核或已批准迁移，失败允许从已批准或广播中迁移，其余组合一律冲突。
/// 已产生链上交易哈希的请求不得通过该路径自动解冻；调用方持有事务并负责同时提交审核状态。
/// 锁序固定为先按主键锁提现单、再锁钱包账户行，与创建和确认路径同向，杜绝审核与链回执并发时的死锁。
/// 释放前复核 frozen 不小于预留额，不足即返回冲突并由调用方回滚，防止把冻结桶退成负数。
/// available 增 total_reserved、frozen 减同额、locked 不变，两侧均按 18 位定点计算；只写一条 `withdrawal_release` available 正流水，业务引用指向该申请，frozen 变化记录在三桶 after。
/// 状态更新同时按目标状态分别落审核意见、失败原因、失败时间与操作人，并把下次尝试时刻清空以退出广播重试队列。
/// 钱包更新与状态同事务提交并保持三桶总额守恒，目标状态重放直接返回且不重复退款。
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

/// 锁定已批准或广播中的提现并记录链交易哈希及确认进度。
/// 交易哈希先做格式规范：裁剪首尾空白后不得为空、不得超长、不得含空白字符，不合法直接返回校验错误。
/// 若申请已处于已广播且哈希完全相同，则转交进度更新入口只做单调推进，不重复改写广播时间与操作人。
/// 只允许从已批准或广播中迁移；写入哈希、区块高度、确认数与广播时刻，同时清空下次尝试时刻以退出重试队列。
/// 同哈希重放仅更新进度；该状态转换不核销 frozen，也不写任何资金流水，失败时由调用方事务整体回滚。
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
/// 这是提现路径上唯一让资金真正离开钱包的步骤：预留额从 frozen 永久扣除，不回流 available，因此三桶总额在此减少。
/// 仅接受 broadcasted 或人工审核状态；冻结额不足会中止事务，防止账本确认超过真实预留。
/// 锁序沿用先锁提现单再锁钱包账户行，与创建和释放路径同向，保证链回执与后台操作并发时不会互相等待成环。
/// available/locked 原值回写、frozen 减 total_reserved 且按 18 位定点计算；写一条 `withdrawal_confirm` frozen 负流水，金额包含本金和服务端费用，业务引用指向该申请。
/// 状态更新按原状态为已广播或人工审核作为条件，区块高度择非空保留、确认数取历史与本次的较大值，避免链回执乱序回退进度。
/// 已确认请求幂等返回且不二次扣减，钱包扣减、确认流水及提现状态由调用方事务原子提交，任一步失败整体回滚。
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

/// 按链网关 request_id 锁定提现请求，供回调状态机串行处理。
/// 请求锁必须先于钱包锁获取，避免并发链回调重复核销或释放 frozen 预留额。
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

/// 锁定提现并在交易哈希一致时单调增加区块高度与确认数。
/// 入参哈希先经格式规范，随后必须与申请上已记录的哈希完全相同，不同即返回冲突，防止把另一笔链上交易的进度写进本申请。
/// 仅允许广播后、人工审核或已确认状态；区块高度择非空保留、确认数取较大值，因此乱序到达的旧回执不会让进度倒退。
/// 该入口纯粹推进链上观测进度，不移动 available 或 frozen，也不追加资金流水或改变申请状态。
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

/// 把已广播提现转入人工审核并截断保存失败原因，原因按字符截断到五百个以内以适配存储列宽。
/// 只允许从已广播迁移，因为这正是资金已上链但结果不确定的区间；其他状态返回带原状态的冲突错误。
/// 转入后清空下次尝试时刻，使该申请退出自动广播重试，改由人工决定继续确认还是判定失败。
/// 目标状态重放直接返回；冻结预留额继续保留在 frozen，禁止在链结果不明时自动退款或核销。
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

/// 返回提现申请的统一选择列与来源表，供用户清单、后台分页、幂等键查询与各类加锁回读复用同一投影。
/// 投影同时覆盖金额三元组、状态机字段、链上进度、四类操作人和各阶段时间戳，使任一入口都能还原完整申请轨迹。
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

/// 校验并裁剪链上标识，拒绝空串、超长值以及任何含空白字符的取值，错误消息带上字段名便于定位。
/// 长度按字节数而非字符数比较，与数据库列宽口径一致；标识为大小写敏感原文，函数不做大小写归一。
fn normalize_chain_value(value: &str, label: &str, max_length: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_length || value.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(format!("{label} format is invalid")));
    }
    Ok(value.to_owned())
}

/// 在事务内按主键回读提现申请的最新快照，供各状态迁移函数把结果返回给调用方。
/// 该读取刻意不加锁，因为调用方在本次迁移开始时已持有同一行的排他锁，重复加锁只增加等待。
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

/// 按主键对提现申请加排他锁并读出当前状态，是所有状态迁移的统一入口和串行化起点。
/// 该锁必须先于钱包账户锁获取，本文件全部资金路径据此维持先单据后钱包的同向锁序；申请不存在返回未找到。
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
