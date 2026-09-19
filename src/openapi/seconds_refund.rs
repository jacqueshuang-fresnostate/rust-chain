//! 秒合约前瞻性退款文档；根与 /api 文档共享合并结果，不声明默认激活。

use utoipa::{OpenApi, ToSchema};

#[derive(ToSchema)]
struct SecondsRefundPolicy {
    product_id: u64,
    version: u64,
    enabled: bool,
    wait_seconds: Option<u32>,
}

#[derive(ToSchema, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SecondsRefundPolicyUpdate {
    expected_version: u64,
    enabled: bool,
    wait_seconds: Option<u32>,
    reason: String,
}

#[derive(ToSchema, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SecondsPrincipalRefundRequest {
    idempotency_key: String,
    reason: String,
}

#[derive(ToSchema)]
struct SecondsPrincipalRefundReceipt {
    order_id: u64,
    admin_id: u64,
    idempotency_key: String,
    debit_ledger_id: u64,
    user_id: u64,
    asset_id: u64,
    /// 原始本金 Decimal 字符串，不含盈利。
    amount: String,
    policy_version: u64,
    original_order: serde_json::Value,
    eligibility_evidence: serde_json::Value,
    reason: String,
    created_at: i64,
}

#[derive(ToSchema)]
struct SecondsRefundContext {
    order_id: u64,
    policy_version: Option<u64>,
    wait_seconds: Option<u32>,
    eligible_at: Option<i64>,
    checked_at: i64,
    receipt: Option<SecondsPrincipalRefundReceipt>,
}

/// 产品只读策略，缺失返回关闭/null/version0，要求 seconds.products.read。
#[utoipa::path(get, path = "/admin/api/v1/seconds-contracts/products/{id}/refund-policy",
    params(("id" = u64, Path)), responses((status = 200, body = SecondsRefundPolicy)),
    security(("bearerAuth" = [])), tag = "seconds-refund")]
fn get_policy() {}

/// 版本化更新，要求 seconds.products.write、认证原因及显式间隔，只影响未来新单。
#[utoipa::path(patch, path = "/admin/api/v1/seconds-contracts/products/{id}/refund-policy",
    params(("id" = u64, Path)), request_body = SecondsRefundPolicyUpdate,
    responses((status = 200, body = SecondsRefundPolicy), (status = 409, description = "版本冲突")),
    security(("bearerAuth" = [])), tag = "seconds-refund")]
fn update_policy() {}

/// 只读订单开仓快照和回执，要求 seconds.orders.read；到时不等于已通过证据校验。
#[utoipa::path(get, path = "/admin/api/v1/seconds-contracts/orders/{id}/principal-refund",
    params(("id" = u64, Path)), responses((status = 200, body = SecondsRefundContext)),
    security(("bearerAuth" = [])), tag = "seconds-refund")]
fn get_context() {}

/// 要求 seconds.orders.settle；仅开仓已固化策略的无证据 manual_review 订单可退原本金。
/// 原 actor/key/trimmed reason 重放返回原凭证，任何差异/碰撞冲突。数据库失败或损坏证据拒绝。
/// 已付佣金或支付流水异常拒绝，不自动冲回；成功后订单为 refunded，result/settlement_price 保持空。
#[utoipa::path(post, path = "/admin/api/v1/seconds-contracts/orders/{id}/principal-refund",
    params(("id" = u64, Path)), request_body = SecondsPrincipalRefundRequest,
    responses((status = 200, body = SecondsPrincipalRefundReceipt),
        (status = 401, description = "未认证"), (status = 403, description = "权限不足"),
        (status = 409, description = "无开仓快照、未到期、有证据、已付佣金或幂等冲突")),
    security(("bearerAuth" = [])), tag = "seconds-refund")]
fn refund_principal() {}

#[derive(OpenApi)]
#[openapi(paths(get_policy, update_policy, get_context, refund_principal),
    components(schemas(SecondsRefundPolicy, SecondsRefundPolicyUpdate, SecondsPrincipalRefundRequest,
        SecondsPrincipalRefundReceipt, SecondsRefundContext)),
    tags((name = "seconds-refund", description = "默认关闭、仅新订单快照的本金退款")))]
pub(super) struct SecondsRefundApiDoc;
