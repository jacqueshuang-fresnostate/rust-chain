//! 交易对常驻行情配置用例：协调锁先于行锁，保存不绕过启用开关，所有变更与审计共用事务。

use crate::{
    error::{AppError, AppResult},
    modules::{
        admin::{
            application::admin_mysql_pool,
            infrastructure::{
                AdminAuditLogEntry, DefaultMarketRecord, insert_admin_audit_log_entry_in_tx,
                invalidate_default_market_runtime_in_tx, load_admin_trading_pair_in_tx,
                load_default_market_bootstrap_in_tx, load_default_market_in_tx,
                load_default_market_reference_archive_in_tx, load_default_market_reference_in_tx,
                load_default_market_runtime_in_tx, lock_admin_trading_pair_in_tx,
                save_default_market_in_tx,
            },
            presentation::{
                AdminTradingPairResponse, DefaultMarketPreviewResponse,
                DefaultMarketReferencePairResponse, DefaultMarketResponse,
                DefaultMarketRuntimeResponse, MarketStrategyRecoverySampleResponse,
                PauseDefaultMarketRequest, PreviewDefaultMarketRequest, SaveDefaultMarketRequest,
            },
            service::{
                DefaultMarketFollowPreviewInput, next_default_market_version,
                replay_default_market_follow_preview, required_admin_audit_reason,
                validate_default_market_initial_price,
            },
        },
        market::{
            infrastructure::default_runtime::PairGenerationLock,
            synthetic_default::{DefaultMarketParameters, generate_default_1m},
        },
    },
};
use chrono::Utc;
use serde_json::json;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

/// 读取一致的配置与共享运行快照；未配置返回系统参数与 disabled，而不是创建记录或启动行情。
pub(crate) async fn get_admin_default_market(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<DefaultMarketResponse> {
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let pair = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    ensure_supported_pair(&pair)?;
    let saved = load_default_market_in_tx(&mut tx, pair_id).await?;
    let runtime = load_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    let response = default_market_response(&mut tx, pair, saved, runtime).await?;
    tx.commit().await?;
    Ok(response)
}

/// 保存完整配置并追加不可变版本；显式 enabled 才允许启动，无历史时须提供起价，审计失败整体回滚。
/// 先获取共享写入者锁以排空旧推送，再锁交易对；改参数不重置当前分钟状态，停用只撤销默认写入者。
pub(crate) async fn save_admin_default_market(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: SaveDefaultMarketRequest,
) -> AppResult<DefaultMarketResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let mut guard = acquire_generation_lock(&pool, pair_id).await?;
    let mut tx = pool.begin().await?;
    let pair = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    ensure_supported_pair(&pair)?;
    let saved = load_default_market_in_tx(&mut tx, pair_id).await?;
    let version = next_default_market_version(
        saved.as_ref().map_or(0, |v| v.version),
        request.expected_version,
    )?;
    request.config.validate(
        pair_precision(pair.price_precision)?,
        pair_precision(pair.qty_precision)?,
    )?;
    validate_default_market_reference(&mut tx, pair_id, &request.config, true).await?;
    validate_default_market_initial_price(request.initial_price.as_ref(), pair.price_precision)?;
    let runtime = load_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    if request.enabled {
        if pair.status != "active" {
            return Err(AppError::Validation(
                "请先启用交易对，再启用默认行情".to_owned(),
            ));
        }
        let start_price =
            resolve_start_price(&mut tx, &pair, &runtime, request.initial_price.as_ref()).await?;
        if request
            .config
            .price_min
            .as_ref()
            .is_some_and(|min| &start_price < min)
            || request
                .config
                .price_max
                .as_ref()
                .is_some_and(|max| &start_price > max)
        {
            return Err(AppError::Validation(
                "当前起价不在默认行情价格边界内，请调整边界后重试".to_owned(),
            ));
        }
    }
    let before = saved.as_ref().map(record_audit_json);
    let record = DefaultMarketRecord {
        pair_id,
        enabled: request.enabled,
        all_market_paused: saved.as_ref().is_some_and(|v| v.all_market_paused),
        initial_price: request.initial_price,
        config_json: SqlxJson(json!(request.config)),
        version,
        seed: saved
            .as_ref()
            .map_or_else(|| default_market_seed(pair_id), |v| v.seed.clone()),
        updated_at: Utc::now(),
    };
    guard.ensure_owned().await?;
    save_default_market_in_tx(&mut tx, &record, admin_id).await?;
    if !record.enabled && runtime.active_source == "default" {
        invalidate_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    }
    audit_default_market(
        &mut tx,
        admin_id,
        &record,
        before,
        reason,
        "default_market.save",
    )
    .await?;
    let saved = load_default_market_in_tx(&mut tx, pair_id).await?;
    let runtime = load_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    let response = default_market_response(&mut tx, pair, saved, runtime).await?;
    guard.ensure_owned().await?;
    tx.commit().await?;
    Ok(response)
}

/// 全部暂停独立于策略状态和交易对上架；可在尚无默认配置时直接急停，恢复仅清除总开关而不隐式启用。
/// 共享锁排空在途行情后才修改标志与代际；连续价格保留，事务审计失败时标志和代际一并回滚。
pub(crate) async fn pause_admin_default_market(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: PauseDefaultMarketRequest,
) -> AppResult<DefaultMarketResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let mut guard = acquire_generation_lock(&pool, pair_id).await?;
    let mut tx = pool.begin().await?;
    let pair = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    ensure_supported_pair(&pair)?;
    let saved = load_default_market_in_tx(&mut tx, pair_id).await?;
    let version = next_default_market_version(
        saved.as_ref().map_or(0, |v| v.version),
        request.expected_version,
    )?;
    let before = saved.as_ref().map(record_audit_json);
    let record = saved.map_or_else(
        || DefaultMarketRecord {
            pair_id,
            enabled: false,
            all_market_paused: request.all_market_paused,
            initial_price: None,
            config_json: SqlxJson(json!(DefaultMarketParameters::default())),
            version,
            seed: default_market_seed(pair_id),
            updated_at: Utc::now(),
        },
        |saved| DefaultMarketRecord {
            all_market_paused: request.all_market_paused,
            version,
            ..saved
        },
    );
    guard.ensure_owned().await?;
    save_default_market_in_tx(&mut tx, &record, admin_id).await?;
    if record.all_market_paused {
        invalidate_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    }
    audit_default_market(
        &mut tx,
        admin_id,
        &record,
        before,
        reason,
        "default_market.pause_all",
    )
    .await?;
    let saved = load_default_market_in_tx(&mut tx, pair_id).await?;
    let runtime = load_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    let response = default_market_response(&mut tx, pair, saved, runtime).await?;
    guard.ensure_owned().await?;
    tx.commit().await?;
    Ok(response)
}

async fn acquire_generation_lock(
    pool: &Pool<MySql>,
    pair_id: u64,
) -> AppResult<PairGenerationLock> {
    PairGenerationLock::acquire(pool, pair_id, 5)
        .await?
        .ok_or_else(|| AppError::Conflict("行情生成正在切换，请稍后重试".to_owned()))
}

fn ensure_supported_pair(pair: &AdminTradingPairResponse) -> AppResult<()> {
    if !matches!(pair.market_type.as_str(), "strategy" | "internal") {
        return Err(AppError::Validation(
            "默认行情只支持内部或策略交易对".to_owned(),
        ));
    }
    Ok(())
}

fn default_market_seed(pair_id: u64) -> String {
    format!("default-market:{pair_id}")
}

async fn resolve_start_price(
    tx: &mut Transaction<'_, MySql>,
    pair: &AdminTradingPairResponse,
    runtime: &DefaultMarketRuntimeResponse,
    initial_price: Option<&bigdecimal::BigDecimal>,
) -> AppResult<bigdecimal::BigDecimal> {
    // 与运行端一致：归档可能已提交而检查点尚未推进，不能让较旧检查点覆盖权威成交时点价格。
    let price = load_default_market_bootstrap_in_tx(tx, &pair.symbol)
        .await?
        .or_else(|| {
            runtime
                .last_tick_at
                .filter(|tick| *tick <= Utc::now())
                .and_then(|_| runtime.last_price.clone())
        })
        .or_else(|| initial_price.cloned())
        .ok_or_else(|| AppError::Validation("尚无历史行情，请先配置默认行情起始价".to_owned()))?;
    validate_default_market_initial_price(Some(&price), pair.price_precision)?;
    Ok(price)
}

fn record_audit_json(record: &DefaultMarketRecord) -> serde_json::Value {
    json!({"version":record.version,"enabled":record.enabled,"all_market_paused":record.all_market_paused,
        "initial_price":record.initial_price,"config":record.config_json.0,"seed":record.seed})
}

async fn audit_default_market(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    record: &DefaultMarketRecord,
    before: Option<serde_json::Value>,
    reason: String,
    action: &'static str,
) -> AppResult<()> {
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "default_market",
            target_id: record.pair_id,
            before_json: before,
            after_json: Some(record_audit_json(record)),
            reason: Some(reason),
        },
    )
    .await
}

async fn default_market_response(
    tx: &mut Transaction<'_, MySql>,
    pair: AdminTradingPairResponse,
    saved: Option<DefaultMarketRecord>,
    runtime: DefaultMarketRuntimeResponse,
) -> AppResult<DefaultMarketResponse> {
    let config: DefaultMarketParameters = saved
        .as_ref()
        .map(|record| serde_json::from_value(record.config_json.0.clone()))
        .transpose()
        .map_err(|_| AppError::Internal("默认行情配置格式异常".to_owned()))?
        .unwrap_or_default();
    let reference_pair = match config.follow.as_ref() {
        Some(follow) => {
            load_default_market_reference_in_tx(tx, follow.reference_pair_id, false).await?
        }
        None => None,
    };
    Ok(DefaultMarketResponse {
        pair_id: pair.id,
        symbol: pair.symbol,
        market_type: pair.market_type,
        configured: saved.is_some(),
        version: saved.as_ref().map_or(0, |r| r.version),
        enabled: saved.as_ref().is_some_and(|r| r.enabled),
        all_market_paused: saved.as_ref().is_some_and(|r| r.all_market_paused),
        initial_price: saved.as_ref().and_then(|r| r.initial_price.clone()),
        config,
        seed: saved
            .as_ref()
            .map_or_else(|| default_market_seed(pair.id), |r| r.seed.clone()),
        updated_at: saved.map(|r| r.updated_at),
        runtime,
        reference_pair,
    })
}

/// 独立模式预览未来六十根示例分钟线，跟随模式按秒回放过去六十个完整分钟的已归档证据。
/// 只读一致快照，不写配置、检查点、行情归档或资金；采样缺失或超限时明确标识独立降级。
/// 起价优先最新已提交价格，再使用显式初始价；版本过期或精度不符直接拒绝，所有样本前收等于后开。
pub(crate) async fn preview_admin_default_market(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
    request: PreviewDefaultMarketRequest,
) -> AppResult<DefaultMarketPreviewResponse> {
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let pair = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    ensure_supported_pair(&pair)?;
    let saved = load_default_market_in_tx(&mut tx, pair_id).await?;
    let version = next_default_market_version(
        saved.as_ref().map_or(0, |v| v.version),
        request.expected_version,
    )?;
    let price_precision = pair_precision(pair.price_precision)?;
    let qty_precision = pair_precision(pair.qty_precision)?;
    request.config.validate(price_precision, qty_precision)?;
    let reference =
        validate_default_market_reference(&mut tx, pair_id, &request.config, false).await?;
    validate_default_market_initial_price(request.initial_price.as_ref(), pair.price_precision)?;
    let runtime = load_default_market_runtime_in_tx(&mut tx, pair_id).await?;
    let start_price =
        resolve_start_price(&mut tx, &pair, &runtime, request.initial_price.as_ref()).await?;
    if request
        .config
        .price_min
        .as_ref()
        .is_some_and(|min| &start_price < min)
        || request
            .config
            .price_max
            .as_ref()
            .is_some_and(|max| &start_price > max)
    {
        return Err(AppError::Validation(
            "当前起价不在默认行情价格边界内，请调整边界后重试".to_owned(),
        ));
    }
    let seed = saved.map_or_else(|| default_market_seed(pair_id), |v| v.seed);
    let now = Utc::now();
    let start = chrono::DateTime::from_timestamp(now.timestamp().div_euclid(60) * 60, 0)
        .ok_or_else(|| AppError::Validation("预览时间超出支持范围".to_owned()))?;
    if let Some(reference) = reference {
        let follow = request
            .config
            .follow
            .as_ref()
            .expect("validated follow reference");
        let replay_start = start - chrono::Duration::minutes(60);
        let archive = load_default_market_reference_archive_in_tx(
            &mut tx,
            &reference.symbol,
            replay_start - chrono::Duration::seconds(i64::from(follow.stale_after_seconds)),
            start,
        )
        .await?;
        tx.commit().await?;
        let (samples, follow_preview) = replay_default_market_follow_preview(
            &DefaultMarketFollowPreviewInput {
                symbol: &pair.symbol,
                seed: &seed,
                version,
                price_precision,
                qty_precision,
                start: replay_start,
                start_price: &start_price,
                parameters: &request.config,
                reference_symbol: &reference.symbol,
            },
            &archive.observations,
            archive.truncated,
        )?;
        return Ok(DefaultMarketPreviewResponse {
            pair_id,
            version,
            seed,
            start_price,
            samples,
            follow_preview: Some(follow_preview),
        });
    }
    tx.commit().await?;
    let mut price = start_price.clone();
    let mut samples = Vec::with_capacity(60);
    for minute in 0..60 {
        let candle = generate_default_1m(
            &pair.symbol,
            &seed,
            version,
            price_precision,
            qty_precision,
            start + chrono::Duration::minutes(minute),
            &price,
            &start_price,
            &request.config,
        )?;
        price = candle.values.close.clone();
        samples.push(MarketStrategyRecoverySampleResponse {
            open_time: candle.open_time,
            open: candle.values.open,
            high: candle.values.high,
            low: candle.values.low,
            close: candle.values.close,
            volume: candle.values.volume,
        });
    }
    Ok(DefaultMarketPreviewResponse {
        pair_id,
        version,
        seed,
        start_price,
        samples,
        follow_preview: None,
    })
}

fn pair_precision(value: i32) -> AppResult<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value <= 18)
        .ok_or_else(|| AppError::Validation("交易对价格和数量精度必须在 0～18 之间".to_owned()))
}

async fn validate_default_market_reference(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    config: &DefaultMarketParameters,
    lock: bool,
) -> AppResult<Option<DefaultMarketReferencePairResponse>> {
    let Some(follow) = config.follow.as_ref() else {
        return Ok(None);
    };
    if follow.reference_pair_id == pair_id {
        return Err(AppError::Validation(
            "默认行情不能跟随自身交易对".to_owned(),
        ));
    }
    let reference = load_default_market_reference_in_tx(tx, follow.reference_pair_id, lock)
        .await?
        .ok_or_else(|| AppError::Validation("参考交易对不存在，请重新选择".to_owned()))?;
    if reference.status != "active" || reference.market_type != "external" {
        return Err(AppError::Validation(
            "只能跟随已启用的外部行情交易对".to_owned(),
        ));
    }
    Ok(Some(reference))
}
