//! convert bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//!
//! 闪兑的持久化被切成两条互不重叠的链路。报价链路写 `convert_quotes` 并把同一份快照
//! 以 `convert:quote:{uuid}` 为键缓存进 Redis，二者不共享事务，缓存靠键 TTL 自然淘汰。
//! 结算链路在单个 MySQL 事务内完成：先锁 quote 并复核归属、指纹、过期、消费与当前配置，
//! 再插入并锁定 pending 订单，依 `(user_id, asset_id)` 全序锁双钱包，从源资产 available 扣款、
//! 向目标资产 available 入账，
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
                ConvertAuthoritativeQuoteRecord, ConvertMarketPriceSnapshot, ConvertPairRule,
                ConvertPairRuleDbRecord, ConvertSettlementOrderRecord,
                ConvertSettlementWalletRecord, WalletBalanceRecord,
            },
            service::{
                convert_market_pricing_source, convert_pair_rule_from_record,
                convert_quote_amounts, convert_quote_fingerprint, ensure_asset_precision_scale,
                ensure_convert_amount_precision, normalize_convert_rate_for_storage,
                resolve_fixed_convert_rate, validate_quote_amount,
            },
        },
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

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
                to_amount, rate, spread_rate, fee_rate, fee_amount, request_fingerprint,
                price_source, price_symbol, price_observed_at, price_version, expires_at, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'quoted')
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
        .bind(quote.request_fingerprint)
        .bind(quote.price_source)
        .bind(quote.price_symbol)
        .bind(quote.price_observed_at.naive_utc())
        .bind(quote.price_version)
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
        r#"SELECT pairs.id, pairs.enabled,
                  pairs.from_asset AS from_asset_id, pairs.to_asset AS to_asset_id,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate,
                  pairs.min_amount, pairs.max_amount,
                  pairs.target_min_amount, pairs.target_max_amount,
                  rules.fixed_rate,
                  market_pairs.symbol AS market_pair_symbol,
                  market_pairs.base_asset AS market_base_asset_id,
                  market_pairs.quote_asset AS market_quote_asset_id,
                  GREATEST(pairs.updated_at, COALESCE(rules.updated_at, pairs.updated_at))
                      AS pricing_updated_at
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

/// 从 MySQL append-only 行情历史选择该交易对事件时间最新的 ticker。
/// 本查询刻意不跳过未来、陈旧或异常快照；服务层校验最新行并 fail closed，禁止回退旧价掩盖坏数据。
pub(crate) async fn latest_market_price(
    pool: &Pool<MySql>,
    pair_symbol: &str,
) -> AppResult<Option<ConvertMarketPriceSnapshot>> {
    sqlx::query_as::<_, ConvertMarketPriceSnapshot>(
        r#"SELECT price, source, symbol, observed_at, source_version
           FROM market_price_ticks
           WHERE symbol = REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
           ORDER BY observed_at DESC,
                    CASE source
                        WHEN 'bitget' THEN 0
                        WHEN 'htx' THEN 1
                        WHEN 'coinbase' THEN 2
                        WHEN 'strategy' THEN 3
                        ELSE 9
                    END ASC,
                    source_version DESC,
                    id DESC
           LIMIT 1"#,
    )
    .bind(pair_symbol)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 返回 MySQL 当前时间，quote 创建与过期边界均以该时钟为准。
pub(crate) async fn database_now(pool: &Pool<MySql>) -> AppResult<DateTime<Utc>> {
    let value = sqlx::query_scalar::<_, chrono::NaiveDateTime>("SELECT CURRENT_TIMESTAMP(6)")
        .fetch_one(pool)
        .await?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
}

/// 通过调用方提供的执行器读取活动资产 precision_scale，并校验其处于钱包支持的 0..=18 范围。
/// 资产缺失、停用或精度损坏都会阻止报价/结算；本函数不截断现有余额或流水。
pub(crate) async fn load_asset_precision_scale<'e, E>(executor: E, asset_id: u64) -> AppResult<i32>
where
    E: sqlx::Executor<'e, Database = MySql>,
{
    let (precision_scale,): (i32,) = sqlx::query_as(
        "SELECT precision_scale FROM assets WHERE id = ? AND status = 'active' LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(executor)
    .await?
    .ok_or(AppError::NotFound)?;
    ensure_asset_precision_scale(precision_scale)?;
    Ok(precision_scale)
}

/// 为报价创建唯一 pending 订单，并在自有 MySQL 事务内完成闪兑资金结算。
/// 实际锁序是：quote→convert_pair/计价规则→按 asset_id 升序的资产行→pending 订单→
/// 按 asset_id 升序的两个 `(user_id, asset_id)` 钱包行。
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
    let quote = lock_authoritative_quote(&mut tx, &quote_id_value).await?;
    if quote.user_id != user_id {
        return Err(AppError::NotFound);
    }
    if quote.status != "quoted" || quote.consumed_at.is_some() {
        return Err(AppError::Conflict(
            "convert quote has already been confirmed".to_owned(),
        ));
    }
    let database_now = database_now_in_tx(&mut tx).await?;
    if database_now >= quote.expires_at {
        return Err(AppError::Validation("convert quote is expired".to_owned()));
    }
    if quote.from_asset_id == quote.to_asset_id
        || quote.from_amount <= 0
        || quote.to_amount <= 0
        || quote.rate <= 0
    {
        return Err(AppError::Conflict(
            "convert quote contains invalid authoritative amounts".to_owned(),
        ));
    }
    if convert_quote_fingerprint(&quote) != quote.request_fingerprint {
        return Err(AppError::Conflict(
            "convert quote fingerprint verification failed".to_owned(),
        ));
    }
    validate_authoritative_quote_config_in_tx(&mut tx, &quote, database_now).await?;
    insert_order_for_quote_in_tx(&mut tx, &quote).await?;
    settle_convert_order_in_tx(&mut tx, &quote_id_value, user_id).await?;
    let consumed = sqlx::query(
        r#"UPDATE convert_quotes
           SET status = 'consumed', consumed_at = ?
           WHERE quote_id = ? AND user_id = ? AND status = 'quoted'
             AND consumed_at IS NULL AND expires_at > ?"#,
    )
    .bind(database_now.naive_utc())
    .bind(&quote_id_value)
    .bind(user_id)
    .bind(database_now.naive_utc())
    .execute(&mut *tx)
    .await?;
    if consumed.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "convert quote could not be consumed exactly once".to_owned(),
        ));
    }
    tx.commit().await?;
    Ok(())
}

/// 在 quote 行锁之后锁定当前闪兑配置并复核报价快照。
/// 锁序固定为 quote→convert_pair→计价规则/现货对→按 asset_id 升序的资产行，
/// 随后才会创建订单和锁钱包。任一配置在 quote 后改变都返回冲突且零动账。
async fn validate_authoritative_quote_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    quote: &ConvertAuthoritativeQuoteRecord,
    database_now: DateTime<Utc>,
) -> AppResult<()> {
    let row = sqlx::query_as::<_, ConvertPairRuleDbRecord>(
        r#"SELECT pairs.id, pairs.enabled,
                  pairs.from_asset AS from_asset_id, pairs.to_asset AS to_asset_id,
                  pairs.pricing_mode, pairs.spread_rate, pairs.fee_rate,
                  pairs.min_amount, pairs.max_amount,
                  pairs.target_min_amount, pairs.target_max_amount,
                  rules.fixed_rate,
                  market_pairs.symbol AS market_pair_symbol,
                  market_pairs.base_asset AS market_base_asset_id,
                  market_pairs.quote_asset AS market_quote_asset_id,
                  GREATEST(pairs.updated_at, COALESCE(rules.updated_at, pairs.updated_at))
                      AS pricing_updated_at
           FROM convert_pairs pairs
           LEFT JOIN new_coin_convert_rules rules
             ON rules.convert_pair_id = pairs.id AND rules.status = 'active'
            AND rules.rate_source = 'fixed'
           LEFT JOIN trading_pairs market_pairs
             ON ((market_pairs.base_asset = pairs.from_asset AND market_pairs.quote_asset = pairs.to_asset)
                 OR (market_pairs.base_asset = pairs.to_asset AND market_pairs.quote_asset = pairs.from_asset))
            AND market_pairs.status = 'active'
           WHERE pairs.id = ?
           ORDER BY market_pairs.id DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote.convert_pair_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(convert_quote_config_conflict)?;
    let assets_match = (row.from_asset_id == quote.from_asset_id
        && row.to_asset_id == quote.to_asset_id)
        || (row.from_asset_id == quote.to_asset_id && row.to_asset_id == quote.from_asset_id);
    if !row.enabled || !assets_match {
        return Err(convert_quote_config_conflict());
    }
    let pair = convert_pair_rule_from_record(row, quote.from_asset_id, quote.to_asset_id)
        .map_err(|_| convert_quote_config_conflict())?;
    if pair.spread_rate != quote.spread_rate || pair.fee_rate != quote.fee_rate {
        return Err(convert_quote_config_conflict());
    }
    validate_quote_amount(&quote.from_amount, &pair)
        .map_err(|_| convert_quote_config_conflict())?;

    let (first_asset_id, second_asset_id) = if quote.from_asset_id < quote.to_asset_id {
        (quote.from_asset_id, quote.to_asset_id)
    } else {
        (quote.to_asset_id, quote.from_asset_id)
    };
    let first_precision = lock_active_asset_precision_in_tx(tx, first_asset_id).await?;
    let second_precision = lock_active_asset_precision_in_tx(tx, second_asset_id).await?;
    let (from_precision, to_precision) = if quote.from_asset_id == first_asset_id {
        (first_precision, second_precision)
    } else {
        (second_precision, first_precision)
    };
    ensure_convert_amount_precision(&quote.from_amount, from_precision, "from_amount")
        .map_err(|_| convert_quote_config_conflict())?;

    match pair.pricing_mode.as_str() {
        "fixed" => {
            let expected_rate = normalize_convert_rate_for_storage(
                &resolve_fixed_convert_rate(&pair).map_err(|_| convert_quote_config_conflict())?,
            )
            .map_err(|_| convert_quote_config_conflict())?;
            let expected_version = format!(
                "convert_pair:{}:{}",
                pair.id,
                pair.pricing_updated_at.timestamp_micros()
            );
            if quote.rate != expected_rate
                || quote.price_source != "fixed"
                || quote.price_symbol.is_some()
                || quote.price_observed_at != pair.pricing_updated_at
                || quote.price_version != expected_version
            {
                return Err(convert_quote_config_conflict());
            }
        }
        "market" => {
            let (expected_symbol, market_base_asset_id, market_quote_asset_id) =
                convert_market_pricing_source(&pair)
                    .map_err(|_| convert_quote_config_conflict())?;
            let evidence_symbol = quote
                .price_symbol
                .as_deref()
                .ok_or_else(convert_quote_config_conflict)?;
            let normalize_symbol = |value: &str| {
                value
                    .trim()
                    .chars()
                    .filter(|character| !matches!(character, '-' | '/' | '_'))
                    .flat_map(char::to_uppercase)
                    .collect::<String>()
            };
            let direction_matches = (quote.from_asset_id == market_base_asset_id
                && quote.to_asset_id == market_quote_asset_id)
                || (quote.from_asset_id == market_quote_asset_id
                    && quote.to_asset_id == market_base_asset_id);
            if !direction_matches
                || normalize_symbol(evidence_symbol) != normalize_symbol(expected_symbol)
                || !matches!(
                    quote.price_source.as_str(),
                    "bitget" | "htx" | "coinbase" | "strategy"
                )
                || quote.price_version.trim().is_empty()
                || quote.price_observed_at > database_now
            {
                return Err(convert_quote_config_conflict());
            }
        }
        _ => return Err(convert_quote_config_conflict()),
    }

    let expected_amounts = convert_quote_amounts(
        &quote.from_amount,
        &pair,
        &quote.rate,
        from_precision,
        to_precision,
    )
    .map_err(|_| convert_quote_config_conflict())?;
    if expected_amounts.to_amount != quote.to_amount
        || expected_amounts.fee_amount != quote.fee_amount
    {
        return Err(convert_quote_config_conflict());
    }
    Ok(())
}

async fn lock_active_asset_precision_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<i32> {
    let (precision_scale, status): (i32, String) = sqlx::query_as(
        "SELECT precision_scale, status FROM assets WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(convert_quote_config_conflict)?;
    if status != "active" {
        return Err(convert_quote_config_conflict());
    }
    ensure_asset_precision_scale(precision_scale).map_err(|_| convert_quote_config_conflict())?;
    Ok(precision_scale)
}

fn convert_quote_config_conflict() -> AppError {
    AppError::Conflict("convert configuration changed after quote creation".to_owned())
}

/// 以 `FOR UPDATE` 锁定 MySQL 权威 quote；确认流程的第一把锁始终是报价行。
async fn lock_authoritative_quote(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
) -> AppResult<ConvertAuthoritativeQuoteRecord> {
    sqlx::query_as::<_, ConvertAuthoritativeQuoteRecord>(
        r#"SELECT quote_id, convert_pair_id, user_id,
                  from_asset AS from_asset_id, to_asset AS to_asset_id,
                  from_amount, to_amount, rate, spread_rate, fee_rate, fee_amount,
                  request_fingerprint, price_source, price_symbol, price_observed_at,
                  price_version, expires_at, status, consumed_at
           FROM convert_quotes
           WHERE quote_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn database_now_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<DateTime<Utc>> {
    let value = sqlx::query_scalar::<_, chrono::NaiveDateTime>("SELECT CURRENT_TIMESTAMP(6)")
        .fetch_one(&mut **tx)
        .await?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
}

/// 在结算事务内把报价快照复制成一条 pending 订单，并以返回值告知调用方是否为首次插入。
/// 订单字段全部由 `convert_quotes` 行 SELECT 而来，调用方无法覆写金额、汇率或费率。
/// quote_id 上的唯一约束是本次结算的幂等键：重放时 ON DUPLICATE 走空更新，
/// `last_insert_id` 为零因而返回 false，调用方据此回滚并报冲突，绝不重复扣款。
/// 报价行不存在时 SELECT 命中零行，同样返回 false，语义上与重复确认合并处理。
async fn insert_order_for_quote_in_tx(
    tx: &mut Transaction<'_, MySql>,
    quote: &ConvertAuthoritativeQuoteRecord,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
            to_amount, rate, fee_rate, fee_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')"#,
    )
    .bind(&quote.quote_id)
    .bind(quote.convert_pair_id)
    .bind(quote.user_id)
    .bind(quote.from_asset_id)
    .bind(quote.to_asset_id)
    .bind(&quote.from_amount)
    .bind(&quote.to_amount)
    .bind(&quote.rate)
    .bind(&quote.fee_rate)
    .bind(&quote.fee_amount)
    .execute(&mut **tx)
    .await;
    match result {
        Ok(_) => Ok(()),
        Err(error)
            if error
                .as_database_error()
                .is_some_and(|item| item.is_unique_violation()) =>
        {
            Err(AppError::Conflict(
                "convert quote has already produced an order".to_owned(),
            ))
        }
        Err(error) => Err(AppError::Database(error)),
    }
}

/// 在调用方已开启的事务内完成一笔闪兑的全部资金移动，调用前订单必须已插入且状态为 pending。
/// 加锁顺序固定为：先 FOR UPDATE 锁定该用户的 pending 订单行，再按 asset_id 升序锁两个钱包。
/// 同一用户的正反向闪兑因此遵守同一 `(user_id, asset_id)` 全序，不会形成钱包等待环。
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

    if order.from_asset_id == order.to_asset_id {
        return Err(AppError::Conflict(
            "convert settlement assets must be distinct".to_owned(),
        ));
    }
    let (first_asset_id, second_asset_id) = if order.from_asset_id < order.to_asset_id {
        (order.from_asset_id, order.to_asset_id)
    } else {
        (order.to_asset_id, order.from_asset_id)
    };
    let first_wallet = lock_wallet_row(tx, user_id, first_asset_id).await?;
    let second_wallet = lock_wallet_row(tx, user_id, second_asset_id).await?;
    let (from_wallet, to_wallet) = if order.from_asset_id == first_asset_id {
        (first_wallet, second_wallet)
    } else {
        (second_wallet, first_wallet)
    };
    if from_wallet.available < order.from_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for convert settlement: requested {}, available {}, locked {}",
            order.from_amount, from_wallet.available, from_wallet.locked
        )));
    }
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
