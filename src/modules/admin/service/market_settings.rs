//! 后台模拟行情高级参数、seed 策略、场景预设与版本快照映射的纯业务规则。
//!
//! 本模块不访问数据库，也不生成或发布 K 线。管理员请求先在这里被收敛为领域枚举和高精度数值，
//! 应用层随后决定 seed 的创建/继承语义，并把完整显式参数写入不可变版本 JSON。

use super::*;
use crate::modules::{
    admin::presentation::{
        MarketStrategyGeneratorPresetResponse, MarketStrategyGeneratorRequest,
        MarketStrategyGeneratorResponse, MarketStrategyPresetNodeResponse,
        MarketStrategyPresetResponse, MarketStrategyPresetsResponse,
    },
    market::{
        SyntheticGeneratorSettings, SyntheticScenario, SyntheticSeedMode, SyntheticVolumeShape,
        synthetic_generator_settings_from_snapshot, synthetic_scenario_from_code,
        synthetic_seed_mode_from_code, synthetic_volume_shape_from_code,
    },
};

/// 已通过白名单、范围与 seed 交叉校验的高级参数；应用层可据此安全决定实际版本 seed。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedMarketStrategyGenerator {
    pub(crate) settings: SyntheticGeneratorSettings,
    pub(crate) requested_seed: Option<String>,
    pub(crate) regenerate_seed: bool,
}

/// 校验并规范化管理员提交的场景、seed 模式和高级数值，不读取当前版本或修改请求。
/// 固定 seed 必须为 1～128 个字符且不允许“重新生成”；自动模式忽略空 seed，是否继承由具体用例决定。
/// 均值回归强度限制在 0～2，噪声和影线强度限制在 0～5，越界直接返回中文校验错误而不夹紧。
pub(crate) fn validate_market_strategy_generator(
    request: &MarketStrategyGeneratorRequest,
) -> AppResult<ValidatedMarketStrategyGenerator> {
    let scenario = synthetic_scenario_from_code(request.scenario.trim())
        .map_err(|_| AppError::Validation("模拟行情场景不受支持".to_owned()))?;
    let seed_mode = synthetic_seed_mode_from_code(request.seed_mode.trim())
        .map_err(|_| AppError::Validation("Seed 模式只支持自动或固定".to_owned()))?;
    let volume_shape = synthetic_volume_shape_from_code(request.volume_shape.trim())
        .map_err(|_| AppError::Validation("成交量形态不受支持".to_owned()))?;
    let requested_seed = optional_string(request.seed.clone());
    if seed_mode == SyntheticSeedMode::Fixed {
        let seed = requested_seed
            .as_ref()
            .ok_or_else(|| AppError::Validation("固定 Seed 模式必须填写 Seed".to_owned()))?;
        if seed.chars().count() > 128 {
            return Err(AppError::Validation(
                "Seed 长度必须为 1～128 个字符".to_owned(),
            ));
        }
        if request.regenerate_seed {
            return Err(AppError::Validation(
                "固定 Seed 模式不能同时选择重新生成".to_owned(),
            ));
        }
    }
    let zero = BigDecimal::from(0);
    let two = BigDecimal::from(2);
    let five = BigDecimal::from(5);
    if request.mean_reversion_strength < zero || request.mean_reversion_strength > two {
        return Err(AppError::Validation(
            "均值回归强度必须在 0～2 之间".to_owned(),
        ));
    }
    if request.noise_scale < zero || request.noise_scale > five {
        return Err(AppError::Validation("噪声强度必须在 0～5 之间".to_owned()));
    }
    if request.wick_scale < zero || request.wick_scale > five {
        return Err(AppError::Validation("影线强度必须在 0～5 之间".to_owned()));
    }
    Ok(ValidatedMarketStrategyGenerator {
        settings: SyntheticGeneratorSettings {
            scenario,
            seed_mode,
            mean_reversion_strength: request.mean_reversion_strength.clone(),
            noise_scale: request.noise_scale.clone(),
            wick_scale: request.wick_scale.clone(),
            volume_shape,
        },
        requested_seed,
        regenerate_seed: request.regenerate_seed,
    })
}

/// 为新建或独立预览请求确定实际 seed：固定模式使用管理员值，自动模式生成一次 UUIDv7 并显式返回。
pub(crate) fn resolve_new_market_strategy_seed(
    generator: &ValidatedMarketStrategyGenerator,
) -> String {
    match generator.settings.seed_mode {
        SyntheticSeedMode::Fixed => generator
            .requested_seed
            .clone()
            .expect("validated fixed seed must exist"),
        SyntheticSeedMode::Auto => Uuid::now_v7().to_string(),
    }
}

/// 为策略编辑确定新版本 seed：自动模式默认继承当前激活版本，只有显式重新生成才换 UUIDv7；固定模式使用提交值。
/// 当前 seed 为空说明历史版本已经违反数据库与生成器合同，此时返回校验错误而不是生成新值掩盖问题。
pub(crate) fn resolve_updated_market_strategy_seed(
    generator: &ValidatedMarketStrategyGenerator,
    active_seed: &str,
) -> AppResult<String> {
    match generator.settings.seed_mode {
        SyntheticSeedMode::Fixed => Ok(generator
            .requested_seed
            .clone()
            .expect("validated fixed seed must exist")),
        SyntheticSeedMode::Auto if generator.regenerate_seed => Ok(Uuid::now_v7().to_string()),
        SyntheticSeedMode::Auto => {
            let seed = active_seed.trim();
            if seed.is_empty() {
                Err(AppError::Validation(
                    "当前激活版本 Seed 为空，无法继承".to_owned(),
                ))
            } else {
                Ok(seed.to_owned())
            }
        }
    }
}

/// 将已校验高级参数序列化为不可变版本中的 `generator` 对象；命令字段与实际 seed 均不重复写入 JSON。
/// 实际 seed 继续由 `strategy_versions.seed` 保存，避免一个版本出现两份可能不一致的随机源。
pub(crate) fn market_strategy_generator_snapshot_json(
    generator: &ValidatedMarketStrategyGenerator,
) -> Value {
    json!({
        "scenario": generator.settings.scenario.as_str(),
        "seed_mode": generator.settings.seed_mode.as_str(),
        "mean_reversion_strength": generator.settings.mean_reversion_strength,
        "noise_scale": generator.settings.noise_scale,
        "wick_scale": generator.settings.wick_scale,
        "volume_shape": generator.settings.volume_shape.as_str(),
    })
}

/// 把已校验参数与版本行实际 seed 映射为后台友好读模型，不暴露编辑命令 `regenerate_seed`。
pub(crate) fn market_strategy_generator_response(
    generator: &ValidatedMarketStrategyGenerator,
    seed: String,
) -> MarketStrategyGeneratorResponse {
    generator_response_from_settings(&generator.settings, seed)
}

/// 从历史版本 JSON 兼容解析高级参数并附上版本行实际 seed；缺少 generator 的旧快照返回旧算法默认值。
pub(crate) fn market_strategy_generator_response_from_snapshot(
    config_json: &Value,
    seed: String,
) -> AppResult<MarketStrategyGeneratorResponse> {
    let settings = synthetic_generator_settings_from_snapshot(config_json)
        .map_err(|error| AppError::Validation(error.to_string()))?;
    Ok(generator_response_from_settings(&settings, seed))
}

/// 生成后端权威场景目录；预设只返回显式参数与相对节点模板，应用预设本身不会改变生成器分支。
pub(crate) fn market_strategy_presets() -> MarketStrategyPresetsResponse {
    MarketStrategyPresetsResponse {
        presets: vec![
            preset(
                SyntheticScenario::CustomPath,
                "自定义路径",
                "完全依据起止价格与管理员节点生成，适合精确规划新币走势。",
                0,
                55,
                100,
                75,
                SyntheticVolumeShape::Uniform,
                vec![],
            ),
            preset(
                SyntheticScenario::TrendUp,
                "稳步上涨",
                "以温和波动逐步上涨，并在尾段保持成交量增长。",
                25,
                45,
                80,
                60,
                SyntheticVolumeShape::Trend,
                vec![(35, 8), (70, 18)],
            ),
            preset(
                SyntheticScenario::TrendDown,
                "缓慢下行",
                "形成受控下跌路径，保留轻微反弹纹理。",
                -20,
                50,
                90,
                65,
                SyntheticVolumeShape::Trend,
                vec![(35, -6), (70, -14)],
            ),
            preset(
                SyntheticScenario::Range,
                "区间震荡",
                "围绕起始价格多次往返，最终回到接近起点的位置。",
                2,
                90,
                115,
                95,
                SyntheticVolumeShape::Uniform,
                vec![(25, 6), (50, -4), (75, 5)],
            ),
            preset(
                SyntheticScenario::HighVolatility,
                "高波动",
                "提高价格噪声与影线，适合压力测试快速变化的行情界面。",
                12,
                120,
                240,
                180,
                SyntheticVolumeShape::Bell,
                vec![(30, 18), (58, -9), (82, 20)],
            ),
            preset(
                SyntheticScenario::CrashRecovery,
                "急跌修复",
                "前半段快速下跌，随后逐步恢复至目标价格。",
                8,
                75,
                150,
                130,
                SyntheticVolumeShape::Bell,
                vec![(32, -35), (62, -18), (82, -5)],
            ),
            preset(
                SyntheticScenario::PumpThenDump,
                "拉升回落",
                "先快速拉升并放量，随后回落到最终目标。",
                10,
                80,
                170,
                150,
                SyntheticVolumeShape::EndSpike,
                vec![(30, 42), (58, 65), (80, 24)],
            ),
        ],
    }
}

/// 将领域枚举与高精度参数映射为后台响应，同时附上版本表保存的实际 seed。
/// 映射不读取请求中的命令字段，也不重新生成 seed，因此详情、版本列表和回滚响应都可据此准确重放。
fn generator_response_from_settings(
    settings: &SyntheticGeneratorSettings,
    seed: String,
) -> MarketStrategyGeneratorResponse {
    MarketStrategyGeneratorResponse {
        scenario: settings.scenario.as_str().to_owned(),
        seed_mode: settings.seed_mode.as_str().to_owned(),
        seed,
        mean_reversion_strength: settings.mean_reversion_strength.clone(),
        noise_scale: settings.noise_scale.clone(),
        wick_scale: settings.wick_scale.clone(),
        volume_shape: settings.volume_shape.as_str().to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
/// 用稳定代码、中文说明和百分之一精度参数构造单个后端权威场景预设。
/// 节点时间只保存全局进度百分比，目标值显式采用相对起始价百分比；后台应用后仍可逐项编辑，不触发隐藏分支。
fn preset(
    scenario: SyntheticScenario,
    name: &str,
    description: &str,
    target_change_percent: i64,
    mean_reversion_hundredths: i64,
    noise_hundredths: i64,
    wick_hundredths: i64,
    volume_shape: SyntheticVolumeShape,
    nodes: Vec<(u32, i64)>,
) -> MarketStrategyPresetResponse {
    MarketStrategyPresetResponse {
        code: scenario.as_str().to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        target_price_change_percent: BigDecimal::from(target_change_percent),
        generator: MarketStrategyGeneratorPresetResponse {
            scenario: scenario.as_str().to_owned(),
            seed_mode: "auto".to_owned(),
            mean_reversion_strength: BigDecimal::new(mean_reversion_hundredths.into(), 2),
            noise_scale: BigDecimal::new(noise_hundredths.into(), 2),
            wick_scale: BigDecimal::new(wick_hundredths.into(), 2),
            volume_shape: volume_shape.as_str().to_owned(),
        },
        nodes: nodes
            .into_iter()
            .map(
                |(progress_percent, target_value)| MarketStrategyPresetNodeResponse {
                    progress_percent,
                    target_type: "percent_from_start".to_owned(),
                    target_value: BigDecimal::from(target_value),
                    execution_mode: "soft".to_owned(),
                    tolerance: BigDecimal::from(1),
                    volatility: BigDecimal::new(noise_hundredths.into(), 4),
                    volume_min: None,
                    volume_max: None,
                },
            )
            .collect(),
    }
}
