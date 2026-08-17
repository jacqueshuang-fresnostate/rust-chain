//! 承载交易对配置、行情源订阅与凭据、以及行情策略与手动 K 线补偿的 HTTP 传输入口。
//!
//! 本文件是后台路由中少数需要触碰运行时依赖的一组：行情状态查询要汇集 supervisor 快照，
//! 凭据 upsert 要传入配置里的凭据加密密钥，补偿预览与执行要传入 JWT 密钥用于签发和验签短时确认令牌，
//! 缺口检测与补偿还要把 Mongo 句柄一并转发。即便如此，路由仍不发起任何 provider 请求、不生成 K 线、
//! 不写 MySQL/Mongo/Redis，密钥只作为参数向下传递且不出现在响应里；事务、版本推进与审计留痕都在下层完成。

use super::*;
use secrecy::ExposeSecret;

/// 构建交易对、行情源和市场策略的后台传输路由。
///
/// 所有入口保持管理员鉴权和既有 DTO；行情运行态只在传输边界汇集当前 supervisor 快照，
/// 配置、凭据、重载及策略变更继续转发给应用层。凭据密钥只作为受保护配置参数传递，
/// 路由不执行行情 provider 请求，也不改变应用层错误映射。
/// 策略补偿被拆成缺口检测、无写入预览和凭令牌执行三个独立入口，必须按该顺序调用，
/// 因为执行入口只接受预览签发且尚未过期的确认令牌。
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
            "/market-strategies/presets",
            get(list_market_strategy_presets),
        )
        .route("/market-strategies/preview", post(preview_market_strategy))
        .route(
            "/market-strategies/:id",
            get(get_market_strategy).patch(update_market_strategy),
        )
        .route(
            "/market-strategies/:id/status",
            patch(update_market_strategy_status),
        )
        .route(
            "/market-strategies/:id/versions",
            get(list_market_strategy_versions),
        )
        .route(
            "/market-strategies/:id/versions/:version/restore",
            post(restore_market_strategy_version),
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

/// 处理 GET /market-feed/config，读取全站唯一的一份行情订阅配置。
/// 响应类型是 Option，尚未初始化过配置时返回 JSON null 而不是 404；读取不加行锁，也不访问行情监督器。
async fn get_market_feed_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<MarketFeedConfigResponse>>> {
    Ok(Json(
        get_market_feed_config_use_case(state.mysql.clone()).await?,
    ))
}

/// 处理 PATCH /market-feed/config，覆盖保存订阅的交易对、K 线周期与行情提供商。
/// 请求必须携带审计原因，提供商去重后必须恰好一个，配置启用时交易对不得为空。
/// 保存只推进数据库中的配置版本号并不触发监督器重载，重载需另行调用重载入口，因此该接口不具幂等性。
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

/// 处理 GET /market-feed/credentials，列出各行情提供商已保存的凭据条目。
/// 响应只含认证类型、API Key 掩码与启用状态，密文既不解密也不外泄；该入口不写审计。
async fn list_market_feed_credentials(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketSourceCredentialsResponse>> {
    Ok(Json(
        list_market_feed_credentials_use_case(state.mysql.clone()).await?,
    ))
}

/// 处理 PATCH /market-feed/credentials/:provider，按提供商创建或覆盖行情源接入凭据。
/// 路由从运行配置中取出凭据加密密钥并向下传递，密钥本身不写日志也不回显到响应。
/// 应用层按 provider 加锁后只加密本次提交的字段，未提交的密钥沿用既有密文，因此可以单独轮换某一项。
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

/// 处理 POST /market-feed/reload，把已保存的行情配置真正加载进运行时监督器。
/// 与保存入口不同，这里必须传入完整 `AppState` 才能拿到监督器句柄；请求必须携带操作原因。
/// 配置处于停用状态时只停止监督器并标记为跳过，启用时会先解密校验凭据再重载；
/// 运行时切换与数据库状态不在同一事务内，失败会持久化失败状态并写审计后返回原错误。
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

/// 处理 GET /market-pairs，按符号、状态和市场类型筛选交易对。
/// 与多数列表入口不同，这里的筛选值会经过与写入相同的规范化和枚举校验，非法取值直接返回校验错误而非空列表。
async fn list_trading_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminTradingPairQuery>,
) -> AppResult<Json<AdminTradingPairsResponse>> {
    Ok(Json(
        list_trading_pairs_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 处理 GET /market-pairs/:id，读取单个交易对的资产、符号、图标、精度、最小下单额与市场类型。
/// 查询不加锁，交易对缺失返回未找到；本入口只读取配置，不会启动行情订阅或撮合组件。
async fn get_trading_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    Ok(Json(
        get_trading_pair_use_case(state.mysql.clone(), pair_id).await?,
    ))
}

/// 处理 PATCH /market-pairs/:id，整体覆盖交易对的图标、精度、最小下单额、状态与市场类型。
/// 请求必须携带审计原因，且是全量覆盖而非局部合并；基准资产、计价资产和交易对符号在此入口不可修改。
/// 应用层锁定旧值后写入并记录 before/after，提交后不会自动重载行情或处理已挂出的订单。
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

/// 处理 PATCH /market-pairs/:id/status，单独切换交易对的启用与停用状态。
/// 请求必须携带审计原因；与配置更新入口相比，这里只改状态字段，不会触及精度与最小下单额。
/// 停用不检查是否仍有活动订单或持仓，也不发布市场状态事件，需要人工确认无残留业务后再操作。
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

/// 处理 POST /market-pairs，新建现货交易对并写入精度与最小下单额等撮合参数。
/// 应用层会先确认基准资产与计价资产均处于可用状态再插入，两者相同或符号非法直接返回校验错误。
/// 状态缺省为 disabled、市场类型缺省为 external，因此新建交易对默认不对外开放，也不会自动订阅行情。
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

/// 处理 GET /market-strategies，按交易对和状态分页检索模拟行情策略。
/// 响应含策略配置、运行检查点与补偿相关字段；本入口只读，不会锁定策略版本或改变 worker 的运行状态。
async fn list_market_strategies(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarketStrategyQuery>,
) -> AppResult<Json<AdminMarketStrategiesResponse>> {
    Ok(Json(
        list_market_strategies_use_case(state.mysql.clone(), query).await?,
    ))
}

/// 返回模拟行情场景预设目录；该入口只做管理员鉴权并调用无 I/O 的应用用例。
/// 响应中的中文名称、显式参数和相对节点模板均由后端维护，不读取当前策略或生成实际 seed。
async fn list_market_strategy_presets(
    _auth: AdminAuth,
) -> AppResult<Json<MarketStrategyPresetsResponse>> {
    Ok(Json(list_market_strategy_presets_use_case()))
}

/// 对完整行情策略草稿执行无副作用预览；路由只传入 MySQL 连接用于读取交易对目录。
/// 生成过程不接收 Mongo、Redis 或广播句柄，因此即使预览失败也不会留下行情、任务或检查点写入。
async fn preview_market_strategy(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<PreviewMarketStrategyRequest>,
) -> AppResult<Json<MarketStrategyPreviewResponse>> {
    Ok(Json(
        preview_market_strategy_use_case(state.mysql.clone(), request).await?,
    ))
}

/// 分页读取指定策略的不可变版本历史；策略 ID 来自路径，筛选只允许分页参数。
/// 响应标出当前激活版本并返回兼容解析后的高级参数，不改变策略运行状态。
async fn list_market_strategy_versions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Query(query): Query<MarketStrategyVersionsQuery>,
) -> AppResult<Json<MarketStrategyVersionsResponse>> {
    Ok(Json(
        list_market_strategy_versions_use_case(state.mysql.clone(), strategy_id, query).await?,
    ))
}

/// 复制指定历史版本为递增新版本；管理员身份与审计原因由应用层写入同一事务。
/// active 策略和当前激活版本会被拒绝，路由不直接更新主表、节点、运行检查点或版本行。
async fn restore_market_strategy_version(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path((strategy_id, version)): Path<(u64, i32)>,
    Json(request): Json<RestoreMarketStrategyVersionRequest>,
) -> AppResult<Json<AdminMarketStrategyDetailResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        restore_market_strategy_version_use_case(
            state.mysql.clone(),
            admin_id,
            strategy_id,
            version,
            request,
        )
        .await?,
    ))
}

/// 读取策略主配置、运行快照和有序节点；路由只解析策略 ID 并转发读用例。
/// 主读模型与节点集合分两次无锁查询取得，并发更新时二者可能短暂来自不同快照，刷新即可收敛。
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
/// 路由在此额外传入 Mongo 句柄与当前时刻，应用层据此把查询上界收敛到最近一根已闭合的 UTC 分钟，
/// 因此检测结果永远不包含仍在形成中的当前分钟；该入口只读，不签发任何补偿令牌。
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
/// 预览会重新确认所选范围内每个 1m 槽位确实缺失，并把策略版本与缺口摘要绑定进令牌，
/// 因此预览之后策略若被改动或缺口被别的路径补上，后续执行会因不匹配而失败。
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
/// 与预览入口的关键差异是这里必须解析管理员审计主体且请求原因为必填，令牌哈希同时充当幂等键：
/// 同一令牌重放在任务已达终态时直接回读既有结果，不会重复写入 K 线。
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
/// 应用层会先确认策略存在，因此策略不存在返回未找到，而策略存在但无任务返回空列表，二者可以区分。
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

/// 处理 POST /market-strategies，新建模拟行情策略并一次性建立首个版本与运行检查点。
/// 应用层在同一事务内插入策略主表、价格节点、版本 1 快照和运行行，运行行的活动版本外键指向刚建的版本。
/// 初始状态缺省为 draft，创建只落库而不会直接拉起策略 worker，需要另行切换状态才会开始产出行情。
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

/// 处理 PATCH /market-strategies/:id，整体更新策略配置与价格节点并追加一个新版本。
/// 请求必须携带审计原因；应用层锁定策略后若发现状态仍是 active 会直接返回冲突，必须先暂停或停用再改。
/// 每次成功调用都会替换节点、生成新版本号并重置运行检查点，因此重复提交同样内容不是幂等操作。
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

/// 处理 PATCH /market-strategies/:id/status，在 draft、active、paused、disabled 之间切换策略状态。
/// 这是唯一能把策略切到 active 的入口，且与配置更新入口不同，它不要求显式审计原因也不施加迁移图约束。
/// 应用层锁定后同时更新业务状态与派生的运行状态，运行检查点缺失时整体回滚；状态生效后由运行组件自行观察数据库。
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
