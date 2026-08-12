use super::*;

/// 校验新交易对的 base/quote 资产不可相同，并复核符号、精度、最小下单额、状态和市场类型。
/// 资产是否存在、symbol 是否重复由创建事务查询确认；本纯规则不访问数据库。
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

/// 校验交易对更新后的完整配置快照，包括符号、精度、最小下单额、状态和市场类型。
/// 不判断当前状态是否允许迁移；应用层锁定交易对后负责并发与生命周期检查。
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

/// 将交易对符号去除空白、转为大写并把下划线或斜杠统一为连字符。
/// 仅接受不超过 64 字节的 ASCII 字母数字及 `-_/`；空值或非法字符返回校验错误，不查询符号唯一性。
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

/// 规范化交易对状态，仅接受后台合同支持的 active、disabled 或 maintenance 代码。
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

/// 规范化交易对市场类型，仅允许现货或合约等实现中明确列出的稳定代码。
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

/// 将交易对资产、符号、精度、最小订单额、状态和市场类型映射为后台审计快照。
/// 快照包含展示用资产名和创建时间但不含订单或行情状态；应用层负责随配置写入一并保存。
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

/// 校验新建行情策略的交易对、类型、正数价格、有效时段、波动率及成交量上下界。
/// 该纯规则不访问数据库；可选初始状态非法或任一数值组合不成立时返回校验错误。
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

/// 校验行情策略更新中的类型、正数价格、有效时段、非负波动率和成交量上下界。
/// 该纯规则不读取策略状态或访问数据库；活跃策略禁止更新的并发约束由应用层锁行后判断。
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

/// 规范化行情策略目标状态，仅允许草稿、启用、暂停或禁用四种稳定代码。
/// 该纯规则不判断状态迁移合法性、不访问数据库；空白或未知状态直接返回校验错误。
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

/// 将领域策略状态映射为运行检查点状态：active/running、paused/paused、disabled/stopped，其余为 draft。
/// 该总映射不校验未知输入，也不启动或停止 worker；应用用例负责先规范化状态并持久化结果。
pub(crate) fn market_strategy_run_status(status: &str) -> &'static str {
    match status {
        "active" => "running",
        "paused" => "paused",
        "disabled" => "stopped",
        _ => "draft",
    }
}

/// 把新建策略请求连同 pair_id、market_type 和目标状态组装为首个版本的 JSON 配置。
/// strategy_type 会去除首尾空白，时间转为毫秒时间戳；函数不再校验数值、不推进版本或触发运行器。
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

/// 把策略更新请求、market_type 和当前状态组装为不含 pair_id 的后续版本 JSON 配置。
/// 输出保留价格、时段、波动率和量级快照并规范化 strategy_type；持久化版本、事件和审计由更新事务完成。
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

/// 将行情策略配置、运行进度、恢复状态和最近生成时间映射为完整审计快照。
/// 映射不读取版本表或运行事件；调用方在策略配置事务中保存锁后快照。
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
