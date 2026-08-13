//! convert bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//!
//! 闪兑的持久化被切成两条互不重叠的链路。报价链路写 `convert_quotes` 并把同一份快照
//! 以 `convert:quote:{uuid}` 为键缓存进 Redis，二者不共享事务，缓存靠键 TTL 自然淘汰。
//! 结算链路在单个 MySQL 事务内完成：先按 quote_id 幂等插入 pending 订单，再锁定订单行，
//! 依「源资产、目标资产」顺序锁钱包，从源资产 available 扣款、向目标资产 available 入账，
//! 同步落两条 `convert_settlement` 流水与一条代理佣金记录，最后把订单置为 completed。
//! frozen 与 locked 在整个闪兑流程中都不参与，手续费已折进 to_amount 不再单独扣钱包。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_CONVERT,
        },
        convert::{
            ConvertConfirmationInsert, ConvertQuoteCacheEntry, ConvertQuoteInsert,
            ConvertQuoteInsertResult, ConvertRepositoryError, QuoteId,
            presentation::{ConvertOrderResponse, ConvertPairResponse},
            repository::{
                ConvertPairRule, ConvertPairRuleDbRecord, ConvertSettlementOrderRecord,
                ConvertSettlementWalletRecord, WalletBalanceRecord,
            },
            service::{convert_pair_rule_from_record, ensure_asset_precision_scale},
        },
        market::market_ticker_redis_key,
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use redis::AsyncCommands;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};
use std::str::FromStr;

impl From<sqlx::Error> for ConvertRepositoryError {
    /// 将 SQLx 故障文本保留为闪兑存储错误；不执行重试或回滚已提交事务。
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<redis::RedisError> for ConvertRepositoryError {
    /// 将 Redis 连接或命令故障映射为存储错误，不把缓存故障伪装为未命中。
    fn from(error: redis::RedisError) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<serde_json::Error> for ConvertRepositoryError {
    /// 将报价缓存序列化/反序列化故障映射为独立序列化错误。
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error.to_string())
    }
}

/// 报价快照的 Redis 适配器，所有报价以 `convert:quote:{uuid}` 为键并依赖键 TTL 自然过期。
/// 它只负责缓存读写，不做归属校验和资金操作；缓存不可用会以错误暴露，不会伪装成报价不存在。
#[derive(Clone)]
pub struct RedisConvertQuoteCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisConvertQuoteCache {
    /// 保存 Redis 连接管理器供报价写入、读取和单次消费复用；构造阶段不发送命令。
    /// Redis 连接、序列化或 TTL 错误在具体报价操作时返回，不能伪装为报价不存在。
    pub fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }

    /// 借用底层 Redis 连接管理器，供应用层读取同一行情或报价基础设施。
    pub fn manager(&self) -> &redis::aio::ConnectionManager {
        &self.manager
    }

    /// 将报价快照写入 Redis 并设置精确 TTL；缓存键由报价标识稳定派生。
    /// Redis 失败返回仓储错误，不能把未缓存报价交给确认流程。
    pub async fn save_quote_ttl(
        &self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError> {
        let payload = serde_json::to_string(&entry)?;
        let mut connection = self.manager.clone();
        let _: () = connection
            .set_ex(&entry.redis_key, payload, entry.ttl_seconds as u64)
            .await?;
        Ok(())
    }

    /// 按报价 UUID 派生 Redis 键并读取完整报价快照；缓存过期或不存在返回 `None`。
    /// JSON 损坏和 Redis 连接故障作为仓储错误返回，不回退到 MySQL 报价行，也不延长 TTL。
    pub async fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError> {
        let mut connection = self.manager.clone();
        let payload: Option<String> = connection.get(quote_redis_key(quote_id)).await?;
        payload
            .map(|value| serde_json::from_str::<ConvertQuoteCacheEntry>(&value))
            .transpose()
            .map_err(Into::into)
    }
}

/// 由报价 UUID 拼出唯一的 Redis 缓存键，格式必须与写入时 `ConvertQuoteCacheEntry.redis_key`
/// 及领域层 `ConvertQuote::idempotency_key` 完全一致，否则写得进去却读不出来。
/// 键名只含报价标识不含用户，归属校验依赖快照里的 user_id 字段而非键空间隔离。
fn quote_redis_key(quote_id: &QuoteId) -> String {
    format!("convert:quote:{}", quote_id.0)
}

/// 报价与订单的 MySQL 适配器，持有连接池并按需开启短事务。
/// 其上的方法各自独立提交，跨报价与结算的原子性由自由函数 `confirm_and_settle_convert_quote` 保证。
#[derive(Debug, Clone)]
pub struct MySqlConvertRepository {
    pool: Pool<MySql>,
}

impl MySqlConvertRepository {
    /// 保存 MySQL 池供闪兑交易对、订单和钱包结算仓储方法复用；构造时不获取连接或开启事务。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// 借用该仓储使用的 MySQL 连接池，不执行查询或资金写入。
    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    /// 持久化报价两侧资产、金额、汇率、价差、费用及到期快照；quote_id 唯一冲突时回读既有行号。
    /// 该单语句不锁钱包、不扣 available，也不写流水；`inserted=false` 只说明报价行已存在。
    /// MySQL 与 Redis 不共享事务，本入口成功不代表缓存已写入，后续缓存失败不会撤销该报价行。
    pub async fn insert_quote(
        &self,
        quote: ConvertQuoteInsert,
    ) -> Result<ConvertQuoteInsertResult, ConvertRepositoryError> {
        // 以 quote_id 幂等落库，重复提交只返回已有记录，避免重复开仓。
        let insert_result = sqlx::query(
            r#"INSERT INTO convert_quotes
               (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
                to_amount, rate, spread_rate, fee_rate, fee_amount, expires_at, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'quoted')
               ON DUPLICATE KEY UPDATE quote_id = quote_id"#,
        )
        .bind(quote.quote_id.0.to_string())
        .bind(quote.convert_pair_id)
        .bind(quote.user_id)
        .bind(quote.from_asset_id)
        .bind(quote.to_asset_id)
        .bind(quote.from_amount)
        .bind(quote.to_amount)
        .bind(quote.rate)
        .bind(quote.spread_rate)
        .bind(quote.fee_rate)
        .bind(quote.fee_amount)
        .bind(quote.expires_at.naive_utc())
        .execute(&self.pool)
        .await?;

        let quote_row_id = if insert_result.last_insert_id() == 0 {
            self.quote_row_id(&quote.quote_id).await?
        } else {
            insert_result.last_insert_id()
        };

        Ok(ConvertQuoteInsertResult {
            quote_row_id,
            inserted: insert_result.rows_affected() == 1,
        })
    }

    /// 直接以连接池把报价快照原样复制为 pending 订单，金额、汇率、费率全部取自 `convert_quotes` 行。
    /// 报价行不存在时 INSERT ... SELECT 命中零行，`last_insert_id` 为零因而同样返回 `Duplicate`，
    /// 调用方无法据此区分「已确认过」和「报价根本不存在」两种情况。
    /// quote_id 唯一约束把并发重复调用收敛为一次插入，是闪兑资金幂等的实际依据。
    /// 此兼容入口自成一条自动提交语句，不锁钱包、不完成双资产结算，也不与后续资金写入同事务。
    pub async fn insert_order_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError> {
        let result = sqlx::query(
            r#"INSERT INTO convert_orders
               (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
                to_amount, rate, fee_rate, fee_amount, status)
               SELECT quotes.quote_id, quotes.convert_pair_id, quotes.user_id, quotes.from_asset,
                      quotes.to_asset, quotes.from_amount, quotes.to_amount, quotes.rate,
                      quotes.fee_rate, quotes.fee_amount, 'pending'
               FROM convert_quotes quotes
               WHERE quotes.quote_id = ?
               ON DUPLICATE KEY UPDATE quote_id = convert_orders.quote_id"#,
        )
        .bind(quote_id.0.to_string())
        .execute(&self.pool)
        .await?;

        if result.last_insert_id() == 0 {
            Ok(ConvertConfirmationInsert::Duplicate)
        } else {
            Ok(ConvertConfirmationInsert::Inserted)
        }
    }

    /// 在 `insert_quote` 命中唯一键冲突、拿不到自增主键时回读既有报价行的编号。
    /// 用 `fetch_one` 而非 `fetch_optional`，因为能走到这里说明冲突分支已确认该行存在，
    /// 查不到只可能是并发删除等异常状态，直接以存储错误上报而不是静默返回零。
    async fn quote_row_id(&self, quote_id: &QuoteId) -> Result<u64, ConvertRepositoryError> {
        let row =
            sqlx::query_as::<_, (u64,)>("SELECT id FROM convert_quotes WHERE quote_id = ? LIMIT 1")
                .bind(quote_id.0.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}

/// 按配置行编号倒序读取所有 enabled 为真的闪兑对，供前端渲染可兑换列表。
/// 两次 INNER JOIN assets 分别取源侧与目标侧的符号和 logo_url，因此资产被删除时该对整行消失。
/// 同时返回正向 min/max 与反向 target_min/target_max 两套限额，前端切换方向时无需再次请求。
/// 该查询不读取行情或钱包余额，不生成报价，也不把配置费率换算成任何具体资金金额。
/// 分页量由调用方经 `route_limit` 夹紧后传入，本函数不再二次校验。
pub(crate) async fn list_convert_pairs(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<Vec<ConvertPairResponse>> {
    let pairs = sqlx::query_as::<_, ConvertPairResponse>(
        r#"SELECT pairs.id,
                  pairs.from_asset AS from_asset_id,
                  from_assets.symbol AS from_asset_symbol,
                  from_assets.logo_url AS from_asset_logo_url,
                  pairs.to_asset AS to_asset_id,
                  to_assets.symbol AS to_asset_symbol,
                  to_assets.logo_url AS to_asset_logo_url,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate, pairs.min_amount,
                  pairs.max_amount, pairs.target_min_amount, pairs.target_max_amount,
                  pairs.enabled
           FROM convert_pairs pairs
           INNER JOIN assets from_assets ON from_assets.id = pairs.from_asset
           INNER JOIN assets to_assets ON to_assets.id = pairs.to_asset
           WHERE pairs.enabled = true
           ORDER BY pairs.id DESC
           LIMIT ?"#,
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(pairs)
}

/// 按认证用户倒序读取其闪兑订单，`status` 非空时追加等值过滤，为空则返回全部状态。
/// user_id 条件恒定拼入且以绑定参数下推，动态部分只有状态和分页量，不存在跨用户越权读取。
/// 用 QueryBuilder 拼装是为了让状态过滤可选，所有变量仍走 push_bind 而非字符串插值。
/// 查询不加任何行锁，也不触碰钱包表；返回的汇率与费用是确认时固化的快照，不重新计算。
/// 结果按订单自增编号倒序，因此翻页语义等价于按创建时间从新到旧。
pub(crate) async fn list_convert_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<String>,
    limit: u32,
) -> AppResult<Vec<ConvertOrderResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, quote_id, convert_pair_id, from_asset AS from_asset_id,
                  to_asset AS to_asset_id, from_amount, to_amount, rate,
                  fee_rate, fee_amount, status, created_at
           FROM convert_orders
           WHERE user_id = "#,
    );
    builder.push_bind(user_id);

    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let orders = builder
        .build_query_as::<ConvertOrderResponse>()
        .fetch_all(pool)
        .await?;

    Ok(orders)
}

/// 为一次报价请求定位唯一可用的闪兑规则，同时接受配置中正向与反向两种资产排列。
/// ORDER BY 里的 CASE 让与请求方向完全一致的配置排在前面，方向相反的作为兜底，最后取一行。
/// LEFT JOIN 只关联 status 为 active 且 rate_source 为 fixed 的新币规则来取固定汇率，
/// 另一侧 LEFT JOIN 取 status 为 active 的现货交易对作为市场计价来源，两者都可能为空。
/// 未匹配到任何启用配置时返回 NotFound；本函数只读配置，不创建报价行也不产生资金副作用。
/// 返回前交由服务层按请求方向归一化限额并在反向时对固定汇率取倒数。
pub(crate) async fn load_pair_rule(
    pool: &Pool<MySql>,
    from_asset_id: u64,
    to_asset_id: u64,
) -> AppResult<ConvertPairRule> {
    let row = sqlx::query_as::<_, ConvertPairRuleDbRecord>(
        r#"SELECT pairs.id, pairs.from_asset AS from_asset_id, pairs.to_asset AS to_asset_id,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate,
                  pairs.min_amount, pairs.max_amount,
                  pairs.target_min_amount, pairs.target_max_amount,
                  rules.fixed_rate,
                  market_pairs.symbol AS market_pair_symbol,
                  market_pairs.base_asset AS market_base_asset_id,
                  market_pairs.quote_asset AS market_quote_asset_id
           FROM convert_pairs pairs
           LEFT JOIN new_coin_convert_rules rules
             ON rules.convert_pair_id = pairs.id AND rules.status = 'active' AND rules.rate_source = 'fixed'
           LEFT JOIN trading_pairs market_pairs
             ON ((market_pairs.base_asset = pairs.from_asset AND market_pairs.quote_asset = pairs.to_asset)
                 OR (market_pairs.base_asset = pairs.to_asset AND market_pairs.quote_asset = pairs.from_asset))
            AND market_pairs.status = 'active'
           WHERE ((pairs.from_asset = ? AND pairs.to_asset = ?)
                  OR (pairs.from_asset = ? AND pairs.to_asset = ?))
             AND pairs.enabled = true
           ORDER BY CASE WHEN pairs.from_asset = ? AND pairs.to_asset = ? THEN 0 ELSE 1 END,
                    pairs.id DESC
           LIMIT 1"#,
    )
    .bind(from_asset_id)
    .bind(to_asset_id)
    .bind(to_asset_id)
    .bind(from_asset_id)
    .bind(from_asset_id)
    .bind(to_asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    convert_pair_rule_from_record(row, from_asset_id, to_asset_id)
}

/// 非锁定读取用户在源资产上的 available 与 locked，供报价阶段做一次提示性余额校验。
/// 钱包账户尚未开通时不报错而是返回双零，让余额不足的提示语义统一，不额外创建账户行。
/// 刻意不加 FOR UPDATE：报价是高频只读操作，锁住钱包会与结算事务争锁并拖慢下单。
/// 因此该快照可能在用户确认前失效，真正的扣款判定发生在结算事务内重新锁行之后。
/// 返回值只用于生成友好错误提示，不冻结资金，也不为后续确认预留任何额度。
pub(crate) async fn load_wallet_balance(
    pool: &Pool<MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletBalanceRecord> {
    let row = sqlx::query_as::<_, WalletBalanceRecord>(
        "SELECT available, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or_else(|| WalletBalanceRecord {
        available: BigDecimal::from(0),
        locked: BigDecimal::from(0),
    }))
}

/// 从行情接入链写入的 ticker 缓存里取出指定交易对的最新成交价，作为市场计价的权威汇率来源。
/// Redis 未配置或键不存在都返回空值，由调用方决定是拒绝报价还是走其他分支，本函数不自行兜底。
/// 载荷必须是含字符串 last_price 字段的 JSON，解析失败或字段缺失一律按内部错误上报。
/// 价格解析成功后还要求严格为正，非正价格返回参数错误，避免下游取倒数时出现除零或负汇率。
/// 只读缓存，不回源现货撮合、不刷新 TTL，也绝不接受客户端提交的价格作为替代。
pub(crate) async fn latest_market_price(
    redis: Option<redis::aio::ConnectionManager>,
    pair_symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    let Some(mut connection) = redis else {
        return Ok(None);
    };
    let payload: Option<String> = connection
        .get(market_ticker_redis_key(pair_symbol))
        .await
        .map_err(AppError::from)?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;
    let last_price = value
        .get("last_price")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Internal("cached ticker is missing last_price".to_owned()))?;
    let price = BigDecimal::from_str(last_price)
        .map_err(|_| AppError::Internal("cached ticker last_price is invalid".to_owned()))?;
    if price <= 0 {
        return Err(AppError::Validation(
            "convert market price must be positive".to_owned(),
        ));
    }
    Ok(Some(price))
}

/// 通过调用方提供的执行器读取资产 precision_scale，并校验其处于钱包支持的 0..=18 范围。
/// 缺失或损坏配置会阻止报价/结算；本函数只读元数据，不截断现有余额或流水。
pub(crate) async fn load_asset_precision_scale<'e, E>(executor: E, asset_id: u64) -> AppResult<i32>
where
    E: sqlx::Executor<'e, Database = MySql>,
{
    let (precision_scale,): (i32,) =
        sqlx::query_as("SELECT precision_scale FROM assets WHERE id = ? LIMIT 1")
            .bind(asset_id)
            .fetch_optional(executor)
            .await?
            .ok_or(AppError::NotFound)?;
    ensure_asset_precision_scale(precision_scale)?;
    Ok(precision_scale)
}

/// 在进入结算事务前核对该报价行确实存在且属于当前用户，避免拿别人的 quote_id 触发结算。
/// 条件同时约束 quote_id 与 user_id，命中失败一律返回 false 由调用方转成 NotFound，
/// 不区分「报价不存在」和「报价属于他人」，防止通过错误码探测他人报价是否存在。
/// 刻意不检查订单状态和 expires_at：有效期以 Redis 快照为准，已确认与否交给订单唯一键裁决。
/// 查询不加行锁，本次通过不代表结算一定成功，真正的幂等保障在确认事务内的订单插入语句。
pub(crate) async fn quote_exists_for_user(
    pool: &Pool<MySql>,
    quote_id: &QuoteId,
    user_id: u64,
) -> AppResult<bool> {
    let row = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM convert_quotes WHERE quote_id = ? AND user_id = ? LIMIT 1",
    )
    .bind(quote_id.0.to_string())
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

/// 为报价创建唯一 pending 订单，并在自有 MySQL 事务内完成闪兑资金结算。
/// 实际锁序是：依 quote_id 插入订单，锁该用户 pending 订单，再依“源资产、目标资产”顺序锁钱包；代码不按资产编号重排。
/// 结算从源资产 available 扣除完整 from_amount，向目标资产 available 增加报价 to_amount；frozen/locked 保持不变。
/// 两条 `convert_settlement` 流水分别记录源资产负额和目标资产正额，均引用 quote_id；费用只保存在订单快照，已包含在 to_amount 计算中，不另扣钱包。
/// 订单完成、代理佣金记录、两侧余额和流水同事务提交；唯一订单已存在、余额不足或任一步失败都会回滚本次写入。
pub(crate) async fn confirm_and_settle_convert_quote(
    pool: &Pool<MySql>,
    quote_id: &QuoteId,
    user_id: u64,
) -> AppResult<()> {
    let quote_id_value = quote_id.0.to_string();
    let mut tx = pool.begin().await?;
    let inserted = insert_order_for_quote_in_tx(&mut tx, &quote_id_value).await?;
    if !inserted {
        return Err(AppError::Conflict(
            "convert quote has already been confirmed".to_owned(),
        ));
    }
    settle_convert_order_in_tx(&mut tx, &quote_id_value, user_id).await?;
    tx.commit().await?;
    Ok(())
}

/// 在结算事务内把报价快照复制成一条 pending 订单，并以返回值告知调用方是否为首次插入。
/// 订单字段全部由 `convert_quotes` 行 SELECT 而来，调用方无法覆写金额、汇率或费率。
/// quote_id 上的唯一约束是本次结算的幂等键：重放时 ON DUPLICATE 走空更新，
/// `last_insert_id` 为零因而返回 false，调用方据此回滚并报冲突，绝不重复扣款。
/// 报价行不存在时 SELECT 命中零行，同样返回 false，语义上与重复确认合并处理。
async fn insert_order_for_quote_in_tx(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
) -> AppResult<bool> {
    // 同一事务内先锁定并插入订单，再完成钱包结算；任意一步失败都会整体回滚，避免留下不可恢复的 pending 订单。
    let result = sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
            to_amount, rate, fee_rate, fee_amount, status)
           SELECT quotes.quote_id, quotes.convert_pair_id, quotes.user_id, quotes.from_asset,
                  quotes.to_asset, quotes.from_amount, quotes.to_amount, quotes.rate,
                  quotes.fee_rate, quotes.fee_amount, 'pending'
           FROM convert_quotes quotes
           WHERE quotes.quote_id = ?
           ON DUPLICATE KEY UPDATE quote_id = convert_orders.quote_id"#,
    )
    .bind(quote_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.last_insert_id() != 0)
}

/// 在调用方已开启的事务内完成一笔闪兑的全部资金移动，调用前订单必须已插入且状态为 pending。
/// 加锁顺序固定为：先 FOR UPDATE 锁定该用户的 pending 订单行，再锁源资产钱包，最后锁目标资产钱包。
/// 顺序按订单记录的「源、目标」而非资产编号大小排列，因此同一用户反向对敲存在理论上的锁序交叉。
/// 源资产从 available 全额扣除 from_amount，扣前先比对余额，不足则整个事务回滚且不留 pending 订单。
/// 目标资产 available 加上 to_amount 后按目标资产 precision_scale 向零截断再写回，
/// 截断作用于加总后的余额而非增量，因此极端情况下入账可能比 to_amount 少一个最小单位。
/// frozen 与 locked 全程不变，手续费已折进 to_amount，不产生独立的手续费流水。
/// 随后把订单置为 completed，写入一条代理业务佣金记录，并落两条 convert_settlement 钱包流水：
/// 源资产记负额、目标资产记正额，二者均以 quote_id 作为 convert_order 引用。
/// 任一步失败都由调用方回滚，不会留下只扣款未入账或只入账未记流水的中间状态。
async fn settle_convert_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
    user_id: u64,
) -> AppResult<()> {
    let order = sqlx::query_as::<_, ConvertSettlementOrderRecord>(
        r#"SELECT from_asset AS from_asset_id, to_asset AS to_asset_id, from_amount, to_amount
           FROM convert_orders
           WHERE quote_id = ? AND user_id = ? AND status = 'pending'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let from_wallet = lock_wallet_row(tx, user_id, order.from_asset_id).await?;
    if from_wallet.available < order.from_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for convert settlement: requested {}, available {}, locked {}",
            order.from_amount, from_wallet.available, from_wallet.locked
        )));
    }
    let to_wallet = lock_wallet_row(tx, user_id, order.to_asset_id).await?;
    let to_precision_scale = load_asset_precision_scale(&mut **tx, order.to_asset_id).await?;

    let from_available_after = from_wallet.available.clone() - order.from_amount.clone();
    let raw_to_available_after = to_wallet.available.clone() + order.to_amount.clone();
    let to_available_after =
        truncate_amount_to_asset_precision(&raw_to_available_after, to_precision_scale);

    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&from_available_after)
        .bind(user_id)
        .bind(order.from_asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&to_available_after)
        .bind(user_id)
        .bind(order.to_asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE convert_orders SET status = 'completed' WHERE quote_id = ? AND user_id = ?",
    )
    .bind(quote_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    insert_agent_business_commission_in_tx(
        tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_CONVERT,
            source_type: "convert_order",
            source_id: quote_id,
            source_amount: &order.from_amount,
            payout_asset_id: order.from_asset_id,
        },
    )
    .await?;

    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'convert_settlement', ?, 'available', ?, ?, ?, ?, 'convert_order', ?),
                  (?, ?, 'convert_settlement', ?, 'available', ?, ?, ?, ?, 'convert_order', ?)"#,
    )
    .bind(user_id)
    .bind(order.from_asset_id)
    .bind(-order.from_amount.clone())
    .bind(&from_available_after)
    .bind(&from_available_after)
    .bind(&from_wallet.frozen)
    .bind(&from_wallet.locked)
    .bind(quote_id)
    .bind(user_id)
    .bind(order.to_asset_id)
    .bind(&order.to_amount)
    .bind(&to_available_after)
    .bind(&to_available_after)
    .bind(&to_wallet.frozen)
    .bind(&to_wallet.locked)
    .bind(quote_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// 在结算事务内取得某个用户资产维度钱包行的排他锁，并返回锁定瞬间的三段余额。
/// 先做一次幂等的账户初始化再 SELECT ... FOR UPDATE，保证首次接触该资产的用户也能入账。
/// 返回的 frozen 与 locked 不参与闪兑计算，只用于写流水时记录当时的完整余额切片。
/// 初始化后仍查不到行属于异常状态，按参数错误上报并让整个结算事务回滚。
/// 锁在事务提交或回滚时释放，调用顺序由 `settle_convert_order_in_tx` 统一决定。
async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<ConvertSettlementWalletRecord> {
    // 若用户首次接触某资产，先按缺省值创建账本行，避免确认时出现“wallet account is required”报错。
    ensure_wallet_account_in_tx(tx, user_id, asset_id).await?;
    sqlx::query_as::<_, ConvertSettlementWalletRecord>(
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
    .ok_or_else(|| {
        AppError::Validation("wallet account is required for convert settlement".to_owned())
    })
}

/// 幂等地确保用户在该资产上存在钱包账户行，余额字段一律沿用表默认值不做任何赋值。
/// 已存在时通过 `updated_at = updated_at` 走空更新，既不重置余额也不刷新时间戳。
/// 该写入与结算共用同一事务，若后续步骤失败，新建的空账户行会随事务一并回滚。
/// SQLx 错误在此包装为内部错误，因为账户初始化失败属于存储故障而非用户输入问题。
async fn ensure_wallet_account_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| AppError::Internal(format!("failed to initialize wallet account: {error}")))?;

    Ok(())
}
