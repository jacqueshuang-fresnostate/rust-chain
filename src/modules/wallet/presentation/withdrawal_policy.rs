//! 提现策略管理与用户地址登记传输合同。

use super::super::domain::withdrawal_policy::WithdrawalPolicy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct WithdrawalPolicyResponse {
    pub asset_id: u64,
    pub revision: u64,
    pub policy: WithdrawalPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveWithdrawalPolicyRequest {
    pub expected_revision: u64,
    pub policy: WithdrawalPolicy,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterWithdrawalAddressRequest {
    pub network: String,
    pub address: String,
    pub fund_password: Option<String>,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct WithdrawalAddressResponse {
    pub id: u64,
    pub network: String,
    pub address: String,
    pub created_at: DateTime<Utc>,
}
