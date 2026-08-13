//! 闪兑交易对配置的纯业务规则层，只包含配置校验与审计快照两类函数。
//!
//! 校验刻意设计成对「最终配置快照」而非「请求增量」生效：创建路径先补齐默认值、更新路径先与锁定的旧值合并，
//! 二者随后调用同一个校验入口，从而保证两条路径不会出现约束覆盖不一致。
//! 本层不查资产是否存在、不判断交易对是否重复、也不计算任何报价，这些都由应用事务与数据库约束负责。

use super::*;

/// 校验新建闪兑交易对的资产编号、汇率、费率、最小/最大兑换量及状态组合约束。
/// 这里只验证请求值；资产存在性和交易对唯一性由创建事务负责，失败前不产生资金或审计副作用。
/// 校验前先在本地补齐三个缺省值：费率缺省为 0、目标侧最小额缺省沿用源侧最小额、目标侧最大额缺省沿用源侧最大额，
/// 这套补齐规则与应用层落库时使用的完全一致，因此校验对象就是实际会写入的配置。
pub(crate) fn validate_create_convert_pair(request: &CreateConvertPairRequest) -> AppResult<()> {
    let zero = BigDecimal::from(0);
    let fee_rate = request.fee_rate.as_ref().unwrap_or(&zero);
    let target_min_amount = request
        .target_min_amount
        .as_ref()
        .unwrap_or(&request.min_amount);
    let target_max_amount = request
        .target_max_amount
        .as_ref()
        .or(request.max_amount.as_ref());

    validate_convert_pair_values(
        request.from_asset_id,
        request.to_asset_id,
        &request.pricing_mode,
        &request.spread_rate,
        fee_rate,
        &request.min_amount,
        request.max_amount.as_ref(),
        target_min_amount,
        target_max_amount,
    )
}

/// 校验换币交易对完整配置，防止同资产兑换、空计价模式及非法费率或金额区间入库。
/// 调用方须传入创建默认值或更新合并后的最终值，而不是仅校验局部请求字段。
/// 费率保持在 `[0, 1)`，源/目标最小额不得为负，最大额存在时不得小于对应最小额。
/// 这是无 I/O 的纯校验，不涉及事务、资金或审计；失败返回首个校验错误且不产生副作用。
#[allow(clippy::too_many_arguments)] // 校验最终配置快照；字段保持显式可避免创建/更新路径遗漏约束。
pub(crate) fn validate_convert_pair_values(
    from_asset_id: u64,
    to_asset_id: u64,
    pricing_mode: &str,
    spread_rate: &BigDecimal,
    fee_rate: &BigDecimal,
    min_amount: &BigDecimal,
    max_amount: Option<&BigDecimal>,
    target_min_amount: &BigDecimal,
    target_max_amount: Option<&BigDecimal>,
) -> AppResult<()> {
    if from_asset_id == to_asset_id {
        return Err(AppError::Validation(
            "convert pair assets must be different".to_owned(),
        ));
    }
    if optional_string(Some(pricing_mode.to_owned())).is_none() {
        return Err(AppError::Validation("pricing_mode is required".to_owned()));
    }
    let zero = BigDecimal::from(0);
    if min_amount < &zero {
        return Err(AppError::Validation(
            "min_amount must be non-negative".to_owned(),
        ));
    }
    if spread_rate < &zero {
        return Err(AppError::Validation(
            "spread_rate must be non-negative".to_owned(),
        ));
    }
    if fee_rate < &zero || fee_rate >= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "fee_rate must be greater than or equal to 0 and less than 1".to_owned(),
        ));
    }
    if let Some(max_amount) = max_amount
        && max_amount < min_amount
    {
        return Err(AppError::Validation(
            "max_amount must be greater than or equal to min_amount".to_owned(),
        ));
    }
    if target_min_amount < &zero {
        return Err(AppError::Validation(
            "target_min_amount must be non-negative".to_owned(),
        ));
    }
    if let Some(target_max_amount) = target_max_amount
        && target_max_amount < target_min_amount
    {
        return Err(AppError::Validation(
            "target_max_amount must be greater than or equal to target_min_amount".to_owned(),
        ));
    }

    Ok(())
}

/// 将闪兑交易对的资产、汇率、费率、限额和状态映射为稳定审计 JSON。
/// 快照保留资产符号和完整源/目标金额边界，不执行汇率计算；应用层在交易对写事务中持久化前后值。
/// 资产同时记录编号与符号，使得资产后续改名也不影响回溯当时的配置含义。
/// 创建、更新与删除三类操作共用该结构，删除时只写 before 值，after 留空表示记录已不存在。
pub(crate) fn convert_pair_audit_json(pair: &ConvertPairResponse) -> Value {
    json!({
        "id": pair.id,
        "from_asset_id": pair.from_asset_id,
        "from_asset_symbol": pair.from_asset_symbol,
        "to_asset_id": pair.to_asset_id,
        "to_asset_symbol": pair.to_asset_symbol,
        "pricing_mode": pair.pricing_mode,
        "spread_rate": pair.spread_rate,
        "fee_rate": pair.fee_rate,
        "min_amount": pair.min_amount,
        "max_amount": pair.max_amount,
        "target_min_amount": pair.target_min_amount,
        "target_max_amount": pair.target_max_amount,
        "enabled": pair.enabled,
    })
}
