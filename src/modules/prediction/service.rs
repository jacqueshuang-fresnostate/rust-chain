//! prediction bounded context service layer.
//!
//! 服务层：封装可复用业务规则、数据格式化与纯计算逻辑。
//! 本文件集中三类内容：一是状态与策略的字符串常量，它们同时是数据库枚举值和对外接口取值，
//! 二是请求参数与后台配置的校验和归一化函数，三是 Polymarket 上游载荷的解析与容错函数。
//! 全部函数都是无 I/O 的纯函数：不访问数据库、不发起 HTTP、不开事务、不改钱包、不写日志。
//! 归一化函数一律先裁剪空白再转小写，因此对上游与前端的大小写差异不敏感；
//! 但除结算结果与终局结果外都不接受同义词，未知取值直接拒绝而不静默降级。
//! 概率类数值统一约束在开区间零到一之间，落库前收敛到 0.01 与 0.99 并保留 8 位小数；
//! 金额类数值使用 `BigDecimal` 且按资产自身的精度位数校验，本层不做四舍五入。
//! 解析上游载荷时遵循「宁缺勿造」：无法解析的元素直接跳过而不伪造零值，
//! 只有概率与结果标签这类有明确业务默认的字段才会补默认，且默认值在各函数注释中写明。

use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::HashSet, str::FromStr};

pub(crate) const STATUS_ACTIVE: &str = "active";
pub(crate) const STATUS_HIDDEN: &str = "hidden";
pub(crate) const SETTLEMENT_OPEN: &str = "open";
pub(crate) const SETTLEMENT_PENDING_CONFIRMATION: &str = "pending_confirmation";
pub(crate) const SETTLEMENT_SETTLED: &str = "settled";
pub(crate) const SETTLEMENT_REFUNDED: &str = "refunded";
pub(crate) const ORDER_STATUS_OPEN: &str = "open";
pub(crate) const OUTCOME_YES: &str = "yes";
pub(crate) const OUTCOME_NO: &str = "no";
pub(crate) const OUTCOME_INVALID: &str = "invalid";
pub(crate) const SETTLEMENT_MODE_MANUAL: &str = "manual_confirm";
pub(crate) const SETTLEMENT_MODE_AUTO: &str = "auto";
pub(crate) const REFUND_STAKE_AND_FEE: &str = "refund_stake_and_fee";
pub(crate) const REFUND_STAKE_ONLY: &str = "refund_stake_only";
pub(crate) const REFUND_MANUAL: &str = "manual";
pub(crate) const DEFAULT_SYNC_POLL_SECONDS: u64 = 30;
pub(crate) const DEFAULT_SYNC_LIMIT: &str = "100";
pub(crate) const POLYMARKET_GAMMA_EVENTS_URL: &str = "https://gamma-api.polymarket.com/events";
pub(crate) const REF_TYPE_PREDICTION_ORDER: &str = "prediction_order";
const ADMIN_AUDIT_REASON_MAX_LEN: usize = 512;

#[derive(Debug, Default)]
pub(crate) struct SyncCounts {
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
}

#[derive(Debug)]
pub(crate) struct EffectiveMarketConfig {
    pub(crate) allowed_asset_ids: Vec<u64>,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) payout_cap_overrides: Option<Value>,
}

#[derive(Debug)]
pub(crate) struct ParsedPolymarketMarket {
    pub(crate) external_event_id: Option<String>,
    pub(crate) external_market_id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) tags_json: Value,
    pub(crate) outcome_yes_label: String,
    pub(crate) outcome_no_label: String,
    pub(crate) yes_price: BigDecimal,
    pub(crate) no_price: BigDecimal,
    pub(crate) volume: Option<BigDecimal>,
    pub(crate) liquidity: Option<BigDecimal>,
    pub(crate) end_at: Option<DateTime<Utc>>,
    pub(crate) source_status: String,
    pub(crate) external_resolution: Option<String>,
    pub(crate) payload: Value,
}

/// 将预测市场分页条数默认设为 50，并夹取到 1 到 200 的闭区间。
/// 采用夹取而非报错，使超范围参数退化为边界值而不让整个请求失败；传 0 会被抬到 1。
/// 上限取 200 而非其他列表常见的 100，是为了容纳后台市场与订单的批量查看。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// 把分页偏移缺省为 0 并封顶在十万，超大 offset 会让日志类大表退化为全表扫描加文件排序。
/// 这里只封上界不封下界，因为参数本身无符号，负偏移在反序列化阶段就已被拒。
/// 超限时静默截断而不报错，代价是深翻页会停在第十万条，属于有意的可用性取舍。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 裁剪可选筛选文本并把纯空白折成 `None`，使「未传」与「传了空串」在下游等价。
/// 这一等价对查询构造很关键：`None` 表示不追加该筛选条件，
/// 若保留空串则会拼出永远匹配不到任何行的等值条件。
/// 只裁剪首尾空白，不改大小写、不做长度校验，也不拒绝任何取值。
pub(crate) fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
}

/// 裁剪必填文本并校验长度；空值或超限在构建 SQL 和上游请求前返回参数错误。
/// 返回的是裁剪后的新串，调用方须使用返回值而不是原始入参，否则首尾空白会被写进数据库。
/// 长度按字节数而非字符数计量，因此中文等多字节内容的实际可填字符数少于 `max_len`，
/// 这与数据库列的字节长度限制方向一致，属于有意为之的保守判定。
/// `field` 只参与错误文案，用于告知调用方是哪个字段越界。
pub(crate) fn required_text(value: String, field: &str, max_len: usize) -> AppResult<String> {
    let normalized = value.trim().to_owned();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if normalized.len() > max_len {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(normalized)
}

/// 从 `user:{id}` 会话 subject 解析用户编号，格式不符返回 Unauthorized。
/// 前缀缺失或数字部分溢出 `u64` 都归为鉴权失败而非参数错误，
/// 因为这类 subject 只可能来自伪造或过期令牌，不应把解析细节回吐给调用方。
/// 解析结果是下注、查单与钱包变动的租户隔离依据，绝不允许由请求体覆盖。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从 `admin:{id}` 会话 subject 解析管理员编号，结果只用于配置审计的 actor，绝不接受请求体覆盖。
/// 前缀缺失、空编号、非数字或数值溢出统一返回 Unauthorized，避免向调用方泄露令牌内部格式。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 归一预测配置写入的审计原因；缺失、空白或超过审计列上限时在开启事务前拒绝请求。
/// 返回值已裁剪首尾空白，应用层必须把该值原样交给事务审计，不能继续使用未经校验的请求字段。
pub(crate) fn required_admin_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = reason.map(|value| value.trim().to_owned()) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.is_empty() {
        return Err(AppError::Validation("reason is required".to_owned()));
    }
    if reason.chars().count() > ADMIN_AUDIT_REASON_MAX_LEN {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

/// 金额必须严格为正，零和负数一并拒绝，失败时不得创建报价、订单、钱包变更或流水。
/// 这是下注路径的第一道守卫，必须在开启事务与加锁之前调用，避免为无效请求占用行锁。
/// 只判正负不判精度，小数位是否超出资产限制由 `ensure_amount_precision` 单独把关。
pub(crate) fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!("{field} must be positive")));
    }
    Ok(())
}

/// 后台费率、赔付上限等配置字段不得为负，非法值在保存设置前返回参数错误。
/// 与下注金额的守卫不同，此处允许取零：零费率表示免手续费，零上限表示不设赔付封顶，
/// 两者都是有效配置，因此不能沿用「必须为正」的判定。
/// 该校验只保证符号，不校验费率是否小于一，也不校验上限与投注额的相对关系。
pub(crate) fn ensure_non_negative_decimal(value: &BigDecimal, field: &str) -> AppResult<()> {
    if value < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{field} must not be negative"
        )));
    }
    Ok(())
}

/// 按资产自身的 `precision_scale` 校验金额小数位，超限金额不得进入钱包和账本。
/// 判定委托给钱包上下文的共享规则，保证预测下注与其他业务对同一资产使用同一套精度口径。
/// 这里只拒绝不截断：超限金额直接报错而非静默舍入，
/// 避免用户实际支付额与其提交额不一致而产生对账争议。
/// 校验通过的金额在落库时仍受数据库列小数位约束，本函数不替代该约束。
pub(crate) fn ensure_amount_precision(
    amount: &BigDecimal,
    precision_scale: i32,
    field: &str,
) -> AppResult<()> {
    use crate::modules::wallet::amount_fits_asset_precision;

    if !amount_fits_asset_precision(amount, precision_scale) {
        return Err(AppError::Validation(format!(
            "{field} exceeds asset precision scale {precision_scale}"
        )));
    }
    Ok(())
}

/// 概率价格必须严格位于零与一之间，端点和越界值不得进入报价与赔付计算。
/// 两个端点同样被拒：价格为零会让赔付倍数无界，价格为一则意味着无收益，两者都无法构成有效下注。
/// 与 `clamp_probability` 的取舍不同，此处面向用户输入选择直接拒绝而非收敛，
/// 因为静默改价会让用户按未同意的赔率成交；收敛只用于容忍上游同步来的异常值。
pub(crate) fn ensure_probability_price(price: &BigDecimal) -> AppResult<()> {
    if price <= &BigDecimal::from(0) || price >= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "prediction probability price must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

/// 归一化用户下注方向，只接受 yes 或 no 两种二元结果，未知取值不得用于建单。
/// 输入先裁剪空白再转小写，因此大小写与首尾空格差异会被吸收。
/// 与结算结果的归一化不同，此处刻意不接受 invalid：
/// 用户只能押注某一方，「无效」是市场的终局形态而非可下注的方向。
pub(crate) fn normalize_binary_outcome(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(OUTCOME_YES.to_owned()),
        "no" => Ok(OUTCOME_NO.to_owned()),
        _ => Err(AppError::Validation(
            "prediction outcome must be yes or no".to_owned(),
        )),
    }
}

/// 归一化后台提交的结算结果，把 yes 与 no 规范为小写，
/// 并把 invalid 及英美两种拼写的 cancelled、canceled 统一折成 invalid；其他输入返回参数错误。
/// 三个取值直接决定资金去向：yes 或 no 触发按方向派奖，invalid 触发按退款策略返还，
/// 因此这里不设默认值，任何无法识别的输入都必须让结算整体失败而不是猜测。
pub(crate) fn normalize_settlement_result(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(OUTCOME_YES.to_owned()),
        "no" => Ok(OUTCOME_NO.to_owned()),
        "invalid" | "cancelled" | "canceled" => Ok(OUTCOME_INVALID.to_owned()),
        _ => Err(AppError::Validation(
            "prediction settlement result must be yes, no, or invalid".to_owned(),
        )),
    }
}

/// 归一化结算模式，只接受人工确认与自动结算两种，非法后台配置不会生效。
/// 人工确认要求运营在上游给出终局结果后仍需显式操作才派奖，
/// 自动模式则允许同步流程在读到终局结果时直接推进结算。
/// 该配置直接影响资金何时离开平台账户，因此未知取值一律拒绝而不回退到较保守的一方。
pub(crate) fn normalize_settlement_mode(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        SETTLEMENT_MODE_MANUAL => Ok(SETTLEMENT_MODE_MANUAL.to_owned()),
        SETTLEMENT_MODE_AUTO => Ok(SETTLEMENT_MODE_AUTO.to_owned()),
        _ => Err(AppError::Validation(
            "settlement mode must be manual_confirm or auto".to_owned(),
        )),
    }
}

/// 归一化市场判定为无效时的退款策略，未知取值不得触发任何资金返还。
/// 三种策略分别是连手续费一并退还、只退本金而手续费不退，以及完全交由人工逐笔处理。
/// 该配置决定无效市场退款时用户实际拿回多少钱，属于资金口径而非展示选项，
/// 因此不接受同义词也不设默认值，非法输入必须让配置保存整体失败。
pub(crate) fn normalize_invalid_refund_policy(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        REFUND_STAKE_AND_FEE => Ok(REFUND_STAKE_AND_FEE.to_owned()),
        REFUND_STAKE_ONLY => Ok(REFUND_STAKE_ONLY.to_owned()),
        REFUND_MANUAL => Ok(REFUND_MANUAL.to_owned()),
        _ => Err(AppError::Validation(
            "invalid refund policy is unsupported".to_owned(),
        )),
    }
}

/// 归一化市场对用户的展示状态，只接受可见与隐藏两种，避免后台配置产生未定义可见性。
/// 该状态只控制市场是否出现在用户侧列表，不影响已有订单的结算与派奖，
/// 因此把市场改为隐藏不会冻结资金，也不会阻止到期结算。
/// 未知取值一律拒绝，防止因拼写错误让本应下架的市场继续对外可见。
pub(crate) fn normalize_display_status(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        STATUS_ACTIVE => Ok(STATUS_ACTIVE.to_owned()),
        STATUS_HIDDEN => Ok(STATUS_HIDDEN.to_owned()),
        _ => Err(AppError::Validation(
            "display_status must be active or hidden".to_owned(),
        )),
    }
}

/// 把上游给出的终局结果规范为 yes、no 或 invalid，其他值返回空以保持未决而不自动结算。
/// 与后台提交结果的归一化相比，这里用 `Option` 而非错误：上游字段五花八门，
/// 读到看不懂的值属于常态，只应放弃自动结算而不该让整轮同步失败。
/// 取消类文案同样折成 invalid，兼容上游 cancelled 与 canceled 两种拼写。
/// 返回空是保守选择，市场会停在未决状态等待人工确认，绝不会因此误派奖。
pub(crate) fn normalize_external_resolution(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(OUTCOME_YES.to_owned()),
        "no" => Some(OUTCOME_NO.to_owned()),
        "invalid" | "canceled" | "cancelled" => Some(OUTCOME_INVALID.to_owned()),
        _ => None,
    }
}

/// 按首次出现顺序去重无符号标识，同时剔除零值，保持后台资产范围输入的确定性。
/// 零被视为无效资产编号而非合法取值，因此会被静默丢弃而不报错。
/// 保留首次出现顺序而不排序，使后台配置的书写次序在存回 JSON 后仍然稳定，
/// 避免每次保存都产生无意义的字段差异。
pub(crate) fn unique_u64_list(values: Vec<u64>) -> Vec<u64> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| *value > 0 && seen.insert(*value))
        .collect()
}

/// 裁剪、去空并去重文本集合，保持首次出现顺序供同步配置稳定使用。
/// 去重在裁剪之后进行，因此仅首尾空白不同的两项会被合并为一项。
/// 大小写不做归一，`BTC` 与 `btc` 视为不同项，这与上游标签区分大小写的实际情况一致。
/// 与无符号标识去重函数共用「保序不排序」的策略，保证配置回写时字段内容稳定。
pub(crate) fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
}

/// 从持久化 JSON 读取无符号标识数组，非法项被逐个忽略且不触发任何写入。
/// 同时接受数字与数字字符串两种表示，兼容历史配置里以字符串保存资产编号的写法。
/// 顶层不是数组时返回空列表而不是报错，因为配置列可能为 null 或旧结构，
/// 让读取失败退化为「没有配置任何资产」比中断整个查询更合适。
/// 本函数不去重也不过滤零值，需要这些语义时应再经 `unique_u64_list` 处理。
pub(crate) fn json_u64_array(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64().or_else(|| item.as_str()?.parse::<u64>().ok()))
                .collect()
        })
        .unwrap_or_default()
}

/// 从 JSON 提取文本数组，兼容上游把数组整体编码成字符串再传回的常见做法。
/// 入参本身是字符串时先尝试二次解析，解析失败退化为空数组而不是把整串当作单个元素。
/// 数组元素既可以是纯字符串，也可以是带 label 或 name 字段的对象，
/// 后者按 label 优先、name 次之的顺序取值，用于兼容 Polymarket 的 outcomes 与 tokens 两种结构。
/// 两者都取不到的元素被跳过；顶层不是数组时返回空列表。
/// 保留原始顺序至关重要，因为调用方按下标区分 yes 与 no 两个结果标签。
pub(crate) fn json_string_array(value: &Value) -> Vec<String> {
    let parsed = match value {
        Value::String(text) => {
            serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::Array(Vec::new()))
        }
        other => other.clone(),
    };
    parsed
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(text) = item.as_str() {
                        Some(text.to_owned())
                    } else {
                        item.get("label")
                            .or_else(|| item.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 从上游 JSON 提取十进制数组，无法解析的元素被跳过而不会被伪造为零。
/// 与文本数组提取一样先处理「数组被编码成字符串」的情形，
/// 但二次解析失败时退化为 null 而非空数组，最终同样得到空列表。
/// 跳过而非补零是刻意的：概率数组按下标对应 yes 与 no，
/// 补零会让缺失项变成一个看似合法的极端概率，进而算出错误赔率。
/// 因此调用方必须自行检查长度，不能假定返回的元素个数与上游字段一致。
pub(crate) fn json_decimal_array(value: &Value) -> Vec<BigDecimal> {
    let parsed = match value {
        Value::String(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
        other => other.clone(),
    };
    parsed
        .as_array()
        .map(|items| items.iter().filter_map(decimal_from_json).collect())
        .unwrap_or_default()
}

/// 将 JSON 数字或字符串精确解析为十进制值，布尔、null、数组与对象一律返回空。
/// 数字分支先转成字符串再解析，绕开浮点中间表示，因此高精度小数不会在解析途中丢位。
/// 字符串分支会先裁剪首尾空白，容忍上游在数值周围附带的空格。
/// 这是本文件所有金额与概率解析的底层入口，保持「解析不了就返回空」而非抛错，
/// 由上层决定是跳过该元素还是回退到业务默认值。
pub(crate) fn decimal_from_json(value: &Value) -> Option<BigDecimal> {
    match value {
        Value::Number(number) => BigDecimal::from_str(&number.to_string()).ok(),
        Value::String(text) => BigDecimal::from_str(text.trim()).ok(),
        _ => None,
    }
}

/// 按候选字段顺序返回首个存在的值，统一兼容 Polymarket 多版本字段名。
/// 命中的字段若是字符串且内容本身是合法 JSON，则返回二次解析后的结构而非原始字符串，
/// 这样调用方无需关心上游把数组编码成字符串还是直接给出数组。
/// 二次解析失败时返回原始字符串值；一旦某个键存在就立即返回，
/// 即使其值为 null 也不会继续尝试后续候选键，因此候选顺序即优先级。
pub(crate) fn first_jsonish_value(value: &Value, keys: &[&str]) -> Option<Value> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str()
            && let Ok(parsed) = serde_json::from_str::<Value>(text)
        {
            return Some(parsed);
        }
        return Some(candidate.clone());
    }
    None
}

/// 按候选字段顺序读取首个可作为文本使用的值，不修改上游载荷。
/// 字符串原样返回；数字与布尔转成其 JSON 字面量文本，
/// 用于兼容上游有时把市场标识写成数字、有时写成字符串的情况。
/// null、数组与对象不被视为文本，会继续尝试下一个候选键，
/// 这与 `first_jsonish_value` 的「键存在即返回」不同，两者不可互换。
/// 返回值不做裁剪或空串过滤，调用方若要求非空需自行判断。
pub(crate) fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str() {
            return Some(text.to_owned());
        }
        if candidate.is_number() || candidate.is_boolean() {
            return Some(candidate.to_string());
        }
    }
    None
}

/// 按候选字段顺序读取首个可精确解析为十进制的值，非法值不回退为零。
/// 与文本读取不同，此处会跳过存在但无法解析的键继续尝试后续候选，
/// 因此上游把成交量写成空串时仍有机会从备用字段取到有效数值。
/// 全部候选都取不到时返回空，调用方据此把成交量、流动性等字段留空而不是记为零，
/// 避免前端把「上游未提供」误显示成「确实为零」。
pub(crate) fn first_decimal(value: &Value, keys: &[&str]) -> Option<BigDecimal> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(decimal_from_json))
}

/// 从上游 JSON 读取布尔字段，字段缺失或非布尔类型一律按 false 处理。
/// 不接受字符串形式的 true 与 false，也不把数字视为真值，因此上游改用文本表示时会退化为假。
/// 该保守默认对调用点是安全的：判断市场是否已关闭时取不到值即视为仍开放，
/// 结果是市场继续可见而非被误下架，后续同步仍有机会纠正。
pub(crate) fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// 把形态各异的 Polymarket 响应体拉平成一组市场级 JSON，供解析器统一处理。
/// 按四种形态依次尝试：顶层直接是数组、顶层对象带 markets 数组、
/// 顶层对象带 events 或 data 或 items 数组，三类都不匹配时返回空列表。
/// 顶层就是单个事件时，其下每个市场都会被补入事件级上下文再输出，
/// 因此调用方拿到的每一项都已是可独立解析的完整市场。
/// 返回空列表表示本次响应没有可用市场，属于正常结果而非错误，同步流程应视为零条处理。
pub(crate) fn extract_market_values(payload: Value) -> Vec<Value> {
    if let Some(items) = payload.as_array() {
        return items
            .iter()
            .flat_map(extract_market_values_from_item)
            .collect();
    }
    if let Some(markets) = payload.get("markets").and_then(Value::as_array) {
        return markets
            .iter()
            .map(|market| merge_event_context(&payload, market))
            .collect();
    }
    for key in ["events", "data", "items"] {
        if let Some(items) = payload.get(key).and_then(Value::as_array) {
            return items
                .iter()
                .flat_map(extract_market_values_from_item)
                .collect();
        }
    }
    Vec::new()
}

/// 把数组中的单个元素展开为一到多个市场级 JSON，是拉平函数处理每一项时的分支。
/// 元素带 markets 数组时视为事件，逐个市场补入事件级上下文后展开；
/// 否则视为市场本身，原样克隆成单元素列表返回。
/// 只向下展开一层，不递归处理嵌套事件，因为上游结构最多只有事件套市场两级。
/// 本函数不发起网络请求也不写库，输入载荷保持只读。
pub(crate) fn extract_market_values_from_item(item: &Value) -> Vec<Value> {
    if let Some(markets) = item.get("markets").and_then(Value::as_array) {
        return markets
            .iter()
            .map(|market| merge_event_context(item, market))
            .collect();
    }
    vec![item.clone()]
}

/// 把事件级的编号、别名、分类、图片和标签补入市场级载荷，使每个市场都能独立解析。
/// 补入遵循「市场自身优先」：目标键已存在时一律跳过，即便其值为 null 也不覆盖，
/// 因此同一事件下各市场的自有信息不会被事件级信息冲掉。
/// 每个目标键各有一组候选来源键，按顺序取事件上首个存在的值，兼容上游多版本命名。
/// 市场不是 JSON 对象时原样返回，不做任何补写。
/// 事件与市场两个入参都只读，返回的是市场的克隆副本，调用方可安全并发使用同一事件。
pub(crate) fn merge_event_context(event: &Value, market: &Value) -> Value {
    let mut merged = market.clone();
    let Some(object) = merged.as_object_mut() else {
        return merged;
    };
    for (target, keys) in [
        ("eventId", &["id", "eventId", "event_id"][..]),
        ("eventSlug", &["slug", "eventSlug", "event_slug"][..]),
        ("category", &["category", "categorySlug"][..]),
        ("image", &["image", "icon", "imageUrl"][..]),
    ] {
        if object.get(target).is_none()
            && let Some(value) = keys.iter().find_map(|key| event.get(*key)).cloned()
        {
            object.insert(target.to_owned(), value);
        }
    }
    if object.get("tags").is_none()
        && let Some(tags) = event.get("tags").cloned()
    {
        object.insert("tags".to_owned(), tags);
    }
    merged
}

/// 解析 Polymarket 事件与市场载荷，校验标识、标题、概率及可选终局结果。
/// 只有市场标识与标题是必填：两者缺失分别返回校验错误，使该条市场被跳过而不落库；
/// 标题还要求裁剪后非空，纯空白视同缺失。其余字段缺失一律留空或取业务默认。
/// 结果标签取自 outcomes 或 tokens 数组的前两项，缺失时分别默认为 Yes 与 No。
/// 概率取自 outcomePrices 或 prices 数组：首项缺失时默认 0.5，
/// 次项缺失时取一减首项以保持互补；两者最终都经收敛限制在 0.01 到 0.99。
/// 上游标记 closed 或 archived 任一为真即视为已关闭并置为隐藏状态，否则为可见。
/// 终局结果优先从四个候选结果字段归一化取得，取不到时退而根据已关闭市场的
/// 极端价格组合推断；两者都取不到则留空，市场保持未决等待人工确认。
/// 完整原始载荷一并保留，便于日后回溯上游口径变化；本函数不发起网络请求也不写库。
pub(crate) fn parse_polymarket_market(value: &Value) -> AppResult<ParsedPolymarketMarket> {
    let external_market_id = first_string(value, &["id", "conditionId", "questionID"])
        .ok_or_else(|| AppError::Validation("polymarket market id is missing".to_owned()))?;
    let external_event_id = first_string(value, &["eventId", "event_id", "groupItemTitle"]);
    let title = first_string(value, &["question", "title", "name"])
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| AppError::Validation("polymarket market title is missing".to_owned()))?;
    let outcome_labels = json_string_array(
        &first_jsonish_value(value, &["outcomes", "tokens"]).unwrap_or(Value::Null),
    );
    let outcome_yes_label = outcome_labels
        .first()
        .cloned()
        .unwrap_or_else(|| "Yes".to_owned());
    let outcome_no_label = outcome_labels
        .get(1)
        .cloned()
        .unwrap_or_else(|| "No".to_owned());
    let prices = json_decimal_array(
        &first_jsonish_value(value, &["outcomePrices", "prices"]).unwrap_or(Value::Null),
    );
    let yes_price = prices
        .first()
        .cloned()
        .unwrap_or_else(|| decimal_str("0.5"));
    let no_price = prices
        .get(1)
        .cloned()
        .unwrap_or_else(|| decimal_str("1") - yes_price.clone());
    let is_closed = bool_field(value, "closed") || bool_field(value, "archived");
    let source_status = if is_closed {
        STATUS_HIDDEN.to_owned()
    } else {
        STATUS_ACTIVE.to_owned()
    };
    let external_resolution = first_string(
        value,
        &[
            "resolutionOutcome",
            "resolvedOutcome",
            "winningOutcome",
            "outcome",
        ],
    )
    .and_then(|outcome| normalize_external_resolution(&outcome))
    .or_else(|| closed_binary_price_resolution(is_closed, &prices));

    Ok(ParsedPolymarketMarket {
        external_event_id,
        external_market_id,
        slug: first_string(value, &["slug"]),
        title,
        description: first_string(value, &["description"]),
        image_url: first_string(value, &["image", "icon", "imageUrl"]),
        category: first_string(value, &["category", "categorySlug"]),
        tags_json: first_jsonish_value(value, &["tags"])
            .unwrap_or_else(|| Value::Array(Vec::new())),
        outcome_yes_label,
        outcome_no_label,
        yes_price: clamp_probability(yes_price),
        no_price: clamp_probability(no_price),
        volume: first_decimal(value, &["volume", "volumeNum", "volume24hr"]),
        liquidity: first_decimal(value, &["liquidity", "liquidityNum"]),
        end_at: first_string(value, &["endDate", "end_date"])
            .and_then(|text| parse_datetime(&text)),
        source_status,
        external_resolution,
        payload: value.clone(),
    })
}

/// 在上游未给出显式结果字段时，尝试从已关闭市场的极端价格反推终局结果。
/// 只有市场确已关闭且价格数组至少两项才进入判定，否则直接返回空。
/// 判定极为严格：仅当两个概率恰好是一与零、或零与一时才分别推断为 yes 与 no，
/// 任何接近但不等于端点的组合都返回空，保持市场未决。
/// 这样苛刻是因为该推断会直接触发派奖，宁可停在人工确认也不能凭近似值误判。
fn closed_binary_price_resolution(is_closed: bool, prices: &[BigDecimal]) -> Option<String> {
    if !is_closed || prices.len() < 2 {
        return None;
    }
    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);
    match (&prices[0], &prices[1]) {
        (yes, no) if yes == &one && no == &zero => Some(OUTCOME_YES.to_owned()),
        (yes, no) if yes == &zero && no == &one => Some(OUTCOME_NO.to_owned()),
        _ => None,
    }
}

/// 取理论赔付与赔付上限的较小值，是单笔派奖金额离开平台账户前的最后一道封顶。
/// 上限为零或负数表示未配置封顶，此时原样返回理论赔付而不截断，
/// 因此不能用零上限来表达「不赔付」，那会被当成不设限。
/// 截断只在理论赔付严格大于上限时发生，相等时返回原值，两者数值一致不影响结果。
/// 本函数是纯计算，不读配置、不改余额、不写账本，实际扣减由结算事务负责。
pub(crate) fn capped_payout(theoretical_payout: &BigDecimal, cap: &BigDecimal) -> BigDecimal {
    if cap > &BigDecimal::from(0) && theoretical_payout > cap {
        cap.clone()
    } else {
        theoretical_payout.clone()
    }
}

/// 生成用户可见的预测订单号，格式为 PM 前缀加当日日期再加左补零到八位的主键。
/// 唯一性完全来自主键，日期只是可读前缀，因此主键超过八位时订单号会自然变长而不截断。
/// 日期取生成时刻的 UTC 当天，与订单创建时间可能不在同一天，
/// 所以订单号中的日期只能当作参考，不得用于按日筛选或对账。
/// 仅格式化不改变内部数据库标识，同一订单重复调用可能得到不同结果，需落库固化。
pub(crate) fn prediction_order_no(order_id: u64) -> String {
    format!("PM{}{:08}", Utc::now().format("%Y%m%d"), order_id)
}

/// 解析 RFC3339 格式的上游时间并统一转换为 UTC 存储，带偏移量的输入会被正确换算。
/// 只认 RFC3339，其他格式包括秒级或毫秒级纯时间戳一律返回空而不做猜测解析。
/// 返回空意味着市场的结束时间留空，调用方不得回退为当前时刻或纪元起点，
/// 否则会让本无期限的市场看起来已经到期。
pub(crate) fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
}

/// 压缩同步错误文本中的连续空白为单个空格并截断到 512 字符，防止日志字段无界增长。
/// 换行与制表符一并折成空格，使多行的上游报错能落进单行日志列而不破坏结构。
/// 截断按字符而非字节计量，因此中文报错不会被切在半个字符上产生乱码。
/// 截断不追加省略标记，读到恰好 512 字符的消息时应当意识到其可能已被裁剪。
pub(crate) fn compact_error_message(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(512).collect()
}

/// 解析十进制字面量字符串，解析失败按同步兼容合同回退为零而不是报错。
/// 该回退只对本文件内部以字面量常量调用的场景安全，例如 0.5、1、0.01 这类固定值，
/// 它们必然解析成功，零值分支实际不会被触达。
/// 不得用它解析用户输入或上游数据，那些场景应改用会返回空的 `decimal_from_json`，
/// 否则非法金额会被静默当成零参与资金计算。
pub(crate) fn decimal_str(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap_or_else(|_| BigDecimal::from(0))
}

/// 把上游概率收敛到可用于定价的开区间，保护异常值不突破预测定价边界。
/// 小于等于零收敛为 0.01，大于等于一收敛为 0.99，两个端点本身也被收敛，
/// 因为端点会让赔付倍数无界或收益为零，无法构成可下注的报价。
/// 区间内的值保留原值并统一重设为 8 位小数，使同一市场的历史报价具有一致的可比精度；
/// 该操作可能按 `BigDecimal` 的定标规则丢弃更高位小数，属于同步侧可接受的精度收敛。
/// 与面向用户输入的 `ensure_probability_price` 不同，此处选择静默收敛而非拒绝，
/// 因为上游异常不应导致整条市场同步失败。
pub(crate) fn clamp_probability(value: BigDecimal) -> BigDecimal {
    if value <= 0 {
        decimal_str("0.01")
    } else if value >= 1 {
        decimal_str("0.99")
    } else {
        value.with_scale(8)
    }
}
