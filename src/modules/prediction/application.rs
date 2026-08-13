//! prediction bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 本文件同时充当 Axum 处理器与用例编排：函数直接接收提取器，完成鉴权、参数归一化与校验，
//! 再调用基础设施完成查询或事务，最后组装响应 DTO。
//! 端点分三类：无需登录的市场浏览、需要用户令牌的报价与下单查单、需要管理员令牌的配置与结算。
//! 用户身份一律从 `UserAuth` 的会话 subject 解析，管理员端点只验令牌不读身份。
//! 涉及资金的三个用例是创建报价、创建订单和人工结算，它们的原子性全部由基础设施的事务保证，
//! 本层只负责在进入事务前把非法输入挡住，事务开始后不再介入。
//! 后台配置类端点遵循「先归一化再校验再写入」的顺序，任一步失败都不会留下部分配置。
//! 另含一个常驻同步循环，按后台配置的间隔周期性拉取上游市场；
//! 单轮失败只记录告警不终止循环，因此同步故障不会导致整个进程退出。
//! 本层不发布领域事件，也不直接持有连接池，池句柄每次从应用状态取出。

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

/// 判断本轮轮询是否到达同步时点，到了才真正触发一次 Polymarket 同步并标记来源为 scheduled。
/// 同步开关关闭时直接返回，不做任何上游访问，因此关掉开关即可完全停用自动同步。
/// 到期判定基于设置里记录的上次同步开始时间：间隔取配置值且强制不低于 30 秒，
/// 与轮询周期一致，避免把间隔配得过小导致每轮都拉取上游。
/// 上次开始时间为空表示从未同步过，此时立即执行首轮。
/// 计算出的间隔为负说明上次开始时间在未来，多半是时钟回拨或人为改库，
/// 此时不跳过而是照常同步，宁可多同步一次也不会因异常时间戳永久停摆。
/// 该判定不加锁，多实例部署时可能出现同一轮被重复触发，需要由部署侧保证单实例运行。
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
/// 响应除配置项外还带出最近一次同步的状态、错误文本与导入更新计数，供后台一屏查看运行状况。
/// 设置行缺失会以内部错误暴露而非静默补默认值，因为费率与退款策略直接决定资金口径。
/// 仅需管理员令牌，不解析具体管理员身份，也不记录读取审计。
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
/// 同步间隔下限 30 秒，与轮询周期对齐，防止把间隔配到比轮询还短而实际无法生效。
/// 报价有效期必须落在 1 到 120 秒之间：为零会让报价一生成即失效，过长则让用户能长时间锁定旧赔率。
/// 资产范围逐个校验存在且启用，任一非法即整体拒绝，不保存部分有效的列表。
/// 标签与资产列表在写入前分别去空去重，因此重复填写不会污染配置。
/// 写入是整体覆盖而非增量合并，调用方必须提交完整配置，遗漏字段会被入参值覆盖。
/// 保存成功后回读完整设置返回，回读与写入不在同一事务，极端并发下可能读到他人刚保存的值。
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

/// 返回后台预测资产配置及与之口径一致的总数，查询失败不拼接部分配置。
/// 列表覆盖全部启用资产而非仅已配置资产，未配置项以未启用与零上限呈现，
/// 因此新上线的资产也能在此被找到并开启下注。
/// 条数与偏移经公共规则夹取，超范围参数退化为边界值而不报错。
/// 只读端点，不会因为查看而创建任何缺失的资产配置行。
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

/// 返回用户下注前需要的公共配置：可用资产清单、默认费率与报价有效期。
/// 可用资产需同时满足两个条件才会出现：落在后台设置的允许范围内，
/// 且在资产配置表中已启用、其资产本身也处于启用状态，两侧取交集。
/// 允许范围为空时明确返回空清单而不是放行全部资产，
/// 这样「未配置」的默认行为是禁止下注而非全面开放，符合资金业务的保守原则。
/// 费率与有效期原样透出，让前端能在本地预估手续费并倒计时报价失效。
/// 这是无需登录的公共端点，不含任何用户维度数据。
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

/// 以请求体中的资产编号新增或覆盖其下注启用状态与赔付上限；写入不移动任何用户资金。
/// 资产存在性与上限非负两项校验由基础设施在写入前完成，本处理器不重复判断。
/// 与按路径编号的更新端点共用同一段落库逻辑，区别仅在资产编号从请求体还是路径取，
/// 因此两个端点的语义与副作用完全等价，可按前端习惯任选。
/// 上限为零表示不设封顶而非禁止赔付，停用资产应把启用标记置假。
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

/// 以路径段中的资产编号新增或覆盖其下注启用状态与赔付上限；校验失败不保存部分字段。
/// 请求体只含启用标记与上限两项，资产编号取自路径，因此不存在两处编号冲突的可能。
/// 名为更新但底层是 upsert：资产尚无配置行时会新建而不是返回 `NotFound`，
/// 这使前端可以对任意启用资产直接调用本端点而无需先创建。
/// 两个字段整体覆盖，调用方必须同时提交当前的启用状态与上限。
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

/// 返回用户可浏览的预测市场列表，可见性条件直接写进 SQL 而非在内存里过滤。
/// 两个条件缺一不可：展示状态为可见，且结算状态处于开放或待确认之一；
/// 纳入待确认是为了让上游已关闭但尚未派奖的市场仍能被查到，用户可据此确认自己的持仓去向。
/// 已 settled 或 refunded 的市场从列表消失，历史成绩需通过订单列表查看。
/// 排序先按最近同步时间倒序再补主键，使刚更新的市场靠前且相同时间戳下顺序稳定。
/// 只按条数截断不支持偏移，因此本列表不提供深翻页。
/// 这是无需登录的公共端点，不含任何用户维度过滤。
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

/// 返回单个市场的公开详情，展示状态不是可见时一律按 `NotFound` 处理。
/// 把已隐藏市场折成不存在而非返回禁止访问，避免通过状态码差异探测出后台下架了哪些标的。
/// 与列表端点不同，这里不限制结算状态，因此已结算市场的详情仍可访问，
/// 使用户能在下注后继续查看该市场的最终结果。
/// 无需登录，返回内容与后台详情共用同一份读模型，包含覆盖配置字段。
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

/// 按后台展示状态、结算状态和标题关键字筛选市场，并返回与筛选口径一致的总数。
/// 三个筛选项都可选，空串等同于未传；行查询与计数查询在同一个循环里追加完全相同的条件，
/// 这是总数不会与实际可翻页行数脱节的关键，新增筛选项时必须同时作用于两个构建器。
/// 起始的恒真条件只为让后续条件都能以 AND 拼接，不影响执行计划。
/// 关键字按标题做前后通配匹配，因此无法命中索引，属于后台可接受的全表扫描。
/// 与用户侧列表不同，这里不施加任何可见性限制，已隐藏与已结算的市场都能查到。
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

/// 读取后台视角的市场完整详情，含上游结果、本地结果与四项覆盖配置，记录缺失返回 `NotFound`。
/// 与用户侧详情共用同一读模型，区别仅在于此处不校验展示状态，
/// 因此已隐藏市场对管理员始终可见，便于确认下架后的处置情况。
/// 纯读端点，不触发同步也不推进结算。
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
/// 展示状态必填并须归一化为可见或隐藏；结算模式覆盖为空或纯空白时视为清除覆盖而非报错，
/// 因此前端传空串即可让该市场回退到全局默认模式。
/// 允许资产覆盖先去重再逐个校验存在与启用，费率覆盖只校验非负，
/// 赔付上限覆盖原样透传不做结构校验，其键值语义由报价阶段解释。
/// 四项覆盖都是整体替换，未传即清除，因此调用方必须回填当前值以免误清。
/// 命中不到市场时把基础设施返回的假值转成 `NotFound`，随后回读最新详情返回；
/// 更新与回读不在同一事务，并发修改下可能读到他人刚写入的配置。
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

/// 按认证用户读取其预测订单，用户标识固定作为第一个 SQL 条件以阻止跨账户泄露。
/// 用户编号取自会话 subject 而非请求参数，因此调用方无法通过改参数查看他人订单。
/// 可选按订单状态与市场编号进一步筛选，空串状态视为未传；
/// 不限制结算状态，因此已派奖和已退款的历史订单同样可查。
/// 排序先按创建时间倒序再补主键，保证同一毫秒内创建的多笔订单顺序稳定。
/// 只按条数截断不支持偏移，也不返回总数，因此本列表面向最近记录而非完整对账。
/// 纯读端点，不会因查询而推进任何订单状态。
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

/// 按订单状态、市场编号与用户邮箱筛选全平台预测订单，并返回与筛选一致的总数。
/// 与用户侧列表的根本差别在于不施加用户维度限制，可跨账户查看，仅凭管理员令牌授权。
/// 三个筛选项都可选，且在同一循环内同时追加到行查询与计数查询，避免总数与实际行数脱节。
/// 邮箱按前后通配匹配，便于模糊查找，代价是无法命中索引。
/// 排序先按创建时间倒序再补主键，与用户侧保持一致，使同一批订单在两端顺序相同。
/// 支持偏移分页，条数与偏移经公共规则夹取；纯读端点，不触发结算也不发起退款。
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

/// 按订单主键读取后台订单详情，含用户邮箱、市场标题、资产符号与全部金额字段。
/// 未结算订单的派奖额、退款额与结算时间为空，调用方应据状态而非金额是否为零判断进度。
/// 不做用户维度过滤，凭管理员令牌即可查看任意用户的订单，记录缺失返回 `NotFound`。
/// 纯读端点，不修改钱包也不推进订单状态。
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

/// 按主键倒序分页返回同步日志及全表总数，每条含触发来源、状态、导入更新计数与起止时间。
/// 主键倒序等价于时间倒序，最近一轮恒在首页首行，便于排障时直接看最新结果。
/// 触发来源可区分定时轮询与后台手动触发，失败记录带有压缩截断后的错误文本。
/// 不支持按状态或时间筛选，因此总数就是全表条数。
/// 纯读端点：不会重新执行上游同步，也不会把长时间停留在 running 的记录改判为失败。
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
