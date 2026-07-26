use super::*;

pub(crate) fn validate_create_trading_pair_request(
    request: &CreateTradingPairRequest,
) -> AppResult<()> {
    if request.base_asset_id == request.quote_asset_id {
        return Err(AppError::Validation(
            "trading pair assets must be different".to_owned(),
        ));
    }
    normalize_trading_pair_symbol(&request.symbol)?;
    validate_trading_pair_config(
        request.price_precision,
        request.qty_precision,
        &request.min_order_value,
    )?;
    if let Some(status) = request.status.as_deref() {
        validate_trading_pair_status(status)?;
    }
    if let Some(market_type) = request.market_type.as_deref() {
        validate_trading_pair_market_type(market_type)?;
    }
    Ok(())
}

pub(crate) fn validate_update_trading_pair_request(
    request: &UpdateTradingPairRequest,
) -> AppResult<()> {
    validate_trading_pair_config(
        request.price_precision,
        request.qty_precision,
        &request.min_order_value,
    )?;
    validate_trading_pair_status(&request.status)?;
    validate_trading_pair_market_type(&request.market_type)?;
    Ok(())
}

pub(crate) fn normalize_trading_pair_symbol(value: &str) -> AppResult<String> {
    let Some(value) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("symbol is required".to_owned()));
    };
    if value.len() > 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/'))
    {
        return Err(AppError::Validation(
            "trading pair symbol format is invalid".to_owned(),
        ));
    }
    Ok(value.to_ascii_uppercase().replace(['_', '/'], "-"))
}

pub(crate) fn validate_trading_pair_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported trading pair status".to_owned(),
        )),
    }
}

pub(crate) fn validate_trading_pair_market_type(value: &str) -> AppResult<String> {
    let Some(market_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("market_type is required".to_owned()));
    };
    match market_type.as_str() {
        "external" | "internal" | "strategy" => Ok(market_type),
        _ => Err(AppError::Validation(
            "unsupported trading pair market_type".to_owned(),
        )),
    }
}

pub(crate) fn trading_pair_audit_json(pair: &AdminTradingPairResponse) -> Value {
    json!({
        "id": pair.id,
        "base_asset_id": pair.base_asset_id,
        "quote_asset_id": pair.quote_asset_id,
        "symbol": pair.symbol,
        "logo_url": pair.logo_url,
        "base_asset": pair.base_asset,
        "quote_asset": pair.quote_asset,
        "price_precision": pair.price_precision,
        "qty_precision": pair.qty_precision,
        "min_order_value": pair.min_order_value,
        "status": pair.status,
        "market_type": pair.market_type,
        "created_at": pair.created_at.timestamp_millis(),
    })
}

pub(crate) fn validate_create_market_strategy(
    request: &CreateMarketStrategyRequest,
) -> AppResult<()> {
    if request.pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    validate_market_strategy_config(MarketStrategyConfigValidation {
        strategy_type: &request.strategy_type,
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
    })?;
    if let Some(status) = request.status.as_deref() {
        validate_market_strategy_status(status)?;
    }
    Ok(())
}

pub(crate) fn validate_update_market_strategy(
    request: &UpdateMarketStrategyRequest,
) -> AppResult<()> {
    validate_market_strategy_config(MarketStrategyConfigValidation {
        strategy_type: &request.strategy_type,
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
    })
}

struct MarketStrategyConfigValidation<'a> {
    strategy_type: &'a str,
    start_price: &'a BigDecimal,
    target_price: &'a BigDecimal,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    volatility: &'a BigDecimal,
    volume_min: &'a BigDecimal,
    volume_max: &'a BigDecimal,
}

fn validate_market_strategy_config(config: MarketStrategyConfigValidation<'_>) -> AppResult<()> {
    if optional_string(Some(config.strategy_type.to_owned())).is_none() {
        return Err(AppError::Validation("strategy_type is required".to_owned()));
    }
    if config.start_price <= &BigDecimal::from(0) || config.target_price <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "strategy prices must be positive".to_owned(),
        ));
    }
    if config.end_time <= config.start_time {
        return Err(AppError::Validation(
            "end_time must be after start_time".to_owned(),
        ));
    }
    if config.volatility < &BigDecimal::from(0)
        || config.volume_min < &BigDecimal::from(0)
        || config.volume_max < &BigDecimal::from(0)
    {
        return Err(AppError::Validation(
            "volatility and volume must be non-negative".to_owned(),
        ));
    }
    if config.volume_max < config.volume_min {
        return Err(AppError::Validation(
            "volume_max must be greater than or equal to volume_min".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_market_strategy_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "active" | "paused" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported market strategy status".to_owned(),
        )),
    }
}

pub(crate) fn market_strategy_run_status(status: &str) -> &'static str {
    match status {
        "active" => "running",
        "paused" => "paused",
        "disabled" => "stopped",
        _ => "draft",
    }
}

pub(crate) fn market_strategy_config_json(
    request: &CreateMarketStrategyRequest,
    status: &str,
    market_type: &str,
) -> Value {
    market_strategy_config_value(MarketStrategyConfigValue {
        pair_id: Some(request.pair_id),
        market_type,
        strategy_type: request.strategy_type.trim(),
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
        status,
    })
}

pub(crate) fn market_strategy_update_config_json(
    request: &UpdateMarketStrategyRequest,
    status: &str,
    market_type: &str,
) -> Value {
    market_strategy_config_value(MarketStrategyConfigValue {
        pair_id: None,
        market_type,
        strategy_type: request.strategy_type.trim(),
        start_price: &request.start_price,
        target_price: &request.target_price,
        start_time: request.start_time,
        end_time: request.end_time,
        volatility: &request.volatility,
        volume_min: &request.volume_min,
        volume_max: &request.volume_max,
        status,
    })
}

struct MarketStrategyConfigValue<'a> {
    pair_id: Option<u64>,
    market_type: &'a str,
    strategy_type: &'a str,
    start_price: &'a BigDecimal,
    target_price: &'a BigDecimal,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    volatility: &'a BigDecimal,
    volume_min: &'a BigDecimal,
    volume_max: &'a BigDecimal,
    status: &'a str,
}

fn market_strategy_config_value(config: MarketStrategyConfigValue<'_>) -> Value {
    let mut value = json!({
        "market_type": config.market_type,
        "strategy_type": config.strategy_type,
        "start_price": config.start_price,
        "target_price": config.target_price,
        "start_time": config.start_time.timestamp_millis(),
        "end_time": config.end_time.timestamp_millis(),
        "volatility": config.volatility,
        "volume_min": config.volume_min,
        "volume_max": config.volume_max,
        "status": config.status,
    });
    if let Some(pair_id) = config.pair_id {
        value["pair_id"] = json!(pair_id);
    }
    value
}

pub(crate) fn market_strategy_audit_json(strategy: &AdminMarketStrategyResponse) -> Value {
    json!({
        "id": strategy.id,
        "pair_id": strategy.pair_id,
        "symbol": strategy.symbol,
        "market_type": strategy.market_type,
        "strategy_type": strategy.strategy_type,
        "start_price": strategy.start_price,
        "target_price": strategy.target_price,
        "start_time": strategy.start_time.timestamp_millis(),
        "end_time": strategy.end_time.timestamp_millis(),
        "volatility": strategy.volatility,
        "volume_min": strategy.volume_min,
        "volume_max": strategy.volume_max,
        "status": strategy.status,
        "run_status": strategy.run_status,
        "current_price": strategy.current_price,
        "last_generated_at": strategy.last_generated_at.map(|value| value.timestamp_millis()),
        "last_kline_open_time": strategy.last_kline_open_time.map(|value| value.timestamp_millis()),
        "recovery_status": strategy.recovery_status,
        "created_at": strategy.created_at.timestamp_millis(),
    })
}

fn validate_trading_pair_config(
    price_precision: i32,
    qty_precision: i32,
    min_order_value: &BigDecimal,
) -> AppResult<()> {
    if price_precision < 0 || qty_precision < 0 {
        return Err(AppError::Validation(
            "trading pair precision must be non-negative".to_owned(),
        ));
    }
    if min_order_value <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "min_order_value must be positive".to_owned(),
        ));
    }
    Ok(())
}
