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

#[utoipa::path(
    get,
    path = "/admin/api/v1/wallet/withdrawals",
    tag = "admin-wallet",
    summary = "后台查询提现申请",
    params(
        ("status" = Option<String>, Query, description = "提现状态"),
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("limit" = Option<u32>, Query, description = "返回数量")
    ),
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "查询成功", body = WalletWithdrawalsResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 401, description = "未登录", body = ErrorResponse),
        (status = 403, description = "无后台权限", body = ErrorResponse)
    )
)]
fn list_admin_wallet_withdrawals() {}

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

#[utoipa::path(
    get,
    path = "/admin/api/v1/wallet/deposits",
    tag = "admin-wallet",
    summary = "后台查询链上充值事件",
    params(
        ("user_id" = Option<u64>, Query, description = "用户 ID"),
        ("limit" = Option<u32>, Query, description = "返回数量")
    ),
    security(("bearerAuth" = [])),
    responses((status = 200, description = "查询成功", body = WalletDepositsResponse))
)]
fn list_admin_wallet_deposits() {}

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
        ("limit" = Option<u32>, Query, description = "分页数量")
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
