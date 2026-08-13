//! quick_recharge bounded context infrastructure layer.
//!
//! 基础设施层：快速充值上下文全部 MySQL 访问与 GMPay HTTP 调用的唯一出口。
//! 三类职责分别是渠道单例配置的读写与加锁、充值订单的建单与状态流转，以及回调确认后的钱包入账与流水。
//!
//! 数据库函数按调用形态分两组：接收 `&Pool<MySql>` 的自动提交且不持锁，用于列表查询、建单与失败标记；
//! 接收 `&mut Transaction` 的由调用方开启并负责提交回滚，用于回调入账与配置变更这类必须原子的路径。
//! 涉及资金的加锁顺序固定为先锁订单行、再锁钱包行，与后台删除路径保持同序以避免死锁。
//!
//! 渠道配置是按固定名称寻址的单例行，写入走 `INSERT ... ON DUPLICATE KEY UPDATE`，因此首次保存即建行。
//! 商户密钥在本层只以密文形态出入库，解密发生在服务层；本层的错误信息与日志都不含密钥。
//!
//! GMPay 调用刻意不在任何事务内进行，且对失败响应做了分类：Cloudflare 拦截和 HTML 页面会被识别出来
//! 并给出可执行的中文排障提示，而不是把整页 HTML 原样抛给调用方。

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

/// 渠道配置表的原始查询映射，字段顺序与三处 SELECT 语句严格对应。
/// 仅在本层内部存在，向上返回前会转换成仓储层的配置快照类型，两者字段一一对应但归属不同层。
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
    /// 将配置 SQL 行逐字段搬运为仓储层的配置快照，是本层与上层之间的类型边界转换。
    /// 商户密钥密文原样携带而不解密，掩码也原样保留，解密只发生在服务层的运行时配置构造中。
    /// 转换不做任何默认值填充或格式归一，字段语义与数据库列完全一致，
    /// 因此上层看到的 `None` 就代表该列在库中为 NULL 而非被本函数丢弃。
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

/// 充值订单联表查询的原始映射，字段顺序与公共 SELECT 骨架严格对应。
/// `user_email` 取订单上的冗余邮箱，为空时回落到用户表当前邮箱，因此改邮箱不会让历史订单丢失联系方式。
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
    /// 将订单联表 SQL 行搬运为仓储层订单快照，纯字段拷贝且不访问任何外部系统。
    /// 快照反映的是本地数据库当刻的记录，不代表支付方侧的最新状态，
    /// 因此看到 `pending` 只说明本地尚未收到回调，不能据此断定用户未付款。
    /// 转换不触发支付方查询、不改订单状态、不产生钱包入账。
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

/// 资产表的最小查询映射，只取建单时需要快照到订单行的编号与符号两列。
#[derive(Debug, sqlx::FromRow)]
struct QuickRechargeAssetSqlRow {
    id: u64,
    symbol: String,
}

impl From<QuickRechargeAssetSqlRow> for QuickRechargeAssetRow {
    /// 把资产查询行搬运为仓储层资产标识，仅两个字段的直接拷贝。
    /// 不校验资产状态（调用方查询时已限定为启用），也不为该资产创建钱包账户。
    fn from(row: QuickRechargeAssetSqlRow) -> Self {
        Self {
            id: row.id,
            symbol: row.symbol,
        }
    }
}

/// 钱包账户三段余额的查询映射，仅在入账事务内加锁读取时使用。
#[derive(Debug, sqlx::FromRow)]
struct QuickRechargeWalletSqlRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

impl From<QuickRechargeWalletSqlRow> for QuickRechargeWalletRow {
    /// 把已加锁读取的钱包行搬运为仓储层余额快照，三段余额原值拷贝。
    /// 快充只增加可用余额，冻结与锁定两项在此仅用于写流水时记录变更当时的完整分布，本转换不修改任何一项。
    fn from(row: QuickRechargeWalletSqlRow) -> Self {
        Self {
            available: row.available,
            frozen: row.frozen,
            locked: row.locked,
        }
    }
}

/// GMPay 建单接口的响应外壳，业务成败由 `status_code` 表达而非 HTTP 状态码。
/// 因此 HTTP 200 也可能是业务失败，必须先判 `status_code` 为 200 才能取用 `data`。
#[derive(Debug, Deserialize)]
struct GmpayCreateOrderResponse {
    /// 支付方业务状态码，200 表示建单成功。
    status_code: i32,
    /// 业务失败时的说明文本，成功时通常为空。
    message: Option<String>,
    /// 建单成功后的订单数据；业务失败时为空。
    data: Option<GmpayCreateOrderData>,
}

/// GMPay 建单成功后返回的订单数据，是本地订单补齐收款信息的唯一来源。
/// 调用方必须核对 `order_id` 与 `amount` 与本次请求一致，防止把另一笔订单的收款地址写到本地订单上。
#[derive(Debug, Deserialize)]
pub(crate) struct GmpayCreateOrderData {
    /// 支付方侧的交易号，与本地订单号构成双向映射，回调时用于二次比对。
    pub(crate) trade_id: String,
    /// 回显的商户订单号，应与本次请求发出的本地订单号完全一致。
    pub(crate) order_id: String,
    /// 回显的法币金额，应与本次请求金额完全一致。
    pub(crate) amount: BigDecimal,
    /// 回显的法币币种。
    pub(crate) currency: String,
    /// 按当时汇率折算出的应付加密货币数量，也是回调到账时的入账基数。
    pub(crate) actual_amount: BigDecimal,
    /// 用户应向其付款的收款地址。
    pub(crate) receive_address: String,
    /// 收款币种代码。
    pub(crate) token: String,
    /// 收款地址的失效时间戳，为空表示支付方未给出有效期。
    pub(crate) expiration_time: Option<i64>,
    /// 支付方托管的收银台地址，前端可直接跳转。
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

/// 读取某个用户的充值订单列表，用户编号作为第一个 WHERE 条件直接绑定，查询不可能越过用户维度。
/// 状态为可选筛选，`None` 时返回该用户全部状态的订单。
/// 排序为创建时间倒序加订单主键倒序，主键作为唯一列参与排序，避免同一时刻的订单在结果中不稳定。
/// 只取 `limit` 条且不返回总数，用户侧不提供偏移分页。
/// 走连接池只读查询，不加行锁，也不触发任何支付方调用或钱包变动。
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

/// 为后台查询充值订单分页与匹配总数，支持用户编号、邮箱、状态、本地订单号、支付方交易号五个可选条件。
/// 五个条件在同一个循环里同时追加到行查询与 COUNT 查询，两者谓词逐字一致，总数才会跟随当前筛选。
/// 全部为精确相等匹配而非模糊搜索，订单号与交易号因此可直接命中索引，适合掉单排查时的点查。
/// 恒真的 `WHERE 1 = 1` 打底，使各条件可无差别地以 AND 追加。
/// 该入口只读，不加行锁，也不修改订单状态、钱包余额或流水。
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

/// 补齐后台分页查询的尾段：给行查询追加排序与 LIMIT/OFFSET，再单独执行一次 COUNT 取总数。
/// 行查询与 COUNT 查询必须由调用方用同一组过滤谓词构建，返回总数才能与当前筛选一致。
/// `order_by` 需包含唯一列，仅按时间排序会让相邻页出现重复行或漏行。
/// 两次查询各自独立执行且不在事务内，并发写入时总数与行集可能短暂不一致，属于列表接口可接受的偏差。
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

/// 在调用方事务内按对外订单号加 `FOR UPDATE` 锁定充值订单，是回调入账与后台删除的并发串行点。
/// 支付方可能对同一订单并发或重复投递回调，行锁保证同一时刻只有一路能读到状态并推进，
/// 后到者会阻塞至前者提交，届时读到的状态已是 `paid`，从而走幂等短路而不重复入账。
/// 锁的获取顺序固定为先订单后钱包，与本模块其他资金路径一致以避免死锁。
/// 订单不存在返回 `AppError::NotFound`；锁在调用方事务提交或回滚时释放。
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

/// 在调用方事务内按自增主键物理删除一笔充值订单，供后台清理废单。
/// 本函数不做任何前置判断：是否已支付、是否已有钱包流水必须由调用方在持有订单行锁后先行确认，
/// 误删已入账订单会让资金流水失去对应凭证。删除与审计写入须由同一事务提交。
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
/// 交易号与收款地址用合并写法处理：仅在订单原本为空时才采用回调值，否则保留建单时支付方给出的原值，
/// 避免回调覆盖掉更权威的建单回执；到账数量与链上哈希则以回调值为准直接覆盖。
/// 回调原文整体落库存档，供事后复核验签与处理资金争议。
/// 支付完成时刻由数据库当前时间生成，不取应用侧时钟，避免多实例时钟漂移影响对账。
/// 本函数不检查前置状态也不动钱包，状态判断与入账由调用方在同一事务内完成，失败时一并回滚。
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

/// 从连接池读取名为 `default` 的渠道单例配置，整个模块只维护这一行配置。
/// 记录缺失返回 `AppError::NotFound`，表示渠道尚未初始化；这与「配置存在但未启用」是两种不同状态。
/// 返回值携带商户密钥密文，供服务层在受控范围内解密使用；调用方不得把该行直接序列化进响应或日志。
/// 只读不加锁，因此配置变更事务进行中读到的仍是旧版本，用于展示与下单足够，配置写入路径必须改用加锁版本。
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

/// 在调用方事务内回读渠道单例配置，SQL 与连接池版本完全相同但执行在事务连接上。
/// 因此能读到本事务中刚刚写入尚未提交的配置，配置保存流程正是靠它取 after 审计快照。
/// 不加 `FOR UPDATE`，排他性由调用方更早通过 `lock_config_in_tx` 取得的行锁提供。
/// 记录缺失返回 `AppError::NotFound`；在保存流程中出现该错误说明 upsert 未生效，应整体回滚。
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

/// 以 `FOR UPDATE` 锁定渠道单例配置行，把并发的后台配置保存串行化。
/// 返回 `Option` 而非在缺失时报错，因为首次保存时配置行尚不存在，此时返回 `None` 属于正常路径，
/// 调用方据此判定「新建」还是「更新」，并决定审计是否带 before 镜像。
/// 加锁读回的旧密文是密钥沿用逻辑的输入：本次未提交新密钥时直接复用它，从而实现改配置不改密钥。
/// 行锁持续到配置与审计共同提交，中途失败时旧配置与旧密钥继续保持有效。
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

/// 在调用方事务内写入渠道单例配置，采用 `INSERT ... ON DUPLICATE KEY UPDATE` 使首次保存即建行。
/// 配置名与渠道商标识由本层写死为常量，不接受调用方指定，从结构上保证只会存在这一行配置。
/// 更新分支逐列覆盖为本次提交的值，包括密钥密文与掩码，因此调用方必须传入完整配置；
/// 若本次不换密钥，调用方应把从加锁读取中拿到的旧密文原样回填，否则密钥会被写成空值而导致渠道失效。
/// 创建时间不在更新列中，保持首次建行时刻不变；`updated_by` 记录本次操作的管理员编号。
/// 本函数不提交事务、不校验字段合法性，也不做启用态必填断言，这些都由服务层在更早阶段完成。
/// 写入必须与管理员审计在同一事务提交，中途失败时旧配置继续生效。
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

/// 读取下单用户的邮箱，用于在建单时把联系方式冗余快照到订单行上。
/// 返回值是双层可空的展开结果：用户不存在返回 `AppError::NotFound`，
/// 用户存在但邮箱列为 NULL 时返回 `Ok(None)`，两者语义不同不可混用。
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
/// 在调用方事务内写入后台审计记录，覆盖渠道配置保存、连通性测试与订单删除三类操作。
/// `target_id` 以字符串落库，兼容审计表对不同业务主键类型的统一存储。
/// `before_json` 与 `after_json` 均可为空：首次建配置没有前镜像，删除订单没有后镜像。
/// `reason` 原样绑定而不再裁剪，调用方在服务层已完成必填与长度校验。
/// 审计写入失败必须阻止对应的配置保存或订单删除一并回滚，不允许后台改动生效却缺少追踪。
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

/// 构造充值订单查询的公共 SELECT 骨架，用户列表、后台列表、详情与加锁读取共用同一份字段集。
/// 共用的意义在于四条路径产出的行结构完全一致，任何新增列只需在此一处补齐。
/// 邮箱用 `COALESCE` 优先取订单上的冗余值，缺失时回落到用户表当前邮箱，因此改邮箱不影响历史订单归属展示。
/// 用户表用 LEFT JOIN 连接，用户被删除时订单仍能查出而不是凭空消失。
/// 返回的 builder 不含 WHERE、排序与分页，调用方须自行补齐；也不含 `FOR UPDATE`，加锁由调用方追加。
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

/// 构造与订单行查询配套的 COUNT 骨架，表与 JOIN 结构必须与行查询保持一致。
/// 特别是同样保留对用户表的 LEFT JOIN：若改成 INNER JOIN，按邮箱筛选时两边口径会出现偏差导致总数不准。
fn quick_recharge_order_count_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM quick_recharge_orders orders
           LEFT JOIN users ON users.id = orders.user_id"#,
    )
}

/// 在入账事务内确保钱包账户存在并加锁读回余额，分两步完成。
/// 第一步用 `INSERT ... ON DUPLICATE KEY UPDATE updated_at = updated_at` 做无副作用的占位插入：
/// 账户已存在时该语句不改动任何列，只为「不存在则建账」提供一条不会因唯一键冲突而失败的路径。
/// 第二步再以 `FOR UPDATE` 读回三段余额，此时无论账户是新建还是既有都必然存在。
/// 之所以允许自动建账，是因为充值可能发生在用户从未持有该资产之前，若此时报错会导致到账失败。
/// 这与秒合约结算路径的取舍相反，那里账户缺失属于异常应当中止。
/// 加锁顺序要求调用方已先锁订单行；读回失败返回校验错误并使整笔入账回滚。
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

/// 把支付方的非 2xx 响应整理成可读的错误说明，按三种情形分级处理。
/// 先识别 Cloudflare 人机校验页并给出改用后端 API 域名或加白名单的具体建议，
/// 再识别普通 HTML 页面并提示地址很可能配成了门户站点而非接口域名，
/// 两者都是运维配错地址时的高频现象，直接回抛原始 HTML 对排障毫无帮助。
/// 其余情况回退到「状态码加压缩后的响应体」，响应体为空时也给出明确措辞而不是留白。
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

/// 生成命中 Cloudflare 人机校验时的中文提示，直接给出两条可执行的处置建议。
/// 该提示会经 `AppError::Api` 返回到后台页面，因此写成运维能照做的操作说明而非技术堆栈信息。
fn format_gmpay_cloudflare_message(http_status: StatusCode) -> String {
    format!(
        "gmpay returned http status {http_status}; GMPay 接口被 Cloudflare 防护拦截，请将 API 基础地址改为服务商提供的后端 API 域名，或联系 GMPay 将本服务器 IP/API 路径加入放行名单后再测试。"
    )
}

/// 生成支付方返回 HTML 页面而非 JSON 时的中文提示，指向 API 基础地址配置错误这一最可能原因。
/// 该分支同时服务于 HTTP 失败和 JSON 解析失败两条路径，两处措辞保持一致以免运维误判为两种故障。
fn format_gmpay_html_response_message(http_status: StatusCode) -> String {
    format!(
        "gmpay returned http status {http_status}; 服务商返回的是 HTML 页面而不是 JSON API 响应，请确认 API 基础地址是否为 GMPay 后端接口域名。"
    )
}

/// 依据响应体中的特征串判断是否命中 Cloudflare 人机校验页，命中任一特征即判定为真。
/// 特征取自校验页常见的脚本前缀、平台路径与标题文案；判定前统一转小写以忽略大小写差异。
/// 这是启发式识别而非精确判定，误判的后果仅是错误提示措辞不同，不影响资金安全。
fn is_gmpay_cloudflare_challenge(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("__cf_chl")
        || body.contains("challenge-platform")
        || body.contains("challenges.cloudflare.com")
        || body.contains("just a moment")
}

/// 判断响应是否为 HTML 页面，先看 Content-Type 是否含 `text/html`，再看正文是否以文档声明或 html 标签开头。
/// 之所以不只看响应头，是因为部分网关返回错误页时并不带正确的 Content-Type。
/// 比较前统一转小写并忽略前导空白，避免大小写或缩进导致漏判。
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

/// 把支付方响应体压成单行摘要，供拼进错误信息。
/// 先按空白切分再以单个空格重连，消除换行与缩进使错误信息保持一行；随后按字符数截断到 240，
/// 截断按 `chars` 而非字节进行，保证多字节中文不会被切成乱码，超长时追加省略号提示内容已被裁剪。
/// 上限是为了避免把整页错误文档灌进日志和 API 响应。
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
