//! risk bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 风控闸门必须在任何冻结、账本写入之前调用，规则缺失时保持放行。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::risk::{
        domain::{RiskDecision, RiskReject, RiskRequest, evaluate_risk},
        infrastructure::{bump_user_request_count, insert_risk_event, load_enabled_risk_rules},
        repository::RiskEventWrite,
        service::{RiskPolicy, RiskScope, resolve_risk_policy},
    },
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use serde_json::json;
use sqlx::{MySql, Pool};
use tracing::warn;

#[derive(Debug)]
pub struct RiskGuardInput {
    pub user_id: u64,
    pub operation: &'static str,
    pub scopes: Vec<RiskScope>,
    pub amount: Option<BigDecimal>,
    pub price: Option<BigDecimal>,
    pub reference_price: Option<BigDecimal>,
}

impl ApplicationLayer for RiskGuardInput {}

/// 实时读取启用规则，按操作和作用域合并后依次评估黑名单、限频、金额及价格偏离；命中返回 403。
/// Redis 计数故障按放行处理，拒绝事件落库故障仅告警；调用方必须在任何钱包冻结、扣款和订单写入前调用本闸门。
pub async fn enforce_risk_control(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    input: RiskGuardInput,
) -> AppResult<()> {
    let stored = load_enabled_risk_rules(pool).await?;
    if stored.is_empty() {
        return Ok(());
    }

    let policy = resolve_risk_policy(&stored, input.operation, &input.scopes);
    if policy.rules.is_unrestricted() {
        return Ok(());
    }

    let request = RiskRequest {
        operation: input.operation.to_owned(),
        request_count: resolve_request_count(redis, &input, &policy).await,
        amount: input.amount.clone(),
        price: input.price.clone(),
        reference_price: input.reference_price.clone(),
    };
    let RiskDecision::Rejected(reject) = evaluate_risk(&request, &policy.rules) else {
        return Ok(());
    };

    record_rejected_risk_event(pool, &input, &request, &policy, &reject).await;
    Err(AppError::security_forbidden(
        reject.code(),
        reject.message(),
    ))
}

/// 只有配置了限频规则且 Redis 可用时才计数；计数失败按放行处理，不因缓存故障阻断交易。
async fn resolve_request_count(
    redis: Option<&ConnectionManager>,
    input: &RiskGuardInput,
    policy: &RiskPolicy,
) -> Option<u32> {
    policy.rules.max_requests?;
    let redis = redis?;

    match bump_user_request_count(
        redis,
        input.user_id,
        input.operation,
        &policy.rate_limit_scope,
        policy.rate_limit_window_seconds,
    )
    .await
    {
        Ok(count) => Some(count),
        Err(error) => {
            warn!(user_id = input.user_id, operation = input.operation, %error, "风控请求计数失败");
            None
        }
    }
}

/// 风控事件只做审计留痕，落库失败不能覆盖调用方已经得到的拒绝原因。
async fn record_rejected_risk_event(
    pool: &Pool<MySql>,
    input: &RiskGuardInput,
    request: &RiskRequest,
    policy: &RiskPolicy,
    reject: &RiskReject,
) {
    let payload = json!({
        "operation": request.operation,
        "scopes": input
            .scopes
            .iter()
            .map(|scope| json!({ "dimension": scope.dimension, "value": scope.value }))
            .collect::<Vec<_>>(),
        "request": {
            "request_count": request.request_count,
            "amount": request.amount.as_ref().map(ToString::to_string),
            "price": request.price.as_ref().map(ToString::to_string),
            "reference_price": request.reference_price.as_ref().map(ToString::to_string),
        },
        "rules": {
            "max_requests": policy.rules.max_requests,
            "max_amount": policy.rules.max_amount.as_ref().map(ToString::to_string),
            "max_price_deviation_bps": policy.rules.max_price_deviation_bps,
            "blocked_operations": policy.rules.blocked_operations,
            "rate_limit_scope": policy.rate_limit_scope,
        },
    });

    if let Err(error) = insert_risk_event(
        pool,
        RiskEventWrite {
            user_id: input.user_id,
            event_type: input.operation.to_owned(),
            risk_level: "high",
            decision: "reject",
            reason: reject.message().to_owned(),
            payload,
        },
    )
    .await
    {
        warn!(user_id = input.user_id, operation = input.operation, %error, "风控事件落库失败");
    }
}
