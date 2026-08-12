use super::*;
use secrecy::ExposeSecret;

/// 构建交易对、行情源和市场策略的后台传输路由。
///
/// 所有入口保持管理员鉴权和既有 DTO；行情运行态只在传输边界汇集当前 supervisor 快照，
/// 配置、凭据、重载及策略变更继续转发给应用层。凭据密钥只作为受保护配置参数传递，
/// 路由不执行行情 provider 请求，也不改变应用层错误映射。
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/market-pairs",
            get(list_trading_pairs).post(create_trading_pair),
        )
        .route(
            "/market-pairs/:id",
            get(get_trading_pair).patch(update_trading_pair),
        )
        .route(
            "/market-pairs/:id/status",
            patch(update_trading_pair_status),
        )
        .route(
            "/market-feed/config",
            get(get_market_feed_config).patch(save_market_feed_config),
        )
        .route("/market-feed/reload", post(reload_market_feed_config))
        .route("/market-feed/status", get(get_market_feed_status))
        .route(
            "/market-feed/credentials",
            get(list_market_feed_credentials),
        )
        .route(
            "/market-feed/credentials/:provider",
            patch(upsert_market_feed_credential),
        )
        .route(
            "/market-strategies",
            get(list_market_strategies).post(create_market_strategy),
        )
        .route(
            "/market-strategies/:id",
            get(get_market_strategy).patch(update_market_strategy),
        )
        .route(
            "/market-strategies/:id/status",
            patch(update_market_strategy_status),
        )
        .route(
            "/market-strategies/:id/kline-gaps",
            get(detect_market_strategy_gaps),
        )
        .route(
            "/market-strategies/:id/kline-recovery/preview",
            post(preview_market_strategy_recovery),
        )
        .route(
            "/market-strategies/:id/kline-recovery/execute",
            post(execute_market_strategy_recovery),
        )
        .route(
            "/market-strategies/:id/kline-recovery/jobs",
            get(list_market_strategy_recovery_jobs),
        )
}

async fn get_market_feed_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<MarketFeedConfigResponse>>> {
    Ok(Json(
        get_market_feed_config_use_case(state.mysql.clone()).await?,
    ))
}

async fn save_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveMarketFeedConfigRequest>,
) -> AppResult<Json<MarketFeedConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_market_feed_config_use_case(state.mysql.clone(), admin_id, request).await?;
    Ok(Json(config))
}

/// 汇集当前行情 supervisor 运行快照后转发状态查询；快照缺失仍按既有空运行态处理，
/// 配置读取和状态组装由应用层负责，传输层不访问 provider。
async fn get_market_feed_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketFeedStatusResponse>> {
    let runtime = load_market_feed_runtime(&state).await;
    Ok(Json(
        get_market_feed_status_use_case(state.mysql.clone(), runtime).await?,
    ))
}

async fn list_market_feed_credentials(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketSourceCredentialsResponse>> {
    Ok(Json(
        list_market_feed_credentials_use_case(state.mysql.clone()).await?,
    ))
}

async fn upsert_market_feed_credential(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(request): Json<UpsertMarketSourceCredentialRequest>,
) -> AppResult<Json<MarketSourceCredentialResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let key = state.settings.exposed_credential_encryption_key();
    let credential = upsert_market_feed_credential_use_case(
        state.mysql.clone(),
        admin_id,
        provider,
        key,
        request,
    )
    .await?;
    Ok(Json(credential))
}

async fn reload_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<ReloadMarketFeedRequest>,
) -> AppResult<Json<ReloadMarketFeedResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reload_market_feed_config_use_case(state, admin_id, request).await?,
    ))
}

async fn list_trading_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminTradingPairQuery>,
) -> AppResult<Json<AdminTradingPairsResponse>> {
    Ok(Json(
        list_trading_pairs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_trading_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    Ok(Json(
        get_trading_pair_use_case(state.mysql.clone(), pair_id).await?,
    ))
}

async fn update_trading_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateTradingPairRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_trading_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?,
    ))
}

async fn update_trading_pair_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateTradingPairStatusRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_trading_pair_status_use_case(state.mysql.clone(), admin_id, pair_id, request)
            .await?,
    ))
}

async fn create_trading_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateTradingPairRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_trading_pair_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn list_market_strategies(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarketStrategyQuery>,
) -> AppResult<Json<AdminMarketStrategiesResponse>> {
    Ok(Json(
        list_market_strategies_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 读取策略主配置、运行快照和有序节点；路由只解析策略 ID 并转发读用例。
async fn get_market_strategy(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
) -> AppResult<Json<AdminMarketStrategyDetailResponse>> {
    Ok(Json(
        get_market_strategy_use_case(state.mysql.clone(), strategy_id).await?,
    ))
}

/// 转发策略时间窗口的 1m K 线缺口检测；传输层不读 Mongo 文档也不计算缺口。
async fn detect_market_strategy_gaps(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Query(query): Query<MarketStrategyGapQuery>,
) -> AppResult<Json<MarketStrategyGapsResponse>> {
    Ok(Json(
        detect_market_strategy_gaps_use_case(
            state.mysql.clone(),
            state.mongo.clone(),
            strategy_id,
            query,
            chrono::Utc::now(),
        )
        .await?,
    ))
}

/// 转发无写入的补偿预览，并使用运行时 JWT 密钥对短时确认令牌签名。
/// 路由不生成 OHLCV、不暴露密钥，所有版本/缺口校验由应用层完成。
async fn preview_market_strategy_recovery(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<PreviewMarketStrategyRecoveryRequest>,
) -> AppResult<Json<MarketStrategyRecoveryPreviewResponse>> {
    let token_key = state.settings.jwt_secret.expose_secret().as_bytes();
    Ok(Json(
        preview_market_strategy_recovery_use_case(
            state.mysql.clone(),
            state.mongo.clone(),
            strategy_id,
            request,
            token_key,
            chrono::Utc::now(),
        )
        .await?,
    ))
}

/// 将已认证管理员、预览令牌和审计原因转发给执行用例，返回新建 pending 任务。
/// 路由不写 MySQL/Mongo/Redis；令牌验签、唯一性和审计事务都在应用/基础设施层收敛。
async fn execute_market_strategy_recovery(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<ExecuteMarketStrategyRecoveryRequest>,
) -> AppResult<Json<MarketStrategyRecoveryJobResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let token_key = state.settings.jwt_secret.expose_secret().as_bytes();
    Ok(Json(
        execute_market_strategy_recovery_use_case(
            state.mysql.clone(),
            state.mongo.clone(),
            admin_id,
            strategy_id,
            request,
            token_key,
            chrono::Utc::now(),
        )
        .await?,
    ))
}

/// 按路径策略和查询状态转发补偿任务历史分页，不返回预览令牌哈希。
async fn list_market_strategy_recovery_jobs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Query(query): Query<MarketStrategyRecoveryJobsQuery>,
) -> AppResult<Json<MarketStrategyRecoveryJobsResponse>> {
    Ok(Json(
        list_market_strategy_recovery_jobs_use_case(state.mysql.clone(), strategy_id, query)
            .await?,
    ))
}

async fn create_market_strategy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateMarketStrategyRequest>,
) -> AppResult<Json<AdminMarketStrategyDetailResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_market_strategy_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_market_strategy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<UpdateMarketStrategyRequest>,
) -> AppResult<Json<AdminMarketStrategyDetailResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_market_strategy_use_case(state.mysql.clone(), admin_id, strategy_id, request)
            .await?,
    ))
}

async fn update_market_strategy_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<UpdateMarketStrategyStatusRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_market_strategy_status_use_case(state.mysql.clone(), admin_id, strategy_id, request)
            .await?,
    ))
}
