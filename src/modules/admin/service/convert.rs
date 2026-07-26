use super::*;

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
