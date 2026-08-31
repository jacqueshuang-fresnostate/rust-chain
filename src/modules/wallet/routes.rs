//! wallet bounded context HTTP 路由层。
//!
//! 把钱包用例挂到 Axum 路由树上，分成用户端与后台两组：用户端只能操作自己的资金，后台可跨用户审核充提。
//! 处理器统一遵循同一骨架：先由提取器完成鉴权，再从令牌主体解析用户或管理员编号，取连接池，最后转交应用层用例。
//! 用户编号一律取自令牌而非请求体或路径，因此用户端接口不存在通过参数越权读取他人钱包的路径。
//! 本层不做业务判断、不开启事务、不访问数据库，参数校验与资金语义全部由应用层和基础设施承担。

use crate::{
    error::AppResult,
    modules::{
        auth::{AdminAuth, UserAuth},
        user::service::user_id_from_subject,
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};

use super::{
    application::{
        admin_id_from_subject, approve_withdrawal as approve_withdrawal_use_case,
        broadcast_withdrawal as broadcast_withdrawal_use_case, build_wallet_ledger_filter,
        confirm_withdrawal as confirm_withdrawal_use_case,
        create_withdrawal_quote as create_withdrawal_quote_use_case,
        create_withdrawal_request as create_withdrawal_request_use_case,
        fail_withdrawal as fail_withdrawal_use_case,
        get_or_assign_deposit_address as get_or_assign_deposit_address_use_case,
        get_return_history as get_return_history_use_case,
        get_today_return as get_today_return_use_case,
        list_admin_deposits as list_admin_deposits_use_case,
        list_admin_withdrawals as list_admin_withdrawals_use_case,
        list_deposit_assets as list_deposit_assets_use_case,
        list_deposit_networks_by_query as list_deposit_networks_use_case,
        list_user_withdrawals as list_user_withdrawals_use_case,
        list_wallet_accounts as list_wallet_accounts_use_case,
        list_wallet_ledger as list_wallet_ledger_use_case,
        list_withdraw_assets as list_withdraw_assets_use_case, mysql_pool,
        normalize_deposit_networks_query_asset, observe_deposit as observe_deposit_use_case,
        reject_withdrawal as reject_withdrawal_use_case,
        reverse_deposit as reverse_deposit_use_case, validate_return_history_days,
    },
    presentation::{
        AdminWalletListQuery, AdminWalletWithdrawalsResponse, BroadcastWithdrawalRequest,
        ConfirmWithdrawalRequest, CreateWithdrawalQuoteRequest, CreateWithdrawalRequest,
        DepositAddressRequest, DepositAddressResponse, DepositAssetsResponse, DepositNetworksQuery,
        DepositNetworksResponse, FailWithdrawalRequest, ObserveDepositRequest, ReturnHistoryQuery,
        ReturnHistoryResponse, ReverseDepositRequest, ReviewWithdrawalRequest, TodayReturnResponse,
        WalletAccountsResponse, WalletDepositEventResponse, WalletDepositsResponse,
        WalletLedgerQuery, WalletLedgerResponse, WalletWithdrawalQuery, WalletWithdrawalResponse,
        WalletWithdrawalsResponse, WithdrawalQuoteResponse, WithdrawalRequestResponse,
    },
};

/// 组装用户端钱包路由：账户余额、今日收益、收益历史、资金流水、充提资产与网络、充值地址和提现申请。
/// 提现路径同时挂载查询与创建两个方法，前者读本人申请列表，后者发起冻结并落申请。
/// 全部处理器都要求用户令牌，路由本身不附加额外中间件，限流与审计由上层路由树统一装配。
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wallet/accounts", get(list_accounts))
        .route("/wallet/today-return", get(get_today_return))
        .route("/wallet/return-history", get(get_return_history))
        .route("/wallet/ledger", get(list_ledger))
        .route("/wallet/deposit-assets", get(list_deposit_assets))
        .route("/wallet/deposit-networks", get(list_deposit_networks))
        .route("/wallet/withdraw-assets", get(list_withdraw_assets))
        .route(
            "/wallet/deposit-address",
            post(get_or_assign_deposit_address),
        )
        .route(
            "/wallet/withdrawals",
            get(list_user_withdrawals).post(create_withdrawal_request),
        )
        .route("/wallet/withdrawals/quote", post(create_withdrawal_quote))
}

/// 组装后台钱包路由：提现分页查询与审批、拒绝、广播补录、确认、失败五个状态迁移，以及充值事件查询、观测与冲正。
/// 状态迁移和冲正均按资源编号走路径参数，写操作全部使用 POST，不提供批量接口以保证每次资金动作可单独审计。
/// 全部处理器都要求后台令牌，跨用户可见性由管理员权限承担，路由层不再按用户过滤。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/wallet/withdrawals", get(list_admin_withdrawals))
        .route("/wallet/withdrawals/:id/approve", post(approve_withdrawal))
        .route("/wallet/withdrawals/:id/reject", post(reject_withdrawal))
        .route(
            "/wallet/withdrawals/:id/broadcast",
            post(broadcast_withdrawal),
        )
        .route("/wallet/withdrawals/:id/confirm", post(confirm_withdrawal))
        .route("/wallet/withdrawals/:id/fail", post(fail_withdrawal))
        .route("/wallet/deposits", get(list_admin_deposits))
        .route("/wallet/deposits/observe", post(observe_deposit))
        .route("/wallet/deposits/:id/reverse", post(reverse_deposit))
}

/// 处理充值地址申请：解析令牌用户，把资产与网络交给分配用例，返回复用或新分配的地址。
/// 用例内部保证同一用户同一地址组不会轮换地址，因此重复调用通常返回同一条记录。
/// 地址池耗尽会由用例返回未找到，路由层不做重试也不降级到其他网络。
async fn get_or_assign_deposit_address(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<DepositAddressRequest>,
) -> AppResult<Json<DepositAddressResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let address = get_or_assign_deposit_address_use_case(&pool, user_id, request).await?;

    Ok(Json(address))
}

/// 返回可充值资产清单。这里只校验令牌合法性并丢弃解析出的用户编号，因为资产配置对所有登录用户一致。
/// 响应按资产维度给出精度、最小充值额与费用，供前端渲染充值币种选择页。
async fn list_deposit_assets(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<DepositAssetsResponse>> {
    user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let assets = list_deposit_assets_use_case(&pool).await?;

    Ok(Json(DepositAssetsResponse { assets }))
}

/// 返回可用充值网络清单，可按资产代码过滤。同样只验令牌不关心具体用户。
/// 这里先单独执行一次资产代码归一以尽早暴露参数错误，随后用例会对同一查询再归一一次，两次口径完全一致。
/// 响应中的地址组代码说明哪些网络共用同一地址池，前端可据此提示地址可跨网络接收。
async fn list_deposit_networks(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<DepositNetworksQuery>,
) -> AppResult<Json<DepositNetworksResponse>> {
    user_id_from_subject(&claims.sub)?;
    let _ = normalize_deposit_networks_query_asset(&query)?;
    let pool = mysql_pool(&state)?;
    let networks = list_deposit_networks_use_case(&pool, &query).await?;

    Ok(Json(DepositNetworksResponse { networks }))
}

/// 返回可提现资产清单及其固定费用与阶梯费率，用于提现表单的币种选择与费用预估。
/// 与充值清单结构相同但过滤开关不同，因此同一资产可能只出现在其中一侧。
/// 预估费用不具约束力，真实扣费在创建申请时由服务端重新计算并以服务端结果为准。
async fn list_withdraw_assets(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<DepositAssetsResponse>> {
    user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let assets = list_withdraw_assets_use_case(&pool).await?;

    Ok(Json(DepositAssetsResponse { assets }))
}

/// 返回当前登录用户全部资产账户的可用、冻结与锁定三桶余额，按资产代码升序排列。
/// 用户编号取自令牌，接口不接受任何查询参数，因此不存在读取他人余额的入口。
/// 返回的是无锁快照，仅供展示；下单或提现时服务端会在事务内重新锁行核对余额。
async fn list_accounts(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<WalletAccountsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let accounts = list_wallet_accounts_use_case(&pool, user_id).await?;

    Ok(Json(WalletAccountsResponse { accounts }))
}

/// 返回当前用户当日已实现收益，统计区间为服务器 UTC 自然日零点到此刻，以 USDT 计价。
/// Redis 句柄按可选依赖传入，缺失或行情过期时非稳定币资产会缺价，响应状态退化为 partial 并列出缺价资产。
/// 该接口只读结算事实与行情，不锁钱包也不写流水，重复调用不产生任何资金影响。
async fn get_today_return(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<TodayReturnResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = get_today_return_use_case(&pool, state.redis.as_ref(), user_id).await?;

    Ok(Json(response))
}

/// 返回当前用户指定窗口的逐日已实现收益曲线，窗口天数在进入用例前先行校验，只接受一、七、三十或一百八十。
/// 历史日估值取 Mongo 的 UTC 日线收盘价，当日取 Redis 实时价，两个依赖都按可选传入，缺失时对应日期缺价。
/// 任一日缺价会让该日及其后的累计值置空且总摘要为空，响应整体标记 partial，前端不得把空值当作零收益。
async fn get_return_history(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ReturnHistoryQuery>,
) -> AppResult<Json<ReturnHistoryResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let period_days = validate_return_history_days(query.days)?;
    let pool = mysql_pool(&state)?;
    let response = get_return_history_use_case(
        &pool,
        state.mongo.as_ref(),
        state.redis.as_ref(),
        user_id,
        period_days,
    )
    .await?;

    Ok(Json(response))
}

/// 返回当前用户的资金流水分页，支持按现货/杠杆账户、资产、变更类型、业务分类、引用和时间范围筛选。
/// 查询参数先构建成过滤器，非法账户类型、分类或资产代码在取连接池前就返回校验错误。
/// 响应每条流水都带变更金额、所属余额桶和三桶账后快照，可据此逐笔还原余额变化。
async fn list_ledger(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<WalletLedgerQuery>,
) -> AppResult<Json<WalletLedgerResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let filter = build_wallet_ledger_filter(query)?;
    let pool = mysql_pool(&state)?;
    let ledger = list_wallet_ledger_use_case(&pool, user_id, filter).await?;

    Ok(Json(ledger))
}

/// 受理提现申请，是本路由组中唯一会冻结用户资金的入口。
/// 除连接池外还需传入全局配置，因为用例要据此完成资金密码或两步验证等安全校验。
/// 用例会依次执行参数校验、费用计算、幂等重放、风控与安全校验，通过后在单事务内落申请并把本金加费用从可用转入冻结。
/// 请求体中的幂等键决定重放语义：参数完全一致返回既有申请，任一关键参数不同则返回冲突，绝不重复冻结。
async fn create_withdrawal_request(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateWithdrawalRequest>,
) -> AppResult<Json<WithdrawalRequestResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let withdrawal =
        create_withdrawal_request_use_case(&pool, state.settings.as_ref(), user_id, request)
            .await?;
    Ok(Json(withdrawal))
}

/// 为当前用户持久化一份有限期的权威提现报价，后续提交必须消费返回的 quote_id。
async fn create_withdrawal_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateWithdrawalQuoteRequest>,
) -> AppResult<Json<WithdrawalQuoteResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    Ok(Json(
        create_withdrawal_quote_use_case(&pool, user_id, request).await?,
    ))
}

/// 返回当前用户的提现申请列表，可按状态筛选并限制条数，结果按申请编号倒序。
/// 用户编号强制取自令牌覆盖查询条件，即使请求携带其他用户编号也无法读到他人申请。
/// 该接口只读申请与链上进度快照，不返回总数、不翻页，也不触发任何状态迁移。
async fn list_user_withdrawals(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<WalletWithdrawalQuery>,
) -> AppResult<Json<WalletWithdrawalsResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let withdrawals = list_user_withdrawals_use_case(&mysql_pool(&state)?, user_id, query).await?;
    Ok(Json(WalletWithdrawalsResponse { withdrawals }))
}

/// 后台提现分页查询，可按用户与状态筛选并返回匹配总数，供运营翻页审阅。
/// 这里不解析管理员编号，因为纯读取无需记录操作人，鉴权由后台令牌提取器完成。
/// 用户条件缺省时跨用户返回全量申请，状态取值非法则在查库前返回校验错误。
async fn list_admin_withdrawals(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletListQuery>,
) -> AppResult<Json<AdminWalletWithdrawalsResponse>> {
    Ok(Json(
        list_admin_withdrawals_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

/// 后台批准提现：把待审核申请推进为已批准，冻结额原地保留等待链网关广播。
/// 管理员编号从后台令牌主体解析并记为审核人，请求体只提供可选审核意见，不得携带操作人身份。
/// 批准会重置下次尝试时刻，使广播 worker 在下一轮即可认领，因此这一步等同于放行上链。
async fn approve_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<ReviewWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        approve_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

/// 后台拒绝提现：把待审核或已批准申请置为已拒绝，并把本金加费用的全额冻结退回可用余额。
/// 审核意见在此为必填，缺失或超长返回校验错误，退款结果与拒绝状态在同一事务内提交。
/// 已经进入广播的申请无法由此拒绝，只能走失败或人工审核路径，避免链上已发出却在本地退款。
async fn reject_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<ReviewWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reject_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

/// 后台补录提现的链上交易哈希与确认进度，用于自动广播缺失或回执迟迟未回的场景。
/// 本接口不调用链网关、不发出真实交易，只把外部已确认存在的哈希写进申请记录。
/// 冻结额在此保持不动，资金核销要等到确认接口或链回执把状态推进到已确认才发生。
async fn broadcast_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<BroadcastWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        broadcast_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request)
            .await?,
    ))
}

/// 后台确认提现到账，核销冻结中的预留额并写确认流水，是资金真正离开钱包的一步。
/// 只接受已广播或人工审核状态；确认数缺省按一处理，区块高度可缺省，两者写入时都不会让链上进度倒退。
/// 已确认申请重复调用幂等返回，不会二次扣减冻结余额。
async fn confirm_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<ConfirmWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        confirm_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

/// 后台判定提现失败并全额退回冻结资金，适用于已批准或广播中但确认未能上链的申请。
/// 失败原因为请求体必填字段，与拒绝共用同一释放实现，因此退款金额同样是本金加费用的完整预留额。
/// 已经取得链上交易哈希的申请不适用本接口，应先转人工审核，避免链上成功与本地退款并存。
async fn fail_withdrawal(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(withdrawal_id): Path<u64>,
    Json(request): Json<FailWithdrawalRequest>,
) -> AppResult<Json<WalletWithdrawalResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        fail_withdrawal_use_case(&mysql_pool(&state)?, admin_id, withdrawal_id, request).await?,
    ))
}

/// 后台充值链事件分页查询，可按用户筛选并返回匹配总数，用于核对链上到账与入账状态。
/// 与后台提现列表同样不解析管理员编号，因为只读查询无需登记操作人。
/// 该接口不触发入账或冲正，也不推进链网关游标，返回的确认数与状态均为当前存量快照。
async fn list_admin_deposits(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletListQuery>,
) -> AppResult<Json<WalletDepositsResponse>> {
    Ok(Json(
        list_admin_deposits_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

/// 后台手工上报一笔链上充值观测，与链网关 worker 走完全相同的幂等入账用例。
/// 事件身份由网络、交易哈希和事件序号唯一确定，重复上报只单调推进确认数，达到阈值时才首次增加可用余额。
/// 同一身份若带着不同地址、金额或备注再次上报，会返回冲突而不是覆盖既有事件。
/// 请求体不含用户编号，入账对象由链上地址反查已分配用户决定，因此地址未分配时直接返回未找到。
async fn observe_deposit(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<ObserveDepositRequest>,
) -> AppResult<Json<WalletDepositEventResponse>> {
    Ok(Json(
        observe_deposit_use_case(&mysql_pool(&state)?, request).await?,
    ))
}

/// 后台对已入账充值执行链重组冲正，按事件编号扣回原到账金额并写负向流水。
/// 冲正原因为必填项，会随状态一并存档；已冲正的事件重复调用直接幂等返回。
/// 若用户可用余额已不足以扣回，用例不会扣任何余额，而是把事件转入人工审核并记录原因，保留处置痕迹。
async fn reverse_deposit(
    AdminAuth(_claims): AdminAuth,
    State(state): State<AppState>,
    Path(deposit_id): Path<u64>,
    Json(request): Json<ReverseDepositRequest>,
) -> AppResult<Json<WalletDepositEventResponse>> {
    Ok(Json(
        reverse_deposit_use_case(&mysql_pool(&state)?, deposit_id, request).await?,
    ))
}
#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_routes_tests.rs"]
mod tests;
