//! 提现策略管理和用户地址登记的公开合同，金额为原资产十进制字符串。
use super::*;

#[derive(ToSchema)]
pub(super) struct WithdrawalAllowance {
    user_id: Option<u64>,
    kyc_level: Option<i32>,
    #[schema(example = "rolling")]
    window: String,
    window_seconds: u32,
    #[schema(example = "all_outstanding")]
    pending_mode: String,
    max_amount: String,
}

#[derive(ToSchema)]
pub(super) struct WithdrawalReviewTier {
    min_amount: String,
    required_approvals: u32,
}

#[derive(ToSchema)]
pub(super) struct WithdrawalPolicy {
    enabled: bool,
    #[schema(example = "principal")]
    amount_basis: String,
    allowances: Vec<WithdrawalAllowance>,
    address_cooling_seconds: Option<u32>,
    security_cooling_seconds: Option<u32>,
    review_tiers: Vec<WithdrawalReviewTier>,
}

#[derive(ToSchema)]
pub(super) struct WithdrawalPolicyResponse {
    asset_id: u64,
    revision: u64,
    policy: WithdrawalPolicy,
}

#[derive(ToSchema)]
pub(super) struct SaveWithdrawalPolicyRequest {
    expected_revision: u64,
    policy: WithdrawalPolicy,
    reason: String,
}

#[derive(ToSchema)]
pub(super) struct RegisterWithdrawalAddressRequest {
    network: String,
    address: String,
    fund_password: Option<String>,
    totp_code: Option<String>,
}

#[derive(ToSchema)]
pub(super) struct WithdrawalAddressResponse {
    id: u64,
    network: String,
    address: String,
    #[schema(format = DateTime)]
    created_at: String,
}

#[utoipa::path(get, path = "/admin/api/v1/wallet/withdrawal-policies/{asset_id}", tag = "admin",
    params(("asset_id" = u64, Path)),
    security(("bearerAuth" = [])),
    responses((status = 200, body = WithdrawalPolicyResponse)))]
pub(super) fn get_withdrawal_policy() {}

#[utoipa::path(patch, path = "/admin/api/v1/wallet/withdrawal-policies/{asset_id}", tag = "admin",
    params(("asset_id" = u64, Path)),
    security(("bearerAuth" = [])),
    request_body = SaveWithdrawalPolicyRequest,
    responses((status = 200, body = WithdrawalPolicyResponse),
              (status = 400, body = ErrorResponse), (status = 409, body = ErrorResponse)))]
pub(super) fn save_withdrawal_policy() {}

#[utoipa::path(get, path = "/api/v1/wallet/withdrawal-addresses", tag = "wallet",
    security(("bearerAuth" = [])),
    responses((status = 200, body = [WithdrawalAddressResponse])))]
pub(super) fn get_withdrawal_addresses() {}

#[utoipa::path(post, path = "/api/v1/wallet/withdrawal-addresses", tag = "wallet",
    security(("bearerAuth" = [])),
    request_body = RegisterWithdrawalAddressRequest,
    responses((status = 200, body = WithdrawalAddressResponse),
              (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse)))]
pub(super) fn register_withdrawal_address() {}
