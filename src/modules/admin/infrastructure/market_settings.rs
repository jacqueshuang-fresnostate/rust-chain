//! 模拟行情设置中心的版本读取适配器。
//!
//! 本模块只执行 `strategy_versions`、`strategy_runs` 及其策略/交易对关联查询，不决定 seed、回滚状态或审计策略。
//! 所有写入仍由应用层持有事务，并复用现有策略、节点、运行检查点与版本写适配器。

use super::*;

/// 一个不可变策略版本的持久化读模型；`active_flag` 来自与运行行版本号的比较，不单独存储。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AdminMarketStrategyVersionRecord {
    pub(crate) version: i32,
    pub(crate) effective_time: DateTime<Utc>,
    pub(crate) config_json: SqlxJson<Value>,
    pub(crate) seed: String,
    pub(crate) created_by: Option<u64>,
    pub(crate) created_at: DateTime<Utc>,
    /// MySQL `CAST(... AS SIGNED)` 的协议类型是 BIGINT，必须用 i64 解码，避免线上出现整数列类型不匹配。
    pub(crate) active_flag: i64,
}

/// 读取策略当前 `active_version` 对应的版本行；缺少运行行或版本行均按未找到处理。
/// 查询不加锁，适合详情展示；配置更新与回滚必须改用事务内锁定版本函数。
pub(crate) async fn load_active_market_strategy_version_from_store(
    pool: &Pool<MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyVersionRecord> {
    sqlx::query_as::<_, AdminMarketStrategyVersionRecord>(
        r#"SELECT versions.version, versions.effective_time, versions.config_json,
                  versions.seed, versions.created_by, versions.created_at,
                  CAST(1 AS SIGNED) AS active_flag
           FROM strategy_runs runs
           INNER JOIN strategy_versions versions
             ON versions.strategy_id = runs.strategy_id
            AND versions.version = runs.active_version
           WHERE runs.strategy_id = ?"#,
    )
    .bind(strategy_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在调用方事务中锁定当前激活版本并返回完整版本行，供编辑继承 seed。
/// 调用方应先锁主策略，再调用本函数，保持“策略→版本/运行行”的固定锁序；本函数不提交事务。
pub(crate) async fn lock_active_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyVersionRecord> {
    sqlx::query_as::<_, AdminMarketStrategyVersionRecord>(
        r#"SELECT versions.version, versions.effective_time, versions.config_json,
                  versions.seed, versions.created_by, versions.created_at,
                  CAST(1 AS SIGNED) AS active_flag
           FROM strategy_runs runs
           INNER JOIN strategy_versions versions
             ON versions.strategy_id = runs.strategy_id
            AND versions.version = runs.active_version
           WHERE runs.strategy_id = ?
           FOR UPDATE"#,
    )
    .bind(strategy_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 分页读取单策略全部不可变版本并标出当前激活项；列表和总数使用同一策略 ID 条件。
/// 行按版本号倒序，不锁版本；并发新增版本时行与 total 可能短暂来自不同快照，刷新即可收敛。
pub(crate) async fn list_market_strategy_versions_from_store(
    pool: &Pool<MySql>,
    strategy_id: u64,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<AdminMarketStrategyVersionRecord>, i64)> {
    let rows = sqlx::query_as::<_, AdminMarketStrategyVersionRecord>(
        r#"SELECT versions.version, versions.effective_time, versions.config_json,
                  versions.seed, versions.created_by, versions.created_at,
                  CAST(versions.version = runs.active_version AS SIGNED) AS active_flag
           FROM strategy_versions versions
           INNER JOIN strategy_runs runs ON runs.strategy_id = versions.strategy_id
           WHERE versions.strategy_id = ?
           ORDER BY versions.version DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(strategy_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM strategy_versions WHERE strategy_id = ?",
    )
    .bind(strategy_id)
    .fetch_one(pool)
    .await?;
    Ok((rows, total))
}

/// 在已锁定主策略的事务中锁定指定历史版本，供复制回滚读取原始 JSON 与 seed。
/// 找不到版本返回未找到；函数不判断它是否为当前激活版本，也不创建新版本。
pub(crate) async fn lock_market_strategy_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    version: i32,
) -> AppResult<AdminMarketStrategyVersionRecord> {
    sqlx::query_as::<_, AdminMarketStrategyVersionRecord>(
        r#"SELECT versions.version, versions.effective_time, versions.config_json,
                  versions.seed, versions.created_by, versions.created_at,
                  CAST(versions.version = runs.active_version AS SIGNED) AS active_flag
           FROM strategy_versions versions
           INNER JOIN strategy_runs runs ON runs.strategy_id = versions.strategy_id
           WHERE versions.strategy_id = ? AND versions.version = ?
           FOR UPDATE"#,
    )
    .bind(strategy_id)
    .bind(version)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在回滚事务中按指定版本读取生成器所需的策略、交易对精度、seed 与 JSON 快照。
/// 主表列只作为旧版本缺字段时的兼容回落；调用方应先锁主策略和目标版本，函数不额外改变锁序。
pub(crate) async fn load_admin_synthetic_strategy_snapshot_version_in_tx(
    tx: &mut Transaction<'_, MySql>,
    strategy_id: u64,
    version: i32,
) -> AppResult<AdminSyntheticStrategySnapshot> {
    sqlx::query_as::<_, AdminSyntheticStrategySnapshot>(
        r#"SELECT pairs.symbol, pairs.price_precision, strategies.start_price,
                  strategies.target_price, strategies.start_time, strategies.end_time,
                  strategies.volatility, strategies.volume_min, strategies.volume_max,
                  versions.version AS config_version, versions.seed, versions.config_json
           FROM market_strategies strategies
           INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
           INNER JOIN strategy_versions versions ON versions.strategy_id = strategies.id
           WHERE strategies.id = ? AND versions.version = ?"#,
    )
    .bind(strategy_id)
    .bind(version)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}
