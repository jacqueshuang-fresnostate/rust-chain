//! quick_recharge bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use super::{
    repository::{
        QuickRechargeAdminOrderFilter, QuickRechargeAssetRow, QuickRechargeConfigRow,
        QuickRechargeConfigWrite, QuickRechargeOrderCreateWrite, QuickRechargeOrderPaidUpdate,
        QuickRechargeOrderProviderUpdate, QuickRechargeOrderRow, QuickRechargeUserOrderFilter,
        QuickRechargeWalletRow,
    },
    service::{QuickRechargeRuntimeConfig, decimal_to_gmpay_string, gmpay_signature, optional_str},
};
use crate::error::{AppError, AppResult};
use axum::http::StatusCode;
use bigdecimal::BigDecimal;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::collections::BTreeMap;

const DEFAULT_CONFIG_NAME: &str = "default";
const DEFAULT_PROVIDER: &str = "gmpay";
pub(crate) const GMPAY_REQUEST_FAILED_CODE: &str = "GMPAY_REQUEST_FAILED";
const GMPAY_USER_AGENT: &str = "RustChain/1.0 quick-recharge";
const QUICK_RECHARGE_CHANGE_TYPE: &str = "quick_recharge";
const QUICK_RECHARGE_REF_TYPE: &str = "quick_recharge";

#[derive(Debug, Clone, sqlx::FromRow)]
struct QuickRechargeConfigSqlRow {
    id: u64,
    name: String,
    provider: String,
    enabled: bool,
    api_base_url: Option<String>,
    merchant_pid: Option<String>,
    merchant_secret_ciphertext: Option<String>,
    merchant_secret_mask: Option<String>,
    currency: String,
    token: String,
    network: String,
    notify_url: Option<String>,
    redirect_url: Option<String>,
    pc_app_redirect_url: Option<String>,
    mac_app_redirect_url: Option<String>,
    ios_app_redirect_url: Option<String>,
    android_app_redirect_url: Option<String>,
    mobile_web_redirect_url: Option<String>,
    desktop_web_redirect_url: Option<String>,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    updated_by: Option<u64>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<QuickRechargeConfigSqlRow> for QuickRechargeConfigRow {
    /// 将配置 SQL 行逐字段转换为领域侧持久化快照，不解密商户密钥。
    fn from(row: QuickRechargeConfigSqlRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            provider: row.provider,
            enabled: row.enabled,
            api_base_url: row.api_base_url,
            merchant_pid: row.merchant_pid,
            merchant_secret_ciphertext: row.merchant_secret_ciphertext,
            merchant_secret_mask: row.merchant_secret_mask,
            currency: row.currency,
            token: row.token,
            network: row.network,
            notify_url: row.notify_url,
            redirect_url: row.redirect_url,
            pc_app_redirect_url: row.pc_app_redirect_url,
            mac_app_redirect_url: row.mac_app_redirect_url,
            ios_app_redirect_url: row.ios_app_redirect_url,
            android_app_redirect_url: row.android_app_redirect_url,
            mobile_web_redirect_url: row.mobile_web_redirect_url,
            desktop_web_redirect_url: row.desktop_web_redirect_url,
            min_amount: row.min_amount,
            max_amount: row.max_amount,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct QuickRechargeOrderSqlRow {
    id: u64,
    order_id: String,
    user_id: u64,
    user_email: Option<String>,
    asset_id: u64,
    asset_symbol: String,
    currency: String,
    token: String,
    network: String,
    fiat_amount: BigDecimal,
    actual_amount: Option<BigDecimal>,
    provider_trade_id: Option<String>,
    receive_address: Option<String>,
    payment_url: Option<String>,
    return_target: Option<String>,
    redirect_url: Option<String>,
    expiration_time: Option<i64>,
    status: String,
    block_transaction_id: Option<String>,
    paid_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<QuickRechargeOrderSqlRow> for QuickRechargeOrderRow {
    /// 将订单联表 SQL 行映射为本地订单快照，不触发支付方查询或钱包入账。
    fn from(row: QuickRechargeOrderSqlRow) -> Self {
        Self {
            id: row.id,
            order_id: row.order_id,
            user_id: row.user_id,
            user_email: row.user_email,
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            currency: row.currency,
            token: row.token,
            network: row.network,
            fiat_amount: row.fiat_amount,
            actual_amount: row.actual_amount,
            provider_trade_id: row.provider_trade_id,
            receive_address: row.receive_address,
            payment_url: row.payment_url,
            return_target: row.return_target,
            redirect_url: row.redirect_url,
            expiration_time: row.expiration_time,
            status: row.status,
            block_transaction_id: row.block_transaction_id,
            paid_at: row.paid_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct QuickRechargeAssetSqlRow {
    id: u64,
    symbol: String,
}

impl From<QuickRechargeAssetSqlRow> for QuickRechargeAssetRow {
    /// 将活动资产查询行映射为快充资产标识，不创建钱包账户。
    fn from(row: QuickRechargeAssetSqlRow) -> Self {
        Self {
            id: row.id,
            symbol: row.symbol,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct QuickRechargeWalletSqlRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

impl From<QuickRechargeWalletSqlRow> for QuickRechargeWalletRow {
    /// 将已锁钱包 SQL 行映射为 available/frozen/locked 快照，不修改任一余额桶。
    fn from(row: QuickRechargeWalletSqlRow) -> Self {
        Self {
            available: row.available,
            frozen: row.frozen,
            locked: row.locked,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GmpayCreateOrderResponse {
    status_code: i32,
    message: Option<String>,
    data: Option<GmpayCreateOrderData>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GmpayCreateOrderData {
    pub(crate) trade_id: String,
    pub(crate) order_id: String,
    pub(crate) amount: BigDecimal,
    pub(crate) currency: String,
    pub(crate) actual_amount: BigDecimal,
    pub(crate) receive_address: String,
    pub(crate) token: String,
    pub(crate) expiration_time: Option<i64>,
    pub(crate) payment_url: String,
}

/// 按运行时配置向 GMPay 创建支付订单，并复用默认商品名称。
/// 该外部调用不持有数据库事务；超时或响应异常由应用层尝试把本地订单标记 failed，远端可能已受理且不会被撤销。
pub(crate) async fn create_gmpay_order(
    config: &QuickRechargeRuntimeConfig,
    order_id: &str,
    amount: &BigDecimal,
    redirect_url: Option<&str>,
) -> AppResult<GmpayCreateOrderData> {
    create_gmpay_order_with_name(config, order_id, amount, "Quick Recharge", redirect_url).await
}

/// 按运行时配置组装并签名 GMPay 创建订单表单；调用方须提供唯一订单号、合法金额和已解密商户密钥。
/// 参数使用稳定键序参与签名，可选跳转地址优先采用本次调用值，否则回退配置默认值。
/// 本函数只执行支付方 HTTP 请求并校验 HTTP、JSON 与业务状态，不开启数据库事务，也不触碰钱包或流水。
/// 外部订单可能在响应失败前已创建；本函数不提供本地重放，调用方须以固定订单号核对响应并决定重试。
pub(crate) async fn create_gmpay_order_with_name(
    config: &QuickRechargeRuntimeConfig,
    order_id: &str,
    amount: &BigDecimal,
    order_name: &str,
    redirect_url: Option<&str>,
) -> AppResult<GmpayCreateOrderData> {
    let mut params = BTreeMap::new();
    params.insert("pid".to_owned(), config.merchant_pid.clone());
    params.insert("order_id".to_owned(), order_id.to_owned());
    params.insert("currency".to_owned(), config.currency.clone());
    params.insert("token".to_owned(), config.token.clone());
    params.insert("network".to_owned(), config.network.clone());
    params.insert("amount".to_owned(), decimal_to_gmpay_string(amount));
    params.insert("notify_url".to_owned(), config.notify_url.clone());
    let redirect_url = redirect_url
        .and_then(optional_str)
        .or_else(|| config.redirect_url.as_deref().and_then(optional_str));
    if let Some(redirect_url) = redirect_url {
        params.insert("redirect_url".to_owned(), redirect_url.to_owned());
    }
    params.insert("name".to_owned(), order_name.to_owned());
    let signature = gmpay_signature(&params, &config.merchant_secret);
    params.insert("signature".to_owned(), signature);

    let url = format!(
        "{}/payments/gmpay/v1/order/create-transaction",
        config.api_base_url.trim_end_matches('/')
    );
    let response = reqwest::Client::new()
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, GMPAY_USER_AGENT)
        .form(&params)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("gmpay request failed: {error}")))?;
    let http_status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .text()
        .await
        .map_err(|error| AppError::Internal(format!("gmpay response read failed: {error}")))?;
    if !http_status.is_success() {
        return Err(AppError::Api {
            status: StatusCode::BAD_GATEWAY,
            code: GMPAY_REQUEST_FAILED_CODE,
            message: format_gmpay_http_error(http_status, content_type.as_deref(), &body),
        });
    }
    let payload = serde_json::from_str::<GmpayCreateOrderResponse>(&body).map_err(|error| {
        if is_gmpay_html_response(content_type.as_deref(), &body) {
            AppError::Api {
                status: StatusCode::BAD_GATEWAY,
                code: GMPAY_REQUEST_FAILED_CODE,
                message: format_gmpay_html_response_message(http_status),
            }
        } else {
            AppError::Internal(format!(
                "gmpay response json is invalid: {error}; body: {}",
                compact_response_body(&body)
            ))
        }
    })?;
    if payload.status_code != 200 {
        return Err(AppError::Api {
            status: StatusCode::BAD_GATEWAY,
            code: GMPAY_REQUEST_FAILED_CODE,
            message: payload
                .message
                .unwrap_or_else(|| "gmpay create order failed".to_owned()),
        });
    }
    payload
        .data
        .ok_or_else(|| AppError::Internal("gmpay response data is missing".to_owned()))
}

/// 按用户、可选状态和数量上限读取快充订单快照。
/// 查询只读本地状态，不锁订单，也不触发支付方调用或钱包入账。
pub(crate) async fn list_user_orders(
    pool: &Pool<MySql>,
    filter: QuickRechargeUserOrderFilter,
) -> AppResult<Vec<QuickRechargeOrderRow>> {
    let mut builder = quick_recharge_order_query();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(filter.user_id);
    if let Some(status) = filter.status {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);
    let rows = builder
        .build_query_as::<QuickRechargeOrderSqlRow>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// 后台快充订单列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 使用同一后台筛选谓词查询快充订单行和总数。
/// 该只读入口不锁订单，也不修改支付状态、钱包余额或流水。
pub(crate) async fn list_admin_orders(
    pool: &Pool<MySql>,
    filter: QuickRechargeAdminOrderFilter,
) -> AppResult<(Vec<QuickRechargeOrderRow>, i64)> {
    let mut rows = quick_recharge_order_query();
    let mut total = quick_recharge_order_count_query();
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            builder.push(" AND orders.user_id = ");
            builder.push_bind(user_id);
        }
        if let Some(email) = filter.email.clone() {
            builder.push(" AND users.email = ");
            builder.push_bind(email);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND orders.status = ");
            builder.push_bind(status);
        }
        if let Some(order_id) = filter.order_id.clone() {
            builder.push(" AND orders.order_id = ");
            builder.push_bind(order_id);
        }
        if let Some(provider_trade_id) = filter.provider_trade_id.clone() {
            builder.push(" AND orders.provider_trade_id = ");
            builder.push_bind(provider_trade_id);
        }
    }

    let (rows, total) = fetch_admin_page::<QuickRechargeOrderSqlRow>(
        pool,
        rows,
        total,
        " ORDER BY orders.created_at DESC, orders.id DESC",
        filter.limit,
        filter.offset,
    )
    .await?;
    Ok((rows.into_iter().map(Into::into).collect(), total))
}

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

/// 按公开订单号读取快充订单及关联用户邮箱，缺失时返回未找到。
/// 查询不加行锁，仅用于展示或提交后的回读，不可作为资金回调的并发判定依据。
pub(crate) async fn load_order_by_order_id(
    pool: &Pool<MySql>,
    order_id: &str,
) -> AppResult<QuickRechargeOrderRow> {
    let mut builder = quick_recharge_order_query();
    builder.push(" WHERE orders.order_id = ");
    builder.push_bind(order_id.to_owned());
    builder
        .build_query_as::<QuickRechargeOrderSqlRow>()
        .fetch_optional(pool)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 在调用支付方前持久化 created 快充订单，记录币种、资产和回跳地址快照。
/// 每次调用写入新的业务订单号且无请求幂等键；外部调用失败后应用层另行推进 failed。
/// 本入口不修改 available/frozen/locked、不创建资金流水，后续支付请求也不与该写入共享事务。
pub(crate) async fn insert_created_order(
    pool: &Pool<MySql>,
    write: &QuickRechargeOrderCreateWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO quick_recharge_orders
           (order_id, user_id, user_email, asset_id, asset_symbol, currency, token, network,
            fiat_amount, return_target, redirect_url, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'created')"#,
    )
    .bind(&write.order_id)
    .bind(write.user_id)
    .bind(&write.user_email)
    .bind(write.asset_id)
    .bind(&write.asset_symbol)
    .bind(&write.currency)
    .bind(&write.token)
    .bind(&write.network)
    .bind(&write.fiat_amount)
    .bind(&write.return_target)
    .bind(&write.redirect_url)
    .execute(pool)
    .await?;
    Ok(())
}

/// 把支付方订单号、到账数量、地址和支付链接写回本地并推进为 pending。
/// 该自动提交更新发生在外部订单已创建之后；失败不会撤销远端订单，本地记录可能继续停在 created。
/// pending 仅表示支付准备完成，不代表到账，也不增加钱包 available 或写流水。
pub(crate) async fn mark_order_pending_with_provider(
    pool: &Pool<MySql>,
    update: &QuickRechargeOrderProviderUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE quick_recharge_orders
           SET status = 'pending',
               provider_trade_id = ?,
               actual_amount = ?,
               receive_address = ?,
               payment_url = ?,
               expiration_time = ?,
               currency = ?,
               token = ?
           WHERE order_id = ?"#,
    )
    .bind(&update.provider_trade_id)
    .bind(&update.actual_amount)
    .bind(&update.receive_address)
    .bind(&update.payment_url)
    .bind(update.expiration_time)
    .bind(update.currency.to_ascii_lowercase())
    .bind(update.token.to_ascii_lowercase())
    .bind(&update.order_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 在调用方事务中按公开订单号锁定快充订单，串行处理回调或后台删除。
/// 资金回调保持订单先锁、钱包后锁；事务结束前其他处理不得越过状态判断。
pub(crate) async fn lock_order_by_order_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: &str,
) -> AppResult<QuickRechargeOrderRow> {
    let mut builder = quick_recharge_order_query();
    builder.push(" WHERE orders.order_id = ");
    builder.push_bind(order_id.to_owned());
    builder.push(" FOR UPDATE");
    builder
        .build_query_as::<QuickRechargeOrderSqlRow>()
        .fetch_optional(&mut **tx)
        .await?
        .map(Into::into)
        .ok_or(AppError::NotFound)
}

/// 尝试把指定本地订单标记为 failed；不检查当前状态或受影响行数，也不取消可能已创建的 GMPay 订单。
/// 本函数不触碰钱包；SQL 失败时调用方会收到错误，本地订单可能继续保持 created/pending。
pub(crate) async fn mark_order_failed(pool: &Pool<MySql>, order_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE quick_recharge_orders SET status = 'failed' WHERE order_id = ?")
        .bind(order_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 在当前事务按稳定引用检查快充订单是否已有入账流水，作为删除与重放护栏。
/// 只要存在流水就视为资金已发生，调用方不得删除订单或再次增加 available。
pub(crate) async fn has_wallet_ledger_for_order(
    tx: &mut Transaction<'_, MySql>,
    order_id: &str,
) -> AppResult<bool> {
    let ledger_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE ref_type = ? AND ref_id = ?"#,
    )
    .bind(QUICK_RECHARGE_REF_TYPE)
    .bind(order_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(ledger_count > 0)
}

/// 在调用方事务删除已确认未支付且无资金流水的快充订单。
pub(crate) async fn delete_order_by_id(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM quick_recharge_orders WHERE id = ?")
        .bind(order_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方已锁订单的回调事务中保存 paid、交易信息、actual_amount 与已验签原始载荷。
/// 本函数本身不检查前置状态、不修改钱包；应用层随后在同一事务增加 available 并写流水，失败时 paid 更新一起回滚。
pub(crate) async fn mark_order_paid_from_notify(
    tx: &mut Transaction<'_, MySql>,
    update: &QuickRechargeOrderPaidUpdate,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE quick_recharge_orders
           SET status = 'paid',
               provider_trade_id = COALESCE(provider_trade_id, ?),
               actual_amount = ?,
               receive_address = COALESCE(?, receive_address),
               block_transaction_id = ?,
               callback_payload_json = ?,
               paid_at = CURRENT_TIMESTAMP(6)
           WHERE order_id = ?"#,
    )
    .bind(&update.provider_trade_id)
    .bind(&update.actual_amount)
    .bind(&update.receive_address)
    .bind(&update.block_transaction_id)
    .bind(SqlxJson(update.callback_payload_json.clone()))
    .bind(&update.order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 读取默认快充单例配置，缺失时返回未找到。
/// 返回密钥密文仅供受控解密，用户响应和日志不得暴露密钥明文。
pub(crate) async fn load_config_row(pool: &Pool<MySql>) -> AppResult<QuickRechargeConfigRow> {
    sqlx::query_as::<_, QuickRechargeConfigSqlRow>(
        r#"SELECT id, name, provider, enabled, api_base_url, merchant_pid,
                  merchant_secret_ciphertext, merchant_secret_mask, currency, token, network,
                  notify_url, redirect_url, pc_app_redirect_url, mac_app_redirect_url,
                  ios_app_redirect_url, android_app_redirect_url, mobile_web_redirect_url,
                  desktop_web_redirect_url, min_amount, max_amount, updated_by, created_at, updated_at
           FROM quick_recharge_configs
           WHERE name = ?"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_optional(pool)
    .await?
    .map(Into::into)
    .ok_or(AppError::NotFound)
}

/// 在当前事务快照中回读默认快充配置，供保存后审计。
/// 读取沿用调用方事务，不自行提交，确保审计对应同一配置版本。
pub(crate) async fn load_config_row_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<QuickRechargeConfigRow> {
    sqlx::query_as::<_, QuickRechargeConfigSqlRow>(
        r#"SELECT id, name, provider, enabled, api_base_url, merchant_pid,
                  merchant_secret_ciphertext, merchant_secret_mask, currency, token, network,
                  notify_url, redirect_url, pc_app_redirect_url, mac_app_redirect_url,
                  ios_app_redirect_url, android_app_redirect_url, mobile_web_redirect_url,
                  desktop_web_redirect_url, min_amount, max_amount, updated_by, created_at, updated_at
           FROM quick_recharge_configs
           WHERE name = ?"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_optional(&mut **tx)
    .await?
    .map(Into::into)
    .ok_or(AppError::NotFound)
}

/// 锁定默认快充配置行，串行执行密钥沿用、更新和前后审计。
/// 行锁持续到配置和审计共同提交，失败时旧密钥与配置继续保持有效。
pub(crate) async fn lock_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<Option<QuickRechargeConfigRow>> {
    let row = sqlx::query_as::<_, QuickRechargeConfigSqlRow>(
        r#"SELECT id, name, provider, enabled, api_base_url, merchant_pid,
                  merchant_secret_ciphertext, merchant_secret_mask, currency, token, network,
                  notify_url, redirect_url, pc_app_redirect_url, mac_app_redirect_url,
                  ios_app_redirect_url, android_app_redirect_url, mobile_web_redirect_url,
                  desktop_web_redirect_url, min_amount, max_amount, updated_by, created_at, updated_at
           FROM quick_recharge_configs
           WHERE name = ?
           FOR UPDATE"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)?;
    Ok(row.map(Into::into))
}

/// 在调用方事务插入或覆盖快充单例配置及加密密钥字段。
/// 配置写入必须与管理员审计原子提交，失败时旧配置继续生效。
pub(crate) async fn upsert_config(
    tx: &mut Transaction<'_, MySql>,
    write: &QuickRechargeConfigWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO quick_recharge_configs
           (name, provider, enabled, api_base_url, merchant_pid, merchant_secret_ciphertext,
            merchant_secret_mask, currency, token, network, notify_url, redirect_url,
            pc_app_redirect_url, mac_app_redirect_url, ios_app_redirect_url, android_app_redirect_url,
            mobile_web_redirect_url, desktop_web_redirect_url,
            min_amount, max_amount, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
               enabled = VALUES(enabled),
               api_base_url = VALUES(api_base_url),
               merchant_pid = VALUES(merchant_pid),
               merchant_secret_ciphertext = VALUES(merchant_secret_ciphertext),
               merchant_secret_mask = VALUES(merchant_secret_mask),
               currency = VALUES(currency),
               token = VALUES(token),
               network = VALUES(network),
               notify_url = VALUES(notify_url),
               redirect_url = VALUES(redirect_url),
               pc_app_redirect_url = VALUES(pc_app_redirect_url),
               mac_app_redirect_url = VALUES(mac_app_redirect_url),
               ios_app_redirect_url = VALUES(ios_app_redirect_url),
               android_app_redirect_url = VALUES(android_app_redirect_url),
               mobile_web_redirect_url = VALUES(mobile_web_redirect_url),
               desktop_web_redirect_url = VALUES(desktop_web_redirect_url),
               min_amount = VALUES(min_amount),
               max_amount = VALUES(max_amount),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .bind(DEFAULT_PROVIDER)
    .bind(write.enabled)
    .bind(&write.api_base_url)
    .bind(&write.merchant_pid)
    .bind(&write.merchant_secret_ciphertext)
    .bind(&write.merchant_secret_mask)
    .bind(&write.currency)
    .bind(&write.token)
    .bind(&write.network)
    .bind(&write.notify_url)
    .bind(&write.redirect_url)
    .bind(&write.pc_app_redirect_url)
    .bind(&write.mac_app_redirect_url)
    .bind(&write.ios_app_redirect_url)
    .bind(&write.android_app_redirect_url)
    .bind(&write.mobile_web_redirect_url)
    .bind(&write.desktop_web_redirect_url)
    .bind(&write.min_amount)
    .bind(&write.max_amount)
    .bind(write.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按配置 token 的大写代码加载活动资产，供创建本地快充订单时快照 asset_id/symbol。
/// 该读取发生在请求支付方之前，不创建钱包账户，也不检查 actual_amount 是否符合资产 precision_scale。
pub(crate) async fn load_active_asset_by_symbol(
    pool: &Pool<MySql>,
    symbol: &str,
) -> AppResult<QuickRechargeAssetRow> {
    sqlx::query_as::<_, QuickRechargeAssetSqlRow>(
        "SELECT id, symbol FROM assets WHERE symbol = ? AND status = 'active' LIMIT 1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?
    .map(Into::into)
    .ok_or_else(|| AppError::Validation("quick recharge asset is not active".to_owned()))
}

/// 读取快充订单所需用户邮箱，用户缺失时返回未找到。
pub(crate) async fn load_user_email(pool: &Pool<MySql>, user_id: u64) -> AppResult<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>("SELECT email FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在已锁快充订单的回调事务中创建/锁定钱包，再把已验签 actual_amount 原值增加到 available。
/// frozen/locked 保持原值；写一条 `quick_recharge` available 正流水，ref_type/ref_id 关联业务订单号并保存同一三桶账后快照。
/// 当前函数只要求上层传入金额，内部不校验正数或资产 precision_scale，也不查重流水；订单 paid 状态短路负责回调幂等。
/// 锁序为订单→钱包，订单状态、余额和流水由调用方事务提交；SQL 失败回滚本次全部本地变化。
pub(crate) async fn credit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    // 钱包余额和流水必须在同一个事务中写入，确保快速充值到账可审计且可回放核对。
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(QUICK_RECHARGE_CHANGE_TYPE)
    .bind(amount)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(QUICK_RECHARGE_REF_TYPE)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// 在调用方事务写入快充配置或订单操作的前后快照与原因。
/// 审计失败必须阻止对应配置保存或订单删除，避免后台副作用缺少追踪。
pub(crate) async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn quick_recharge_order_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id,
                  orders.order_id,
                  orders.user_id,
                  COALESCE(orders.user_email, users.email) AS user_email,
                  orders.asset_id,
                  orders.asset_symbol,
                  orders.currency,
                  orders.token,
                  orders.network,
                  orders.fiat_amount,
                  orders.actual_amount,
                  orders.provider_trade_id,
                  orders.receive_address,
                  orders.payment_url,
                  orders.return_target,
                  orders.redirect_url,
                  orders.expiration_time,
                  orders.status,
                  orders.block_transaction_id,
                  orders.paid_at,
                  orders.created_at,
                  orders.updated_at
           FROM quick_recharge_orders orders
           LEFT JOIN users ON users.id = orders.user_id"#,
    )
}

fn quick_recharge_order_count_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM quick_recharge_orders orders
           LEFT JOIN users ON users.id = orders.user_id"#,
    )
}

async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<QuickRechargeWalletRow> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, QuickRechargeWalletSqlRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(Into::into)
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

fn format_gmpay_http_error(
    http_status: StatusCode,
    content_type: Option<&str>,
    body: &str,
) -> String {
    if is_gmpay_cloudflare_challenge(body) {
        return format_gmpay_cloudflare_message(http_status);
    }
    if is_gmpay_html_response(content_type, body) {
        return format_gmpay_html_response_message(http_status);
    }
    let body = compact_response_body(body);
    if body.is_empty() {
        format!("gmpay returned http status {http_status} with empty response body")
    } else {
        format!("gmpay returned http status {http_status}: {body}")
    }
}

fn format_gmpay_cloudflare_message(http_status: StatusCode) -> String {
    format!(
        "gmpay returned http status {http_status}; GMPay 接口被 Cloudflare 防护拦截，请将 API 基础地址改为服务商提供的后端 API 域名，或联系 GMPay 将本服务器 IP/API 路径加入放行名单后再测试。"
    )
}

fn format_gmpay_html_response_message(http_status: StatusCode) -> String {
    format!(
        "gmpay returned http status {http_status}; 服务商返回的是 HTML 页面而不是 JSON API 响应，请确认 API 基础地址是否为 GMPay 后端接口域名。"
    )
}

fn is_gmpay_cloudflare_challenge(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("__cf_chl")
        || body.contains("challenge-platform")
        || body.contains("challenges.cloudflare.com")
        || body.contains("just a moment")
}

fn is_gmpay_html_response(content_type: Option<&str>, body: &str) -> bool {
    content_type
        .map(|value| value.to_ascii_lowercase().contains("text/html"))
        .unwrap_or(false)
        || body
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("<!doctype html")
        || body.trim_start().to_ascii_lowercase().starts_with("<html")
}

fn compact_response_body(body: &str) -> String {
    const MAX_PROVIDER_ERROR_BODY_CHARS: usize = 240;
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_PROVIDER_ERROR_BODY_CHARS {
        return compact;
    }
    let truncated = compact
        .chars()
        .take(MAX_PROVIDER_ERROR_BODY_CHARS)
        .collect::<String>();
    format!("{truncated}...")
}
