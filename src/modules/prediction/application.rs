//! prediction bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::{
        auth::{AdminAuth, UserAuth},
        prediction::{
            infrastructure, presentation, repository,
            service::{self, DEFAULT_SYNC_POLL_SECONDS},
        },
    },
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use std::collections::HashSet;
use tokio::time::sleep;

/// 每 30 秒检查后台同步开关与间隔，到期时以 Polymarket 响应更新预测标的和上游结果。
/// 单轮失败只记录告警并等待下次轮询；自动结算模式遇到明确终局时会进入本地钱包结算事务。
pub async fn run_sync_loop(state: AppState) -> AppResult<()> {
    loop {
        if let Err(error) = run_due_sync_once(&state).await {
            tracing::warn!(%error, "prediction market sync tick failed");
        }
        sleep(tokio::time::Duration::from_secs(DEFAULT_SYNC_POLL_SECONDS)).await;
    }
}

async fn run_due_sync_once(state: &AppState) -> AppResult<()> {
    let pool = infrastructure::mysql_pool(state)?;
    let settings = infrastructure::load_settings(&pool).await?;
    if !settings.sync_enabled {
        return Ok(());
    }
    let now = Utc::now();
    if let Some(last_started) = settings.last_sync_started_at {
        let elapsed = now.signed_duration_since(last_started).num_seconds();
        if elapsed >= 0 && elapsed < i64::from(settings.sync_interval_seconds.max(30)) {
            return Ok(());
        }
    }
    infrastructure::sync_polymarket_markets(&pool, "scheduled").await?;
    Ok(())
}

/// 读取预测市场后台同步、费率与结算设置；数据库缺失或失败不使用进程默认值伪造响应。
pub(crate) async fn get_admin_settings(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionSettingsResponse>> {
    Ok(Json(presentation::PredictionSettingsResponse::from(
        infrastructure::load_settings(&infrastructure::mysql_pool(&state)?).await?,
    )))
}

/// 校验结算模式、退款策略、非负费率、同步周期、报价 TTL 及资产存在性后保存后台设置。
/// 保存只改变配置和同步参数，不结算市场或移动用户资金；任一校验/SQL 失败不返回部分配置。
pub(crate) async fn save_admin_settings(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::SavePredictionSettingsRequest>,
) -> AppResult<Json<presentation::PredictionSettingsResponse>> {
    let pool = infrastructure::mysql_pool(&state)?;
    let settlement_mode = service::normalize_settlement_mode(&request.default_settlement_mode)?;
    let refund_policy =
        service::normalize_invalid_refund_policy(&request.default_invalid_refund_policy)?;
    service::ensure_non_negative_decimal(&request.default_fee_rate, "default_fee_rate")?;
    if request.sync_interval_seconds < 30 {
        return Err(AppError::Validation(
            "sync_interval_seconds must be at least 30".to_owned(),
        ));
    }
    if request.quote_ttl_seconds == 0 || request.quote_ttl_seconds > 120 {
        return Err(AppError::Validation(
            "quote_ttl_seconds must be between 1 and 120".to_owned(),
        ));
    }
    infrastructure::validate_asset_ids_exist(&pool, &request.allowed_asset_ids).await?;
    let sync_tags = service::normalize_string_list(request.sync_tags);
    let allowed_asset_ids = service::unique_u64_list(request.allowed_asset_ids);

    infrastructure::save_admin_settings(
        &pool,
        request.sync_enabled,
        request.sync_interval_seconds,
        &sync_tags,
        &allowed_asset_ids,
        request.default_fee_rate,
        settlement_mode,
        refund_policy,
        request.quote_ttl_seconds,
    )
    .await?;

    Ok(Json(presentation::PredictionSettingsResponse::from(
        infrastructure::load_settings(&pool).await?,
    )))
}

/// 返回后台预测资产配置及一致总数，查询失败不拼接部分配置。
pub(crate) async fn list_admin_asset_configs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::AdminListQuery>,
) -> AppResult<Json<presentation::PredictionAssetConfigsResponse>> {
    let (rows, total) = infrastructure::list_admin_asset_configs(
        &infrastructure::mysql_pool(&state)?,
        service::route_limit(query.limit),
        service::route_offset(query.offset),
    )
    .await?;
    let configs = rows
        .into_iter()
        .map(presentation::PredictionAssetConfigResponse::from)
        .collect();
    Ok(Json(presentation::PredictionAssetConfigsResponse {
        configs,
        total,
    }))
}

/// 按后台允许资产集合过滤启用资产，集合为空时明确返回空目录而非全部资产。
pub(crate) async fn get_user_config(
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionUserConfigResponse>> {
    let pool = infrastructure::mysql_pool(&state)?;
    let settings = infrastructure::load_settings(&pool).await?;
    let allowed_ids = service::json_u64_array(&settings.allowed_asset_ids_json);
    if allowed_ids.is_empty() {
        return Ok(Json(presentation::PredictionUserConfigResponse {
            allowed_assets: Vec::new(),
            default_fee_rate: settings.default_fee_rate,
            quote_ttl_seconds: settings.quote_ttl_seconds,
        }));
    }
    let allowed_set = allowed_ids.into_iter().collect::<HashSet<_>>();
    let allowed_assets = infrastructure::list_stake_assets(&pool)
        .await?
        .into_iter()
        .filter(|row| allowed_set.contains(&row.asset_id))
        .map(presentation::PredictionStakeAssetResponse::from)
        .collect();
    Ok(Json(presentation::PredictionUserConfigResponse {
        allowed_assets,
        default_fee_rate: settings.default_fee_rate,
        quote_ttl_seconds: settings.quote_ttl_seconds,
    }))
}

/// 新增或覆盖投注资产启用状态与赔付上限；资产必须存在，上限不得为负，写入不移动用户资金。
pub(crate) async fn upsert_admin_asset_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::UpsertPredictionAssetConfigRequest>,
) -> AppResult<Json<presentation::PredictionAssetConfigResponse>> {
    infrastructure::upsert_asset_config(
        &infrastructure::mysql_pool(&state)?,
        request.asset_id,
        request.enabled,
        request.max_payout_amount,
    )
    .await
    .map(Json)
}

/// 按路径资产编号新增或覆盖投注资产启用状态与赔付上限；校验失败不保存部分字段。
pub(crate) async fn update_admin_asset_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<presentation::UpdatePredictionAssetConfigRequest>,
) -> AppResult<Json<presentation::PredictionAssetConfigResponse>> {
    infrastructure::upsert_asset_config(
        &infrastructure::mysql_pool(&state)?,
        asset_id,
        request.enabled,
        request.max_payout_amount,
    )
    .await
    .map(Json)
}

/// 只返回 active 且仍开放或待确认的预测市场，SQL 始终施加公共可见性条件。
pub(crate) async fn list_user_markets(
    State(state): State<AppState>,
    Query(query): Query<presentation::ListQuery>,
) -> AppResult<Json<presentation::PredictionMarketsResponse>> {
    let mut builder = infrastructure::prediction_market_query_builder();
    builder.push(" WHERE markets.display_status = ");
    builder.push_bind(service::STATUS_ACTIVE);
    builder.push(" AND markets.settlement_status IN ('open', 'pending_confirmation')");
    builder.push(" ORDER BY markets.last_synced_at DESC, markets.id DESC LIMIT ");
    builder.push_bind(service::route_limit(query.limit) as i64);
    let rows = builder
        .build_query_as::<repository::PredictionMarketRow>()
        .fetch_all(&infrastructure::mysql_pool(&state)?)
        .await?;
    let markets = rows
        .into_iter()
        .map(presentation::PredictionMarketResponse::from)
        .collect();
    Ok(Json(presentation::PredictionMarketsResponse { markets }))
}

/// 市场隐藏时按 NotFound 处理，避免公共详情泄露后台下架标的。
pub(crate) async fn get_user_market(
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
) -> AppResult<Json<presentation::PredictionMarketResponse>> {
    let market =
        infrastructure::load_market_response(&infrastructure::mysql_pool(&state)?, market_id)
            .await?;
    if market.display_status != service::STATUS_ACTIVE {
        return Err(AppError::NotFound);
    }
    Ok(Json(market))
}

/// 按后台展示、结算和关键字条件返回市场及一致总数，不修改同步状态。
pub(crate) async fn list_admin_markets(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::AdminMarketQuery>,
) -> AppResult<Json<presentation::AdminPredictionMarketsResponse>> {
    let display_status = service::optional_text(query.display_status);
    let settlement_status = service::optional_text(query.settlement_status);
    let keyword = service::optional_text(query.keyword);
    let mut rows = infrastructure::prediction_market_query_builder();
    let mut total = infrastructure::prediction_market_count_query_builder();
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(status) = display_status.clone() {
            builder.push(" AND markets.display_status = ");
            builder.push_bind(status);
        }
        if let Some(status) = settlement_status.clone() {
            builder.push(" AND markets.settlement_status = ");
            builder.push_bind(status);
        }
        if let Some(keyword) = keyword.clone() {
            builder.push(" AND markets.title LIKE ");
            builder.push_bind(format!("%{keyword}%"));
        }
    }

    // 按主键排序：last_synced_at 每轮同步都会被批量刷新，翻页时行会在页间跳动。
    let (rows, total) = infrastructure::fetch_admin_page::<repository::PredictionMarketRow>(
        &infrastructure::mysql_pool(&state)?,
        rows,
        total,
        " ORDER BY markets.id DESC",
        service::route_limit(query.limit),
        service::route_offset(query.offset),
    )
    .await?;
    let markets = rows
        .into_iter()
        .map(presentation::PredictionMarketResponse::from)
        .collect();
    Ok(Json(presentation::AdminPredictionMarketsResponse {
        markets,
        total,
    }))
}

/// 读取后台市场完整详情与覆盖配置，记录缺失返回 NotFound。
pub(crate) async fn get_admin_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
) -> AppResult<Json<presentation::PredictionMarketResponse>> {
    Ok(Json(
        infrastructure::load_market_response(&infrastructure::mysql_pool(&state)?, market_id)
            .await?,
    ))
}

/// 更新单个市场的展示状态及可选结算/资产/赔付/费率覆盖；资产不存在或费率非法时在写入前拒绝。
/// 该配置更新不结算既有订单、不冻结或释放钱包资金，市场不存在返回 NotFound。
pub(crate) async fn update_admin_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
    Json(request): Json<presentation::UpdatePredictionMarketRequest>,
) -> AppResult<Json<presentation::PredictionMarketResponse>> {
    let pool = infrastructure::mysql_pool(&state)?;
    let display_status = service::normalize_display_status(&request.display_status)?;
    let settlement_mode_override = match request.settlement_mode_override {
        Some(value) if !value.trim().is_empty() => {
            Some(service::normalize_settlement_mode(&value)?)
        }
        _ => None,
    };
    let allowed_override = request
        .allowed_asset_ids_override
        .map(service::unique_u64_list);
    if let Some(ids) = allowed_override.as_ref() {
        infrastructure::validate_asset_ids_exist(&pool, ids).await?;
    }
    if let Some(rate) = request.fee_rate_override.as_ref() {
        service::ensure_non_negative_decimal(rate, "fee_rate_override")?;
    }

    let updated = infrastructure::update_admin_market(
        &pool,
        market_id,
        &display_status,
        settlement_mode_override.as_deref(),
        allowed_override.as_deref(),
        request.payout_cap_overrides.as_ref(),
        request.fee_rate_override.as_ref(),
    )
    .await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Json(
        infrastructure::load_market_response(&pool, market_id).await?,
    ))
}

/// 为认证用户创建预测报价：依次读取设置、市场与资产配置，校验开放状态、结果、正本金与精度后持久化短期报价。
/// 报价只快照概率、费率、理论赔付和过期时间，不冻结或扣减钱包；失败不留下可下单报价。
pub(crate) async fn create_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::CreatePredictionQuoteRequest>,
) -> AppResult<Json<presentation::PredictionQuoteResponse>> {
    let user_id = service::user_id_from_subject(&claims.sub)?;
    let quote =
        infrastructure::create_quote_in_db(&infrastructure::mysql_pool(&state)?, user_id, request)
            .await?;
    Ok(Json(quote))
}

/// 以报价和幂等键创建预测订单；事务内锁定报价、市场与钱包，校验归属/未过期后冻结本金并扣费、写流水和订单。
/// 相同用户幂等键命中时直接返回原订单且 `changed=false`，当前实现不比较本次 `quote_id`；新订单任一步失败整体回滚且不发布提交外事件。
pub(crate) async fn create_order(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<presentation::CreatePredictionOrderRequest>,
) -> AppResult<Json<presentation::PredictionOrderActionResponse>> {
    let user_id = service::user_id_from_subject(&claims.sub)?;
    let (order, changed) =
        infrastructure::create_order_in_tx(&infrastructure::mysql_pool(&state)?, user_id, request)
            .await?;
    Ok(Json(presentation::PredictionOrderActionResponse {
        order,
        changed,
    }))
}

/// 按认证用户读取预测订单，用户标识固定进入 SQL 条件以阻止跨账户泄露。
pub(crate) async fn list_user_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::OrdersQuery>,
) -> AppResult<Json<presentation::PredictionOrdersResponse>> {
    let user_id = service::user_id_from_subject(&claims.sub)?;
    let mut builder = infrastructure::prediction_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    if let Some(status) = service::optional_text(query.status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    if let Some(market_id) = query.market_id {
        builder.push(" AND orders.market_id = ");
        builder.push_bind(market_id);
    }
    builder.push(" ORDER BY orders.created_at DESC, orders.id DESC LIMIT ");
    builder.push_bind(service::route_limit(query.limit) as i64);
    let rows = builder
        .build_query_as::<repository::PredictionOrderRow>()
        .fetch_all(&infrastructure::mysql_pool(&state)?)
        .await?;
    let orders = rows
        .into_iter()
        .map(presentation::PredictionOrderResponse::from)
        .collect();
    Ok(Json(presentation::PredictionOrdersResponse { orders }))
}

/// 按后台筛选返回预测订单与总数，读取不触发结算或退款。
pub(crate) async fn list_admin_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::AdminOrdersQuery>,
) -> AppResult<Json<presentation::AdminPredictionOrdersResponse>> {
    let status = service::optional_text(query.status);
    let email = service::optional_text(query.email);
    let mut rows = infrastructure::prediction_order_query_builder();
    let mut total = infrastructure::prediction_order_count_query_builder();
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(status) = status.clone() {
            builder.push(" AND orders.status = ");
            builder.push_bind(status);
        }
        if let Some(market_id) = query.market_id {
            builder.push(" AND orders.market_id = ");
            builder.push_bind(market_id);
        }
        if let Some(email) = email.clone() {
            builder.push(" AND users.email LIKE ");
            builder.push_bind(format!("%{email}%"));
        }
    }

    let (rows, total) = infrastructure::fetch_admin_page::<repository::PredictionOrderRow>(
        &infrastructure::mysql_pool(&state)?,
        rows,
        total,
        " ORDER BY orders.created_at DESC, orders.id DESC",
        service::route_limit(query.limit),
        service::route_offset(query.offset),
    )
    .await?;
    let orders = rows
        .into_iter()
        .map(presentation::PredictionOrderResponse::from)
        .collect();
    Ok(Json(presentation::AdminPredictionOrdersResponse {
        orders,
        total,
    }))
}

/// 读取后台预测订单详情，记录缺失返回 NotFound 且不修改钱包。
pub(crate) async fn get_admin_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<presentation::PredictionOrderResponse>> {
    Ok(Json(
        infrastructure::load_order_response(&infrastructure::mysql_pool(&state)?, order_id).await?,
    ))
}

/// 人工结算市场：事务内锁定市场及全部 open 订单，再按 yes/no 派奖或按 invalid 策略退款本金/手续费。
/// 每单钱包余额、冻结额、流水与订单终态同事务提交；市场已终态时直接返回 `changed=false`，当前实现不比较重放结果或退款策略。
pub(crate) async fn settle_admin_market(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(market_id): Path<u64>,
    Json(request): Json<presentation::SettlePredictionMarketRequest>,
) -> AppResult<Json<presentation::PredictionSettlementResponse>> {
    let result = service::normalize_settlement_result(&request.result)?;
    let refund_policy = match request.invalid_refund_policy {
        Some(value) if !value.trim().is_empty() => {
            Some(service::normalize_invalid_refund_policy(&value)?)
        }
        _ => None,
    };
    let (market, settled_orders, changed) = infrastructure::settle_market_in_tx(
        &infrastructure::mysql_pool(&state)?,
        market_id,
        result,
        refund_policy,
    )
    .await?;
    Ok(Json(presentation::PredictionSettlementResponse {
        market,
        settled_orders,
        changed,
    }))
}

/// 立即执行一次 Polymarket 同步并记录 `manual` 触发来源，更新标的、价格、状态和上游结果快照。
/// 自动结算模式遇到明确终局时会调用本地结算事务移动钱包资金；人工模式只转为待确认。
pub(crate) async fn trigger_admin_sync(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<presentation::PredictionSyncResponse>> {
    let response =
        infrastructure::sync_polymarket_markets(&infrastructure::mysql_pool(&state)?, "manual")
            .await?;
    Ok(Json(response))
}

/// 按时间倒序返回预测同步日志及总数，不重新执行上游同步。
pub(crate) async fn list_admin_sync_logs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<presentation::AdminListQuery>,
) -> AppResult<Json<presentation::PredictionSyncLogsResponse>> {
    let (rows, total) = infrastructure::list_admin_sync_logs(
        &infrastructure::mysql_pool(&state)?,
        service::route_limit(query.limit),
        service::route_offset(query.offset),
    )
    .await?;
    let logs = rows
        .into_iter()
        .map(presentation::PredictionSyncLogResponse::from)
        .collect();
    Ok(Json(presentation::PredictionSyncLogsResponse {
        logs,
        total,
    }))
}
