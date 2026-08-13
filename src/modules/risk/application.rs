//! risk bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 风控闸门必须在任何冻结、账本写入之前调用，规则缺失时保持放行。
//! 一次闸门调用的完整链路是：实时读取启用规则，按操作与作用域合并出策略，必要时在 Redis 累加限频计数，
//! 交由领域层评估，命中则写风控事件并返回带错误码的 403。
//! 闸门自身不开事务、不碰钱包，也不回滚已发生的计数，因此调用方只应在真正要执行业务动作前调用一次。

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

/// 一次风控闸门调用的全部输入事实，可选字段为空表示该路径拿不到对应数据，相应维度将被跳过。
#[derive(Debug)]
pub struct RiskGuardInput {
    pub user_id: u64,
    /// 业务操作标识，同时用作规则匹配依据与限频计数键的一段。
    pub operation: &'static str,
    /// 本次请求归属的作用域维度集合，规则只有目标落在其中才会命中。
    pub scopes: Vec<RiskScope>,
    /// 参与限额比较的金额，单位口径必须与操作登记的口径一致。
    pub amount: Option<BigDecimal>,
    pub price: Option<BigDecimal>,
    /// 价格偏离判定的基准价，与委托价同时具备才会执行该项校验。
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

/// 取得本次请求在限频窗口内的累计次数，仅当策略确实含请求数上限且 Redis 可用时才真正自增。
/// 计数键按操作、限频作用域和用户三段隔离，因此不同作用域的规则各用各的配额互不干扰。
/// 注意本函数一旦执行就已经把计数加一，即便后续因其他维度拒绝也不会回退，这是固定窗口计数的既定语义。
/// Redis 报错时只告警并返回空值，让限频维度在评估阶段被跳过，缓存故障不阻断正常交易。
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

/// 为一次被拒请求写下风控事件留痕，固定以高风险等级和 reject 决策入库。
/// payload 内同时快照三部分现场：触发操作与全部作用域维度、参与判定的请求事实、以及合并后生效的阈值集合，
/// 金额与价格转成字符串保留原始精度，便于事后复盘当时为何被拦而无需重建规则版本。
/// 本函数不返回结果也不上抛错误，落库失败只告警，绝不能因为留痕失败把已成立的拒绝改写成放行。
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
