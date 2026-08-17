//! 后台模拟行情设置中心的预设、无副作用预览、版本历史与复制回滚用例。
//!
//! 预览只读取交易对目录并调用纯生成器；版本列表只读 MySQL。回滚由应用层持有单一事务，
//! 按“主策略→目标版本→关系节点→新版本→运行检查点→事件与审计”的顺序完成，任何一步失败整体回滚。

use super::*;
use crate::modules::{
    admin::{
        infrastructure::{
            AdminMarketStrategyUpdate, AdminMarketStrategyVersionRecord,
            insert_market_strategy_event_in_tx, insert_market_strategy_version_in_tx,
            list_market_strategy_nodes_in_tx, list_market_strategy_versions_from_store,
            load_active_market_strategy_version_from_store, load_admin_market_strategy_from_store,
            load_admin_synthetic_strategy_snapshot_version_in_tx, load_admin_trading_pair,
            lock_admin_market_strategy_in_tx, lock_market_strategy_version_in_tx,
            next_market_strategy_version_in_tx, replace_market_strategy_nodes_in_tx,
            update_admin_market_strategy_in_tx, update_market_strategy_run_checkpoint_in_tx,
        },
        presentation::{
            AdminMarketStrategyDetailResponse, MarketStrategyNodeRequest,
            MarketStrategyPresetsResponse, MarketStrategyPreviewResponse,
            MarketStrategyRecoverySampleResponse, MarketStrategyVersionResponse,
            MarketStrategyVersionsQuery, MarketStrategyVersionsResponse,
            PreviewMarketStrategyRequest, RestoreMarketStrategyVersionRequest,
        },
        service::{
            ValidatedMarketStrategyGenerator, market_strategy_generator_response_from_snapshot,
            market_strategy_presets, market_strategy_run_status, required_admin_audit_reason,
            resolve_new_market_strategy_seed, resolve_updated_market_strategy_seed,
            validate_create_market_strategy, validate_market_strategy_generator,
        },
    },
    market::{
        SyntheticMarketConfig, SyntheticMarketNode, synthetic_execution_mode_from_code,
        synthetic_target_type_from_code,
    },
};
use serde_json::Value;

const MARKET_STRATEGY_PREVIEW_DEFAULT_SAMPLES: u32 = 120;
const MARKET_STRATEGY_PREVIEW_MAX_SAMPLES: u32 = 240;

/// 返回后端权威场景预设目录；函数无数据库或运行时依赖，所有影响输出的参数都显式出现在响应中。
pub(crate) fn list_admin_market_strategy_presets() -> MarketStrategyPresetsResponse {
    market_strategy_presets()
}

/// 对一份完整策略草稿生成均匀采样的确定性 1m OHLCV，不写 MySQL、Mongo、Redis、WebSocket 或检查点。
/// 交易对必须已经启用且属于 internal/strategy；新建自动 seed 每次生成，编辑默认继承当前版本并使用下一版本号。
/// 固定 seed 可精确重放；编辑显式要求重生成时，响应 seed 仅绑定本次预览，正式提交会按命令再生成新 seed。
/// `sample_count` 允许 1～240；当策略总分钟数更少时返回全部分钟，首尾样本始终覆盖策略首根与末根。
pub(crate) async fn preview_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    request: PreviewMarketStrategyRequest,
) -> AppResult<MarketStrategyPreviewResponse> {
    validate_create_market_strategy(&request.strategy)?;
    let generator = validate_market_strategy_generator(&request.strategy.generator)?;
    let sample_count = request
        .sample_count
        .unwrap_or(MARKET_STRATEGY_PREVIEW_DEFAULT_SAMPLES);
    if !(1..=MARKET_STRATEGY_PREVIEW_MAX_SAMPLES).contains(&sample_count) {
        return Err(AppError::Validation(
            "预览采样数量必须在 1～240 之间".to_owned(),
        ));
    }
    let pool = admin_mysql_pool(pool)?;
    let pair = load_admin_trading_pair(&pool, request.strategy.pair_id).await?;
    if pair.status != "active" || !matches!(pair.market_type.as_str(), "internal" | "strategy") {
        return Err(AppError::Validation(
            "模拟行情只能预览已启用的内部或策略交易对".to_owned(),
        ));
    }
    let (seed, preview_version) = if let Some(strategy_id) = request.strategy_id {
        let strategy = load_admin_market_strategy_from_store(&pool, strategy_id).await?;
        if strategy.pair_id != request.strategy.pair_id {
            return Err(AppError::Validation("预览策略与交易对不匹配".to_owned()));
        }
        let active_version =
            load_active_market_strategy_version_from_store(&pool, strategy_id).await?;
        let preview_version = active_version
            .version
            .checked_add(1)
            .and_then(|version| u32::try_from(version).ok())
            .ok_or_else(|| AppError::Validation("策略版本号已超出支持范围".to_owned()))?;
        (
            resolve_updated_market_strategy_seed(&generator, &active_version.seed)?,
            preview_version,
        )
    } else {
        (resolve_new_market_strategy_seed(&generator), 1)
    };
    let config = preview_config(
        &request.strategy,
        &pair,
        &generator,
        seed.clone(),
        preview_version,
    )?;
    let total_minutes = (config.end_time - config.start_time).num_minutes();
    let one_minute_count = u64::try_from(total_minutes)
        .map_err(|_| AppError::Validation("策略时间范围过大".to_owned()))?;
    let actual_samples = sample_count.min(u32::try_from(one_minute_count).unwrap_or(u32::MAX));
    let open_times = preview_open_times(config.start_time, total_minutes, actual_samples);
    let samples = open_times
        .into_iter()
        .map(|open_time| {
            let candle = config
                .generate_1m(open_time)
                .map_err(|error| AppError::Validation(error.to_string()))?;
            Ok(MarketStrategyRecoverySampleResponse {
                open_time: candle.open_time,
                open: candle.values.open,
                high: candle.values.high,
                low: candle.values.low,
                close: candle.values.close,
                volume: candle.values.volume,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(MarketStrategyPreviewResponse {
        preview_seed: seed,
        preview_version,
        one_minute_count,
        sample_count: u32::try_from(samples.len()).unwrap_or(MARKET_STRATEGY_PREVIEW_MAX_SAMPLES),
        samples,
    })
}

/// 分页返回单策略不可变版本历史，并把旧快照缺失的高级参数映射为兼容默认值。
/// 读取前确认策略存在；配置 JSON 损坏会让请求整体失败，避免后台把脏版本展示成默认配置。
pub(crate) async fn list_admin_market_strategy_versions(
    pool: Option<Pool<MySql>>,
    strategy_id: u64,
    query: MarketStrategyVersionsQuery,
) -> AppResult<MarketStrategyVersionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_market_strategy_from_store(&pool, strategy_id).await?;
    let (records, total) = list_market_strategy_versions_from_store(
        &pool,
        strategy_id,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    let versions = records
        .into_iter()
        .map(version_response)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(MarketStrategyVersionsResponse { versions, total })
}

/// 将指定历史版本的原始 JSON 与 seed 复制为新递增版本，并同步主表、节点和运行检查点。
/// active 策略必须先暂停或停用，当前激活版本不能被重复“回滚”；审计原因非空，事件会记录来源与新版本号。
/// 回滚不改写旧版本和已生成历史 K 线，也不直接启动 worker；事务提交后运行组件按新的 active_version 自行观察。
pub(crate) async fn restore_admin_market_strategy_version(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    source_version: i32,
    request: RestoreMarketStrategyVersionRequest,
) -> AppResult<AdminMarketStrategyDetailResponse> {
    if source_version <= 0 {
        return Err(AppError::Validation("版本号必须大于零".to_owned()));
    }
    let reason = required_admin_audit_reason(Some(request.reason))?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    if before.status == "active" {
        return Err(AppError::Conflict(
            "启用中的行情策略必须先暂停或停用再回滚".to_owned(),
        ));
    }
    let source = lock_market_strategy_version_in_tx(&mut tx, strategy_id, source_version).await?;
    if source.active_flag != 0 {
        return Err(AppError::Conflict("所选版本已经是当前激活版本".to_owned()));
    }
    let snapshot =
        load_admin_synthetic_strategy_snapshot_version_in_tx(&mut tx, strategy_id, source_version)
            .await?;
    // 历史版本缺少 nodes 键时代表当时尚未启用节点能力，不能回退到当前关系表节点，
    // 否则“回滚旧版”会意外混入新版路径。实时/补偿读取当前激活旧快照时仍保留关系表兼容逻辑。
    let source_config = super::admin_synthetic_config(snapshot, Vec::new())?;
    let restored_nodes = strategy_node_requests(&source_config);
    let strategy_type = source
        .config_json
        .0
        .get("strategy_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&before.strategy_type)
        .to_owned();
    update_admin_market_strategy_in_tx(
        &mut tx,
        strategy_id,
        AdminMarketStrategyUpdate {
            strategy_type,
            start_price: source_config.start_price.clone(),
            target_price: source_config.target_price.clone(),
            start_time: source_config.start_time,
            end_time: source_config.end_time,
            volatility: source_config.volatility.clone(),
            volume_min: source_config.volume_min.clone(),
            volume_max: source_config.volume_max.clone(),
        },
    )
    .await?;
    replace_market_strategy_nodes_in_tx(&mut tx, strategy_id, &restored_nodes).await?;
    let new_version = next_market_strategy_version_in_tx(&mut tx, strategy_id).await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        new_version,
        source_config.start_time,
        source.config_json.0.clone(),
        source.seed.clone(),
        admin_id,
    )
    .await?;
    update_market_strategy_run_checkpoint_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&before.status),
        &source_config.start_price,
        source_config.start_time,
        new_version,
    )
    .await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    super::record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.version.restore",
        Some(&before),
        Some(&after),
        Some(reason),
    )
    .await?;
    insert_market_strategy_event_in_tx(
        &mut tx,
        strategy_id,
        "market_strategy.version.restored",
        json!({ "source_version": source_version, "new_version": new_version }),
    )
    .await?;
    let nodes = list_market_strategy_nodes_in_tx(&mut tx, strategy_id).await?;
    let generator =
        market_strategy_generator_response_from_snapshot(&source.config_json.0, source.seed)?;
    tx.commit().await?;
    Ok(AdminMarketStrategyDetailResponse {
        strategy: after,
        nodes,
        generator,
    })
}

/// 把已校验草稿、交易对精度、实际预览 seed、目标版本号与节点 DTO 组装成纯内存领域配置。
/// 枚举代码继续走共享白名单，构造失败不会写任何存储；创建使用版本 1，编辑使用当前激活版本加一。
fn preview_config(
    request: &CreateMarketStrategyRequest,
    pair: &AdminTradingPairResponse,
    generator: &ValidatedMarketStrategyGenerator,
    seed: String,
    version: u32,
) -> AppResult<SyntheticMarketConfig> {
    let nodes = request
        .nodes
        .iter()
        .map(|node| {
            Ok(SyntheticMarketNode {
                target_time: node.target_time,
                target_type: synthetic_target_type_from_code(&node.target_type)
                    .map_err(|error| AppError::Validation(error.to_string()))?,
                target_value: node.target_value.clone(),
                execution_mode: synthetic_execution_mode_from_code(&node.execution_mode)
                    .map_err(|error| AppError::Validation(error.to_string()))?,
                tolerance: node.tolerance.clone(),
                volatility: node.volatility.clone(),
                volume_min: node.volume_min.clone(),
                volume_max: node.volume_max.clone(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: pair.symbol.clone(),
        seed,
        version,
        price_precision: u32::try_from(pair.price_precision)
            .map_err(|_| AppError::Validation("交易对价格精度无效".to_owned()))?,
        start_time: request.start_time,
        end_time: request.end_time,
        start_price: request.start_price.clone(),
        target_price: request.target_price.clone(),
        volatility: request.volatility.clone(),
        volume_min: request.volume_min.clone(),
        volume_max: request.volume_max.clone(),
        generator: generator.settings.clone(),
        nodes,
    })
    .map_err(|error| AppError::Validation(error.to_string()))
}

/// 在完整策略半开分钟区间中均匀抽取指定数量的开盘时刻，样本超过一分钟时保证覆盖首尾有效槽位。
/// 整数除法只影响中间抽样密度，不会产生重复或越过结束边界；零长度和零样本请求返回空集合。
fn preview_open_times(
    start_time: DateTime<Utc>,
    total_minutes: i64,
    sample_count: u32,
) -> Vec<DateTime<Utc>> {
    if total_minutes <= 0 || sample_count == 0 {
        return Vec::new();
    }
    if sample_count == 1 {
        return vec![start_time];
    }
    let last_index = total_minutes - 1;
    (0..sample_count)
        .map(|index| {
            let minute_index = i64::from(index) * last_index / i64::from(sample_count - 1);
            start_time + Duration::minutes(minute_index)
        })
        .collect()
}

/// 把版本持久化行映射为后台历史读模型，并通过共享解析器补齐旧快照缺失的兼容生成参数。
/// `active_flag` 只用于展示当前激活项，原始 JSON 与 seed 不被改写，损坏快照直接阻止列表返回。
fn version_response(
    record: AdminMarketStrategyVersionRecord,
) -> AppResult<MarketStrategyVersionResponse> {
    let generator = market_strategy_generator_response_from_snapshot(
        &record.config_json.0,
        record.seed.clone(),
    )?;
    Ok(MarketStrategyVersionResponse {
        version: record.version,
        effective_time: record.effective_time,
        seed: record.seed,
        created_by: record.created_by,
        created_at: record.created_at,
        active: record.active_flag != 0,
        config_json: record.config_json.0,
        generator,
    })
}

/// 将历史领域配置中的有序节点还原成关系表写入 DTO，供复制回滚同步当前节点镜像。
/// 目标类型与执行模式使用稳定代码，十进制和可空成交量边界原样克隆，不重新解释相对目标值。
fn strategy_node_requests(config: &SyntheticMarketConfig) -> Vec<MarketStrategyNodeRequest> {
    config
        .nodes
        .iter()
        .map(|node| MarketStrategyNodeRequest {
            target_time: node.target_time,
            target_type: node.target_type.as_str().to_owned(),
            target_value: node.target_value.clone(),
            execution_mode: node.execution_mode.as_str().to_owned(),
            tolerance: node.tolerance.clone(),
            volatility: node.volatility.clone(),
            volume_min: node.volume_min.clone(),
            volume_max: node.volume_max.clone(),
        })
        .collect()
}
