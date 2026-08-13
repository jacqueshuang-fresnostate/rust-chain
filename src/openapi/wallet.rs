//! 钱包的 OpenAPI 契约：覆盖用户端充值地址与提现申请，以及后台的提现审核与充值地址池管理。
//! 提现按状态机推进，审核通过、驳回、广播、确认与失败各对应一个端点，状态不匹配统一返回冲突。
//! 资金在申请时冻结，只有确认成功才最终扣除；驳回与广播前失败会解冻退回，已广播的不允许自动解冻。
//! 链上充值以观察事件的方式幂等入账，链重组则通过冲正端点处理，余额不足时转入人工处理。

use super::*;

#[derive(ToSchema)]
pub(super) struct CreateWithdrawalRequest {
    asset_symbol: String,
    network: Option<String>,
    address: String,
    amount: String,
    fee: String,
    idempotency_key: String,
    fund_password: Option<String>,
    totp_code: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct WithdrawalRequestResponse {
    id: u64,
    status: String,
    total_reserved: String,
    security_method: SecurityVerificationMethod,
}

#[derive(ToSchema)]
pub(super) struct WalletWithdrawalResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    network: Option<String>,
    address: String,
    amount: String,
    fee: String,
    total_reserved: String,
    status: String,
    security_method: String,
    idempotency_key: String,
    gateway_request_id: String,
    tx_hash: Option<String>,
    block_height: Option<u64>,
    confirmations: u32,
    failure_reason: Option<String>,
    review_reason: Option<String>,
    reviewed_by: Option<u64>,
    broadcasted_by: Option<u64>,
    confirmed_by: Option<u64>,
    failed_by: Option<u64>,
    #[schema(format = Int64)]
    reviewed_at: Option<i64>,
    #[schema(format = Int64)]
    broadcast_at: Option<i64>,
    #[schema(format = Int64)]
    confirmed_at: Option<i64>,
    #[schema(format = Int64)]
    failed_at: Option<i64>,
    #[schema(format = Int64)]
    released_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct WalletWithdrawalsResponse {
    withdrawals: Vec<WalletWithdrawalResponse>,
}

#[derive(ToSchema)]
pub(super) struct AdminWalletWithdrawalsResponse {
    withdrawals: Vec<WalletWithdrawalResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct ReviewWithdrawalRequest {
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct BroadcastWithdrawalRequest {
    tx_hash: String,
    block_height: Option<u64>,
    confirmations: Option<u32>,
}

#[derive(ToSchema)]
pub(super) struct ConfirmWithdrawalRequest {
    block_height: Option<u64>,
    confirmations: Option<u32>,
}

#[derive(ToSchema)]
pub(super) struct FailWithdrawalRequest {
    reason: String,
}

#[derive(ToSchema)]
pub(super) struct ObserveDepositRequest {
    asset_symbol: String,
    network: String,
    address: String,
    memo: Option<String>,
    tx_hash: String,
    event_index: u32,
    amount: String,
    block_height: Option<u64>,
    confirmations: u32,
}

#[derive(ToSchema)]
pub(super) struct ReverseDepositRequest {
    reason: String,
}

#[derive(ToSchema)]
pub(super) struct WalletDepositEventResponse {
    id: u64,
    user_id: u64,
    asset_id: u64,
    asset_symbol: String,
    network: String,
    address: String,
    memo: Option<String>,
    tx_hash: String,
    event_index: u32,
    amount: String,
    block_height: Option<u64>,
    confirmations: u32,
    required_confirmations: u32,
    status: String,
    failure_reason: Option<String>,
    #[schema(format = Int64)]
    credited_at: Option<i64>,
    #[schema(format = Int64)]
    reversed_at: Option<i64>,
    #[schema(format = Int64)]
    created_at: i64,
}

#[derive(ToSchema)]
pub(super) struct WalletDepositsResponse {
    deposits: Vec<WalletDepositEventResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct WithdrawFeeTierResponse {
    min_amount: String,
    max_amount: Option<String>,
    fee_rate_percent: String,
}

#[derive(ToSchema)]
pub(super) struct DepositAssetResponse {
    symbol: String,
    name: String,
    logo_url: Option<String>,
    precision_scale: i32,
    deposit_enabled: bool,
    withdraw_enabled: bool,
    min_deposit_amount: String,
    deposit_fee: String,
    withdraw_fee: String,
    withdraw_fee_tiers: Vec<WithdrawFeeTierResponse>,
}

#[derive(ToSchema)]
pub(super) struct DepositAssetsResponse {
    assets: Vec<DepositAssetResponse>,
}

#[derive(ToSchema)]
pub(super) struct DepositAddressRequest {
    asset_symbol: String,
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
}

#[derive(ToSchema)]
pub(super) struct DepositAddressResponse {
    id: u64,
    asset_symbol: String,
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    memo: Option<String>,
    #[schema(format = Int64)]
    assigned_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminDepositAddressPoolResponse {
    id: u64,
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    asset_symbol: Option<String>,
    asset_symbols: Vec<String>,
    #[schema(pattern = "^(available|assigned|disabled)$")]
    status: String,
    assigned_user_id: Option<u64>,
    assigned_user_email: Option<String>,
    assigned_asset_symbol: Option<String>,
    #[schema(format = Int64)]
    assigned_at: Option<i64>,
    memo: Option<String>,
    remark: Option<String>,
    #[schema(format = Int64)]
    created_at: i64,
    #[schema(format = Int64)]
    updated_at: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminDepositAddressPoolResponseList {
    addresses: Vec<AdminDepositAddressPoolResponse>,
    total: i64,
}

#[derive(ToSchema)]
pub(super) struct AdminDepositAddressPoolBatchResponse {
    addresses: Vec<AdminDepositAddressPoolResponse>,
}

#[derive(ToSchema)]
pub(super) struct CreateDepositAddressPoolRequest {
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
    #[schema(pattern = "^(available|disabled)$")]
    status: Option<String>,
    memo: Option<String>,
    remark: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct CreateDepositAddressPoolEntryRequest {
    address: String,
    memo: Option<String>,
    remark: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct CreateDepositAddressPoolBatchRequest {
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
    #[schema(pattern = "^(available|disabled)$")]
    status: Option<String>,
    entries: Vec<CreateDepositAddressPoolEntryRequest>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct UpdateDepositAddressPoolRequest {
    #[schema(pattern = "^(eth|base|tron|btc|solana)$")]
    network: String,
    address: String,
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
    #[schema(pattern = "^(available|disabled)$")]
    status: String,
    memo: Option<String>,
    remark: Option<String>,
    reason: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct ReclaimDepositAddressPoolRequest {
    reason: Option<String>,
}

/// 返回当前开放普通链上充值的资产清单，供充值页渲染可选币种与网络。
/// 清单由后台资产配置决定，未开放充值的资产不会出现，前端不需要再做一次过滤。
#[utoipa::path(
    get,
    path = "/api/v1/wallet/deposit-assets",
    tag = "wallet",
    summary = "查询当前支持普通充值的资产",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = DepositAssetsResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_deposit_assets() {}

/// 返回当前开放提现的资产清单，与充值清单各自独立，可能出现只能充值不能提现的资产。
/// 响应结构复用充值资产的定义，字段含义一致，差别只在筛选口径。
#[utoipa::path(
    get,
    path = "/api/v1/wallet/withdraw-assets",
    tag = "wallet",
    summary = "查询当前支持提现的资产",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = DepositAssetsResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_withdraw_assets() {}

/// 获取当前用户在指定资产与网络下的充值地址，已绑定过则原样返回，不会重新分配。
/// 尚未绑定时从地址池取一个可用地址并绑定，地址池耗尽或资产不存在都返回资源不存在。
#[utoipa::path(
    post,
    path = "/api/v1/wallet/deposit-address",
    tag = "wallet",
    summary = "从地址池获取或申请充值地址",
    request_body = DepositAddressRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "申请成功，若用户已有绑定则返回原地址", body = DepositAddressResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "资产不存在或地址池无可用地址", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_or_assign_deposit_address() {}

/// 提交提现申请，按后台安全策略校验资金密码等要件后冻结相应资金并进入待审核状态。
/// 资金密码错误返回未认证，安全要件缺失返回参数错误；本步只冻结资金，不做任何链上广播。
#[utoipa::path(
    post,
    path = "/api/v1/wallet/withdrawals",
    tag = "wallet",
    summary = "创建提现申请并按后台策略完成安全校验",
    request_body = CreateWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "创建成功", body = WithdrawalRequestResponse),
        (status = 400, description = "参数错误或安全校验缺失", body = ErrorResponse),
        (status = 401, description = "未登录或资金密码错误", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_withdrawal_request() {}

/// 查询当前用户自己的提现申请，可按状态过滤并限制返回条数，用于提现记录页。
/// 只返回令牌对应用户的数据，不接受用户标识参数，因此无法借此查看他人提现。
#[utoipa::path(
    get,
    path = "/api/v1/wallet/withdrawals",
    tag = "wallet",
    summary = "查询当前用户提现申请",
    params(
        ("status" = Option<String>, Query, description = "提现状态"),
        ("limit" = Option<u32>, Query, description = "返回数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = WalletWithdrawalsResponse),
        (status = 400, description = "状态参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_user_withdrawals() {}

/// 后台分页查询全站提现申请，可按状态与用户过滤，是提现审核工作台的数据来源。
/// 返回的后台视图比用户端多出审核相关字段，需要后台作用域令牌才能访问。
#[utoipa::path(
    get,
    path = "/admin/api/v1/wallet/withdrawals",
    tag = "admin-wallet",
    summary = "后台查询提现申请",
    params(
        ("status" = Option<String>, Query, description = "提现状态"),
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("limit" = Option<u32>, Query, description = "返回数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminWalletWithdrawalsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "无后台权限", body = ErrorResponse)
    )
)]
fn list_admin_wallet_withdrawals() {}

/// 审核通过一笔提现申请，使其从待审核推进到可广播状态，资金保持冻结不做实际扣减。
/// 当前状态不允许该迁移时返回冲突，避免重复审核导致状态回退。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/withdrawals/{id}/approve",
    tag = "admin-wallet",
    summary = "审核通过提现申请",
    params(("id" = u64, Path, description = "提现申请 ID")),
    request_body = ReviewWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "审核成功", body = WalletWithdrawalResponse),
        (status = 409, description = "状态冲突", body = ErrorResponse)
    )
)]
fn approve_admin_wallet_withdrawal() {}

/// 驳回提现申请并把先前冻结的资金解冻退回用户可用余额，是审核环节的终态之一。
/// 状态不允许驳回时返回冲突，防止对已经广播的申请误做解冻。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/withdrawals/{id}/reject",
    tag = "admin-wallet",
    summary = "拒绝提现并解冻资金",
    params(("id" = u64, Path, description = "提现申请 ID")),
    request_body = ReviewWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "拒绝成功", body = WalletWithdrawalResponse),
        (status = 409, description = "状态冲突", body = ErrorResponse)
    )
)]
fn reject_admin_wallet_withdrawal() {}

/// 登记提现已在链上广播的结果，通常需要带上交易哈希，使申请进入等待确认阶段。
/// 登记之后资金不能再自动解冻，后续只能走确认成功或转人工处理两条路。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/withdrawals/{id}/broadcast",
    tag = "admin-wallet",
    summary = "登记提现链上广播结果",
    params(("id" = u64, Path, description = "提现申请 ID")),
    request_body = BroadcastWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "登记成功", body = WalletWithdrawalResponse),
        (status = 409, description = "状态冲突", body = ErrorResponse)
    )
)]
fn broadcast_admin_wallet_withdrawal() {}

/// 确认提现在链上最终成功，把此前冻结的资金正式扣除，完成整笔提现的资金闭环。
/// 这是不可逆的终态操作，状态不符时返回冲突，不会造成重复扣款。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/withdrawals/{id}/confirm",
    tag = "admin-wallet",
    summary = "确认提现并最终扣除冻结资金",
    params(("id" = u64, Path, description = "提现申请 ID")),
    request_body = ConfirmWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "确认成功", body = WalletWithdrawalResponse),
        (status = 409, description = "状态冲突", body = ErrorResponse)
    )
)]
fn confirm_admin_wallet_withdrawal() {}

/// 把尚未广播的提现标记为失败并解冻资金，用于签名失败等链下环节出错的情形。
/// 已广播的申请不允许走这条自动解冻路径，会直接返回冲突要求人工核对链上状态。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/withdrawals/{id}/fail",
    tag = "admin-wallet",
    summary = "标记广播前提现失败并解冻资金",
    params(("id" = u64, Path, description = "提现申请 ID")),
    request_body = FailWithdrawalRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "失败处理成功", body = WalletWithdrawalResponse),
        (status = 409, description = "已广播请求不可自动解冻", body = ErrorResponse)
    )
)]
fn fail_admin_wallet_withdrawal() {}

/// 后台分页查询链上充值事件，可按用户过滤，用于核对入账情况与排查漏充。
/// 返回的是充值事件记录本身，包含确认数与处理状态，而不是钱包余额流水。
#[utoipa::path(
    get,
    path = "/admin/api/v1/wallet/deposits",
    tag = "admin-wallet",
    summary = "后台查询链上充值事件",
    params(
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("limit" = Option<u32>, Query, description = "返回数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses((status = 200, description = "查询成功", body = WalletDepositsResponse))
)]
fn list_admin_wallet_deposits() {}

/// 登记一笔观察到的链上充值，达到确认数要求后入账，重复提交同一链上事件不会重复加款。
/// 幂等依据是链上事件身份，同一身份对应不同内容时返回冲突，而不是覆盖既有记录。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/deposits/observe",
    tag = "admin-wallet",
    summary = "观察链上充值并按确认数幂等入账",
    request_body = ObserveDepositRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "观察成功", body = WalletDepositEventResponse),
        (status = 400, description = "链事件参数错误", body = ErrorResponse),
        (status = 409, description = "外部事件身份冲突", body = ErrorResponse)
    )
)]
fn observe_admin_wallet_deposit() {}

/// 在链重组导致已入账充值失效时发起冲正，可能直接扣回，也可能因余额不足转入人工处理。
/// 充值当前状态不允许冲正时返回冲突，避免对未入账或已冲正的事件重复操作。
#[utoipa::path(
    post,
    path = "/admin/api/v1/wallet/deposits/{id}/reverse",
    tag = "admin-wallet",
    summary = "链重组充值冲正",
    params(("id" = u64, Path, description = "充值事件 ID")),
    request_body = ReverseDepositRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "冲正成功或进入人工处理", body = WalletDepositEventResponse),
        (status = 409, description = "充值状态不允许冲正", body = ErrorResponse)
    )
)]
fn reverse_admin_wallet_deposit() {}

/// 分页查询充值地址池，支持按网络、状态、限定资产、绑定用户与地址片段组合筛选。
/// 主要用于运维盘点可用地址存量，及时补充以免用户申请地址时无址可分。
#[utoipa::path(
    get,
    path = "/admin/api/v1/deposit-address-pool",
    tag = "admin-wallet",
    summary = "查询充值地址池",
    params(
        ("network" = Option<String>, Query, description = "网络：eth/base/tron/btc/solana"),
        ("status" = Option<String>, Query, description = "状态：available/assigned/disabled"),
        ("asset_symbol" = Option<String>, Query, description = "限定资产或已分配资产符号"),
        ("assigned_user_id" = Option<u64>, Query, description = "绑定用户 ID"),
        ("email" = Option<String>, Query, description = "绑定用户邮箱"),
        ("address" = Option<String>, Query, description = "地址模糊查询"),
        ("limit" = Option<u32>, Query, description = "分页数量"),
        ("offset" = Option<u32>, Query, description = "分页偏移")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminDepositAddressPoolResponseList),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn list_admin_deposit_address_pool() {}

/// 向地址池新增单个充值地址，必须填写审计原因，同一网络下地址重复会返回冲突。
/// 可限定该地址只用于特定资产，被限定的资产必须已存在，否则返回资源不存在。
#[utoipa::path(
    post,
    path = "/admin/api/v1/deposit-address-pool",
    tag = "admin-wallet",
    summary = "新增充值地址池地址",
    request_body = CreateDepositAddressPoolRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "新增成功", body = AdminDepositAddressPoolResponse),
        (status = 400, description = "参数错误或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "限定资产不存在", body = ErrorResponse),
        (status = 409, description = "同网络地址已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_deposit_address_pool() {}

/// 批量导入充值地址，一次提交多条记录，适合从冷钱包集中生成大量地址后统一入库。
/// 与单条新增遵循同样的重复校验与审计原因要求，任一条不合法都会导致整批失败。
#[utoipa::path(
    post,
    path = "/admin/api/v1/deposit-address-pool/batch",
    tag = "admin-wallet",
    summary = "批量新增充值地址池地址",
    request_body = CreateDepositAddressPoolBatchRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "新增成功", body = AdminDepositAddressPoolBatchResponse),
        (status = 400, description = "参数错误或缺少审计原因", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "限定资产不存在", body = ErrorResponse),
        (status = 409, description = "同网络地址已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn create_admin_deposit_address_pool_batch() {}

/// 按主键查询单个地址池条目的详情，包括所属网络、当前状态与绑定用户信息。
/// 供运维核对某个具体地址的归属，地址不存在时返回资源不存在。
#[utoipa::path(
    get,
    path = "/admin/api/v1/deposit-address-pool/{id}",
    tag = "admin-wallet",
    summary = "查询充值地址池详情",
    params(("id" = u64, Path, description = "地址池 ID")),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = AdminDepositAddressPoolResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "地址不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn get_admin_deposit_address_pool() {}

/// 修改尚未分配出去的地址池条目，可调整网络、地址、限定资产、状态与备注。
/// 已分配给用户的地址禁止修改并按参数错误拒绝，防止改动影响用户正在使用的收款地址。
#[utoipa::path(
    patch,
    path = "/admin/api/v1/deposit-address-pool/{id}",
    tag = "admin-wallet",
    summary = "修改未分配的充值地址池地址",
    params(("id" = u64, Path, description = "地址池 ID")),
    request_body = UpdateDepositAddressPoolRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "修改成功", body = AdminDepositAddressPoolResponse),
        (status = 400, description = "参数错误、缺少审计原因或地址已分配", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "地址或限定资产不存在", body = ErrorResponse),
        (status = 409, description = "同网络地址已存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn update_admin_deposit_address_pool() {}

/// 回收已分配的充值地址，解除它与用户的绑定使其重新可分配，必须填写审计原因。
/// 地址不处于已分配状态时按参数错误拒绝；回收属于高风险操作，需先确认用户不再使用该地址。
#[utoipa::path(
    post,
    path = "/admin/api/v1/deposit-address-pool/{id}/reclaim",
    tag = "admin-wallet",
    summary = "回收已分配充值地址",
    params(("id" = u64, Path, description = "地址池 ID")),
    request_body = ReclaimDepositAddressPoolRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "回收成功", body = AdminDepositAddressPoolResponse),
        (status = 400, description = "缺少审计原因或地址未分配", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "鉴权 scope 不匹配", body = ErrorResponse),
        (status = 404, description = "地址不存在", body = ErrorResponse),
        (status = 500, description = "服务内部错误", body = ErrorResponse)
    )
)]
fn reclaim_admin_deposit_address_pool() {}
