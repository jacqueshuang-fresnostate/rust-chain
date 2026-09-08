//! 默认行情配置读写；应用层持有交易对协调锁及事务，配置历史与审计必须原子提交。

use super::*;
use crate::modules::admin::presentation::{
    DefaultMarketReferencePairResponse, DefaultMarketRuntimeResponse,
};

/// 已保存的默认行情快照；配置 JSON 按不可变版本归档，不重置运行中的价格或随机状态。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct DefaultMarketRecord {
    pub(crate) pair_id: u64,
    pub(crate) enabled: bool,
    pub(crate) all_market_paused: bool,
    pub(crate) initial_price: Option<BigDecimal>,
    pub(crate) config_json: SqlxJson<Value>,
    pub(crate) version: u32,
    pub(crate) seed: String,
    pub(crate) updated_at: DateTime<Utc>,
}

/// 在调用方只读或写事务中加载配置，写用例须先锁交易对；缺失表示尚未配置而不是隐式创建。
pub(crate) async fn load_default_market_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<Option<DefaultMarketRecord>> {
    Ok(sqlx::query_as("SELECT pair_id, enabled, all_market_paused, initial_price, config_json, version, seed, updated_at FROM market_default_generators WHERE pair_id = ?")
        .bind(pair_id).fetch_optional(&mut **tx).await?)
}

#[derive(sqlx::FromRow)]
struct DefaultMarketRuntimeRecord {
    #[sqlx(flatten)]
    runtime: DefaultMarketRuntimeResponse,
    follow_json: Option<SqlxJson<Value>>,
}

/// 读取共享写入者的持久化快照；缺失时明确返回 none 来源，不访问缓存、资金或启动生成器。
pub(crate) async fn load_default_market_runtime_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<DefaultMarketRuntimeResponse> {
    let row = sqlx::query_as::<_, DefaultMarketRuntimeRecord>("SELECT generation, active_source, strategy_id, strategy_version, default_version, last_price, last_tick_at, error_message, JSON_EXTRACT(state_json, '$.follow.status') AS follow_json FROM market_pair_generation_runs WHERE pair_id = ?")
        .bind(pair_id).fetch_optional(&mut **tx).await?;
    let Some(mut row) = row else {
        return Ok(DefaultMarketRuntimeResponse {
            active_source: "none".to_owned(),
            ..Default::default()
        });
    };
    row.runtime.follow = row
        .follow_json
        .filter(|value| !value.0.is_null())
        .map(|value| serde_json::from_value(value.0))
        .transpose()
        .map_err(|_| AppError::Internal("默认行情跟随运行状态格式异常".to_owned()))?;
    if let Some(follow) = row.runtime.follow.as_mut() {
        let reference =
            load_default_market_reference_in_tx(tx, follow.reference_pair_id, false).await?;
        follow.reference_symbol = reference.map(|pair| pair.symbol).unwrap_or_default();
    }
    Ok(row.runtime)
}

/// 查找本交易对已归档的最新正价格；不借用其他符号、不用历史名义目标价，也不改写归档。
pub(crate) async fn load_default_market_bootstrap_in_tx(
    tx: &mut Transaction<'_, MySql>,
    symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    Ok(sqlx::query_scalar("SELECT price FROM market_price_ticks WHERE symbol = ? AND price > 0 AND observed_at <= UTC_TIMESTAMP(6) ORDER BY observed_at DESC, id DESC LIMIT 1")
        .bind(crate::modules::market::sanitize_symbol(symbol)).fetch_optional(&mut **tx).await?)
}

/// 原子覆盖当前配置并追加不可变版本；调用方先校验乐观版本且持有交易对锁，禁止在这里提交。
pub(crate) async fn save_default_market_in_tx(
    tx: &mut Transaction<'_, MySql>,
    record: &DefaultMarketRecord,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query("INSERT INTO market_default_generators (pair_id, enabled, all_market_paused, initial_price, config_json, version, seed, updated_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON DUPLICATE KEY UPDATE enabled = VALUES(enabled), all_market_paused = VALUES(all_market_paused), initial_price = VALUES(initial_price), config_json = VALUES(config_json), version = VALUES(version), updated_by = VALUES(updated_by)")
        .bind(record.pair_id).bind(record.enabled).bind(record.all_market_paused).bind(&record.initial_price)
        .bind(&record.config_json).bind(record.version).bind(&record.seed).bind(admin_id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO market_default_generator_versions (pair_id, version, initial_price, config_json, seed, created_by) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(record.pair_id).bind(record.version).bind(&record.initial_price).bind(&record.config_json)
        .bind(&record.seed).bind(admin_id).execute(&mut **tx).await?;
    Ok(())
}

/// 在协调锁排空既有推送后撤销写入者代际；只保留连续价格和状态，不撤单、不触碰用户资金。
pub(crate) async fn invalidate_default_market_runtime_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    sqlx::query("INSERT INTO market_pair_generation_runs (pair_id, generation) VALUES (?, 1) ON DUPLICATE KEY UPDATE generation = generation + 1, active_source = 'none', strategy_id = NULL, strategy_version = NULL, default_version = NULL, lease_owner = NULL, lease_expires_at = NULL, error_message = NULL")
        .bind(pair_id).execute(&mut **tx).await?;
    Ok(())
}

/// 按精确 ID 读取参考交易对元数据；写配置时持有共享锁避免验证与保存之间被停用或改来源。
/// 读取历史配置时不筛掉停用引用，使管理员看到实际目录状态；缺失返回 None，不按符号猜测替代项。
pub(crate) async fn load_default_market_reference_in_tx(
    tx: &mut Transaction<'_, MySql>,
    reference_pair_id: u64,
    lock: bool,
) -> AppResult<Option<DefaultMarketReferencePairResponse>> {
    let query = if lock {
        "SELECT id, symbol, status, market_type FROM trading_pairs WHERE id = ? FOR SHARE"
    } else {
        "SELECT id, symbol, status, market_type FROM trading_pairs WHERE id = ?"
    };
    Ok(sqlx::query_as(query)
        .bind(reference_pair_id)
        .fetch_optional(&mut **tx)
        .await?)
}

/// 历史预览一次性读取的有界外部证据；超上限时明确标记，不以不完整片段冒充完整回放。
pub(crate) struct DefaultMarketReferenceArchive {
    pub(crate) observations: Vec<crate::modules::market::synthetic_follow::FollowObservation>,
    pub(crate) truncated: bool,
}

/// 在一致事务内只读指定时间窗内已归档外部采样，保留同时间全部来源记录以检测冲突。
/// 上限两万条；超限后弃用截断证据并展示独立样本，所有未来时间和平台自生成来源均排除。
pub(crate) async fn load_default_market_reference_archive_in_tx(
    tx: &mut Transaction<'_, MySql>,
    reference_symbol: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> AppResult<DefaultMarketReferenceArchive> {
    let symbol = crate::modules::market::sanitize_symbol(reference_symbol);
    let records = sqlx::query_as::<_, (String, String, BigDecimal, DateTime<Utc>)>(
        "SELECT event_key,source,price,observed_at FROM market_price_ticks WHERE symbol=? AND source IN ('bitget','htx','coinbase') AND observed_at>=? AND observed_at<? AND observed_at<=UTC_TIMESTAMP(6) ORDER BY observed_at ASC,id ASC LIMIT 20001",
    )
    .bind(&symbol)
    .bind(start.naive_utc())
    .bind(end.naive_utc())
    .fetch_all(&mut **tx)
    .await?;
    let truncated = records.len() > 20_000;
    let observations = if truncated {
        Vec::new()
    } else {
        records
            .into_iter()
            .map(|(event_key, source, price, observed_at)| {
                crate::modules::market::synthetic_follow::FollowObservation {
                    event_key,
                    source,
                    price,
                    observed_at,
                    symbol: symbol.clone(),
                }
            })
            .collect()
    };
    Ok(DefaultMarketReferenceArchive {
        observations,
        truncated,
    })
}
