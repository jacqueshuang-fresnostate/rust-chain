//! 权威行情 ingestion 基础设施。
//!
//! 仅接收已由 provider 适配器归一化且通过领域校验的快照；外部行情写 Redis/Mongo，
//! synthetic ticker 则必须先在 MySQL 持锁复核租约并归档，随后才允许写 Redis，最后尝试触发现货与杠杆订单；
//! 任一订单成交失败都不回滚已落地行情。
//!
//! 落地目标分三处：Redis 承载实时权威快照，Mongo 的 `market_klines_<SYMBOL>` 集合承载 K 线历史，
//! MySQL `market_price_ticks` 承载可供事件时间结算的 ticker 归档。
//! 写入由缓存的原子时序门禁做总闸：ticker 比较 `observed_at`，
//! K 线比较 `(open_time, observed_at)`，被判为陈旧的快照直接短路，不会继续写 Mongo、不触发撮合、不广播。
//! Redis 与 Mongo 不在同一事务内，Mongo 失败时已写入的 Redis 快照不会回滚，下一次推送会自然修复；
//! Mongo 侧另有一层基于 `updated_at` 的时序过滤和唯一键竞态处理，防止 Redis 判定之后仍被并发写者倒退。
//! 广播只发生在 synthetic 策略行情路径上，第三方 feed 的广播由上层 worker 在摄取成功后负责。

use super::feed::MarketFeedEvent;
use super::provider::provider_name;
use crate::{
    error::{AppError, AppResult},
    infra::mongo::{ensure_kline_indexes, kline_collection_name},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        margin::application::execute_triggered_margin_limit_orders_with_hub as execute_triggered_margin_limit_orders,
        market::{
            KlineUpsertKey, MarketDataProvider, MarketDepthSnapshot, MarketKlineSnapshot,
            MarketTickerSnapshot, ValidatedMarketSymbol,
            infrastructure::{
                MarketCacheError, MarketCacheWriteOutcome, MarketDepthCacheEntry,
                MarketKlineCacheEntry, MarketTickerCacheEntry, RedisMarketCache,
            },
        },
        spot::application::execute_triggered_spot_limit_orders_with_hub as execute_triggered_spot_limit_orders,
    },
    state::AppState,
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use mongodb::bson::{DateTime as BsonDateTime, Document, doc};
use sha2::Digest;
use sqlx::{MySql, Pool, Transaction};

#[async_trait]
pub trait MarketIngestionSink: Clone + Send + Sync + 'static {
    /// 持久化标准 ticker 快照；实现成功返回后，该价格才可供下单、结算和强平消费者读取。
    /// 返回 `Ok` 不代表本次快照一定被采纳，陈旧数据被拒同样是成功结果，实现不应把它变成错误。
    async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()>;
    /// 持久化标准深度快照；不得绕过交易对和数值校验。
    /// 盘口按覆盖语义写入，实现不需要也不应该维护跨快照的增量合并状态。
    async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()>;
    /// 持久化标准 K 线快照；实现必须保持交易对+周期+开盘时间的幂等写入。
    /// 同一根形成中的蜡烛会被反复投递，实现要保证重放只更新那一条记录而不是追加历史。
    async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()>;
}

/// synthetic 行情摄取的时序结果；拒绝表示 Redis 已有更新快照，调用方不得继续任何派生副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticIngestionOutcome {
    Accepted,
    ReplayedIdentical,
    RejectedStale,
}

impl SyntheticIngestionOutcome {
    /// 返回本次 synthetic 快照是否已有可重放的 Redis 载荷且 MySQL 归档已完整。
    /// 首次接受和同载荷回放都可让 worker 继续；只有 stale/冲突分支必须停止后续检查点。
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted | Self::ReplayedIdentical)
    }
}

impl From<MarketCacheWriteOutcome> for SyntheticIngestionOutcome {
    /// 把缓存层的写入结论逐一映射为摄取层结论，两者语义完全对齐，不做任何降级或合并。
    /// 保留独立类型是为了让 worker 依赖摄取层契约而非直接耦合 Redis 缓存的返回值。
    fn from(value: MarketCacheWriteOutcome) -> Self {
        match value {
            MarketCacheWriteOutcome::Accepted => Self::Accepted,
            MarketCacheWriteOutcome::ReplayedIdentical => Self::ReplayedIdentical,
            MarketCacheWriteOutcome::RejectedStale => Self::RejectedStale,
        }
    }
}

/// synthetic ticker 归档所需的策略运行证据；调用方只能传入本轮实际持有的 owner 与版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticTickerProvenance {
    strategy_id: u64,
    active_version: u32,
    lease_owner: String,
}

impl SyntheticTickerProvenance {
    /// 构造一份候选归档证据；真实性不在内存中信任，归档事务会持锁重新核对数据库当前值。
    pub fn new(strategy_id: u64, active_version: u32, lease_owner: impl Into<String>) -> Self {
        Self {
            strategy_id,
            active_version,
            lease_owner: lease_owner.into(),
        }
    }

    /// 返回策略主键，用于锁定唯一的 `strategy_runs` 行。
    pub fn strategy_id(&self) -> u64 {
        self.strategy_id
    }

    /// 返回生成快照时的活跃版本，同时作为历史 ticker 的 generation。
    pub fn active_version(&self) -> u32 {
        self.active_version
    }

    /// 返回本轮租约 owner；归档前必须与持锁查到的运行行完全一致。
    pub fn lease_owner(&self) -> &str {
        &self.lease_owner
    }

    /// 以策略与版本生成稳定来源版本文本，重启和回放得到相同值。
    pub fn source_version(&self) -> String {
        format!("strategy:{}:v{}", self.strategy_id, self.active_version)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SyntheticTickerLeaseRow {
    symbol: String,
    pair_status: String,
    market_type: String,
    strategy_status: String,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    active_version: i32,
    run_status: String,
    lease_owner: Option<String>,
    lease_expires_at: Option<DateTime<Utc>>,
    last_tick_at: Option<DateTime<Utc>>,
    database_now: DateTime<Utc>,
}

#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
struct SyntheticTickerArchiveRow {
    event_key: String,
    symbol: String,
    price: BigDecimal,
    source: String,
    observed_at: DateTime<Utc>,
    generation: u64,
    source_version: String,
    strategy_id: Option<u64>,
    strategy_version: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntheticTickerArchiveOutcome {
    Inserted,
    AlreadyArchived,
}

#[derive(Clone)]
pub struct MarketIngestionService {
    cache: RedisMarketCache,
    database: mongodb::Database,
    mysql: Option<Pool<MySql>>,
    broadcast_hub: Option<EventBroadcastHub>,
}

impl MarketIngestionService {
    /// 以 Redis 实时缓存和 Mongo 历史库构造行情摄取器，默认不触发现货订单，也不持有广播中心。
    /// 构造阶段不探测连接；缓存、索引或写入错误会在对应 `ingest_*` 调用时返回。
    pub fn new(cache: RedisMarketCache, database: mongodb::Database) -> Self {
        Self {
            cache,
            database,
            mysql: None,
            broadcast_hub: None,
        }
    }

    /// 从应用状态取得必需的 Redis、Mongo 以及可选 MySQL、广播中心；缺少前两者立即返回配置错误。
    /// 构造过程不发送 Redis/Mongo/WS 命令，也不探测连接可用性。
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        let redis = state.redis.clone().ok_or_else(|| {
            AppError::Internal("redis connection is not configured for market ingestion".to_owned())
        })?;
        let mongo = state.mongo.clone().ok_or_else(|| {
            AppError::Internal("mongo database is not configured for market ingestion".to_owned())
        })?;
        Ok(Self::new(RedisMarketCache::new(redis), mongo)
            .with_mysql(state.mysql.clone())
            .with_broadcast_hub(state.event_broadcast_hub.clone()))
    }

    /// 注入可选 MySQL 池，供外部 ticker/depth 触发限价单，也供 synthetic ticker 复核租约并归档历史；不测试连接或立即执行 SQL。
    /// 传入 `None` 时外部行情仍可按纯采集进程语义落地，但 synthetic ticker 会 fail closed，不允许跳过 MySQL 结算历史。
    pub fn with_mysql(mut self, mysql: Option<Pool<MySql>>) -> Self {
        self.mysql = mysql;
        self
    }

    /// 注入进程内广播中心，供现货/杠杆触发链发布订单事件和 synthetic 行情发布实时快照；本方法本身不订阅或发布 WS 消息。
    /// 传入 `None` 时所有发布调用都会静默跳过而不报错，行情仍会正常写入 Redis 与 Mongo，
    /// 只是订阅端收不到推送，需要靠客户端轮询缓存补齐，因此生产进程应始终注入。
    pub fn with_broadcast_hub(mut self, broadcast_hub: Option<EventBroadcastHub>) -> Self {
        self.broadcast_hub = broadcast_hub;
        self
    }

    /// 将外部供应商 ticker 快照写入 Redis 权威缓存，只在 CAS 接受后尝试触发现货与杠杆限价单。
    /// 快照必须已由 provider adapter 校验交易对、价格与时间；Strategy 来源在此显式拒绝，必须走带租约证据与 MySQL 归档的专用入口。
    /// 缓存失败时不得触发订单，成交失败只告警且不撤销已写行情。
    pub async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        if snapshot.provider() == MarketDataProvider::Strategy {
            return Err(AppError::Validation(
                "strategy ticker must use provenance-aware synthetic ingestion".to_owned(),
            ));
        }
        let entry = MarketTickerCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_ticker_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if outcome.is_accepted() {
            self.trigger_spot_limit_orders(snapshot.symbol(), snapshot.last_price(), "ticker")
                .await;
            self.trigger_margin_limit_orders(snapshot.symbol(), snapshot.last_price())
                .await;
        }
        Ok(())
    }

    /// 为 synthetic ticker 先在短 MySQL 事务持锁复核策略租约并幂等归档历史，再执行 Redis `observed_at` 原子 CAS。
    /// 过期 owner、旧版本和非法事件时间在接触 Redis 前即被拒绝，因此不会留下无合法归档的实时 ticker。
    /// MySQL 已提交后 Redis 短暂失败时，同事件重试会命中归档并补齐缓存；不重放已归档事件的资金触发或广播。
    /// 归档固定写入 `source=strategy`、活跃版本 generation、稳定 source_version 与 SHA-256 event_key，唯一冲突只有字段全等才按回放成功。
    /// 现货/杠杆触发器和 WebSocket 广播均排在归档事务提交之后；既有归档的重放不重复执行这些副作用。
    pub async fn ingest_and_publish_synthetic_ticker(
        &self,
        snapshot: &MarketTickerSnapshot,
        provenance: &SyntheticTickerProvenance,
    ) -> AppResult<SyntheticIngestionOutcome> {
        let mysql = self.mysql.as_ref().ok_or_else(|| {
            AppError::Internal(
                "mysql is required for synthetic ticker provenance archive".to_owned(),
            )
        })?;
        let archive_outcome = archive_synthetic_ticker(mysql, snapshot, provenance).await?;
        let entry = MarketTickerCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_ticker_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if outcome == MarketCacheWriteOutcome::RejectedStale {
            return Ok(SyntheticIngestionOutcome::RejectedStale);
        }
        if archive_outcome == SyntheticTickerArchiveOutcome::Inserted {
            self.trigger_spot_limit_orders(snapshot.symbol(), snapshot.last_price(), "ticker")
                .await;
            self.trigger_margin_limit_orders(snapshot.symbol(), snapshot.last_price())
                .await;
            self.publish(MarketFeedEvent::from_ticker_snapshot(snapshot)?)?;
        }
        Ok(
            if outcome == MarketCacheWriteOutcome::Accepted
                && archive_outcome == SyntheticTickerArchiveOutcome::Inserted
            {
                SyntheticIngestionOutcome::Accepted
            } else {
                SyntheticIngestionOutcome::ReplayedIdentical
            },
        )
    }

    /// 兼容现有内部调用名并委托 synthetic 时序摄取；返回值显式要求调用方处理拒写。
    /// 新代码应优先使用 [`Self::ingest_and_publish_synthetic_ticker`] 表达仅供策略行情的副作用门禁。
    pub async fn ingest_and_publish_ticker(
        &self,
        snapshot: &MarketTickerSnapshot,
        provenance: &SyntheticTickerProvenance,
    ) -> AppResult<SyntheticIngestionOutcome> {
        self.ingest_and_publish_synthetic_ticker(snapshot, provenance)
            .await
    }

    /// 写入深度快照，并在存在卖一价时把它作为现货触发价候选；深度解析或缓存失败时不触发订单。
    /// 撮合是缓存成功后的独立副作用，失败只告警，不回滚已持久化的市场深度。
    pub async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()> {
        let entry = MarketDepthCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        self.cache
            .save_depth(entry)
            .await
            .map_err(market_cache_error)?;
        if let Some(best_ask) = snapshot.asks().iter().map(|level| &level.price).min() {
            self.trigger_spot_limit_orders(snapshot.symbol(), best_ask, "depth")
                .await;
        }
        Ok(())
    }

    /// 将 K 线依次写入 Redis 最新快照与 Mongo 历史集合；交易对、周期和开盘时间必须先通过领域校验。
    /// Mongo 以 interval+open_time 唯一索引 upsert，重放只覆盖同一根蜡烛；两次写入不在同一事务中，Mongo 失败时 Redis 快照不会回滚。
    pub async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        let entry = MarketKlineCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_kline_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if outcome.is_accepted() {
            self.upsert_kline_mongo(snapshot).await?;
        }
        Ok(())
    }

    /// 为 synthetic K 线执行 `(open_time, observed_at)` 原子时序门禁，accepted 后才 upsert Mongo 并广播。
    /// Redis 的门禁是第一道闸，Mongo 侧还会再按 `updated_at` 比一次，两道判定都通过才算这一版数值生效。
    /// Mongo 写入失败会直接返回错误，此时 Redis 已经更新，缓存与历史短暂不一致，靠后续推送重新对齐。
    /// 广播排在 Mongo 之后，确保订阅端看到的形成中蜡烛一定已经落进历史集合，不会出现只推送不落库的情况。
    /// 同分钟旧 owner 或旧分钟快照被拒后不会覆盖 Mongo、不会发布陈旧 forming candle，也不会允许 worker 推进检查点。
    pub async fn ingest_and_publish_synthetic_kline(
        &self,
        snapshot: &MarketKlineSnapshot,
    ) -> AppResult<SyntheticIngestionOutcome> {
        let entry = MarketKlineCacheEntry::from_snapshot(snapshot)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let outcome = self
            .cache
            .save_kline_if_fresh(entry)
            .await
            .map_err(market_cache_error)?;
        if !outcome.is_accepted() {
            return Ok(SyntheticIngestionOutcome::RejectedStale);
        }
        self.upsert_kline_mongo(snapshot).await?;
        self.publish(MarketFeedEvent::from_kline_snapshot(snapshot)?)?;
        Ok(SyntheticIngestionOutcome::Accepted)
    }

    /// 兼容现有内部调用名并返回 synthetic 时序结果；调用方必须处理 `RejectedStale`，不得默认推进检查点。
    /// 这里没有额外逻辑，完全委托给 [`Self::ingest_and_publish_synthetic_kline`]，保留只为不改动既有调用点。
    /// 新代码应直接使用带 synthetic 字样的入口，以便从名字上看出这条路径专供策略行情且带副作用门禁。
    pub async fn ingest_and_publish_kline(
        &self,
        snapshot: &MarketKlineSnapshot,
    ) -> AppResult<SyntheticIngestionOutcome> {
        self.ingest_and_publish_synthetic_kline(snapshot).await
    }

    /// 把一根 K 线幂等写入 `market_klines_<SYMBOL>` 集合，并在 Mongo 侧再做一次基于 `updated_at` 的时序判定。
    /// 每次写入前先确保 `interval + open_time` 唯一索引存在，该索引既保证同槽只有一条记录，也是并发去重的依据。
    /// 随后按同槽查一次现有文档，分三条路径处理：已有文档更新则返回 `Conflict` 拒绝倒退；
    /// 已有文档不更新则带时序条件 `update_one`，匹配数为 0 说明输给了并发写者，同样返回 `Conflict`；
    /// 完全不存在则走唯一键插入，捕获重复键错误判定为首写竞争失败。
    /// 首写之所以不用带条件的 upsert，是因为条件不满足时 upsert 会尝试插入并触发重复键错误，难以与真实故障区分。
    /// 本方法只写 Mongo，不碰 Redis，因此返回 `Conflict` 时缓存中的新值依然保留。
    async fn upsert_kline_mongo(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        let mongo_write = MarketKlineMongoWrite::from_snapshot(snapshot)?;
        ensure_kline_indexes(&self.database, mongo_write.symbol()).await?;
        let collection = self
            .database
            .collection::<Document>(&mongo_write.collection_name());
        match collection.find_one(mongo_write.upsert_filter()).await? {
            Some(existing) if mongo_write.existing_is_newer(&existing) => {
                return Err(AppError::Conflict(
                    "market kline rejected because a newer Mongo snapshot already exists"
                        .to_owned(),
                ));
            }
            Some(_) => {
                let result = collection
                    .update_one(
                        mongo_write.fresh_existing_filter(),
                        mongo_write.upsert_update(),
                    )
                    .await?;
                if result.matched_count == 0 {
                    return Err(AppError::Conflict(
                        "market kline lost a concurrent freshness race".to_owned(),
                    ));
                }
            }
            None if !collection.insert_if_absent(&mongo_write).await? => {
                return Err(AppError::Conflict(
                    "market kline lost a concurrent first-write race".to_owned(),
                ));
            }
            None => {}
        }
        Ok(())
    }

    /// 把已构造好的行情事件投递到进程内广播中心，供公开 WebSocket 订阅端消费。
    /// 未注入广播中心时静默跳过并返回成功，因此纯采集进程不会因为缺少推送通道而中断摄取。
    /// 只有事件转换失败才会返回错误；投递本身是进程内操作，不做重试，也不保证订阅端一定收到。
    fn publish(&self, event: MarketFeedEvent) -> AppResult<()> {
        if let Some(hub) = &self.broadcast_hub {
            hub.publish(EventBroadcastMessage::from_market_feed_event(&event)?);
        }
        Ok(())
    }

    /// 在行情落地之后，以给定市场价尝试激活该交易对上满足触发条件的现货限价单。
    /// 未配置 MySQL 池时整段跳过，这也是纯行情采集进程的常态，不算异常。
    /// 调用方必须先确认缓存写入被接受再进入本方法，否则会用陈旧价格触发撮合。
    /// `source` 只用于日志区分触发来源是 ticker 的最新价还是 depth 的最优卖价，不影响撮合逻辑。
    /// 本方法有意吞掉撮合错误，只记 warn 日志：行情已经落地且不可回滚，撮合失败不应把摄取整体判为失败，
    /// 未成交的限价单会在后续行情推送时重新获得触发机会。
    async fn trigger_spot_limit_orders(
        &self,
        symbol: &str,
        market_price: &BigDecimal,
        source: &str,
    ) {
        if let Some(pool) = &self.mysql
            && let Err(error) = execute_triggered_spot_limit_orders(
                pool,
                symbol,
                market_price,
                self.broadcast_hub.as_ref(),
            )
            .await
        {
            tracing::warn!(
                error = %error,
                symbol,
                market_price = %market_price,
                source,
                "spot limit order trigger failed after market ingestion"
            );
        }
    }

    /// 只在 ticker 已成为 Redis 权威快照之后，以其 `last_price` 触发杠杆限价挂单。
    /// 深度快照不调用本方法，避免把卖一价当成做空和做多共用的成交价；两个方向统一只认 accepted ticker。
    /// 缺少 MySQL 时按纯行情进程语义静默跳过；触发失败只告警，挂单保留在数据库等待下一笔 accepted ticker。
    async fn trigger_margin_limit_orders(&self, symbol: &str, market_price: &BigDecimal) {
        if let Some(pool) = &self.mysql
            && let Err(error) = execute_triggered_margin_limit_orders(
                pool,
                symbol,
                market_price,
                self.broadcast_hub.as_ref(),
            )
            .await
        {
            tracing::warn!(
                error = %error,
                symbol,
                market_price = %market_price,
                "margin limit order trigger failed after accepted ticker ingestion"
            );
        }
    }
}

/// 在短事务中锁定策略运行行，复核 owner、版本、状态和租约后幂等追加 synthetic ticker 历史。
/// 事务内不访问 Redis、Mongo、撮合或广播，使行锁持有时间只覆盖一次复核和一次归档写入。
async fn archive_synthetic_ticker(
    pool: &Pool<MySql>,
    snapshot: &MarketTickerSnapshot,
    provenance: &SyntheticTickerProvenance,
) -> AppResult<SyntheticTickerArchiveOutcome> {
    if snapshot.provider() != MarketDataProvider::Strategy {
        return Err(AppError::Validation(
            "synthetic ticker archive requires strategy provider".to_owned(),
        ));
    }
    if provenance.strategy_id() == 0
        || provenance.active_version() == 0
        || provenance.lease_owner().trim().is_empty()
    {
        return Err(AppError::Validation(
            "synthetic ticker provenance is incomplete".to_owned(),
        ));
    }
    if snapshot.last_price() <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "synthetic ticker archive price must be positive".to_owned(),
        ));
    }
    let symbol = ValidatedMarketSymbol::from_raw(snapshot.symbol())
        .map_err(|error| AppError::Validation(error.to_string()))?
        .as_str()
        .to_owned();
    let strategy_version = i32::try_from(provenance.active_version()).map_err(|_| {
        AppError::Validation("synthetic strategy version exceeds database range".to_owned())
    })?;
    // Redis ticker 合同以毫秒序列化事件时间；归档使用同一精度，确保逐字节回放总是命中同一时间槽。
    let observed_at =
        DateTime::<Utc>::from_timestamp_millis(snapshot.observed_at().timestamp_millis())
            .ok_or_else(|| {
                AppError::Validation("synthetic ticker event time is invalid".to_owned())
            })?;
    let source_version = provenance.source_version();
    let canonical = format!(
        "strategy|{}|{}|{}|{}|{}",
        symbol,
        observed_at.timestamp_micros(),
        snapshot.last_price().normalized(),
        provenance.active_version(),
        source_version
    );
    let event_key = hex::encode(sha2::Sha256::digest(canonical.as_bytes()));
    let expected_archive = SyntheticTickerArchiveRow {
        event_key: event_key.clone(),
        symbol: symbol.clone(),
        price: snapshot.last_price().clone(),
        source: "strategy".to_owned(),
        observed_at,
        generation: u64::from(provenance.active_version()),
        source_version: source_version.clone(),
        strategy_id: Some(provenance.strategy_id()),
        strategy_version: Some(strategy_version),
    };

    let mut tx = pool.begin().await?;
    let lease = sqlx::query_as::<_, SyntheticTickerLeaseRow>(
        r#"SELECT pairs.symbol,
                  pairs.status AS pair_status,
                  pairs.market_type,
                  strategies.status AS strategy_status,
                  strategies.start_time,
                  strategies.end_time,
                  runs.active_version,
                  runs.run_status,
                  runs.lease_owner,
                  runs.lease_expires_at,
                  runs.last_tick_at,
                  CURRENT_TIMESTAMP(6) AS database_now
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           INNER JOIN strategy_runs runs ON runs.strategy_id = strategies.id
           INNER JOIN strategy_versions versions
                   ON versions.strategy_id = runs.strategy_id
                  AND versions.version = runs.active_version
           WHERE strategies.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(provenance.strategy_id())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(stale_synthetic_provenance_conflict)?;

    let lease_symbol = ValidatedMarketSymbol::from_raw(&lease.symbol)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let lease_is_current = lease.strategy_status == "active"
        && lease.pair_status == "active"
        && matches!(lease.market_type.as_str(), "strategy" | "internal")
        && matches!(lease.run_status.as_str(), "running" | "live")
        && lease.active_version == strategy_version
        && lease.lease_owner.as_deref() == Some(provenance.lease_owner())
        && lease
            .lease_expires_at
            .is_some_and(|expires_at| expires_at > lease.database_now)
        && lease.start_time <= lease.database_now
        && lease.end_time > lease.database_now
        && lease_symbol.as_str() == symbol;
    if !lease_is_current {
        return Err(stale_synthetic_provenance_conflict());
    }
    let event_time_is_valid = lease.start_time <= observed_at
        && lease.end_time > observed_at
        && observed_at <= lease.database_now
        && lease
            .last_tick_at
            .is_none_or(|last_tick_at| last_tick_at <= observed_at);
    if !event_time_is_valid {
        return Err(AppError::Conflict(
            "synthetic ticker event time is outside current strategy bounds or regressed checkpoint"
                .to_owned(),
        ));
    }

    if let Some(existing) =
        load_latest_synthetic_ticker_archive(&mut tx, provenance.strategy_id()).await?
    {
        if existing.observed_at > observed_at {
            return Err(AppError::Conflict(
                "synthetic ticker event time regressed behind archived history".to_owned(),
            ));
        }
        if existing.observed_at == observed_at {
            if synthetic_ticker_archive_matches(&existing, &expected_archive) {
                tx.commit().await?;
                return Ok(SyntheticTickerArchiveOutcome::AlreadyArchived);
            }
            return Err(AppError::Conflict(
                "synthetic ticker archive conflicts with an existing event payload".to_owned(),
            ));
        }
    }

    let insert = sqlx::query(
        r#"INSERT INTO market_price_ticks
           (event_key, symbol, price, source, observed_at, generation, source_version,
            strategy_id, strategy_version)
           VALUES (?, ?, ?, 'strategy', ?, ?, ?, ?, ?)"#,
    )
    .bind(&event_key)
    .bind(&symbol)
    .bind(snapshot.last_price())
    .bind(observed_at.naive_utc())
    .bind(u64::from(provenance.active_version()))
    .bind(&source_version)
    .bind(provenance.strategy_id())
    .bind(strategy_version)
    .execute(&mut *tx)
    .await;
    let archive_outcome = match insert {
        Ok(_) => SyntheticTickerArchiveOutcome::Inserted,
        Err(error) if is_mysql_duplicate_key(&error) => {
            let existing = load_synthetic_ticker_archive_by_event_key(&mut tx, &event_key)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict(
                        "synthetic ticker archive uniqueness conflict has no matching row"
                            .to_owned(),
                    )
                })?;
            if !synthetic_ticker_archive_matches(&existing, &expected_archive) {
                return Err(AppError::Conflict(
                    "synthetic ticker archive conflicts with an existing event payload".to_owned(),
                ));
            }
            SyntheticTickerArchiveOutcome::AlreadyArchived
        }
        Err(error) => return Err(AppError::Database(error)),
    };
    tx.commit().await?;
    Ok(archive_outcome)
}

/// 读取该策略已归档的最新事件；策略运行行已被同一事务锁定，因此可以安全防止重启或 Redis 丢数据后的时间倒退。
async fn load_latest_synthetic_ticker_archive(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<Option<SyntheticTickerArchiveRow>> {
    sqlx::query_as::<_, SyntheticTickerArchiveRow>(
        r#"SELECT event_key, symbol, price, source, observed_at, generation,
                  source_version, strategy_id, strategy_version
           FROM market_price_ticks
           WHERE strategy_id = ?
           ORDER BY observed_at DESC, id DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(strategy_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 插入只在 event_key 全局唯一约束竞态时才走该查询，只有全部归档字段一致才可以按幂等回放处理。
async fn load_synthetic_ticker_archive_by_event_key(
    tx: &mut Transaction<'_, MySql>,
    event_key: &str,
) -> AppResult<Option<SyntheticTickerArchiveRow>> {
    sqlx::query_as::<_, SyntheticTickerArchiveRow>(
        r#"SELECT event_key, symbol, price, source, observed_at, generation,
                  source_version, strategy_id, strategy_version
           FROM market_price_ticks
           WHERE event_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(event_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 逐字段核对冲突行；只有事件键、价格、时间与全部策略来源证据完全一致才视为幂等回放。
fn synthetic_ticker_archive_matches(
    existing: &SyntheticTickerArchiveRow,
    expected: &SyntheticTickerArchiveRow,
) -> bool {
    existing.event_key == expected.event_key
        && existing.symbol == expected.symbol
        && existing.price.normalized() == expected.price.normalized()
        && existing.source == expected.source
        && existing.observed_at == expected.observed_at
        && existing.generation == expected.generation
        && existing.source_version == expected.source_version
        && existing.strategy_id == expected.strategy_id
        && existing.strategy_version == expected.strategy_version
}

/// 统一生成策略运行证据已变更的冲突错误，防止调用方将过期 owner 当作可重试的存储故障。
fn stale_synthetic_provenance_conflict() -> AppError {
    AppError::Conflict(
        "synthetic strategy owner, version, status, or lease changed before archive".to_owned(),
    )
}

/// 识别 MySQL 唯一键冲突，其他数据库错误不参与幂等回放判定。
fn is_mysql_duplicate_key(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|database_error| database_error.code().as_deref() == Some("1062"))
}

#[async_trait]
impl MarketIngestionSink for MarketIngestionService {
    /// 把 trait 调用转交给同名固有方法，使 feed worker 能以泛型 sink 的形式复用生产摄取实现。
    /// 显式写出全限定调用是为了避免与 trait 方法自身递归；此处不添加任何额外校验或副作用。
    async fn ingest_ticker(&self, snapshot: &MarketTickerSnapshot) -> AppResult<()> {
        MarketIngestionService::ingest_ticker(self, snapshot).await
    }

    /// 同样转交给固有的深度摄取实现，保持覆盖写入与卖一价触发撮合的行为完全一致。
    /// 走 trait 的调用方拿不到缓存写入结论，需要区分陈旧与否时应直接使用固有方法。
    async fn ingest_depth(&self, snapshot: &MarketDepthSnapshot) -> AppResult<()> {
        MarketIngestionService::ingest_depth(self, snapshot).await
    }

    /// 转交给固有的 K 线摄取实现，沿用先 Redis 时序门禁、再 Mongo 幂等 upsert 的两段写入。
    /// 通过本入口写入的 K 线不会广播，实时推送由上层 feed worker 在摄取成功后另行发起。
    async fn ingest_kline(&self, snapshot: &MarketKlineSnapshot) -> AppResult<()> {
        MarketIngestionService::ingest_kline(self, snapshot).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketKlineMongoWrite {
    symbol: ValidatedMarketSymbol,
    interval: String,
    open_time: DateTime<Utc>,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
    source: String,
    updated_at: DateTime<Utc>,
}

impl MarketKlineMongoWrite {
    /// 把领域 K 线转换为 Mongo 写模型：数值存十进制字符串，`source` 存 provider 名，`updated_at` 取观察时间。
    /// OHLCV 之所以按字符串落库而不是 BSON 浮点，是为了避免二进制浮点在长周期回放中累积精度误差。
    /// `updated_at` 直接取快照的观察时间而非写入时刻，这样 Mongo 侧的时序过滤才能与 Redis 的判定口径一致。
    /// 交易对和周期会重新校验，非法输入返回 `Validation` 错误；此步骤不访问 Mongo，也不创建索引。
    pub fn from_snapshot(snapshot: &MarketKlineSnapshot) -> AppResult<Self> {
        let symbol = ValidatedMarketSymbol::from_raw(snapshot.symbol())
            .map_err(|error| AppError::Validation(error.to_string()))?;
        KlineUpsertKey::new(snapshot.interval(), snapshot.open_time())
            .map_err(|error| AppError::Validation(error.to_string()))?;
        Ok(Self {
            symbol,
            interval: snapshot.interval().to_owned(),
            open_time: snapshot.open_time(),
            open: snapshot.open().to_string(),
            high: snapshot.high().to_string(),
            low: snapshot.low().to_string(),
            close: snapshot.close().to_string(),
            volume: snapshot.volume().to_string(),
            source: provider_name(snapshot.provider()).to_owned(),
            updated_at: snapshot.observed_at(),
        })
    }

    /// 生成该交易对专属的 Mongo 集合名，形如 `market_klines_BTCUSDT`，与恢复任务共用同一命名入口。
    /// 按交易对分集合意味着索引和查询都被限制在单个交易对内，但也要求集合名的推导规则永远保持稳定。
    pub fn collection_name(&self) -> String {
        kline_collection_name(&self.symbol)
    }

    /// 借出已通过领域校验的交易对值对象，供索引创建等需要「保证已校验」的接口使用。
    /// 返回值对象而非裸字符串，是为了在类型层面阻止未经规范化的名称被拼进集合名。
    pub fn symbol(&self) -> &ValidatedMarketSymbol {
        &self.symbol
    }

    /// 生成 Mongo K 线 upsert 条件，仅以 `interval + open_time` 命中交易对专属集合中的同一根蜡烛。
    /// 条件里不含交易对，因为集合本身已按交易对隔离；这两个字段正好对应集合上的唯一索引。
    /// 该条件不带任何时序限制，用于先读出同槽现有文档来判断新旧，而不是直接用于写入。
    pub fn upsert_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
        }
    }

    /// 生成仅更新既有且观察时间不晚于传入值的 Mongo 条件；同槽旧 owner 不能在 Redis CAS 之后倒退历史值。
    /// 首次插入由独立唯一键 insert 完成，避免带时序条件的 upsert 在竞争失败时触发重复键错误。
    pub fn fresh_existing_filter(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
            "$or": [
                { "updated_at": { "$exists": false } },
                { "updated_at": { "$lte": BsonDateTime::from_millis(self.updated_at.timestamp_millis()) } },
            ],
        }
    }

    /// 判断已存在同槽文档的 `updated_at` 是否严格晚于本次快照；缺失或非法时间按旧数据处理并允许修复。
    /// 采用严格大于而非大于等于，因此时间完全相同的重放会被放行去执行一次等值覆盖，结果保持幂等。
    /// 把缺失时间当作旧数据，是为了让历史遗留或早期写入的无时间戳文档能被新数据自然补齐。
    pub fn existing_is_newer(&self, document: &Document) -> bool {
        document
            .get_datetime("updated_at")
            .ok()
            .is_some_and(|value| value.timestamp_millis() > self.updated_at.timestamp_millis())
    }

    /// 构造首次插入文档；字段与 `$set` 合同一致，唯一索引冲突表示已有并发写者而非第二条历史记录。
    /// 必须与 [`Self::upsert_update`] 保持同一份字段清单，否则首写与后续覆盖会产生结构不一致的历史记录。
    /// OHLCV 以十进制字符串写入，两个时间字段转成 BSON 毫秒时间，`source` 记录该蜡烛的行情来源。
    pub fn insert_document(&self) -> Document {
        doc! {
            "interval": &self.interval,
            "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
            "open": &self.open,
            "high": &self.high,
            "low": &self.low,
            "close": &self.close,
            "volume": &self.volume,
            "source": &self.source,
            "updated_at": BsonDateTime::from_millis(self.updated_at.timestamp_millis()),
        }
    }

    /// 生成 Mongo `$set` 文档，覆盖同一周期和开盘时间的 OHLC、成交量、provider 与观察时间。
    /// 形成中的蜡烛会反复走这条路径，每次都是整段字段替换，不做逐字段的取大取小合并。
    /// `$set` 里同时重写了周期与开盘时间，虽然它们本就是匹配条件，但保留可让文档结构与首写插入完全一致。
    /// 该更新文档必须配合带时序条件的过滤器使用，单独使用会失去防倒退保护。
    pub fn upsert_update(&self) -> Document {
        doc! {
            "$set": {
                "interval": &self.interval,
                "open_time": BsonDateTime::from_millis(self.open_time.timestamp_millis()),
                "open": &self.open,
                "high": &self.high,
                "low": &self.low,
                "close": &self.close,
                "volume": &self.volume,
                "source": &self.source,
                "updated_at": BsonDateTime::from_millis(self.updated_at.timestamp_millis()),
            }
        }
    }
}

trait KlineCollectionFreshInsert {
    /// 仅在当前周期尚无行情时插入，并将并发唯一键冲突视为未接受。
    /// 返回 `false` 表示同槽已被别的写者抢先建立，属于正常竞争结果；真正的存储故障仍以错误形式抛出。
    async fn insert_if_absent(&self, write: &MarketKlineMongoWrite) -> AppResult<bool>;
}

impl KlineCollectionFreshInsert for mongodb::Collection<Document> {
    /// 直接向集合插入首写文档，并把重复键错误翻译成「未接受」而不是失败。
    /// 判定依据是错误文本中的 `E11000` 重复键标记，这是 Mongo 驱动对唯一索引冲突的稳定表述。
    /// 依赖错误文本匹配意味着换用其他存储或驱动改写措辞时需要同步调整，其余错误一律按 Mongo 故障上抛。
    async fn insert_if_absent(&self, write: &MarketKlineMongoWrite) -> AppResult<bool> {
        match self.insert_one(write.insert_document()).await {
            Ok(_) => Ok(true),
            Err(error) if error.to_string().contains("E11000") => Ok(false),
            Err(error) => Err(AppError::Mongo(error)),
        }
    }
}

/// 把缓存层错误翻译成应用错误，按成因区分责任方而不是笼统归为内部故障。
/// Redis 连接与命令错误原样保留为 `Redis`，便于监控识别为基础设施可用性问题；
/// 序列化失败归为 `Internal`，因为它只可能来自本服务的 DTO 定义缺陷；
/// DTO 构造错误归为 `Validation`，说明上游给出的交易对或周期不合法，重试同一份数据也不会成功。
fn market_cache_error(error: MarketCacheError) -> AppError {
    match error {
        MarketCacheError::Redis(error) => AppError::Redis(error),
        MarketCacheError::Json(error) => AppError::Internal(error.to_string()),
        MarketCacheError::Entry(error) => AppError::Validation(error.to_string()),
    }
}
