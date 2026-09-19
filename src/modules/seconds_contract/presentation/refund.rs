//! 前瞻性本金退款策略和不可变退款凭证；不接受客户端金额或操作者身份。

use super::*;
use serde_json::Value;

/// 当前产品策略；未配置时版本零、关闭、等待时间空，不隐式授予退款权利。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct RefundPolicy {
    pub product_id: u64,
    pub version: u64,
    pub enabled: bool,
    pub wait_seconds: Option<u32>,
}

/// 策略替换必须携带操作者看到的版本及审计原因；启用时必须显式指定等待间隔。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateRefundPolicy {
    pub expected_version: u64,
    pub enabled: bool,
    pub wait_seconds: Option<u32>,
    pub reason: String,
}

/// 本金退款只接受稳定键与原因，金额、资格和身份都由服务端不可变证据确定。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefundPrincipalRequest {
    pub idempotency_key: String,
    pub reason: String,
}

/// 原请求的退款回执；原扣款、策略、异常及订单均保留快照，重放不得改写。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct PrincipalRefundReceipt {
    pub order_id: u64,
    pub admin_id: u64,
    pub idempotency_key: String,
    pub debit_ledger_id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub amount: BigDecimal,
    pub policy_version: u64,
    pub original_order: Value,
    pub eligibility_evidence: Value,
    pub reason: String,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

/// 只读展示开仓快照与最早申请时间；真正无证据与资金资格必须在提交事务内重验。
#[derive(Serialize)]
pub(crate) struct RefundContext {
    pub order_id: u64,
    pub policy_version: Option<u64>,
    pub wait_seconds: Option<u32>,
    #[serde(with = "option_unix_millis")]
    pub eligible_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub checked_at: DateTime<Utc>,
    pub receipt: Option<PrincipalRefundReceipt>,
}
