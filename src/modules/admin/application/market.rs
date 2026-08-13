//! 交易对配置、模拟行情策略与手动 K 线补偿的应用用例层。
//!
//! 交易对与策略的写用例遵循同一套编排：开事务、锁定目标行、按锁后旧值校验、写入、回读、写策略事件与后台审计、提交，
//! 因此审计前后值必定来自同一次事务，且任一步失败都整体回滚。
//! 手动补偿是本文件最复杂的一条链路，拆成缺口检测、无写入预览和凭令牌执行三段：
//! 预览把策略版本与缺口摘要绑定进短时令牌，执行以令牌哈希为幂等键落任务表，并在同一次 HTTP 请求内
//! 走完 pending 到终态的全过程。补偿只写 Mongo 的历史 K 线与 MySQL 的任务状态，
//! 全程不获取 Redis 依赖，因此绝不会污染实时 ticker。

use super::*;

const MARKET_RECOVERY_RUNNING_TIMEOUT_MINUTES: i64 = 15;

/// 按规范化符号、状态和市场类型筛选交易对，并返回资产展示字段的分页结果和总数。
/// 非空筛选会执行与写入相同的枚举/格式校验，分页统一裁剪；读取不锁交易对，也不查询活动订单。
/// 复用写入侧的校验意味着非法筛选值会直接报校验错误而不是静默返回空列表，便于及早暴露前端传参问题。
/// 列表与总数由同一组谓词生成，因此翻页时的总数口径与当前筛选保持一致。
pub(crate) async fn list_admin_trading_pairs(
    pool: Option<Pool<MySql>>,
    query: AdminTradingPairQuery,
) -> AppResult<AdminTradingPairsResponse> {
    let symbol = query
        .symbol
        .and_then(optional_string)
        .map(|value| normalize_trading_pair_symbol(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_trading_pair_status(&value))
        .transpose()?;
    let market_type = query
        .market_type
        .and_then(optional_string)
        .map(|value| validate_trading_pair_market_type(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let (pairs, total) = list_admin_trading_pairs_from_store(
        &pool,
        AdminTradingPairListFilter {
            symbol,
            status,
            market_type,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminTradingPairsResponse { pairs, total })
}

/// 按交易对 ID 读取资产、符号、Logo、精度、最小订单额、状态和市场类型。
/// 查询不加锁；记录缺失返回未找到，数据库错误直接返回，不启动行情或撮合组件。
pub(crate) async fn get_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_trading_pair_from_store(&pool, pair_id).await
}

/// 创建现货交易对并返回含资产符号、精度、状态和市场类型的数据库快照。
/// 请求须满足异资产、合法符号、非负精度和正最小订单额；状态/市场类型缺省为 disabled/external，权限由调用方校验。
/// 事务按基准资产 ID 后计价资产 ID 的调用顺序确认两项资产可用，再插入交易对、回读并写审计；唯一键或 SQL 失败整体回滚。
/// 本用例无幂等键，且提交后不启动行情订阅或交易撮合。
pub(crate) async fn create_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateTradingPairRequest,
) -> AppResult<AdminTradingPairResponse> {
    validate_create_trading_pair_request(&request)?;
    let symbol = normalize_trading_pair_symbol(&request.symbol)?;
    let logo_url = validate_optional_image_url(request.logo_url, "trading pair logo_url")?;
    let status = request
        .status
        .as_deref()
        .map(validate_trading_pair_status)
        .transpose()?
        .unwrap_or_else(|| "disabled".to_owned());
    let market_type = request
        .market_type
        .as_deref()
        .map(validate_trading_pair_market_type)
        .transpose()?
        .unwrap_or_else(|| "external".to_owned());
    let pool = admin_mysql_pool(pool)?;

    // 创建交易对前锁定两个启用资产，避免资产状态变更与交易对创建竞态。
    let mut tx = pool.begin().await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.base_asset_id).await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.quote_asset_id).await?;
    let pair_id = insert_admin_trading_pair_in_tx(
        &mut tx,
        AdminTradingPairInsert {
            base_asset_id: request.base_asset_id,
            quote_asset_id: request.quote_asset_id,
            symbol,
            logo_url,
            price_precision: request.price_precision,
            qty_precision: request.qty_precision,
            min_order_value: request.min_order_value,
            status,
            market_type,
        },
    )
    .await?;
    let pair = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.create",
            target_type: "trading_pair",
            target_id: pair.id,
            before_json: None,
            after_json: Some(trading_pair_audit_json(&pair)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(pair)
}

/// 更新交易对 Logo、精度、最小订单额、状态和市场类型，并返回最终配置快照。
/// 调用方须提供审计原因和合法完整配置；基准/计价资产及符号在此用例中不可修改。
/// 事务先锁交易对，再覆盖配置、回读并写 before/after 审计；记录缺失或数据库失败整体回滚。
/// 相同配置重放仍新增审计，提交后不会自动重载行情或处理现有订单。
pub(crate) async fn update_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateTradingPairRequest,
) -> AppResult<AdminTradingPairResponse> {
    validate_update_trading_pair_request(&request)?;
    let status = validate_trading_pair_status(&request.status)?;
    let market_type = validate_trading_pair_market_type(&request.market_type)?;
    let logo_url = validate_optional_image_url(request.logo_url, "trading pair logo_url")?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定交易对旧值再更新，确保后台审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    update_admin_trading_pair_in_tx(
        &mut tx,
        pair_id,
        AdminTradingPairUpdate {
            logo_url,
            price_precision: request.price_precision,
            qty_precision: request.qty_precision,
            min_order_value: request.min_order_value,
            status,
            market_type,
        },
    )
    .await?;
    let after = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.config.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 单独切换交易对 active/disabled 状态，并返回状态更新后的完整交易对。
/// 请求须含受支持状态和审计原因；本函数不检查活动订单、持仓或行情源，权限由调用方保证。
/// 事务先锁交易对，再更新状态、回读并写 before/after 审计；缺失或 SQL 失败整体回滚。
/// 重复设置同一状态仍会留下审计，且不发布市场状态事件。
pub(crate) async fn update_admin_trading_pair_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateTradingPairStatusRequest,
) -> AppResult<AdminTradingPairResponse> {
    let status = validate_trading_pair_status(&request.status)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定交易对旧值再更新，确保后台审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    update_admin_trading_pair_status_in_tx(&mut tx, pair_id, &status).await?;
    let after = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.status.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 按交易对和状态筛选行情策略，并返回配置、运行检查点和恢复字段的分页结果与总数。
/// 状态筛选只去除空白，分页执行统一裁剪；读取不锁策略或版本，不改变 worker 的运行状态。
/// 与交易对列表不同，这里的状态不做枚举校验，因此传入未知状态得到的是空结果而非报错。
/// 响应中的运行检查点和当前价随 worker 实时推进，两次查询之间可能变化，不应作为一致性快照使用。
pub(crate) async fn list_admin_market_strategies(
    pool: Option<Pool<MySql>>,
    query: AdminMarketStrategyQuery,
) -> AppResult<AdminMarketStrategiesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let (strategies, total) = list_admin_market_strategies_from_store(
        &pool,
        AdminMarketStrategyListFilter {
            pair_id: query.pair_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminMarketStrategiesResponse { strategies, total })
}

/// 读取单策略的主表/运行快照与有序节点关系数据，组装为后台详情。
/// 两次查询均不加锁或写入；并发策略更新可使主读模型与节点短暂分属不同快照，调用方可刷新收敛。
pub(crate) async fn get_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    strategy_id: u64,
) -> AppResult<AdminMarketStrategyDetailResponse> {
    let pool = admin_mysql_pool(pool)?;
    let strategy = load_admin_market_strategy_from_store(&pool, strategy_id).await?;
    let nodes = list_market_strategy_nodes_from_store(&pool, strategy_id).await?;
    Ok(AdminMarketStrategyDetailResponse { strategy, nodes })
}

/// 在策略有效时段与最近已闭合 UTC 分钟之间检测 Mongo 权威 1m K 线缺口。
/// 可选查询边界会被策略边界收敛；函数只读 MySQL/Mongo，不生成蜡烛、不写检查点也不签发令牌。
/// 起点取请求值与策略开始时间的较大者，终点取请求值、策略结束时间和当前已闭合分钟三者的最小者，
/// 因此调用方无需自行裁剪即可安全传入任意范围。收敛后若区间为空则直接返回零缺口而不报错。
/// 缺口以半开区间形式合并输出，相邻缺失分钟归为一段，返回的总根数是各段之和。
/// Mongo 未配置时返回内部错误，因为补偿链路完全依赖它作为权威 K 线存储。
pub(crate) async fn detect_admin_market_strategy_gaps(
    pool: Option<Pool<MySql>>,
    mongo: Option<mongodb::Database>,
    strategy_id: u64,
    query: MarketStrategyGapQuery,
    now: DateTime<Utc>,
) -> AppResult<MarketStrategyGapsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let mongo = mongo.ok_or_else(|| {
        AppError::Internal("mongo database is required for market recovery".to_owned())
    })?;
    let snapshot = load_admin_synthetic_strategy_snapshot(&pool, strategy_id).await?;
    let closed_end = floor_admin_recovery_minute(now);
    let range_start = query
        .range_start
        .unwrap_or(snapshot.start_time)
        .max(snapshot.start_time);
    let range_end = query
        .range_end
        .unwrap_or(closed_end.min(snapshot.end_time))
        .min(snapshot.end_time)
        .min(closed_end);
    if range_end <= range_start {
        return Ok(MarketStrategyGapsResponse {
            strategy_id,
            config_version: snapshot.config_version,
            gaps: Vec::new(),
            total_1m_count: 0,
        });
    }
    validate_market_strategy_recovery_range(
        range_start,
        range_end,
        snapshot.start_time,
        snapshot.end_time,
        now,
    )?;
    let missing =
        missing_admin_market_strategy_open_times(&mongo, &snapshot.symbol, range_start, range_end)
            .await?;
    let (gaps, _) = summarize_market_strategy_gaps(missing);
    let total_1m_count = gaps.iter().map(|gap| gap.one_minute_count).sum();
    Ok(MarketStrategyGapsResponse {
        strategy_id,
        config_version: snapshot.config_version,
        gaps,
        total_1m_count,
    })
}

/// 使用最新策略版本 seed 生成指定缺口的确定性 OHLCV 预览，并签发短时 HMAC 确认令牌。
/// 预览会再确认范围内每个 1m 槽仍缺失；仅读 MySQL/Mongo，不写 K 线、Redis、运行检查点或任务表。
/// 与缺口检测不同，这里的范围必须由调用方精确给出且不会被自动收敛，任何越界都直接报校验错误。
/// 范围内根数还要受单次补偿上限约束，并且必须与实际缺失数完全相等，
/// 即所选区间不能夹杂任何已存在的 K 线，否则返回冲突，避免覆盖已有数据。
/// 令牌把策略编号、配置版本、区间和缺口摘要一并签入，有效期十分钟，因此预览后策略被改动或缺口被补上都会让执行失败。
/// 响应中的样本经过抽稀，只保留首尾各若干根，首价与末价则取自完整序列而非样本。
pub(crate) async fn preview_admin_market_strategy_recovery(
    pool: Option<Pool<MySql>>,
    mongo: Option<mongodb::Database>,
    strategy_id: u64,
    request: PreviewMarketStrategyRecoveryRequest,
    token_key: &[u8],
    now: DateTime<Utc>,
) -> AppResult<MarketStrategyRecoveryPreviewResponse> {
    let pool = admin_mysql_pool(pool)?;
    let mongo = mongo.ok_or_else(|| {
        AppError::Internal("mongo database is required for market recovery".to_owned())
    })?;
    let snapshot = load_admin_synthetic_strategy_snapshot(&pool, strategy_id).await?;
    let expected_count = validate_market_strategy_recovery_range(
        request.range_start,
        request.range_end,
        snapshot.start_time,
        snapshot.end_time,
        now,
    )?;
    if expected_count as usize > crate::workers::kline_recovery::MAX_MANUAL_RECOVERY_1M_CANDLES {
        return Err(AppError::Validation(format!(
            "manual recovery is limited to {} 1m candles per execution",
            crate::workers::kline_recovery::MAX_MANUAL_RECOVERY_1M_CANDLES
        )));
    }
    let missing = missing_admin_market_strategy_open_times(
        &mongo,
        &snapshot.symbol,
        request.range_start,
        request.range_end,
    )
    .await?;
    if missing.len() != expected_count as usize {
        return Err(AppError::Conflict(
            "recovery preview range must contain only missing 1m candles".to_owned(),
        ));
    }
    let (_, gap_digest) = summarize_market_strategy_gaps(missing.clone());
    let config = load_admin_recovery_config(&pool, strategy_id, snapshot.clone()).await?;
    let candles = generate_admin_recovery_samples(&config, &missing)?;
    let first_price = candles
        .first()
        .map(|sample| sample.open.clone())
        .ok_or_else(|| AppError::Conflict("recovery range has no gap".to_owned()))?;
    let last_price = candles
        .last()
        .map(|sample| sample.close.clone())
        .ok_or_else(|| AppError::Conflict("recovery range has no gap".to_owned()))?;
    let samples = limited_admin_recovery_samples(&candles);
    let (preview_token, expires_at) = issue_market_strategy_preview_token(
        token_key,
        MarketStrategyPreviewTokenInput {
            strategy_id,
            config_version: snapshot.config_version,
            range_start: request.range_start,
            range_end: request.range_end,
            one_minute_count: expected_count,
            gap_digest: &gap_digest,
        },
        now,
    )?;
    Ok(MarketStrategyRecoveryPreviewResponse {
        strategy_id,
        config_version: snapshot.config_version,
        range_start: request.range_start,
        range_end: request.range_end,
        one_minute_count: expected_count,
        aggregate_intervals: market_strategy_recovery_aggregate_intervals(),
        first_price,
        last_price,
        samples,
        preview_token,
        expires_at,
    })
}

/// 验证预览令牌、审计原因、版本与缺口摘要后，创建任务并在同一 HTTP 请求中走完 pending→running→completed/failed。
/// MySQL 短事务仅负责任务/事件/审计状态，Mongo 以 `interval + open_time` 幂等 upsert；路径不获取 Redis 依赖，因此绝不写 ticker。
/// 同令牌终态重放直接返回；pending 或超时 running 以任务原始完整范围继续写，新鲜 running 冲突，不会因部分 1m 已落库而被新缺口摘要卡死。
/// 首次执行才做验签与缺口摘要复核，续跑路径改为直接比对任务记录的配置版本，因为缺口此时已被自己部分填上。
/// 认领任务采用条件更新，超过十五分钟未完成的 running 会被判定为超时并允许重新认领，防止进程中断后任务永久卡住。
/// 终态写入使用独立短事务，无论成功或失败都会落任务状态、写策略事件并保留实际写入根数，最后统一回读最新任务返回。
pub(crate) async fn execute_admin_market_strategy_recovery(
    pool: Option<Pool<MySql>>,
    mongo: Option<mongodb::Database>,
    admin_id: u64,
    strategy_id: u64,
    request: ExecuteMarketStrategyRecoveryRequest,
    token_key: &[u8],
    now: DateTime<Utc>,
) -> AppResult<MarketStrategyRecoveryJobResponse> {
    let reason = required_admin_audit_reason(Some(request.reason))?;
    let pool = admin_mysql_pool(pool)?;
    let token_hash = hex::encode(Sha256::digest(request.preview_token.as_bytes()));
    let existing = load_market_strategy_recovery_job_by_token_hash(&pool, &token_hash).await?;
    if let Some(existing) = &existing {
        validate_existing_recovery_job(existing, strategy_id)?;
        if matches!(existing.status.as_str(), "completed" | "failed") {
            return Ok(existing.clone());
        }
    }
    let claims = if existing.is_none() {
        let claims = verify_market_strategy_preview_token(token_key, &request.preview_token, now)?;
        if claims.strategy_id != strategy_id {
            return Err(AppError::Validation(
                "preview_token does not belong to this strategy".to_owned(),
            ));
        }
        Some(claims)
    } else {
        None
    };
    let mongo = mongo.ok_or_else(|| {
        AppError::Internal("mongo database is required for market recovery".to_owned())
    })?;

    let (job, config, original_open_times) = match existing {
        Some(job) => {
            let snapshot = load_admin_synthetic_strategy_snapshot(&pool, strategy_id).await?;
            if snapshot.config_version != job.config_version {
                return Err(AppError::Conflict(
                    "market strategy changed before recovery job completed".to_owned(),
                ));
            }
            let config = load_admin_recovery_config(&pool, strategy_id, snapshot).await?;
            let open_times = recovery_open_times(job.range_start, job.range_end)?;
            (job, config, open_times)
        }
        None => {
            let claims = claims.expect("new recovery must have verified token claims");
            let snapshot = load_admin_synthetic_strategy_snapshot(&pool, strategy_id).await?;
            if snapshot.config_version != claims.config_version {
                return Err(AppError::Conflict(
                    "market strategy changed after recovery preview".to_owned(),
                ));
            }
            let open_times = recovery_open_times(claims.range_start, claims.range_end)?;
            if open_times.len() != claims.one_minute_count as usize {
                return Err(AppError::Validation(
                    "preview_token recovery count does not match its range".to_owned(),
                ));
            }
            let missing = missing_admin_market_strategy_open_times(
                &mongo,
                &snapshot.symbol,
                claims.range_start,
                claims.range_end,
            )
            .await?;
            let (_, current_digest) = summarize_market_strategy_gaps(missing.clone());
            if missing.len() != claims.one_minute_count as usize
                || current_digest != claims.gap_digest
            {
                return Err(AppError::Conflict(
                    "market recovery gap changed after preview".to_owned(),
                ));
            }
            let config = load_admin_recovery_config(&pool, strategy_id, snapshot).await?;
            let job = create_admin_market_strategy_recovery_job(
                &pool,
                admin_id,
                strategy_id,
                &claims,
                token_hash,
                reason,
            )
            .await?;
            validate_existing_recovery_job(&job, strategy_id)?;
            validate_recovery_job_matches_claims(&job, &claims)?;
            if matches!(job.status.as_str(), "completed" | "failed") {
                return Ok(job);
            }
            (job, config, open_times)
        }
    };

    let stale_before = now - Duration::minutes(MARKET_RECOVERY_RUNNING_TIMEOUT_MINUTES);
    match claim_market_strategy_recovery_job(&pool, job.id, now, stale_before).await? {
        AdminMarketStrategyRecoveryJobClaim::AlreadyFinished => {
            return load_market_strategy_recovery_job_from_store(&pool, job.id).await;
        }
        AdminMarketStrategyRecoveryJobClaim::Claimed => {}
    }

    let execution = crate::workers::kline_recovery::execute_manual_synthetic_recovery(
        &mongo,
        &config,
        &original_open_times,
        now,
    )
    .await;
    let completed_at = now;
    let mut terminal_tx = pool.begin().await?;
    match execution {
        Ok(counts) => {
            complete_market_strategy_recovery_job_in_tx(
                &mut terminal_tx,
                job.id,
                counts.actual_1m_count,
                counts.actual_aggregate_count,
                completed_at,
            )
            .await?;
            insert_market_strategy_event_in_tx(
                &mut terminal_tx,
                strategy_id,
                "market_strategy.kline_recovery.completed",
                json!({
                    "job_id": job.id,
                    "config_version": job.config_version,
                    "actual_1m_count": counts.actual_1m_count,
                    "actual_aggregate_count": counts.actual_aggregate_count,
                    "skipped_aggregate_count": counts.skipped_aggregate_count,
                    "completed_at": completed_at.timestamp_millis(),
                }),
            )
            .await?;
        }
        Err(error) => {
            let counts = error.counts();
            fail_market_strategy_recovery_job_in_tx(
                &mut terminal_tx,
                job.id,
                counts.actual_1m_count,
                counts.actual_aggregate_count,
                &error.to_string(),
                completed_at,
            )
            .await?;
            insert_market_strategy_event_in_tx(
                &mut terminal_tx,
                strategy_id,
                "market_strategy.kline_recovery.failed",
                json!({
                    "job_id": job.id,
                    "config_version": job.config_version,
                    "actual_1m_count": counts.actual_1m_count,
                    "actual_aggregate_count": counts.actual_aggregate_count,
                    "skipped_aggregate_count": counts.skipped_aggregate_count,
                    "error": error.to_string(),
                    "completed_at": completed_at.timestamp_millis(),
                }),
            )
            .await?;
        }
    }
    terminal_tx.commit().await?;
    load_market_strategy_recovery_job_from_store(&pool, job.id).await
}

/// 断言按令牌哈希查到的补偿任务确实属于当前路径上的策略，且状态落在四个已知取值之内。
/// 策略不匹配报校验错误，因为这通常是把别的策略的令牌拿来复用；状态越界报内部错误，属于数据异常。
/// 该检查既用于复用既有任务的分支，也用于新建任务后的自检，防止并发回读拿到不相干记录。
fn validate_existing_recovery_job(
    job: &MarketStrategyRecoveryJobResponse,
    strategy_id: u64,
) -> AppResult<()> {
    if job.strategy_id != strategy_id {
        return Err(AppError::Validation(
            "preview_token does not belong to this strategy".to_owned(),
        ));
    }
    if !matches!(
        job.status.as_str(),
        "pending" | "running" | "completed" | "failed"
    ) {
        return Err(AppError::Internal(
            "market recovery job has an invalid status".to_owned(),
        ));
    }
    Ok(())
}

/// 断言刚落库的补偿任务与预览令牌声明在版本、区间和预期根数四项上完全一致。
/// 任何不一致都归为内部错误而非校验错误，因为二者本应由同一次插入写入，出现分歧说明落库逻辑或并发处理有缺陷。
/// 这一步是执行前的最后一道自检，通过后才允许真正向 Mongo 写入补偿 K 线。
fn validate_recovery_job_matches_claims(
    job: &MarketStrategyRecoveryJobResponse,
    claims: &MarketStrategyPreviewTokenClaims,
) -> AppResult<()> {
    if job.config_version != claims.config_version
        || job.range_start != claims.range_start
        || job.range_end != claims.range_end
        || job.expected_1m_count != claims.one_minute_count
    {
        return Err(AppError::Internal(
            "stored market recovery job does not match preview token".to_owned(),
        ));
    }
    Ok(())
}

/// 在独立短事务内以令牌哈希为唯一键落一条 pending 补偿任务，并同步写策略事件与后台审计。
/// 事务先锁定策略行，确保任务创建期间策略配置不被并发改动；随后插入任务、回读、写事件、写审计后提交。
/// 唯一键冲突不是失败而是并发信号：此时回滚并按令牌哈希回读既有任务返回，从而让同一令牌的并发执行收敛到同一条任务。
/// 回读仍为空说明另一方也已回滚，此时返回冲突让调用方重试。
/// 本函数只负责建任务，真正的 K 线写入由调用方在认领任务后执行，因此提交成功不代表补偿已完成。
async fn create_admin_market_strategy_recovery_job(
    pool: &Pool<MySql>,
    admin_id: u64,
    strategy_id: u64,
    claims: &MarketStrategyPreviewTokenClaims,
    token_hash: String,
    reason: String,
) -> AppResult<MarketStrategyRecoveryJobResponse> {
    let mut tx = pool.begin().await?;
    let strategy = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    let lookup_token_hash = token_hash.clone();
    let job_id = match insert_market_strategy_recovery_job_in_tx(
        &mut tx,
        AdminMarketStrategyRecoveryJobInsert {
            strategy_id,
            requested_by: admin_id,
            config_version: claims.config_version,
            range_start: claims.range_start,
            range_end: claims.range_end,
            preview_token_hash: token_hash,
            reason: reason.clone(),
            expected_1m_count: claims.one_minute_count,
        },
    )
    .await
    {
        Ok(job_id) => job_id,
        Err(AppError::Conflict(_)) => {
            tx.rollback().await?;
            return load_market_strategy_recovery_job_by_token_hash(pool, &lookup_token_hash)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict("preview_token recovery was concurrently created".to_owned())
                });
        }
        Err(error) => return Err(error),
    };
    let job = load_market_strategy_recovery_job_in_tx(&mut tx, job_id).await?;
    insert_market_strategy_event_in_tx(
        &mut tx,
        strategy_id,
        "market_strategy.kline_recovery.requested",
        json!({
            "job_id": job.id,
            "config_version": job.config_version,
            "range_start": job.range_start.timestamp_millis(),
            "range_end": job.range_end.timestamp_millis(),
            "expected_1m_count": job.expected_1m_count,
        }),
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_strategy.kline_recovery.execute",
            target_type: "market_strategy",
            target_id: strategy_id,
            before_json: Some(market_strategy_audit_json(&strategy)),
            after_json: Some(json!({ "recovery_job_id": job.id, "status": job.status })),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(job)
}

/// 把半开区间展开成逐分钟的开盘时刻序列，作为补偿实际要写入的槽位清单。
/// 分钟数为零或超过单次补偿上限都报校验错误，负数区间在转换为无符号长度时同样被拦下。
/// 展开结果始终是任务记录的原始完整区间，即便其中部分槽位已被并发补上也照常包含，
/// 因为下游写入按开盘时间幂等，重复写不会造成重复数据，反而能避免续跑时因缺口变小而卡死。
fn recovery_open_times(
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> AppResult<Vec<DateTime<Utc>>> {
    let count = (range_end - range_start).num_minutes();
    let count = usize::try_from(count).map_err(|_| {
        AppError::Validation("market recovery range has an invalid minute count".to_owned())
    })?;
    if count == 0 || count > crate::workers::kline_recovery::MAX_MANUAL_RECOVERY_1M_CANDLES {
        return Err(AppError::Validation(format!(
            "manual recovery is limited to {} 1m candles per execution",
            crate::workers::kline_recovery::MAX_MANUAL_RECOVERY_1M_CANDLES
        )));
    }
    Ok((0..count)
        .map(|offset| range_start + Duration::minutes(offset as i64))
        .collect())
}

/// 按单策略与可选状态返回补偿任务历史，分页限制与其他后台列表一致。
/// 为区分空列表与不存在策略，查询前先读策略；全程只读 MySQL，不锁定或推进任务。
/// 状态筛选会走枚举校验，未知状态直接报校验错误而不是返回空列表，避免筛错条件被误读成「没有任务」。
/// 处于 running 的任务可能实际已超时，本查询如实返回其存储状态而不做超时判定，重新认领只发生在执行路径上。
pub(crate) async fn list_admin_market_strategy_recovery_jobs(
    pool: Option<Pool<MySql>>,
    strategy_id: u64,
    query: MarketStrategyRecoveryJobsQuery,
) -> AppResult<MarketStrategyRecoveryJobsResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_market_strategy_from_store(&pool, strategy_id).await?;
    let status = query
        .status
        .as_deref()
        .map(validate_market_strategy_recovery_job_status)
        .transpose()?;
    let (jobs, total) = list_market_strategy_recovery_jobs_from_store(
        &pool,
        AdminMarketStrategyRecoveryJobListFilter {
            strategy_id,
            status,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(MarketStrategyRecoveryJobsResponse { jobs, total })
}

/// 求出半开区间内 Mongo 尚不存在权威 1m K 线的全部开盘时刻。
/// 先一次性把区间内已存在的开盘时间读进哈希集合，再逐分钟推进比对，因此只发起一次查询而非逐根探测。
/// 返回顺序天然按时间升序，可直接交给缺口合并使用；区间为空时返回空列表而不报错。
async fn missing_admin_market_strategy_open_times(
    mongo: &mongodb::Database,
    symbol: &str,
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> AppResult<Vec<DateTime<Utc>>> {
    let existing = list_existing_one_minute_open_times(mongo, symbol, range_start, range_end)
        .await?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut missing = Vec::new();
    let mut open_time = range_start;
    while open_time < range_end {
        if !existing.contains(&open_time) {
            missing.push(open_time);
        }
        open_time += Duration::minutes(1);
    }
    Ok(missing)
}

/// 版本 JSON 显式包含 `nodes` 时以快照为唯一权威（包括有意保存的空数组）；
/// 只有旧版快照缺少该 key 时才读取关系表，与实时 worker 的兼容规则保持一致。
async fn load_admin_recovery_config(
    pool: &Pool<MySql>,
    strategy_id: u64,
    snapshot: AdminSyntheticStrategySnapshot,
) -> AppResult<crate::modules::market::synthetic::SyntheticMarketConfig> {
    let relation_nodes = if snapshot.config_json.0.get("nodes").is_none() {
        list_market_strategy_nodes_from_store(pool, strategy_id).await?
    } else {
        Vec::new()
    };
    admin_synthetic_config(snapshot, relation_nodes)
}

/// 把策略版本快照与可选的关系表节点合成为确定性行情生成器所需的完整配置。
/// 节点来源二选一：版本 JSON 里存在 nodes 键时以其为唯一权威，哪怕是有意保存的空数组；
/// 只有旧版快照完全没有该键时才回落到传入的关系表节点。nodes 存在但不是数组则判为校验错误。
/// 主配置字段逐项优先取版本 JSON 中的值，缺失时才回落到快照上的列值，
/// 这样历史版本即便与当前策略主表已经不一致，也能按当时的参数复现出相同的 K 线。
/// 版本号与价格精度需转换为无符号整数，越界归为内部错误；最终由生成器构造函数做一次整体自洽校验。
fn admin_synthetic_config(
    snapshot: AdminSyntheticStrategySnapshot,
    relation_nodes: Vec<AdminMarketStrategyNodeResponse>,
) -> AppResult<crate::modules::market::synthetic::SyntheticMarketConfig> {
    use crate::modules::market::synthetic::{SyntheticMarketConfig, SyntheticMarketNode};

    let nodes = match snapshot.config_json.0.get("nodes") {
        Some(serde_json::Value::Array(nodes)) => nodes
            .iter()
            .map(|node| {
                let target_type = parse_admin_recovery_target_type(&required_recovery_string(
                    node,
                    "target_type",
                )?)?;
                let execution_mode = parse_admin_recovery_execution_mode(
                    &required_recovery_string(node, "execution_mode")?,
                )?;
                Ok(SyntheticMarketNode {
                    target_time: required_recovery_time(node, "target_time")?,
                    target_type,
                    target_value: required_recovery_decimal(node, "target_value")?,
                    execution_mode,
                    tolerance: required_recovery_decimal(node, "tolerance")?,
                    volatility: required_recovery_decimal(node, "volatility")?,
                    volume_min: optional_recovery_decimal(node, "volume_min")?,
                    volume_max: optional_recovery_decimal(node, "volume_max")?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?,
        Some(_) => {
            return Err(AppError::Validation(
                "stored market strategy version nodes must be an array".to_owned(),
            ));
        }
        None => relation_nodes
            .into_iter()
            .map(|node| {
                Ok(SyntheticMarketNode {
                    target_time: node.target_time,
                    target_type: parse_admin_recovery_target_type(&node.target_type)?,
                    target_value: node.target_value,
                    execution_mode: parse_admin_recovery_execution_mode(&node.execution_mode)?,
                    tolerance: node.tolerance,
                    volatility: node.volatility,
                    volume_min: node.volume_min,
                    volume_max: node.volume_max,
                })
            })
            .collect::<AppResult<Vec<_>>>()?,
    };
    let version = u32::try_from(snapshot.config_version)
        .map_err(|_| AppError::Internal("market strategy version is invalid".to_owned()))?;
    let price_precision = u32::try_from(snapshot.price_precision)
        .map_err(|_| AppError::Internal("market pair price precision is invalid".to_owned()))?;
    SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: snapshot.symbol,
        seed: snapshot.seed,
        version,
        price_precision,
        start_time: recovery_config_time(
            &snapshot.config_json.0,
            "start_time",
            snapshot.start_time,
        )?,
        end_time: recovery_config_time(&snapshot.config_json.0, "end_time", snapshot.end_time)?,
        start_price: recovery_config_decimal(
            &snapshot.config_json.0,
            "start_price",
            snapshot.start_price,
        )?,
        target_price: recovery_config_decimal(
            &snapshot.config_json.0,
            "target_price",
            snapshot.target_price,
        )?,
        volatility: recovery_config_decimal(
            &snapshot.config_json.0,
            "volatility",
            snapshot.volatility,
        )?,
        volume_min: recovery_config_decimal(
            &snapshot.config_json.0,
            "volume_min",
            snapshot.volume_min,
        )?,
        volume_max: recovery_config_decimal(
            &snapshot.config_json.0,
            "volume_max",
            snapshot.volume_max,
        )?,
        nodes,
    })
    .map_err(|error| AppError::Validation(error.to_string()))
}

/// 把存量版本快照里的节点目标类型字符串还原为生成器枚举。
/// 三种取值分别对应绝对价、相对起始价百分比和相对前一节点百分比；未知值判为校验错误，
/// 提示是已落库数据不合法而非本次请求有误，这类脏数据会让该策略无法执行补偿。
fn parse_admin_recovery_target_type(
    value: &str,
) -> AppResult<crate::modules::market::synthetic::SyntheticTargetType> {
    use crate::modules::market::synthetic::SyntheticTargetType;

    match value {
        "absolute_price" => Ok(SyntheticTargetType::AbsolutePrice),
        "percent_from_start" => Ok(SyntheticTargetType::PercentFromStart),
        "percent_from_previous" => Ok(SyntheticTargetType::PercentFromPrevious),
        _ => Err(AppError::Validation(
            "stored market strategy version node target_type is invalid".to_owned(),
        )),
    }
}

/// 把存量版本快照里的节点执行模式字符串还原为生成器枚举。
/// hard 表示必须精确命中目标价，soft 表示允许在容差内逼近，range 表示只需落在区间内；
/// 与目标类型一样，未知值视为已落库数据不合法并返回校验错误。
fn parse_admin_recovery_execution_mode(
    value: &str,
) -> AppResult<crate::modules::market::synthetic::SyntheticExecutionMode> {
    use crate::modules::market::synthetic::SyntheticExecutionMode;

    match value {
        "hard" => Ok(SyntheticExecutionMode::Hard),
        "soft" => Ok(SyntheticExecutionMode::Soft),
        "range" => Ok(SyntheticExecutionMode::Range),
        _ => Err(AppError::Validation(
            "stored market strategy version node execution_mode is invalid".to_owned(),
        )),
    }
}

/// 从版本配置 JSON 中读取时间字段，键不存在时回落到调用方给出的快照列值。
/// 键存在但值非法仍会报错而不是悄悄回落，这样能区分「历史版本没记这一项」和「记了但记错了」。
fn recovery_config_time(
    config: &serde_json::Value,
    key: &str,
    fallback: DateTime<Utc>,
) -> AppResult<DateTime<Utc>> {
    config
        .get(key)
        .map_or(Ok(fallback), |value| recovery_time(value, key))
}

/// 从节点 JSON 中读取必填时间字段，键缺失即报错而没有任何回落值。
/// 节点时间没有可用的兜底来源，缺失会让整条价格路径失去锚点，因此必须直接失败。
fn required_recovery_time(value: &serde_json::Value, key: &str) -> AppResult<DateTime<Utc>> {
    value
        .get(key)
        .ok_or_else(|| AppError::Validation(format!("stored strategy node {key} is required")))
        .and_then(|value| recovery_time(value, key))
}

/// 解析版本快照中的时间值，兼容毫秒时间戳与 RFC3339 字符串两种历史写法。
/// 数字按毫秒时间戳解释，越界报「超出范围」；字符串按 RFC3339 解析后统一换算到 UTC，格式错误报「无效」。
/// 既非数字也非字符串则报明确的类型提示，帮助定位是哪个版本写入了非预期结构。
fn recovery_time(value: &serde_json::Value, key: &str) -> AppResult<DateTime<Utc>> {
    if let Some(millis) = value.as_i64() {
        return DateTime::from_timestamp_millis(millis).ok_or_else(|| {
            AppError::Validation(format!("stored strategy version {key} is out of range"))
        });
    }
    if let Some(raw) = value.as_str() {
        return DateTime::parse_from_rfc3339(raw)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| {
                AppError::Validation(format!("stored strategy version {key} is invalid"))
            });
    }
    Err(AppError::Validation(format!(
        "stored strategy version {key} must be milliseconds or RFC3339"
    )))
}

/// 从版本配置 JSON 中读取十进制字段，键不存在时回落到快照列值。
/// 与时间字段同构：只有键完全缺失才回落，键存在却无法解析为十进制时照常报错。
fn recovery_config_decimal(
    config: &serde_json::Value,
    key: &str,
    fallback: BigDecimal,
) -> AppResult<BigDecimal> {
    config
        .get(key)
        .map_or(Ok(fallback), |value| recovery_decimal(value, key))
}

/// 从节点 JSON 中读取必填十进制字段，用于目标值、容差和波动率这类不可缺省项。
/// 键缺失直接报必填错误；与可选版本的区别在于这里把 JSON null 之外的缺失也一律视为错误。
fn required_recovery_decimal(value: &serde_json::Value, key: &str) -> AppResult<BigDecimal> {
    value
        .get(key)
        .ok_or_else(|| AppError::Validation(format!("stored strategy node {key} is required")))
        .and_then(|value| recovery_decimal(value, key))
}

/// 从节点 JSON 中读取可选十进制字段，用于成交量上下界这类允许不配置的项。
/// 键缺失与显式 JSON null 同样返回 None，二者语义等价，都表示该节点不覆盖策略级的成交量设置。
/// 值存在但无法解析为十进制时仍报错，避免把脏数据当成未配置处理。
fn optional_recovery_decimal(
    value: &serde_json::Value,
    key: &str,
) -> AppResult<Option<BigDecimal>> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => recovery_decimal(value, key).map(Some),
    }
}

/// 把版本快照中的十进制值解析为高精度小数，兼容 JSON 字符串与 JSON 数字两种写法。
/// 字符串取其原文，其他类型退回到 JSON 字面量文本再解析，因此数字形态也能被处理。
/// 全程走十进制字符串解析而非浮点转换，从而避免价格与成交量在往返过程中产生精度漂移。
fn recovery_decimal(value: &serde_json::Value, key: &str) -> AppResult<BigDecimal> {
    let raw = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    raw.parse::<BigDecimal>().map_err(|_| {
        AppError::Validation(format!("stored strategy version {key} must be a decimal"))
    })
}

/// 从节点 JSON 中读取必填字符串字段，用于目标类型与执行模式两个枚举代码。
/// 键缺失或值不是字符串都归为同一类必填错误，且不做去空白处理，
/// 因此带空白的代码会在后续枚举匹配阶段被判为非法而不是被静默接受。
fn required_recovery_string(value: &serde_json::Value, key: &str) -> AppResult<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AppError::Validation(format!("stored strategy node {key} is required")))
}

/// 对每个待补槽位调用确定性生成器产出 1m OHLCV，用于预览展示而不写入任何存储。
/// 生成完全由策略版本的 seed 与开盘时刻决定，因此同一配置下预览与真正执行会得到逐根一致的结果。
/// 任一根生成失败即整体返回校验错误，不产出部分结果，避免让调用方据不完整的预览做确认。
fn generate_admin_recovery_samples(
    config: &crate::modules::market::synthetic::SyntheticMarketConfig,
    open_times: &[DateTime<Utc>],
) -> AppResult<Vec<MarketStrategyRecoverySampleResponse>> {
    open_times
        .iter()
        .map(|open_time| {
            let candle = config
                .generate_1m(*open_time)
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
        .collect()
}

/// 把完整预览序列抽稀到最多十二根，控制响应体积同时保留两端形态。
/// 总数不超过上限时原样返回；超出时只取首六根与末六根拼接，中间部分被丢弃且不作任何标记。
/// 因此响应里的样本在时间上可能不连续，调用方不应据此判断缺口范围，根数以预期计数字段为准。
fn limited_admin_recovery_samples(
    candles: &[MarketStrategyRecoverySampleResponse],
) -> Vec<MarketStrategyRecoverySampleResponse> {
    const LIMIT: usize = 12;
    if candles.len() <= LIMIT {
        return candles.to_vec();
    }
    let mut samples = candles[..LIMIT / 2].to_vec();
    samples.extend_from_slice(&candles[candles.len() - LIMIT / 2..]);
    samples
}

/// 把当前时刻向下取整到整分钟，得到最近一根已经闭合的 1m K 线开盘时刻。
/// 缺口检测据此设定查询上界，从而永远不把仍在形成中的当前分钟当作缺口。
/// 使用欧几里得除法保证负时间戳同样向下取整；秒级时间戳取整后必然仍可表示，故失败分支直接断言。
fn floor_admin_recovery_minute(value: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp(value.timestamp().div_euclid(60) * 60, 0)
        .expect("UTC minute timestamp must remain representable")
}

/// 创建行情策略及其首个运行检查点和版本 1 快照，并返回含有序节点的后台详情。
/// 请求须引用有效交易对并满足价格、时间、波动率和成交量约束；初始状态缺省为 draft，管理员 ID用于版本和审计归属。
/// 同一事务依次确认交易对、插入策略与节点、创建版本，再插入绑定该版本的运行行，最后记录事件和审计；任一步失败整体回滚。
/// 创建无请求幂等键，提交只写数据库，不直接启动策略 worker。
pub(crate) async fn create_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateMarketStrategyRequest,
) -> AppResult<AdminMarketStrategyDetailResponse> {
    validate_create_market_strategy(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 运行行的 active_version 外键要求版本已存在；所有配置与审计仍在同一事务内原子提交。
    let mut tx = pool.begin().await?;
    let market_type = ensure_market_strategy_pair_in_tx(&mut tx, request.pair_id).await?;
    let status = request
        .status
        .as_deref()
        .map(validate_market_strategy_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let strategy_type = optional_string(request.strategy_type.clone()).unwrap();
    let strategy_id = insert_admin_market_strategy_in_tx(
        &mut tx,
        AdminMarketStrategyInsert {
            pair_id: request.pair_id,
            strategy_type,
            start_price: request.start_price.clone(),
            target_price: request.target_price.clone(),
            start_time: request.start_time,
            end_time: request.end_time,
            volatility: request.volatility.clone(),
            volume_min: request.volume_min.clone(),
            volume_max: request.volume_max.clone(),
            status: status.clone(),
        },
    )
    .await?;
    insert_market_strategy_nodes_in_tx(&mut tx, strategy_id, &request.nodes).await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        1,
        request.start_time,
        market_strategy_config_json(&request, &status, &market_type),
        Uuid::now_v7().to_string(),
        admin_id,
    )
    .await?;
    insert_market_strategy_run_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&status),
        &request.start_price,
        request.start_time,
    )
    .await?;
    let strategy = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.create",
        None,
        Some(&strategy),
        request.reason,
    )
    .await?;
    let nodes = list_market_strategy_nodes_in_tx(&mut tx, strategy_id).await?;
    tx.commit().await?;
    Ok(AdminMarketStrategyDetailResponse { strategy, nodes })
}

/// 更新非 active 行情策略的配置，重置运行检查点并追加下一版本快照后返回含节点的新详情。
/// 请求须通过数值校验和审计原因校验；事务锁定策略后若状态仍为 active 则返回冲突。
/// 锁后按“主配置、运行检查点、计算下一版本、回读、版本记录、策略事件、后台审计”顺序写入，失败整体回滚。
/// 每次成功调用都会生成新 UUIDv7 版本和审计，故相同请求重放不是幂等操作；不会直接唤醒 worker。
pub(crate) async fn update_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    request: UpdateMarketStrategyRequest,
) -> AppResult<AdminMarketStrategyDetailResponse> {
    validate_update_market_strategy(&request)?;
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 更新策略配置时先锁定旧值，再重置运行检查点并追加版本快照，保证审计和调度状态一致。
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    let before_nodes = list_market_strategy_nodes_in_tx(&mut tx, strategy_id).await?;
    if before.status == "active" {
        return Err(AppError::Conflict(
            "active market strategy must be paused or disabled before update".to_owned(),
        ));
    }
    let strategy_type = optional_string(request.strategy_type.clone()).unwrap();
    update_admin_market_strategy_in_tx(
        &mut tx,
        strategy_id,
        AdminMarketStrategyUpdate {
            strategy_type,
            start_price: request.start_price.clone(),
            target_price: request.target_price.clone(),
            start_time: request.start_time,
            end_time: request.end_time,
            volatility: request.volatility.clone(),
            volume_min: request.volume_min.clone(),
            volume_max: request.volume_max.clone(),
        },
    )
    .await?;
    replace_market_strategy_nodes_in_tx(&mut tx, strategy_id, &request.nodes).await?;
    let next_version = next_market_strategy_version_in_tx(&mut tx, strategy_id).await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        next_version,
        request.start_time,
        market_strategy_update_config_json(&request, &before.status, &before.market_type),
        Uuid::now_v7().to_string(),
        admin_id,
    )
    .await?;
    update_market_strategy_run_checkpoint_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&before.status),
        &request.start_price,
        request.start_time,
        next_version,
    )
    .await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.update",
        Some(&before),
        Some(&after),
        Some(reason),
    )
    .await?;
    insert_market_strategy_event_in_tx(
        &mut tx,
        strategy_id,
        "market_strategy.nodes.snapshot",
        json!({ "before": before_nodes, "after": request.nodes }),
    )
    .await?;
    let nodes = list_market_strategy_nodes_in_tx(&mut tx, strategy_id).await?;
    tx.commit().await?;
    Ok(AdminMarketStrategyDetailResponse {
        strategy: after,
        nodes,
    })
}

/// 同步切换行情策略业务状态和运行状态，并返回更新后的策略快照。
/// 目标状态仅限 draft/active/paused/disabled；本用例不校验显式审计原因，也不执行额外状态迁移图约束。
/// 事务先锁策略，再更新主状态、映射后的运行状态、回读并写策略事件及后台审计；运行行缺失或 SQL 失败整体回滚。
/// 相同状态重放仍写事件和审计，提交后由其他运行组件观察数据库变化。
pub(crate) async fn update_admin_market_strategy_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    request: UpdateMarketStrategyStatusRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    let status = validate_market_strategy_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 状态和运行状态一起更新；如果运行检查点缺失，整个状态变更回滚。
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    update_market_strategy_status_in_tx(&mut tx, strategy_id, &status).await?;
    update_market_strategy_run_status_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&status),
    )
    .await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.status.update",
        Some(&before),
        Some(&after),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 在调用方事务内把一次策略变更同时写进策略事件流和后台审计日志，两处使用同一份前后值快照。
/// before 与 after 均为可选：创建时只有 after，删除类操作只有 before，二者都为空时仍会写出空快照的记录。
/// 事件面向策略自身的时间线，审计面向管理员操作追溯，二者内容重合是刻意冗余，便于分别按策略和按人检索。
/// 本函数不提交也不回滚事务，失败直接上抛由调用方统一回滚。
async fn record_admin_market_strategy_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    strategy_id: u64,
    action: &'static str,
    before: Option<&AdminMarketStrategyResponse>,
    after: Option<&AdminMarketStrategyResponse>,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = before.map(market_strategy_audit_json);
    let after_json = after.map(market_strategy_audit_json);
    insert_market_strategy_event_in_tx(
        tx,
        strategy_id,
        action,
        json!({
            "before": before_json,
            "after": after_json,
        }),
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "market_strategy",
            target_id: strategy_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}
