//! 交易对配置与模拟行情策略的纯业务规则层，同时承担 K 线补偿预览令牌的签发与验签。
//!
//! 内容分三块：交易对侧负责符号归一与精度、最小下单额等数值约束；策略侧负责配置数值不变量、
//! 价格节点序列校验，以及把请求打包成落库的版本配置 JSON；补偿侧负责时间范围校验、缺口区间合并、
//! 稳定摘要计算和 HMAC 令牌的签发验签。
//! 全部函数都不触库、不加锁、不调度 worker，时间一律以 UTC 处理并要求对齐到整分钟；
//! 令牌只做自证性校验，与数据库当前版本、当前缺口是否仍匹配必须由应用层在执行时再次核对。

use super::*;

const RECOVERY_PREVIEW_TTL_MINUTES: i64 = 10;
const RECOVERY_PREVIEW_TOKEN_VERSION: &str = "v1";
const RECOVERY_AGGREGATE_INTERVALS: [&str; 5] = ["5m", "15m", "1h", "4h", "1d"];

/// 校验新交易对的 base/quote 资产不可相同，并复核符号、精度、最小下单额、状态和市场类型。
/// 资产是否存在、symbol 是否重复由创建事务查询确认；本纯规则不访问数据库。
/// 与更新版校验的差别在于状态和市场类型在创建请求里是可选项，仅在显式提供时才判定，
/// 缺省值由应用层补成停用与外部行情，因此这里放行的请求未必带有完整枚举值。
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
/// 分隔符归一意味着 btc/usdt、BTC_USDT 与 btc-usdt 会落到同一个符号，因此这三种写法在唯一约束下互相冲突。
/// 列表查询也复用本函数处理筛选值，保证检索口径与写入口径完全一致。
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
/// 该函数不检查停用时是否仍有未成交订单或持仓，这类业务约束当前不在任何一层强制，需要人工确认。
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
/// external 表示行情来自外部数据源，internal 与 strategy 表示由平台自行产出，
/// 其中只有 strategy 类型的交易对才能挂接模拟行情策略并参与手动 K 线补偿。
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
/// 创建、配置更新与状态切换三类操作共用这一份结构，因此可以直接对比前后值定位到底改了哪些字段。
/// 资产编号与展示名同时记录，即使资产后来改名也能从审计还原当时的展示内容。
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
/// 与更新版的差别有两点：这里额外要求交易对编号非零，且初始状态是可选项，未提供时由应用层落为草稿。
/// 价格节点同样在此一并校验，节点为空是允许的，表示策略只按起止价线性推进而不设中间锚点。
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
/// 更新请求不含交易对编号，因为策略绑定的交易对不可迁移；状态也不在此变更，需走独立的状态入口。
/// 节点数组是整体替换语义，这里校验的是替换后的完整序列而非增量。
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

/// 集中校验策略主配置的数值不变量，是创建与更新两条路径共用的判定。
/// 依次要求策略类型去空后非空、起始价与目标价均严格大于零、结束时间严格晚于开始时间、
/// 波动率与成交量上下界非负，且成交量上界不小于下界。
/// 起始价与目标价相等是允许的，表示价格在时段内围绕同一水平波动而非单向推进。
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
/// 与更新版配置的唯一区别是这里会写入 pair_id，因为首个版本需要自证绑定的交易对。
/// 生成的快照会被实时 worker 与手动补偿共同读取，其中显式写入的 nodes 数组即便为空也是权威值。
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
/// 省略 pair_id 是因为后续版本继承首版的交易对绑定，读取方需回溯策略主表而非从版本快照取。
/// 传入的状态来自锁定后的旧值而非请求，所以更新配置不会顺带改变策略的启停状态。
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

/// 把策略配置与节点序列序列化成版本快照 JSON，是创建版与更新版配置的共同实现。
/// 节点按数组下标补出 sequence_no，因此传入顺序即最终顺序，本函数不重排也不校验节点合法性。
/// 时间统一转成毫秒时间戳以避免时区歧义，金额与费率保持十进制原值不做浮点转换。
/// pair_id 为 None 时整个键都不会出现在结果里，而不是写成 null，读取方据此区分首版与后续版本。
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
/// 时间必须严格落在策略起止之间的开区间内，与起点或终点重合都会被拒，从而保证节点不与端点争夺同一分钟。
/// 相对前一节点的百分比以上一节点解析出的价格为基准逐级推进，因此中间某节点的改动会连锁影响其后全部节点。
/// 成交量上下界必须同时给出或同时省略，只给一侧视为非法；容差与波动率要求非负，错误信息中的节点序号从 1 开始计。
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
/// 上界比较使用当前时刻向下取整到分钟的结果，因此仍在形成中的当前分钟永远无法进入补偿范围。
/// 起点可以等于策略开始时间，终点可以等于策略结束时间，但终点必须严格大于起点，空范围被判为非法。
/// 返回的分钟数会被转换为 u32，超出该范围时报范围过大的校验错误而不是静默截断。
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
/// 载荷是以竖线分隔的定长七段文本并带版本前缀，签名覆盖包含过期时间在内的全部字段，
/// 最终形态为 URL 安全 Base64 载荷加点号加十六进制签名，因此可直接放进 JSON 与请求体传输。
/// 载荷未加密只做签名，调用方能自行解出其中的范围与摘要，这是可接受的，因为这些信息本就来自预览响应。
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
/// 校验顺序刻意先验签再解析字段，保证任何一个字段被改动都会在解释其含义之前就被拒绝。
/// 分段数必须恰好为八且首段版本标记要匹配，任何解析失败都归并为同一句「令牌无效」而不透出具体是哪段出错。
/// 只有签名密钥本身不可用才归类为内部错误；过期单独返回「令牌已过期」以便前端提示重新预览。
/// 通过验签只说明令牌自身完整，策略版本与缺口是否仍与当时一致必须由调用方另行核对。
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
/// 函数会先对入参排序再去重，因此调用方无需保证顺序，乱序输入也能得到相同的区间划分与摘要。
/// 相邻分钟被合并进同一区间，中断一分钟即另起一段；摘要按排序后的毫秒时间戳大端字节依次喂入哈希，
/// 所以摘要只取决于缺失分钟的集合本身，与区间如何切分无关。
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
/// 固定为 5m、15m、1h、4h、1d 五档，顺序稳定以便前端直接按序渲染；1m 本身不在列表内，因为它是重建来源而非聚合产物。
pub(crate) fn market_strategy_recovery_aggregate_intervals() -> Vec<String> {
    RECOVERY_AGGREGATE_INTERVALS
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// 计算预览令牌载荷的 HMAC-SHA256 并输出十六进制签名。
/// 密钥不可用时归类为内部错误而非校验错误，因为那属于服务配置问题而不是调用方输入问题。
fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> AppResult<String> {
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key)
        .map_err(|_| AppError::Internal("preview token signing key is invalid".to_owned()))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// 把时间向下取整到整分钟，用于求出最近一根已经闭合的 1m K 线的开盘时刻。
/// 采用欧几里得除法以保证 1970 年之前的负时间戳同样向下取整而不是向零取整。
/// 秒级时间戳在可表示范围内取整后必然仍可表示，因此此处对失败分支直接断言。
fn floor_utc_minute(value: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp(value.timestamp().div_euclid(60) * 60, 0)
        .expect("UTC minute timestamp must remain representable")
}

/// 将行情策略配置、运行进度、恢复状态和最近生成时间映射为完整审计快照。
/// 映射不读取版本表或运行事件；调用方在策略配置事务中保存锁后快照。
/// 快照同时覆盖静态配置与动态运行态两类字段，后者包括活动版本号、当前价、最近生成时刻与最近一根 K 线开盘时刻，
/// 因此即便本次只改配置，前后值里的运行态字段也可能因 worker 并发推进而出现差异。
/// 该结构被创建、更新、状态切换以及补偿执行四类操作共用，其中补偿只把它作为 before 值使用。
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

/// 校验交易对的数值不变量：价格精度与数量精度均不得为负，最小下单额必须严格大于零。
/// 创建与更新两条路径共用本判定，因此二者对精度和下单额的要求完全一致。
/// 这里只设下界不设上界，精度过大导致的展示或撮合问题由数据库列类型和上层配置约定兜住。
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
