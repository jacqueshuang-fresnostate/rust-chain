use super::*;

const RECOVERY_PREVIEW_TTL_MINUTES: i64 = 10;
const RECOVERY_PREVIEW_TOKEN_VERSION: &str = "v1";
const RECOVERY_AGGREGATE_INTERVALS: [&str; 5] = ["5m", "15m", "1h", "4h", "1d"];

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

/// 规范化交易对状态，仅接受后台合同支持的 active 或 disabled 代码。
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

/// 规范化交易对市场类型，仅允许外部、内部或策略行情的稳定代码。
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
    validate_market_strategy_nodes(
        &request.nodes,
        &request.start_price,
        request.start_time,
        request.end_time,
    )?;
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
    })?;
    validate_market_strategy_nodes(
        &request.nodes,
        &request.start_price,
        request.start_time,
        request.end_time,
    )
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
        nodes: &request.nodes,
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
        nodes: &request.nodes,
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
    nodes: &'a [MarketStrategyNodeRequest],
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
        "nodes": config.nodes.iter().enumerate().map(|(index, node)| json!({
            "sequence_no": index,
            "target_time": node.target_time.timestamp_millis(),
            "target_type": node.target_type,
            "target_value": node.target_value,
            "execution_mode": node.execution_mode,
            "tolerance": node.tolerance,
            "volatility": node.volatility,
            "volume_min": node.volume_min,
            "volume_max": node.volume_max,
        })).collect::<Vec<_>>(),
    });
    if let Some(pair_id) = config.pair_id {
        value["pair_id"] = json!(pair_id);
    }
    value
}

/// 校验策略节点的 UTC 分钟对齐、严格递增顺序、开区间时间边界、枚举和数值不变量。
/// 目标值依次按绝对价、相对起始价或相对前一节点解析；任一节点最终价格非正时整个请求失败。
/// 节点允许为空以兼容旧策略；本函数无 I/O，不重排或默默修正管理员提交的节点。
pub(crate) fn validate_market_strategy_nodes(
    nodes: &[MarketStrategyNodeRequest],
    start_price: &BigDecimal,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> AppResult<()> {
    let zero = BigDecimal::from(0);
    let hundred = BigDecimal::from(100);
    let mut previous_time = None;
    let mut previous_price = start_price.clone();
    for (index, node) in nodes.iter().enumerate() {
        if node.target_time.timestamp_subsec_millis() != 0
            || node.target_time.timestamp() % 60 != 0
            || node.target_time <= start_time
            || node.target_time >= end_time
            || previous_time.is_some_and(|previous| node.target_time <= previous)
        {
            return Err(AppError::Validation(
                "market strategy node times must be UTC-minute aligned, strictly increasing, and inside strategy range".to_owned(),
            ));
        }
        if !matches!(
            node.target_type.as_str(),
            "absolute_price" | "percent_from_start" | "percent_from_previous"
        ) {
            return Err(AppError::Validation(
                "unsupported market strategy node target_type".to_owned(),
            ));
        }
        if !matches!(node.execution_mode.as_str(), "hard" | "soft" | "range") {
            return Err(AppError::Validation(
                "unsupported market strategy node execution_mode".to_owned(),
            ));
        }
        let resolved_price = match node.target_type.as_str() {
            "absolute_price" => node.target_value.clone(),
            "percent_from_start" => {
                start_price * (BigDecimal::from(1) + (&node.target_value / &hundred))
            }
            "percent_from_previous" => {
                &previous_price * (BigDecimal::from(1) + (&node.target_value / &hundred))
            }
            _ => unreachable!("target_type was validated above"),
        };
        if resolved_price <= zero {
            return Err(AppError::Validation(format!(
                "market strategy node {} resolves to a non-positive price",
                index + 1
            )));
        }
        if node.tolerance < zero || node.volatility < zero {
            return Err(AppError::Validation(
                "market strategy node tolerance and volatility must be non-negative".to_owned(),
            ));
        }
        match (&node.volume_min, &node.volume_max) {
            (None, None) => {}
            (Some(minimum), Some(maximum)) if minimum >= &zero && maximum >= minimum => {}
            _ => {
                return Err(AppError::Validation(
                    "market strategy node volume range is invalid".to_owned(),
                ));
            }
        }
        previous_time = Some(node.target_time);
        previous_price = resolved_price;
    }
    Ok(())
}

/// 校验缺口预览的 `[range_start,range_end)` 半开范围：两端须为 UTC 分钟且位于策略半开时段内，
/// `range_end` 还不得越过当前分钟；返回分钟差用于令牌摘要和任务预期值，不访问存储。
pub(crate) fn validate_market_strategy_recovery_range(
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
    strategy_start: DateTime<Utc>,
    strategy_end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> AppResult<u32> {
    if range_start.timestamp_subsec_millis() != 0
        || range_end.timestamp_subsec_millis() != 0
        || range_start.timestamp() % 60 != 0
        || range_end.timestamp() % 60 != 0
        || range_end <= range_start
        || range_start < strategy_start
        || range_end > strategy_end
        || range_end > floor_utc_minute(now)
    {
        return Err(AppError::Validation(
            "recovery range must contain closed UTC-minute slots inside strategy range".to_owned(),
        ));
    }
    let count = (range_end - range_start).num_minutes();
    u32::try_from(count).map_err(|_| AppError::Validation("recovery range is too large".to_owned()))
}

/// 规范化补偿任务状态筛选，只接受迁移约束中的 pending/running/completed/failed。
/// 本函数仅校验后台查询条件，不读取任务表、不推进执行状态；空白或未知值返回校验错误，
/// 防止列表行与总数查询使用不同谓词，也避免管理员传入任意状态代码绕过审计视图。
pub(crate) fn validate_market_strategy_recovery_job_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    if matches!(
        status.as_str(),
        "pending" | "running" | "completed" | "failed"
    ) {
        Ok(status)
    } else {
        Err(AppError::Validation(
            "unsupported recovery job status".to_owned(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarketStrategyPreviewTokenClaims {
    pub(crate) strategy_id: u64,
    pub(crate) config_version: i32,
    pub(crate) range_start: DateTime<Utc>,
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) one_minute_count: u32,
    pub(crate) gap_digest: String,
    pub(crate) expires_at: DateTime<Utc>,
}

/// 签发 K 线补偿预览令牌的结构化输入；字段与令牌声明一一对应。
/// 将范围、根数和缺口摘要聚合后，可避免呼叫方交换同类参数而签发错误声明。
#[derive(Debug, Clone, Copy)]
pub(crate) struct MarketStrategyPreviewTokenInput<'a> {
    pub(crate) strategy_id: u64,
    pub(crate) config_version: i32,
    pub(crate) range_start: DateTime<Utc>,
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) one_minute_count: u32,
    pub(crate) gap_digest: &'a str,
}

/// 签发有效期十分钟的 HMAC-SHA256 预览令牌，将策略、版本、范围、根数和缺口摘要绑定。
/// 令牌本身不持久化；执行路径必须再验签并对照当前版本/缺口，防止预览后盲写。
pub(crate) fn issue_market_strategy_preview_token(
    key: &[u8],
    input: MarketStrategyPreviewTokenInput<'_>,
    now: DateTime<Utc>,
) -> AppResult<(String, DateTime<Utc>)> {
    let expires_at = now + chrono::Duration::minutes(RECOVERY_PREVIEW_TTL_MINUTES);
    let payload = format!(
        "{RECOVERY_PREVIEW_TOKEN_VERSION}|{}|{}|{}|{}|{}|{}|{}",
        input.strategy_id,
        input.config_version,
        input.range_start.timestamp_millis(),
        input.range_end.timestamp_millis(),
        input.one_minute_count,
        input.gap_digest,
        expires_at.timestamp_millis()
    );
    let signature = hmac_sha256_hex(key, payload.as_bytes())?;
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(payload.as_bytes());
    Ok((format!("{encoded}.{signature}"), expires_at))
}

/// 验证预览令牌的 HMAC、版本标记和过期时间，再返回可供应用层对照数据库的结构化声明。
/// 签名比较使用 HMAC `verify_slice`，篡改、格式错误或过期都映射为明确的校验错误，不查库不写入。
pub(crate) fn verify_market_strategy_preview_token(
    key: &[u8],
    token: &str,
    now: DateTime<Utc>,
) -> AppResult<MarketStrategyPreviewTokenClaims> {
    let (encoded, signature_hex) = token
        .trim()
        .split_once('.')
        .ok_or_else(|| AppError::Validation("preview_token is invalid".to_owned()))?;
    let payload = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let signature = hex::decode(signature_hex)
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key)
        .map_err(|_| AppError::Internal("preview token signing key is invalid".to_owned()))?;
    mac.update(&payload);
    mac.verify_slice(&signature)
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let payload = String::from_utf8(payload)
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let fields = payload.split('|').collect::<Vec<_>>();
    if fields.len() != 8 || fields[0] != RECOVERY_PREVIEW_TOKEN_VERSION {
        return Err(AppError::Validation("preview_token is invalid".to_owned()));
    }
    let parse = |index: usize| {
        fields[index]
            .parse::<i64>()
            .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))
    };
    let strategy_id = fields[1]
        .parse::<u64>()
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let config_version = fields[2]
        .parse::<i32>()
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let range_start = DateTime::from_timestamp_millis(parse(3)?)
        .ok_or_else(|| AppError::Validation("preview_token is invalid".to_owned()))?;
    let range_end = DateTime::from_timestamp_millis(parse(4)?)
        .ok_or_else(|| AppError::Validation("preview_token is invalid".to_owned()))?;
    let one_minute_count = fields[5]
        .parse::<u32>()
        .map_err(|_| AppError::Validation("preview_token is invalid".to_owned()))?;
    let expires_at = DateTime::from_timestamp_millis(parse(7)?)
        .ok_or_else(|| AppError::Validation("preview_token is invalid".to_owned()))?;
    if expires_at <= now {
        return Err(AppError::Validation("preview_token has expired".to_owned()));
    }
    Ok(MarketStrategyPreviewTokenClaims {
        strategy_id,
        config_version,
        range_start,
        range_end,
        one_minute_count,
        gap_digest: fields[6].to_owned(),
        expires_at,
    })
}

/// 对按时间排序的缺失分钟进行连续段合并为半开区间，同时生成令牌绑定所需的稳定 SHA-256 摘要。
/// 本纯函数不查询 Mongo；输入重复时会去重，以保证同一逻辑缺口跨扫描批次摘要一致。
pub(crate) fn summarize_market_strategy_gaps(
    mut missing: Vec<DateTime<Utc>>,
) -> (Vec<MarketStrategyGapRangeResponse>, String) {
    missing.sort_unstable();
    missing.dedup();
    let mut ranges: Vec<MarketStrategyGapRangeResponse> = Vec::new();
    for open_time in &missing {
        if let Some(last) = ranges.last_mut()
            && *open_time == last.range_end
        {
            last.range_end = *open_time + chrono::Duration::minutes(1);
            last.one_minute_count += 1;
            continue;
        }
        ranges.push(MarketStrategyGapRangeResponse {
            range_start: *open_time,
            range_end: *open_time + chrono::Duration::minutes(1),
            one_minute_count: 1,
        });
    }
    let mut digest = sha2::Sha256::new();
    for open_time in missing {
        digest.update(open_time.timestamp_millis().to_be_bytes());
    }
    (ranges, hex::encode(digest.finalize()))
}

/// 返回手动补偿会从权威 1m 重建的稳定聚合周期列表，供预览和 Web 展示共享。
pub(crate) fn market_strategy_recovery_aggregate_intervals() -> Vec<String> {
    RECOVERY_AGGREGATE_INTERVALS
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> AppResult<String> {
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key)
        .map_err(|_| AppError::Internal("preview token signing key is invalid".to_owned()))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn floor_utc_minute(value: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp(value.timestamp().div_euclid(60) * 60, 0)
        .expect("UTC minute timestamp must remain representable")
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
        "active_version": strategy.active_version,
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
