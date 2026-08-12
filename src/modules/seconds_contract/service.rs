//! seconds_contract bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    error::{AppError, AppResult},
    modules::seconds_contract::{
        presentation::{
            CreateSecondsContractProductRequest, OpenSecondsContractOrderResponse,
            SecondsContractOrderResponse, SecondsContractProductCycleInput,
            SecondsContractProductResponse, SettleSecondsContractOrderResponse,
            UpdateSecondsContractProductRequest,
        },
        repository::SecondsContractProductRuleRow,
    },
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        wallet::{amount_fits_asset_precision, truncate_amount_to_asset_precision},
    },
};
use bigdecimal::BigDecimal;
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub(crate) struct NormalizedSecondsContractProductCycle {
    pub(crate) duration_seconds: u32,
    pub(crate) payout_rate: BigDecimal,
    pub(crate) min_stake: BigDecimal,
    pub(crate) max_stake: Option<BigDecimal>,
}

/// 生成秒合约产品及周期审计快照，金额以字符串保精度且无持久化副作用。
pub(crate) fn product_audit_json(product: &SecondsContractProductResponse) -> Value {
    json!({
        "id": product.id,
        "pair_id": product.pair_id,
        "symbol": product.symbol,
        "stake_asset": product.stake_asset,
        "stake_asset_symbol": product.stake_asset_symbol,
        "logo_url": product.logo_url,
        "duration_seconds": product.duration_seconds,
        "payout_rate": product.payout_rate,
        "min_stake": product.min_stake,
        "max_stake": product.max_stake,
        "cycles": product.cycles,
        "status": product.status,
    })
}

/// 生成秒合约订单结算审计快照，使用既有价格与赔付结果而不重新计算资金。
pub(crate) fn order_audit_json(
    order: &SecondsContractOrderResponse,
    payout_amount: BigDecimal,
) -> Value {
    json!({
        "id": order.id,
        "user_id": order.user_id,
        "product_id": order.product_id,
        "pair_id": order.pair_id,
        "stake_asset": order.stake_asset,
        "direction": order.direction,
        "stake_amount": order.stake_amount,
        "duration_seconds": order.duration_seconds,
        "payout_rate": order.payout_rate,
        "entry_price": order.entry_price,
        "settlement_price": order.settlement_price,
        "status": order.status,
        "result": order.result,
        "payout_amount": payout_amount,
        "expires_at": order.expires_at.timestamp_millis(),
    })
}

/// 发布秒合约订单开仓事件：由应用层在资金事务提交后调用，避免重复拼装事件 payload。
/// 前置业务失败时不得发布；未配置广播目标沿用既有降级语义，且不会补做资金写入。
pub(crate) fn publish_seconds_contract_order_opened_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    response: &OpenSecondsContractOrderResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "seconds_contract.order.opened",
            "order_id": response.order.id,
            "product_id": response.order.product_id,
            "pair_id": response.order.pair_id,
            "symbol": response.order.symbol,
            "stake_asset": response.order.stake_asset,
            "stake_asset_symbol": response.order.stake_asset_symbol,
            "direction": response.order.direction,
            "stake_amount": response.order.stake_amount,
            "duration_seconds": response.order.duration_seconds,
            "payout_rate": response.order.payout_rate,
            "entry_price": response.order.entry_price,
            "expires_at": response.order.expires_at.timestamp_millis(),
            "status": response.order.status,
        })
        .to_string(),
    ));
}

/// 在订单新建且广播通道存在时推送秒合约开仓事件。
pub(crate) fn publish_seconds_contract_order_opened_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    response: &OpenSecondsContractOrderResponse,
    is_new_order: bool,
) {
    if is_new_order && let Some(hub) = hub {
        publish_seconds_contract_order_opened_event(hub, user_id, response);
    }
}

/// 发布秒合约订单结算事件：由应用层在结算事务提交后调用，事件构建集中在服务层。
/// 前置业务失败时不得发布；未配置广播目标沿用既有降级语义，且不会补做资金写入。
pub(crate) fn publish_seconds_contract_order_settled_event(
    hub: &EventBroadcastHub,
    user_id: u64,
    response: &SettleSecondsContractOrderResponse,
) {
    hub.publish(EventBroadcastMessage::private_user(
        user_id,
        json!({
            "type": "seconds_contract.order.settled",
            "order_id": response.order.id,
            "product_id": response.order.product_id,
            "pair_id": response.order.pair_id,
            "symbol": response.order.symbol,
            "stake_asset": response.order.stake_asset,
            "stake_asset_symbol": response.order.stake_asset_symbol,
            "direction": response.order.direction,
            "stake_amount": response.order.stake_amount,
            "duration_seconds": response.order.duration_seconds,
            "settlement_price": response.order.settlement_price,
            "payout_amount": response.payout_amount,
            "result": response.order.result,
            "status": response.order.status,
        })
        .to_string(),
    ));
}

/// 在订单结算且广播通道存在时推送秒合约结算事件。
pub(crate) fn publish_seconds_contract_order_settled_event_if_needed(
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    response: &SettleSecondsContractOrderResponse,
    is_new_settlement: bool,
) {
    if is_new_settlement && let Some(hub) = hub {
        publish_seconds_contract_order_settled_event(hub, user_id, response);
    }
}

/// 秒合约同键重放必须与产品、方向和投注金额一致；请求指定周期时还必须匹配原单周期。
pub(crate) fn ensure_existing_order_matches_request(
    existing: &SecondsContractOrderResponse,
    product_id: u64,
    duration_seconds: Option<u32>,
    direction: &str,
    stake_amount: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id
        || duration_seconds
            .is_some_and(|duration_seconds| existing.duration_seconds != duration_seconds)
        || existing.direction != direction
        || existing.stake_amount != *stake_amount
    {
        return Err(AppError::Conflict(
            "seconds contract idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

/// 已结算订单仅允许相同结果重放，异结果返回冲突且不重复赔付。
pub(crate) fn ensure_existing_settlement_matches(
    existing: &SecondsContractOrderResponse,
    result: &str,
) -> AppResult<()> {
    if existing.result.as_deref() != Some(result) {
        return Err(AppError::Conflict(
            "seconds contract order was settled with a different result".to_owned(),
        ));
    }
    Ok(())
}

/// 从订单本金、赔率和结果计算结算入账额；输单为零，赢单按资产精度向零截断。
pub(crate) fn settlement_payout_amount(
    order: &SecondsContractOrderResponse,
    result: &str,
    precision_scale: i32,
) -> BigDecimal {
    seconds_contract_payout_amount(
        &order.stake_amount,
        &order.payout_rate,
        result,
        precision_scale,
    )
}

/// 赢单返回 `本金 × (1 + 赔率)` 并按资产精度向零截断，其他结果返回零；函数不修改钱包。
pub(crate) fn seconds_contract_payout_amount(
    stake_amount: &BigDecimal,
    payout_rate: &BigDecimal,
    result: &str,
    precision_scale: i32,
) -> BigDecimal {
    if result == "win" {
        // 派奖必须按质押资产精度向零截断，禁止把不可入账的小数尾差写进钱包。
        truncate_amount_to_asset_precision(
            &(stake_amount.clone() + stake_amount.clone() * payout_rate.clone()),
            precision_scale,
        )
    } else {
        truncate_amount_to_asset_precision(&BigDecimal::from(0), precision_scale)
    }
}

/// 校验非零交易对/投注资产、周期集合、费率、限额、可选状态及原因长度，并按周期升序返回完整配置。
pub(crate) fn validate_create_product_request(
    request: &CreateSecondsContractProductRequest,
) -> AppResult<Vec<NormalizedSecondsContractProductCycle>> {
    let cycles = normalize_product_cycles(
        request.pair_id,
        request.stake_asset,
        request.cycles.as_ref(),
        request.duration_seconds,
        request.payout_rate.as_ref(),
        request.min_stake.as_ref(),
        &request.max_stake,
    )?;
    if let Some(status) = request.status.as_deref() {
        normalized_product_status(status)?;
    }
    validate_reason_len(request.reason.as_deref())?;
    Ok(cycles)
}

/// 校验更新请求的非零交易对/投注资产、完整周期集合、状态、金额边界和原因长度，失败不进入管理事务。
pub(crate) fn validate_update_product_request(
    request: &UpdateSecondsContractProductRequest,
) -> AppResult<Vec<NormalizedSecondsContractProductCycle>> {
    let cycles = normalize_product_cycles(
        request.pair_id,
        request.stake_asset,
        request.cycles.as_ref(),
        request.duration_seconds,
        request.payout_rate.as_ref(),
        request.min_stake.as_ref(),
        &request.max_stake,
    )?;
    normalized_product_status(&request.status)?;
    validate_reason_len(request.reason.as_deref())?;
    Ok(cycles)
}

fn normalize_product_cycles(
    pair_id: u64,
    stake_asset: u64,
    cycles: Option<&Vec<SecondsContractProductCycleInput>>,
    legacy_duration_seconds: Option<u32>,
    legacy_payout_rate: Option<&BigDecimal>,
    legacy_min_stake: Option<&BigDecimal>,
    legacy_max_stake: &Option<BigDecimal>,
) -> AppResult<Vec<NormalizedSecondsContractProductCycle>> {
    if pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    if stake_asset == 0 {
        return Err(AppError::Validation("stake_asset is required".to_owned()));
    }

    let mut normalized = if let Some(cycles) = cycles {
        if cycles.is_empty() {
            return Err(AppError::Validation(
                "seconds contract cycles must not be empty".to_owned(),
            ));
        }
        cycles
            .iter()
            .map(normalize_product_cycle_input)
            .collect::<AppResult<Vec<_>>>()?
    } else {
        vec![NormalizedSecondsContractProductCycle {
            duration_seconds: legacy_duration_seconds.ok_or_else(|| {
                AppError::Validation("seconds contract duration_seconds is required".to_owned())
            })?,
            payout_rate: legacy_payout_rate.cloned().ok_or_else(|| {
                AppError::Validation("seconds contract payout_rate is required".to_owned())
            })?,
            min_stake: legacy_min_stake.cloned().ok_or_else(|| {
                AppError::Validation("seconds contract min_stake is required".to_owned())
            })?,
            max_stake: legacy_max_stake.clone(),
        }]
    };

    let mut duration_set = HashSet::with_capacity(normalized.len());
    for cycle in &normalized {
        validate_product_cycle_fields(cycle)?;
        if !duration_set.insert(cycle.duration_seconds) {
            return Err(AppError::Validation(
                "seconds contract duration_seconds must be unique".to_owned(),
            ));
        }
    }
    normalized.sort_by_key(|cycle| cycle.duration_seconds);
    Ok(normalized)
}

fn normalize_product_cycle_input(
    cycle: &SecondsContractProductCycleInput,
) -> AppResult<NormalizedSecondsContractProductCycle> {
    Ok(NormalizedSecondsContractProductCycle {
        duration_seconds: cycle.duration_seconds.ok_or_else(|| {
            AppError::Validation("seconds contract duration_seconds is required".to_owned())
        })?,
        payout_rate: cycle.payout_rate.clone().ok_or_else(|| {
            AppError::Validation("seconds contract payout_rate is required".to_owned())
        })?,
        min_stake: cycle.min_stake.clone().ok_or_else(|| {
            AppError::Validation("seconds contract min_stake is required".to_owned())
        })?,
        max_stake: cycle.max_stake.clone(),
    })
}

fn validate_product_cycle_fields(cycle: &NormalizedSecondsContractProductCycle) -> AppResult<()> {
    if cycle.duration_seconds == 0 {
        return Err(AppError::Validation(
            "seconds contract duration_seconds must be positive".to_owned(),
        ));
    }
    validate_payout_rate(&cycle.payout_rate)?;
    validate_stake_amount(&cycle.min_stake)?;
    if let Some(max_stake) = &cycle.max_stake {
        validate_stake_amount(max_stake)?;
        if max_stake < &cycle.min_stake {
            return Err(AppError::Validation(
                "seconds contract max_stake must be greater than or equal to min_stake".to_owned(),
            ));
        }
    }
    Ok(())
}

/// 要求产品启用且投注金额符合资产精度及所选周期上下限。
pub(crate) fn validate_product_stake(
    stake_amount: &BigDecimal,
    product: &SecondsContractProductRuleRow,
) -> AppResult<()> {
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    if !amount_fits_asset_precision(stake_amount, product.stake_asset_precision) {
        return Err(AppError::Validation(format!(
            "seconds contract stake exceeds asset precision {}",
            product.stake_asset_precision
        )));
    }
    if stake_amount < &product.min_stake {
        return Err(AppError::Validation(
            "seconds contract stake is below product minimum".to_owned(),
        ));
    }
    if let Some(max_stake) = &product.max_stake
        && stake_amount > max_stake
    {
        return Err(AppError::Validation(
            "seconds contract stake exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

/// 只接受 up 或 down 方向并转为稳定小写值，未知输入不创建订单。
pub(crate) fn normalize_direction(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "up" => Ok("up".to_owned()),
        "down" => Ok("down".to_owned()),
        _ => Err(AppError::Validation(
            "seconds contract direction must be up or down".to_owned(),
        )),
    }
}

/// 只接受 win 或 loss 并转为稳定小写值；该函数不根据入场价、到期价或方向推导结果。
pub(crate) fn normalize_settlement_result(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "win" => Ok("win".to_owned()),
        "loss" => Ok("loss".to_owned()),
        _ => Err(AppError::Validation(
            "seconds contract settlement result must be win or loss".to_owned(),
        )),
    }
}

/// 只接受 active 或 disabled 产品状态，空值与未知状态返回参数错误。
pub(crate) fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "seconds contract product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "seconds contract product status must be active or disabled".to_owned(),
        )),
    }
}

/// 裁剪并校验后台操作原因，空值或超长时在开启管理事务前失败。
pub(crate) fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "seconds contract reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

fn validate_reason_len(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > SECONDS_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "seconds contract reason is too long".to_owned(),
        ));
    }
    Ok(())
}

fn validate_payout_rate(payout_rate: &BigDecimal) -> AppResult<()> {
    if payout_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "seconds contract payout_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        payout_rate,
        SECONDS_RATE_MAX_SCALE,
        SECONDS_RATE_MAX_INTEGER_DIGITS,
        "seconds contract payout_rate",
    )
}

/// 投注金额必须为正且符合数据库小数及整数位容量，防止落账截断。
pub(crate) fn validate_stake_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "seconds contract stake amount must be positive".to_owned(),
        ));
    }
    validate_decimal_storage(
        amount,
        SECONDS_AMOUNT_MAX_SCALE,
        SECONDS_AMOUNT_MAX_INTEGER_DIGITS,
        "seconds contract stake amount",
    )
}

fn validate_decimal_storage(
    value: &BigDecimal,
    max_scale: i64,
    max_integer_digits: usize,
    label: &str,
) -> AppResult<()> {
    let (digits, scale) = value.as_bigint_and_exponent();
    if scale > max_scale {
        return Err(AppError::Validation(format!(
            "{label} supports at most {max_scale} decimal places"
        )));
    }

    let significant_digits = digits
        .to_str_radix(10)
        .trim_start_matches('-')
        .trim_start_matches('0')
        .len();
    let integer_digits = if scale >= 0 {
        significant_digits.saturating_sub(scale as usize)
    } else {
        significant_digits.saturating_add(scale.unsigned_abs() as usize)
    };
    if integer_digits > max_integer_digits {
        return Err(AppError::Validation(format!(
            "{label} exceeds decimal storage precision"
        )));
    }
    Ok(())
}

/// 裁剪并校验幂等键长度；空值或超长键不得占用唯一约束或触发资金写入。
pub(crate) fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required for seconds contract orders".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for seconds contract orders".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

/// 裁剪可选筛选文本，空字符串转为 `None`。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 裁剪可选图片 URL，空值转 `None`，超出字段长度时返回参数错误。
pub(crate) fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

/// 从 `user:{id}` subject 解析用户编号，格式不符返回 Unauthorized。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从 `admin:{id}` subject 解析管理员编号，格式不符返回 Unauthorized。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 将秒合约列表条数默认设为 50，并限制在 1～100。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

const SECONDS_AUDIT_REASON_MAX_LEN: usize = 512;
const SECONDS_RATE_MAX_SCALE: i64 = 8;
const SECONDS_RATE_MAX_INTEGER_DIGITS: usize = 10;
const SECONDS_AMOUNT_MAX_SCALE: i64 = 18;
const SECONDS_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;
