//! 模拟行情不可变版本快照的统一解析器。
//!
//! 实时 worker、后台配置预览和手动 K 线补偿都必须先把 SQL 行适配为
//! [`SyntheticStrategySnapshot`]，再经本模块构造领域配置。这里兼容历史 JSON 的时间与十进制写法，
//! 但不会用当前配置覆盖快照中已经存在却损坏的字段，避免不同消费路径产生口径漂移。

use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;
use thiserror::Error;

use super::synthetic::{
    SyntheticExecutionMode, SyntheticGeneratorSettings, SyntheticMarketConfig,
    SyntheticMarketError, SyntheticMarketNode, SyntheticScenario, SyntheticSeedMode,
    SyntheticTargetType, SyntheticVolumeShape,
};

/// 数据库版本行和主表兼容字段组成的适配器中间态；该类型不持有连接，也不执行查询。
#[derive(Debug, Clone)]
pub struct SyntheticStrategySnapshot {
    pub symbol: String,
    pub seed: String,
    pub version: i32,
    pub price_precision: i32,
    pub config_json: Value,
    pub fallback_start_time: DateTime<Utc>,
    pub fallback_end_time: DateTime<Utc>,
    pub fallback_start_price: BigDecimal,
    pub fallback_target_price: BigDecimal,
    pub fallback_volatility: BigDecimal,
    pub fallback_volume_min: BigDecimal,
    pub fallback_volume_max: BigDecimal,
    pub fallback_nodes: Vec<SyntheticMarketNode>,
}

/// 快照解析失败表示历史版本结构、枚举、数值或领域不变量不合法；函数无 I/O 和部分状态。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyntheticStrategySnapshotError {
    #[error("synthetic strategy version and price precision must be non-negative")]
    InvalidUnsignedField,
    #[error("synthetic strategy snapshot field {0} is required")]
    MissingField(String),
    #[error("synthetic strategy snapshot field {0} has an invalid type or value")]
    InvalidField(String),
    #[error("unsupported synthetic scenario: {0}")]
    UnsupportedScenario(String),
    #[error("unsupported synthetic seed mode: {0}")]
    UnsupportedSeedMode(String),
    #[error("unsupported synthetic volume shape: {0}")]
    UnsupportedVolumeShape(String),
    #[error("unsupported synthetic target type: {0}")]
    UnsupportedTargetType(String),
    #[error("unsupported synthetic execution mode: {0}")]
    UnsupportedExecutionMode(String),
    #[error(transparent)]
    InvalidConfig(#[from] SyntheticMarketError),
}

/// 将版本 JSON、实际 seed、主表兼容列与关系表兼容节点合成为唯一的领域配置。
/// JSON 中存在 `nodes` 时即为权威值，哪怕它是空数组；只有键完全缺失才使用关系表节点。
/// `generator` 缺失时采用旧固定常量默认值，存在但字段损坏时立即失败，不以默认值掩盖脏快照。
/// 本函数不查询数据库、不生成 K 线；成功返回后实时、预览和补偿可安全共享同一份配置。
pub fn synthetic_config_from_snapshot(
    snapshot: SyntheticStrategySnapshot,
) -> Result<SyntheticMarketConfig, SyntheticStrategySnapshotError> {
    let version = u32::try_from(snapshot.version)
        .map_err(|_| SyntheticStrategySnapshotError::InvalidUnsignedField)?;
    let price_precision = u32::try_from(snapshot.price_precision)
        .map_err(|_| SyntheticStrategySnapshotError::InvalidUnsignedField)?;
    let nodes = match snapshot.config_json.get("nodes") {
        Some(Value::Array(nodes)) => nodes
            .iter()
            .map(snapshot_node)
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(SyntheticStrategySnapshotError::InvalidField(
                "nodes".to_owned(),
            ));
        }
        None => snapshot.fallback_nodes,
    };
    let generator = synthetic_generator_settings_from_snapshot(&snapshot.config_json)?;
    Ok(SyntheticMarketConfig::new(SyntheticMarketConfig {
        symbol: snapshot.symbol,
        seed: snapshot.seed,
        version,
        price_precision,
        start_time: config_time(
            &snapshot.config_json,
            "start_time",
            snapshot.fallback_start_time,
        )?,
        end_time: config_time(
            &snapshot.config_json,
            "end_time",
            snapshot.fallback_end_time,
        )?,
        start_price: config_decimal(
            &snapshot.config_json,
            "start_price",
            snapshot.fallback_start_price,
        )?,
        target_price: config_decimal(
            &snapshot.config_json,
            "target_price",
            snapshot.fallback_target_price,
        )?,
        volatility: config_decimal(
            &snapshot.config_json,
            "volatility",
            snapshot.fallback_volatility,
        )?,
        volume_min: config_decimal(
            &snapshot.config_json,
            "volume_min",
            snapshot.fallback_volume_min,
        )?,
        volume_max: config_decimal(
            &snapshot.config_json,
            "volume_max",
            snapshot.fallback_volume_max,
        )?,
        generator,
        nodes,
    })?)
}

/// 解析显式生成器对象；对象存在时六个字段都必须完整，防止半份新快照悄悄混用未来默认值。
fn generator_settings(
    value: &Value,
) -> Result<SyntheticGeneratorSettings, SyntheticStrategySnapshotError> {
    let object = value
        .as_object()
        .ok_or_else(|| SyntheticStrategySnapshotError::InvalidField("generator".to_owned()))?;
    let string = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| SyntheticStrategySnapshotError::MissingField(format!("generator.{key}")))
    };
    let decimal = |key: &str| {
        object
            .get(key)
            .ok_or_else(|| SyntheticStrategySnapshotError::MissingField(format!("generator.{key}")))
            .and_then(|value| value_decimal(value, &format!("generator.{key}")))
    };
    Ok(SyntheticGeneratorSettings {
        scenario: synthetic_scenario_from_code(string("scenario")?)?,
        seed_mode: synthetic_seed_mode_from_code(string("seed_mode")?)?,
        mean_reversion_strength: decimal("mean_reversion_strength")?,
        noise_scale: decimal("noise_scale")?,
        wick_scale: decimal("wick_scale")?,
        volume_shape: synthetic_volume_shape_from_code(string("volume_shape")?)?,
    })
}

/// 从完整版本 JSON 读取高级生成参数；历史快照缺少 `generator` 时返回旧算法兼容默认值。
/// 一旦该键存在就要求对象和全部字段完整合法，不把半份新快照与默认值混合，以保证所有消费者同错同停。
pub fn synthetic_generator_settings_from_snapshot(
    snapshot: &Value,
) -> Result<SyntheticGeneratorSettings, SyntheticStrategySnapshotError> {
    snapshot
        .get("generator")
        .map(generator_settings)
        .transpose()
        .map(|settings| settings.unwrap_or_default())
}

/// 把 JSON 节点还原为领域节点；时间、数值和枚举均严格解析，不接受缺失必填字段。
fn snapshot_node(value: &Value) -> Result<SyntheticMarketNode, SyntheticStrategySnapshotError> {
    Ok(SyntheticMarketNode {
        target_time: required_time(value, "target_time")?,
        target_type: synthetic_target_type_from_code(required_string(value, "target_type")?)?,
        target_value: required_decimal(value, "target_value")?,
        execution_mode: synthetic_execution_mode_from_code(required_string(
            value,
            "execution_mode",
        )?)?,
        tolerance: required_decimal(value, "tolerance")?,
        volatility: required_decimal(value, "volatility")?,
        volume_min: optional_decimal(value, "volume_min")?,
        volume_max: optional_decimal(value, "volume_max")?,
    })
}

/// 从策略版本根对象读取时间字段；键完全缺失时才使用主表兼容值，显式空值或非法类型不会静默回退。
/// 该区分保证旧快照可以继续运行，同时让已写入但损坏的新快照在实时与补偿链路中以相同方式失败。
fn config_time(
    value: &Value,
    key: &str,
    fallback: DateTime<Utc>,
) -> Result<DateTime<Utc>, SyntheticStrategySnapshotError> {
    value
        .get(key)
        .map_or(Ok(fallback), |value| value_time(value, key))
}

/// 从节点对象读取必填目标时间；缺失和格式错误分别映射为稳定的快照错误，便于定位具体字段。
/// 返回值统一转换为 UTC，后续分钟对齐和策略范围检查仍由领域配置构造器集中执行。
fn required_time(
    value: &Value,
    key: &str,
) -> Result<DateTime<Utc>, SyntheticStrategySnapshotError> {
    value
        .get(key)
        .ok_or_else(|| SyntheticStrategySnapshotError::MissingField(key.to_owned()))
        .and_then(|value| value_time(value, key))
}

/// 兼容解析毫秒整数与 RFC3339 字符串两类历史时间表示，全程保留绝对时刻语义并统一转为 UTC。
/// 数值越界、字符串格式错误或其他 JSON 类型都返回字段级错误，不使用本机时区或当前时间兜底。
fn value_time(value: &Value, key: &str) -> Result<DateTime<Utc>, SyntheticStrategySnapshotError> {
    if let Some(millis) = value.as_i64() {
        return DateTime::from_timestamp_millis(millis)
            .ok_or_else(|| SyntheticStrategySnapshotError::InvalidField(key.to_owned()));
    }
    if let Some(raw) = value.as_str() {
        return DateTime::parse_from_rfc3339(raw)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| SyntheticStrategySnapshotError::InvalidField(key.to_owned()));
    }
    Err(SyntheticStrategySnapshotError::InvalidField(key.to_owned()))
}

/// 从策略版本根对象读取十进制字段；只有键不存在时才使用主表兼容值，存在脏值时立即失败。
/// 该规则避免实时 worker 与手动补偿在同一损坏版本上分别采用快照值和主表值而产生行情分叉。
fn config_decimal(
    value: &Value,
    key: &str,
    fallback: BigDecimal,
) -> Result<BigDecimal, SyntheticStrategySnapshotError> {
    value
        .get(key)
        .map_or(Ok(fallback), |value| value_decimal(value, key))
}

/// 从节点对象读取必填十进制字段，缺失时返回字段名明确的错误，存在时交给统一高精度解析器。
/// 本函数不做范围夹紧；非负、价格有效性及成交量上下界等跨字段约束由领域配置统一验证。
fn required_decimal(
    value: &Value,
    key: &str,
) -> Result<BigDecimal, SyntheticStrategySnapshotError> {
    value
        .get(key)
        .ok_or_else(|| SyntheticStrategySnapshotError::MissingField(key.to_owned()))
        .and_then(|value| value_decimal(value, key))
}

/// 解析节点可选十进制覆盖值；缺失与 JSON null 都表示沿用策略级参数，其余值必须能精确解析。
/// 非法文本不会被当作“未配置”忽略，确保关系表兼容节点与 JSON 快照节点拥有一致的失败语义。
fn optional_decimal(
    value: &Value,
    key: &str,
) -> Result<Option<BigDecimal>, SyntheticStrategySnapshotError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value_decimal(value, key).map(Some),
    }
}

/// 将 JSON 字符串或数字字面量直接解析为 `BigDecimal`，不经过 f64，避免价格、波动率和成交量精度漂移。
/// 布尔、对象、数组以及不可解析文本统一返回携带字段名的错误，调用方不会得到部分或近似结果。
fn value_decimal(value: &Value, key: &str) -> Result<BigDecimal, SyntheticStrategySnapshotError> {
    let raw = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    BigDecimal::from_str(&raw)
        .map_err(|_| SyntheticStrategySnapshotError::InvalidField(key.to_owned()))
}

/// 从节点对象读取枚举代码所需的必填字符串；函数只校验 JSON 类型和存在性，不裁剪或转换大小写。
/// 后续稳定代码解析器负责白名单判断，因此带空白或未知代码会被明确拒绝，而不会落入默认枚举。
fn required_string<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a str, SyntheticStrategySnapshotError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| SyntheticStrategySnapshotError::MissingField(key.to_owned()))
}

/// 将稳定场景代码解析为领域枚举；未知值不会回落到自定义路径，避免错误预设被静默接受。
pub fn synthetic_scenario_from_code(
    value: &str,
) -> Result<SyntheticScenario, SyntheticStrategySnapshotError> {
    match value {
        "custom_path" => Ok(SyntheticScenario::CustomPath),
        "trend_up" => Ok(SyntheticScenario::TrendUp),
        "trend_down" => Ok(SyntheticScenario::TrendDown),
        "range" => Ok(SyntheticScenario::Range),
        "high_volatility" => Ok(SyntheticScenario::HighVolatility),
        "crash_recovery" => Ok(SyntheticScenario::CrashRecovery),
        "pump_then_dump" => Ok(SyntheticScenario::PumpThenDump),
        _ => Err(SyntheticStrategySnapshotError::UnsupportedScenario(
            value.to_owned(),
        )),
    }
}

/// 将后台 seed 模式代码解析为领域枚举；实际 seed 仍由版本行保存，本函数只解释管理语义。
pub fn synthetic_seed_mode_from_code(
    value: &str,
) -> Result<SyntheticSeedMode, SyntheticStrategySnapshotError> {
    match value {
        "auto" => Ok(SyntheticSeedMode::Auto),
        "fixed" => Ok(SyntheticSeedMode::Fixed),
        _ => Err(SyntheticStrategySnapshotError::UnsupportedSeedMode(
            value.to_owned(),
        )),
    }
}

/// 将成交量形态代码解析为领域枚举；未知值直接拒绝，防止不同消费方使用不同默认形态。
pub fn synthetic_volume_shape_from_code(
    value: &str,
) -> Result<SyntheticVolumeShape, SyntheticStrategySnapshotError> {
    match value {
        "uniform" => Ok(SyntheticVolumeShape::Uniform),
        "trend" => Ok(SyntheticVolumeShape::Trend),
        "bell" => Ok(SyntheticVolumeShape::Bell),
        "end_spike" => Ok(SyntheticVolumeShape::EndSpike),
        _ => Err(SyntheticStrategySnapshotError::UnsupportedVolumeShape(
            value.to_owned(),
        )),
    }
}

/// 将节点目标类型代码解析为领域枚举，供关系表兼容节点和 JSON 快照共享同一白名单。
pub fn synthetic_target_type_from_code(
    value: &str,
) -> Result<SyntheticTargetType, SyntheticStrategySnapshotError> {
    match value {
        "absolute_price" => Ok(SyntheticTargetType::AbsolutePrice),
        "percent_from_start" => Ok(SyntheticTargetType::PercentFromStart),
        "percent_from_previous" => Ok(SyntheticTargetType::PercentFromPrevious),
        _ => Err(SyntheticStrategySnapshotError::UnsupportedTargetType(
            value.to_owned(),
        )),
    }
}

/// 将节点执行模式代码解析为领域枚举，供实时与补偿路径共享 hard/soft/range 口径。
pub fn synthetic_execution_mode_from_code(
    value: &str,
) -> Result<SyntheticExecutionMode, SyntheticStrategySnapshotError> {
    match value {
        "hard" => Ok(SyntheticExecutionMode::Hard),
        "soft" => Ok(SyntheticExecutionMode::Soft),
        "range" => Ok(SyntheticExecutionMode::Range),
        _ => Err(SyntheticStrategySnapshotError::UnsupportedExecutionMode(
            value.to_owned(),
        )),
    }
}
