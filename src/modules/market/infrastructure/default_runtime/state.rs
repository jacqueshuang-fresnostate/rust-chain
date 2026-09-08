//! 默认行情配置候选、交易对代际和检查点持久化；调用方先持有交易对命名锁再进入这些短事务。

use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool, types::Json};

use crate::{
    error::{AppError, AppResult},
    modules::market::sanitize_symbol,
};

/// 默认生成候选保留配置 JSON 原值；参数语义由纯生成器校验，数据库不擅自取整价格。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct DefaultPairRow {
    pub pair_id: u64,
    pub symbol: String,
    pub price_precision: i32,
    pub qty_precision: i32,
    pub version: u32,
    pub initial_price: Option<BigDecimal>,
    pub config_json: Json<Value>,
    pub seed: String,
}

/// 已持久化的交易对生成状态；state_json 保存固定分钟锚点，last_price 仅表示已接受观察价。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct PairGenerationRun {
    pub generation: u64,
    pub active_source: String,
    pub strategy_id: Option<u64>,
    pub strategy_version: Option<i32>,
    pub default_version: Option<u32>,
    pub lease_owner: Option<String>,
    pub last_price: Option<BigDecimal>,
    pub last_tick_at: Option<DateTime<Utc>>,
    pub state_json: Option<Json<Value>>,
}

/// 扫描已显式启用默认配置且没有当前人工排期的内部交易对；无策略记录也可命中。
/// 人工排期运行故障不被误当成没有排期，整个交易对暂停优先；按旧检查点优先避免固定 ID 饥饿。
pub(crate) async fn load_default_pairs(
    pool: &Pool<MySql>,
    now: DateTime<Utc>,
    limit: u32,
) -> AppResult<Vec<DefaultPairRow>> {
    Ok(sqlx::query_as::<_, DefaultPairRow>(
        r#"SELECT pairs.id AS pair_id, pairs.symbol, pairs.price_precision, pairs.qty_precision,
                  configs.version, configs.initial_price, configs.config_json, configs.seed
           FROM trading_pairs pairs
           INNER JOIN market_default_generators configs ON configs.pair_id = pairs.id
           LEFT JOIN market_pair_generation_runs runtime ON runtime.pair_id = pairs.id
           WHERE pairs.status = 'active' AND pairs.market_type IN ('strategy', 'internal')
             AND configs.enabled = TRUE AND configs.all_market_paused = FALSE
             AND NOT EXISTS (SELECT 1 FROM market_strategies strategies WHERE strategies.pair_id = pairs.id
                 AND strategies.status = 'active' AND strategies.start_time <= ? AND strategies.end_time > ?)
           ORDER BY runtime.last_tick_at, pairs.id LIMIT ?"#,
    ).bind(now.naive_utc()).bind(now.naive_utc()).bind(limit.clamp(1,100)).fetch_all(pool).await?)
}

/// 在已取得共同锁后重读单个默认配置，避免扫描至抢锁之间后台保存新版本导致新分钟使用旧参数。
pub(crate) async fn reload_default_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<DefaultPairRow> {
    Ok(sqlx::query_as::<_,DefaultPairRow>("SELECT pairs.id AS pair_id,pairs.symbol,pairs.price_precision,pairs.qty_precision,configs.version,configs.initial_price,configs.config_json,configs.seed FROM trading_pairs pairs INNER JOIN market_default_generators configs ON configs.pair_id=pairs.id WHERE pairs.id=?")
        .bind(pair_id).fetch_one(pool).await?)
}

/// 读取交易对检查点，不初始化或改状态；缺少记录表示此交易对尚未被新版生成器接管。
pub(crate) async fn load_pair_run(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<Option<PairGenerationRun>> {
    Ok(sqlx::query_as::<_, PairGenerationRun>(
        "SELECT generation, active_source, strategy_id, strategy_version, default_version, lease_owner, last_price, last_tick_at, state_json FROM market_pair_generation_runs WHERE pair_id = ?",
    ).bind(pair_id).fetch_optional(pool).await?)
}

/// 在交易对命名锁内建立来源代际和短租约；交易对→配置→运行行锁序与后台写入一致。
/// 缺失/暂停/排期变化直接冲突，不发布价格；同一来源重试保留代际，切换才增加代际并保留上一价格。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn activate_pair_source(
    pool: &Pool<MySql>,
    pair_id: u64,
    source: &str,
    strategy_id: Option<u64>,
    version: u32,
    owner: &str,
    now: DateTime<Utc>,
    state: Option<&Value>,
) -> AppResult<u64> {
    let mut tx = pool.begin().await?;
    let (status, market_type) = sqlx::query_as::<_, (String, String)>(
        "SELECT status, market_type FROM trading_pairs WHERE id = ? FOR UPDATE",
    )
    .bind(pair_id)
    .fetch_one(&mut *tx)
    .await?;
    let controls = sqlx::query_as::<_, (bool,bool)>(
        "SELECT enabled, all_market_paused FROM market_default_generators WHERE pair_id = ? FOR UPDATE",
    ).bind(pair_id).fetch_optional(&mut *tx).await?;
    if status != "active"
        || !matches!(market_type.as_str(), "strategy" | "internal")
        || controls.is_some_and(|c| c.1)
    {
        return Err(AppError::Conflict(
            "synthetic pair generation is paused or inactive".into(),
        ));
    }
    if source == "default" {
        if !controls.is_some_and(|c| c.0) {
            return Err(AppError::Conflict(
                "default market generator is disabled".into(),
            ));
        }
        let has_manual = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM market_strategies WHERE pair_id = ? AND status = 'active' AND start_time <= CURRENT_TIMESTAMP(6) AND end_time > CURRENT_TIMESTAMP(6)",
        ).bind(pair_id).fetch_one(&mut *tx).await?;
        if has_manual > 0 {
            return Err(AppError::Conflict(
                "manual market strategy has priority".into(),
            ));
        }
    } else if source == "strategy" {
        let current = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM market_strategies strategies INNER JOIN strategy_runs runs ON runs.strategy_id = strategies.id WHERE strategies.id = ? AND strategies.pair_id = ? AND strategies.status = 'active' AND strategies.start_time <= CURRENT_TIMESTAMP(6) AND strategies.end_time > CURRENT_TIMESTAMP(6) AND runs.active_version = ? AND runs.run_status IN ('live','running')",
        ).bind(strategy_id).bind(pair_id).bind(version).fetch_one(&mut *tx).await?;
        if current != 1 {
            return Err(AppError::Conflict(
                "manual market strategy is no longer current".into(),
            ));
        }
    } else {
        return Err(AppError::Validation(
            "invalid synthetic generation source".into(),
        ));
    }
    sqlx::query("INSERT IGNORE INTO market_pair_generation_runs (pair_id) VALUES (?)")
        .bind(pair_id)
        .execute(&mut *tx)
        .await?;
    let previous = sqlx::query_as::<_, (u64,String,Option<u64>,Option<i32>,Option<u32>)>(
        "SELECT generation, active_source, strategy_id, strategy_version, default_version FROM market_pair_generation_runs WHERE pair_id = ? FOR UPDATE",
    ).bind(pair_id).fetch_one(&mut *tx).await?;
    let source_version = if source == "default" {
        previous.4
    } else {
        previous.3.and_then(|v| u32::try_from(v).ok())
    };
    let same = previous.1 == source && previous.2 == strategy_id && source_version == Some(version);
    let generation = if same {
        previous.0
    } else {
        previous
            .0
            .checked_add(1)
            .ok_or_else(|| AppError::Validation("synthetic generation overflow".into()))?
    };
    sqlx::query(
        "UPDATE market_pair_generation_runs SET generation=?, active_source=?, strategy_id=?, strategy_version=?, default_version=?, lease_owner=?, lease_expires_at=?, state_json=COALESCE(?,state_json) WHERE pair_id=?",
    ).bind(generation).bind(source).bind(strategy_id)
        .bind((source=="strategy").then_some(version)).bind((source=="default").then_some(version))
        .bind(owner).bind((now+TimeDelta::seconds(60)).naive_utc()).bind(state.map(Json))
        .bind(pair_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(generation)
}

/// 从按事件时间排序的已归档价格续接，不读取客户端报价或虚构初价；未来事件不作为启动锚点。
pub(crate) async fn latest_archived_pair_price(
    pool: &Pool<MySql>,
    symbol: &str,
    now: DateTime<Utc>,
) -> AppResult<Option<BigDecimal>> {
    Ok(sqlx::query_scalar::<_, BigDecimal>(
        "SELECT price FROM market_price_ticks WHERE symbol = ? AND observed_at <= ? AND price > 0 ORDER BY observed_at DESC, id DESC LIMIT 1",
    ).bind(sanitize_symbol(symbol)).bind(now.naive_utc()).fetch_optional(pool).await?)
}

/// 成功完成全轮行情副作用后推进交易对检查点；过时代际不允许覆盖新来源的最后价格。
pub(crate) async fn checkpoint_pair(
    pool: &Pool<MySql>,
    pair_id: u64,
    generation: u64,
    owner: &str,
    price: &BigDecimal,
    observed_at: DateTime<Utc>,
) -> AppResult<()> {
    let result=sqlx::query("UPDATE market_pair_generation_runs SET last_price=?, last_tick_at=?, lease_expires_at=?, error_message=NULL WHERE pair_id=? AND generation=? AND lease_owner=? AND (last_tick_at IS NULL OR last_tick_at<=?)")
        .bind(price).bind(observed_at.naive_utc()).bind((observed_at+TimeDelta::seconds(60)).naive_utc())
        .bind(pair_id).bind(generation).bind(owner).bind(observed_at.naive_utc()).execute(pool).await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "synthetic pair checkpoint ownership changed".into(),
        ));
    }
    Ok(())
}

/// 记录单交易对失败供后台查询，不修改最后价格或伪造成功时刻；文本截断避免异常载荷无限增长。
pub(crate) async fn mark_pair_error(pool: &Pool<MySql>, pair_id: u64, error: &str) {
    let message: String = error.chars().take(1000).collect();
    let _ = sqlx::query("INSERT IGNORE INTO market_pair_generation_runs (pair_id) VALUES (?)")
        .bind(pair_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE market_pair_generation_runs SET error_message=? WHERE pair_id=?")
        .bind(message)
        .bind(pair_id)
        .execute(pool)
        .await;
}
