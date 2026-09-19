//! 提现策略配置和经过现有资金安全验证的地址登记用例。

use super::super::{
    infrastructure::withdrawal_policy as storage,
    presentation::withdrawal_policy::{
        RegisterWithdrawalAddressRequest, SaveWithdrawalPolicyRequest, WithdrawalAddressResponse,
        WithdrawalPolicyResponse,
    },
};
use crate::{
    config::Settings,
    error::AppResult,
    modules::security::{SecurityAction, SecurityVerificationInput, verify_user_security_action},
};
use sqlx::{MySql, Pool};

/// 只读后台策略；不存在的资产直接失败，未配置策略明确返回默认停用。
pub(crate) async fn load_policy(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<WithdrawalPolicyResponse> {
    storage::load_policy(pool, asset_id).await
}

/// 以认证管理员身份、必填原因及期待版本保存全量策略；配置和审计原子提交。
pub(crate) async fn save_policy(
    pool: &Pool<MySql>,
    admin_id: u64,
    asset_id: u64,
    request: SaveWithdrawalPolicyRequest,
) -> AppResult<WithdrawalPolicyResponse> {
    let reason = super::required_reason(Some(request.reason), "withdrawal policy reason")?;
    request
        .policy
        .validate(18)
        .map_err(crate::error::AppError::Validation)?;
    let mut tx = pool.begin().await?;
    let result = storage::save_policy_in_tx(
        &mut tx,
        admin_id,
        asset_id,
        request.expected_revision,
        &request.policy,
        &reason,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

/// 地址身份沿用提现网络归一与精确地址文本；先完成现有提现安全验证，再记录首次登记时刻。
/// 重复调用不延长或缩短冷静期，调用者不能提交他人的用户编号或历史登记时间。
pub(crate) async fn register_address(
    pool: &Pool<MySql>,
    settings: &Settings,
    user_id: u64,
    request: RegisterWithdrawalAddressRequest,
) -> AppResult<WithdrawalAddressResponse> {
    let network = super::normalize_deposit_network(&request.network)?;
    let address = super::normalize_chain_identifier(request.address, "address")?;
    verify_user_security_action(
        pool,
        settings,
        user_id,
        SecurityAction::Withdraw,
        SecurityVerificationInput {
            fund_password: request.fund_password.as_deref(),
            totp_code: request.totp_code.as_deref(),
        },
    )
    .await?;
    storage::register_address(pool, user_id, &network, &address).await
}

/// 查询已认证用户的地址登记快照，不操作安全凭据或钱包。
pub(crate) async fn list_addresses(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<WithdrawalAddressResponse>> {
    storage::list_addresses(pool, user_id).await
}
