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
        presentation::{WalletWithdrawalResponse, WithdrawalQuoteResponse},
        repository::{
            WalletChainBroadcastCommand, WalletChainBroadcastResult, WalletChainGateway,
            WalletChainGatewayError, WalletChainGatewayErrorClass, WalletChainPollPage,
            WalletChainWithdrawalQueryResult,
        },
        withdrawal_fee_config_version, withdrawal_quote_fingerprint,
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
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
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError> {
        let mut request = self
            .client
            .post(endpoint)
            .timeout(Duration::from_secs(15))
            .json(command);
        if let Some(token) = bearer_token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(|error| {
            WalletChainGatewayError::new(
                WalletChainGatewayErrorClass::Unknown,
                format!("wallet gateway broadcast outcome is unknown: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            let class = classify_broadcast_http_status(status);
            return Err(WalletChainGatewayError::new(
                class,
                format!("wallet gateway broadcast returned HTTP {status}"),
            ));
        }
        response.json().await.map_err(|error| {
            WalletChainGatewayError::new(
                WalletChainGatewayErrorClass::Unknown,
                format!("wallet gateway broadcast response is invalid: {error}"),
            )
        })
    }

    async fn query_withdrawal(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        request_id: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError> {
        let mut request = self
            .client
            .get(endpoint)
            .timeout(Duration::from_secs(15))
            .query(&[("request_id", request_id)]);
        if let Some(token) = bearer_token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(|error| {
            WalletChainGatewayError::new(
                WalletChainGatewayErrorClass::Unknown,
                format!("wallet gateway status query failed: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(WalletChainGatewayError::new(
                WalletChainGatewayErrorClass::Unknown,
                format!("wallet gateway status query returned HTTP {status}"),
            ));
        }
        response.json().await.map_err(|error| {
            WalletChainGatewayError::new(
                WalletChainGatewayErrorClass::Unknown,
                format!("wallet gateway status response is invalid: {error}"),
            )
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

/// 仅把 HTTP 明确表达“请求未进入受理流程”的客户端错误视为确定拒绝。
/// 408/409/425/429、全部 5xx 与未知扩展状态都可能发生在远端已按 request_id 受理之后，
/// 必须保留冻结并走状态查询，绝不能据此重发或退冻。
pub(crate) fn classify_broadcast_http_status(
    status: reqwest::StatusCode,
) -> WalletChainGatewayErrorClass {
    use reqwest::StatusCode;

    if matches!(
        status,
        StatusCode::BAD_REQUEST
            | StatusCode::UNAUTHORIZED
            | StatusCode::PAYMENT_REQUIRED
            | StatusCode::FORBIDDEN
            | StatusCode::NOT_FOUND
            | StatusCode::METHOD_NOT_ALLOWED
            | StatusCode::NOT_ACCEPTABLE
            | StatusCode::GONE
            | StatusCode::LENGTH_REQUIRED
            | StatusCode::PAYLOAD_TOO_LARGE
            | StatusCode::URI_TOO_LONG
            | StatusCode::UNSUPPORTED_MEDIA_TYPE
            | StatusCode::RANGE_NOT_SATISFIABLE
            | StatusCode::EXPECTATION_FAILED
            | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        WalletChainGatewayErrorClass::DeterministicRejected
    } else {
        WalletChainGatewayErrorClass::Unknown
    }
}
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct WithdrawalAssetRule {
    pub(crate) id: u64,
    pub(crate) precision_scale: i32,
    pub(crate) fee: BigDecimal,
    pub(crate) fee_config_version: String,
}

/// 报价前确认网络当前启用且允许本资产提现。
/// 这个无锁检查只用于阻止创建无效报价；真正动账前会在同一事务内重新加锁确认。
pub(crate) async fn ensure_active_withdrawal_network(
    pool: &Pool<MySql>,
    network: &str,
    asset_symbol: &str,
) -> AppResult<()> {
    let supported = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
               SELECT 1
               FROM deposit_network_configs
               WHERE network = ?
                 AND status = 'active'
                 AND (asset_symbols_json IS NULL
                      OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(?)))
           )"#,
    )
    .bind(network)
    .bind(asset_symbol)
    .fetch_one(pool)
    .await?;
    if supported {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "asset {asset_symbol} does not support withdrawal network {network}"
        )))
    }
}

/// 冻结资金前锁定网络配置并重新校验启用状态与资产白名单。
/// 锁保持到提现单、余额、流水和报价消费一起提交，配置并发变更不会穿过动账边界。
pub(crate) async fn lock_active_withdrawal_network_in_tx(
    tx: &mut Transaction<'_, MySql>,
    network: &str,
    asset_symbol: &str,
) -> AppResult<()> {
    let supported = sqlx::query_scalar::<_, u64>(
        r#"SELECT id
           FROM deposit_network_configs
           WHERE network = ?
             AND status = 'active'
             AND (asset_symbols_json IS NULL
                  OR JSON_CONTAINS(asset_symbols_json, JSON_QUOTE(?)))
           LIMIT 1 FOR UPDATE"#,
    )
    .bind(network)
    .bind(asset_symbol)
    .fetch_optional(&mut **tx)
    .await?
    .is_some();
    if supported {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "withdrawal network configuration changed for asset {asset_symbol} on {network}"
        )))
    }
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
            let fee_config_version =
                withdrawal_fee_config_version(id, precision_scale, &fixed_fee, &tiers);
            Ok(WithdrawalAssetRule {
                id,
                precision_scale,
                fee: calculate_withdraw_fee(amount, &fixed_fee, &tiers, precision_scale),
                fee_config_version,
            })
        }
        Some((_, false, _, _, _)) => Err(AppError::Validation(
            "asset does not support withdraw".to_owned(),
        )),
        None => Err(AppError::NotFound),
    }
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct WithdrawalQuoteRecord {
    pub(crate) quote_id: String,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) network: String,
    pub(crate) amount: BigDecimal,
    pub(crate) fee: BigDecimal,
    pub(crate) net: BigDecimal,
    pub(crate) total_reserved: BigDecimal,
    pub(crate) fee_config_version: String,
    pub(crate) request_fingerprint: String,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) consumed_at: Option<DateTime<Utc>>,
    pub(crate) withdrawal_id: Option<u64>,
}

impl WithdrawalQuoteRecord {
    /// 裁剪持久化报价为公开响应；所有金额均沿用入库快照，不在返回阶段重新计费。
    pub(crate) fn response(&self) -> WithdrawalQuoteResponse {
        WithdrawalQuoteResponse {
            quote_id: self.quote_id.clone(),
            asset_symbol: self.asset_symbol.clone(),
            network: self.network.clone(),
            amount: self.amount.clone(),
            fee: self.fee.clone(),
            net: self.net.clone(),
            total_reserved: self.total_reserved.clone(),
            fee_config_version: self.fee_config_version.clone(),
            expires_at: self.expires_at,
        }
    }
}

/// 在已锁定网络与资产配置的事务中持久化服务端提现报价。
/// 费用、到账额、冻结总额、配置版本、所有者与请求指纹在同一行固化。
pub(crate) async fn insert_withdrawal_quote_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset: &WithdrawalAssetRule,
    asset_symbol: &str,
    network: &str,
    amount: &BigDecimal,
    expires_at: DateTime<Utc>,
) -> AppResult<WithdrawalQuoteResponse> {
    let quote_id = uuid::Uuid::now_v7().to_string();
    // 当前提现模型把手续费加收在本金之外，因此链上净到账等于标准化本金。
    let fee = asset.fee.clone().with_scale(18);
    let net = amount.clone().with_scale(18);
    let total_reserved = (amount.clone() + fee.clone()).with_scale(18);
    let fingerprint =
        withdrawal_quote_fingerprint(user_id, asset.id, asset_symbol, network, amount);
    sqlx::query(
        r#"INSERT INTO wallet_withdrawal_quotes
              (id, user_id, asset_id, asset_symbol, network, amount, fee, net, total_reserved,
               fee_config_version, request_fingerprint, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&quote_id)
    .bind(user_id)
    .bind(asset.id)
    .bind(asset_symbol)
    .bind(network)
    .bind(amount)
    .bind(&fee)
    .bind(&net)
    .bind(&total_reserved)
    .bind(&asset.fee_config_version)
    .bind(&fingerprint)
    .bind(expires_at)
    .execute(&mut **tx)
    .await?;

    Ok(WithdrawalQuoteResponse {
        quote_id,
        asset_symbol: asset_symbol.to_owned(),
        network: network.to_owned(),
        amount: amount.clone(),
        fee,
        net,
        total_reserved,
        fee_config_version: asset.fee_config_version.clone(),
        expires_at,
    })
}

/// 无锁读取用户报价供安全校验前预检；提交事务仍会再次加锁并复核全部版本与参数。
pub(crate) async fn load_withdrawal_quote(
    pool: &Pool<MySql>,
    quote_id: &str,
    user_id: u64,
) -> AppResult<WithdrawalQuoteRecord> {
    sqlx::query_as::<_, WithdrawalQuoteRecord>(&format!(
        "{} WHERE quotes.id = ? AND quotes.user_id = ? LIMIT 1",
        withdrawal_quote_select_sql()
    ))
    .bind(quote_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn load_withdrawal_quote_for_update(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
    user_id: u64,
) -> AppResult<WithdrawalQuoteRecord> {
    sqlx::query_as::<_, WithdrawalQuoteRecord>(&format!(
        "{} WHERE quotes.id = ? AND quotes.user_id = ? LIMIT 1 FOR UPDATE",
        withdrawal_quote_select_sql()
    ))
    .bind(quote_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在提现报价事务内锁定资产配置，并按当前阶梯规则计算本次金额对应的权威手续费。
///
/// 查询同时锁住资产行，使报价保存的费用版本与费率配置来自同一事务快照；资产停用、金额精度非法或
/// 阶梯区间异常都会在写入报价前失败。返回值包含规范化费率版本，后续消费报价时必须再次比对该版本，
/// 从而保证配置变化后的旧报价不会继续冻结用户资金。
pub(crate) async fn load_withdrawal_asset_rule_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_symbol: &str,
    amount: &BigDecimal,
) -> AppResult<WithdrawalAssetRule> {
    let row = sqlx::query_as::<_, (u64, bool, BigDecimal, i32, SqlxJson<Vec<WithdrawFeeTier>>)>(
        r#"SELECT id, withdraw_enabled, withdraw_fee, precision_scale,
                  COALESCE(withdraw_fee_tiers_json, JSON_ARRAY())
           FROM assets
           WHERE symbol = ? AND status = 'active'
           LIMIT 1 FOR UPDATE"#,
    )
    .bind(asset_symbol)
    .fetch_optional(&mut **tx)
    .await?;
    match row {
        Some((id, true, fixed_fee, precision_scale, SqlxJson(tiers))) => {
            let tiers = normalize_withdraw_fee_tiers(tiers).map_err(AppError::Validation)?;
            let fee_config_version =
                withdrawal_fee_config_version(id, precision_scale, &fixed_fee, &tiers);
            Ok(WithdrawalAssetRule {
                id,
                precision_scale,
                fee: calculate_withdraw_fee(amount, &fixed_fee, &tiers, precision_scale),
                fee_config_version,
            })
        }
        Some((_, false, _, _, _)) => Err(AppError::Validation(
            "asset does not support withdraw".to_owned(),
        )),
        None => Err(AppError::NotFound),
    }
}

#[derive(Debug)]
pub(crate) struct ReservedWithdrawal {
    pub(crate) withdrawal: WalletWithdrawalResponse,
    pub(crate) quote: WithdrawalQuoteResponse,
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
    quote_id: &str,
    asset_symbol: &str,
    network: &str,
    address: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
    security_method: &str,
) -> AppResult<ReservedWithdrawal> {
    let gateway_request_id = uuid::Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;

    // 报价是资金事务的第一把锁。报价锁定后才允许锁提现单和钱包，
    // 从而保证同一 quote 的并发提交只有一个能消费并冻结资金。
    let quote = load_withdrawal_quote_for_update(&mut tx, quote_id, user_id).await?;
    let quote_response = quote.response();
    let expected_fingerprint =
        withdrawal_quote_fingerprint(user_id, quote.asset_id, asset_symbol, network, amount);
    if quote.user_id != user_id
        || quote.asset_symbol != asset_symbol
        || quote.network != network
        || quote.amount != *amount
        || quote.request_fingerprint != expected_fingerprint
    {
        return Err(AppError::Conflict(
            "withdrawal quote does not match request parameters".to_owned(),
        ));
    }

    // 已消费报价只能精确重放到原提现单，不会二次冻结。
    if let Some(withdrawal_id) = quote.withdrawal_id {
        let withdrawal = load_withdrawal_by_id_in_tx(&mut tx, withdrawal_id).await?;
        if withdrawal.user_id != user_id
            || withdrawal.withdrawal_quote_id.as_deref() != Some(quote_id)
            || withdrawal.idempotency_key != idempotency_key
            || withdrawal.asset_symbol != asset_symbol
            || withdrawal.network.as_deref() != Some(network)
            || withdrawal.address != address
            || withdrawal.amount != *amount
        {
            return Err(AppError::Conflict(
                "withdrawal quote was already consumed by a different request".to_owned(),
            ));
        }
        tx.commit().await?;
        return Ok(ReservedWithdrawal {
            withdrawal,
            quote: quote_response,
        });
    }
    if quote.consumed_at.is_some() {
        return Err(AppError::Conflict(
            "withdrawal quote is already consumed".to_owned(),
        ));
    }
    if quote.expires_at <= Utc::now() {
        return Err(AppError::Validation(
            "withdrawal quote is expired".to_owned(),
        ));
    }

    // 网络配置与资产费率一样是报价的权威输入。必须在任何钱包动账前加锁复核。
    lock_active_withdrawal_network_in_tx(&mut tx, network, asset_symbol).await?;

    // 消费时再次读取并锁定资产配置。只要费率、阶梯或精度版本有变，
    // 旧报价立即失效，且此分支发生在钱包更新之前。
    let asset = load_withdrawal_asset_rule_in_tx(&mut tx, asset_symbol, amount).await?;
    let current_total = (amount.clone() + asset.fee.clone()).with_scale(18);
    if quote.asset_id != asset.id
        || quote.fee_config_version != asset.fee_config_version
        || quote.fee != asset.fee
        || quote.net != amount.clone().with_scale(18)
        || quote.total_reserved != current_total
    {
        return Err(AppError::Conflict(
            "withdrawal quote fee configuration has changed".to_owned(),
        ));
    }

    let result = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
              (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
               status, security_method, idempotency_key, gateway_request_id, withdrawal_quote_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending_review', ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset.id)
    .bind(asset_symbol)
    .bind(network)
    .bind(address)
    .bind(amount)
    .bind(&quote.fee)
    .bind(&quote.total_reserved)
    .bind(security_method)
    .bind(idempotency_key)
    .bind(&gateway_request_id)
    .bind(quote_id)
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
    if wallet.available < quote.total_reserved {
        return Err(AppError::Validation(format!(
            "insufficient available balance for withdrawal: requested {}, available {}",
            quote.total_reserved, wallet.available
        )));
    }
    let available_after = (wallet.available.clone() - quote.total_reserved.clone()).with_scale(18);
    let frozen_after = (wallet.frozen.clone() + quote.total_reserved.clone()).with_scale(18);
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
        &(-quote.total_reserved.clone()),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        "wallet_withdrawal_request",
        &withdrawal_id.to_string(),
    )
    .await?;
    let consumed = sqlx::query(
        r#"UPDATE wallet_withdrawal_quotes
           SET consumed_at = CURRENT_TIMESTAMP(6), withdrawal_id = ?
           WHERE id = ? AND user_id = ? AND consumed_at IS NULL AND withdrawal_id IS NULL"#,
    )
    .bind(withdrawal_id)
    .bind(quote_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    if consumed.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "withdrawal quote was consumed concurrently".to_owned(),
        ));
    }
    let withdrawal = load_withdrawal_by_id_in_tx(&mut tx, withdrawal_id).await?;
    tx.commit().await?;
    Ok(ReservedWithdrawal {
        withdrawal,
        quote: quote_response,
    })
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
/// 目标状态只接受拒绝与失败两种：拒绝允许从待审核或已批准迁移，通用失败只允许从尚未发送的已批准状态迁移。
/// 广播中/结果不明必须改走带权威未受理证据的专用入口，不允许仅凭人工失败原因解冻。
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
        "rejected" => {
            matches!(withdrawal.status.as_str(), "pending_review" | "approved")
                && withdrawal.acceptance_evidence_at.is_none()
        }
        // 已经取得交易哈希的请求不得自动解冻，必须等待链上确认或进入人工处置。
        // 广播中即使未观测到 tx 也可能已被远端受理；只有专用的权威未受理入口能退冻。
        "failed" => withdrawal.status == "approved" && withdrawal.acceptance_evidence_at.is_none(),
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

/// 只供网关状态查询的权威“未受理”结果使用：将歧义状态标记为可安全释放后，
/// 复用统一退冻逻辑。无交易哈希和来源状态是硬前置，因此人工 fail 路由不能借此释放 unknown。
pub(crate) async fn release_authoritatively_not_accepted_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    reason: &str,
) -> AppResult<WalletWithdrawalResponse> {
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "failed"
        && withdrawal.broadcast_resolution.as_deref() == Some("authoritative_not_accepted")
    {
        return Ok(withdrawal);
    }
    if !matches!(
        withdrawal.status.as_str(),
        "approved" | "unknown_broadcast" | "broadcasting" | "manual_review"
    ) || withdrawal.tx_hash.is_some()
        || withdrawal.acceptance_evidence_at.is_some()
    {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot be released without authoritative non-acceptance from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'approved', broadcast_resolution = 'authoritative_not_accepted'
           WHERE id = ? AND status IN ('approved', 'unknown_broadcast', 'broadcasting', 'manual_review') AND tx_hash IS NULL"#,
    )
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    release_withdrawal_in_tx(tx, withdrawal_id, None, "failed", reason).await
}

/// 将广播结果不明的请求留在 frozen；达到尝试上限时改进人工审核，同样不移动资金。
pub(crate) async fn mark_withdrawal_unknown_broadcast_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    error_class: &str,
    reason: &str,
    next_attempt_seconds: u64,
    manual_review: bool,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = bounded_text(reason, 500);
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "confirmed" || withdrawal.status == "failed" {
        return Ok(withdrawal);
    }
    // 人工审核是自动状态机的吸收态；后台明确处置前，查询失败不得把它重新排入自动队列。
    if withdrawal.status == "manual_review" {
        return Ok(withdrawal);
    }
    if !matches!(
        withdrawal.status.as_str(),
        "broadcasting" | "unknown_broadcast" | "manual_review"
    ) || withdrawal.tx_hash.is_some()
        || withdrawal.acceptance_evidence_at.is_some()
    {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot record an unknown broadcast from status {}",
            withdrawal.status
        )));
    }
    let target = if manual_review {
        "manual_review"
    } else {
        "unknown_broadcast"
    };
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = ?, broadcast_error_class = ?, broadcast_last_error = ?,
               failure_reason = ?, broadcast_resolution = NULL,
               next_attempt_at = CASE WHEN ? THEN NULL
                                      ELSE DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND) END,
               manual_review_at = CASE WHEN ? THEN COALESCE(manual_review_at, CURRENT_TIMESTAMP(6))
                                       ELSE manual_review_at END
           WHERE id = ? AND status IN ('broadcasting', 'unknown_broadcast', 'manual_review')"#,
    )
    .bind(target)
    .bind(error_class)
    .bind(&reason)
    .bind(&reason)
    .bind(manual_review)
    .bind(next_attempt_seconds as i64)
    .bind(manual_review)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

/// 远端权威确认未受理但仍有重试额度时，回到 approved 等待以同一 request_id 重发。
pub(crate) async fn schedule_withdrawal_after_not_accepted_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    reason: &str,
    next_attempt_seconds: u64,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = bounded_text(reason, 500);
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if !matches!(
        withdrawal.status.as_str(),
        "unknown_broadcast" | "broadcasting"
    ) || withdrawal.tx_hash.is_some()
        || withdrawal.acceptance_evidence_at.is_some()
    {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot retry after non-acceptance from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'approved', broadcast_resolution = 'authoritative_not_accepted',
               broadcast_last_error = ?, failure_reason = ?,
               next_attempt_at = DATE_ADD(CURRENT_TIMESTAMP(6), INTERVAL ? SECOND)
           WHERE id = ? AND status IN ('unknown_broadcast', 'broadcasting') AND tx_hash IS NULL"#,
    )
    .bind(&reason)
    .bind(&reason)
    .bind(next_attempt_seconds as i64)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

/// 把交易哈希、区块高度或确认数等受理证据永久钉在提现单上并转人工审核。
/// 即使后续网关返回不带证据的 `not_accepted`，权威释放入口也会因该时间戳拒绝退冻。
pub(crate) async fn mark_withdrawal_acceptance_evidence_for_manual_review_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    reason: &str,
    tx_hash: Option<&str>,
    block_height: Option<u64>,
    confirmations: u32,
) -> AppResult<WalletWithdrawalResponse> {
    let reason = bounded_text(reason, 500);
    let tx_hash = tx_hash
        .map(|value| normalize_chain_value(value, "tx_hash", 255))
        .transpose()?;
    let withdrawal = load_withdrawal_by_id_for_update(tx, withdrawal_id).await?;
    if withdrawal.status == "confirmed" {
        return Ok(withdrawal);
    }
    if !matches!(
        withdrawal.status.as_str(),
        "approved" | "broadcasting" | "unknown_broadcast" | "broadcasted" | "manual_review"
    ) {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot record acceptance evidence from status {}",
            withdrawal.status
        )));
    }
    if let (Some(existing), Some(observed)) = (withdrawal.tx_hash.as_deref(), tx_hash.as_deref())
        && existing != observed
    {
        return Err(AppError::Conflict(
            "withdrawal chain transaction hash does not match".to_owned(),
        ));
    }

    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'manual_review', tx_hash = COALESCE(tx_hash, ?),
               block_height = IF(? IS NULL, block_height,
                                 GREATEST(COALESCE(block_height, 0), ?)),
               confirmations = GREATEST(confirmations, ?),
               acceptance_evidence_at = COALESCE(acceptance_evidence_at, CURRENT_TIMESTAMP(6)),
               broadcast_resolution = CASE WHEN COALESCE(tx_hash, ?) IS NULL
                                           THEN NULL ELSE 'accepted' END,
               failure_reason = ?, next_attempt_at = NULL,
               manual_review_at = COALESCE(manual_review_at, CURRENT_TIMESTAMP(6))
           WHERE id = ?
             AND status IN ('approved', 'broadcasting', 'unknown_broadcast', 'broadcasted', 'manual_review')"#,
    )
    .bind(tx_hash.as_deref())
    .bind(block_height)
    .bind(block_height)
    .bind(confirmations)
    .bind(tx_hash.as_deref())
    .bind(&reason)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

/// 广播与查询的每个决策点都以稳定 event_key 留痕，重启/重放命中唯一键而不重复增长。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_withdrawal_broadcast_audit_in_tx(
    tx: &mut Transaction<'_, MySql>,
    withdrawal_id: u64,
    gateway_request_id: &str,
    event_key: &str,
    event_type: &str,
    outcome_class: &str,
    tx_hash: Option<&str>,
    detail: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_withdrawal_broadcast_audits
              (withdrawal_id, gateway_request_id, event_key, event_type, outcome_class,
               tx_hash, detail)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE event_key = VALUES(event_key)"#,
    )
    .bind(withdrawal_id)
    .bind(gateway_request_id)
    .bind(bounded_text(event_key, 160))
    .bind(event_type)
    .bind(outcome_class)
    .bind(tx_hash)
    .bind(detail.map(|value| bounded_text(value, 500)))
    .execute(&mut **tx)
    .await?;
    Ok(())
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
    if let Some(existing_tx_hash) = withdrawal.tx_hash.as_deref()
        && existing_tx_hash != tx_hash
    {
        return Err(AppError::Conflict(
            "withdrawal chain transaction hash does not match".to_owned(),
        ));
    }
    if !matches!(
        withdrawal.status.as_str(),
        "approved" | "broadcasting" | "unknown_broadcast" | "manual_review"
    ) {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot be broadcast from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'broadcasted', tx_hash = ?,
               block_height = IF(? IS NULL, block_height,
                                 GREATEST(COALESCE(block_height, 0), ?)),
               confirmations = GREATEST(confirmations, ?),
               broadcast_at = COALESCE(broadcast_at, CURRENT_TIMESTAMP(6)),
               broadcasted_by = COALESCE(?, broadcasted_by), next_attempt_at = NULL,
               broadcast_resolution = 'accepted',
               acceptance_evidence_at = COALESCE(acceptance_evidence_at, CURRENT_TIMESTAMP(6))
           WHERE id = ? AND status IN ('approved', 'broadcasting', 'unknown_broadcast', 'manual_review')"#,
    )
    .bind(&tx_hash)
    .bind(block_height)
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
/// 仅接受已有交易哈希的 broadcasted 或人工审核状态；冻结额不足会中止事务，防止账本确认超过真实预留。
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
    if withdrawal.tx_hash.is_none() {
        return Err(AppError::Conflict(
            "withdrawal cannot be confirmed without an accepted transaction hash".to_owned(),
        ));
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
           SET status = 'confirmed',
               block_height = IF(? IS NULL, block_height,
                                 GREATEST(COALESCE(block_height, 0), ?)),
               confirmations = GREATEST(confirmations, ?),
               confirmed_at = CURRENT_TIMESTAMP(6),
               confirmed_by = COALESCE(?, confirmed_by), next_attempt_at = NULL
           WHERE id = ? AND status IN ('broadcasted', 'manual_review')"#,
    )
    .bind(block_height)
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
           SET block_height = IF(? IS NULL, block_height,
                                 GREATEST(COALESCE(block_height, 0), ?)),
               confirmations = GREATEST(confirmations, ?)
           WHERE id = ?"#,
    )
    .bind(block_height)
    .bind(block_height)
    .bind(confirmations)
    .bind(withdrawal_id)
    .execute(&mut **tx)
    .await?;
    load_withdrawal_by_id_in_tx(tx, withdrawal_id).await
}

/// 把已批准、广播中、广播结果不明或已广播的提现转入人工审核，并截断保存失败原因。
/// 这些状态都可能包含远端受理的不确定性；其他状态返回带原状态的冲突错误。
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
    if !matches!(
        withdrawal.status.as_str(),
        "approved" | "broadcasted" | "broadcasting" | "unknown_broadcast"
    ) {
        return Err(AppError::Conflict(format!(
            "withdrawal cannot enter manual review from status {}",
            withdrawal.status
        )));
    }
    sqlx::query(
        r#"UPDATE wallet_withdrawal_requests
           SET status = 'manual_review', failure_reason = ?, next_attempt_at = NULL,
               manual_review_at = COALESCE(manual_review_at, CURRENT_TIMESTAMP(6))
           WHERE id = ? AND status IN ('approved', 'broadcasted', 'broadcasting', 'unknown_broadcast')"#,
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
              requests.idempotency_key, requests.gateway_request_id,
              requests.withdrawal_quote_id, requests.tx_hash,
              requests.block_height, requests.confirmations, requests.failure_reason,
              requests.broadcast_error_class, requests.broadcast_last_error,
              requests.broadcast_resolution, requests.acceptance_evidence_at,
              requests.review_reason,
              requests.reviewed_by, requests.broadcasted_by, requests.confirmed_by,
              requests.failed_by, requests.retry_count, requests.gateway_query_count,
              requests.reviewed_at, requests.broadcast_at,
              requests.confirmed_at, requests.failed_at, requests.released_at, requests.created_at
              , requests.last_gateway_query_at, requests.manual_review_at
       FROM wallet_withdrawal_requests requests"#
}

fn withdrawal_quote_select_sql() -> &'static str {
    r#"SELECT quotes.id AS quote_id, quotes.user_id, quotes.asset_id,
              quotes.asset_symbol, quotes.network, quotes.amount, quotes.fee,
              quotes.net, quotes.total_reserved, quotes.fee_config_version,
              quotes.request_fingerprint, quotes.expires_at, quotes.consumed_at,
              quotes.withdrawal_id
       FROM wallet_withdrawal_quotes quotes"#
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

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
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
