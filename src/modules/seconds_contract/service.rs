//! seconds_contract bounded context service layer.
//!
//! 服务层：承载秒合约限界上下文中不依赖数据库连接的纯业务规则，供应用层与路由层共同复用。
//! 本文件集中三类职责：一是产品配置与下单参数的合法性校验和归一化，包括周期集合去重、赔率与
//! 投注额的小数位/整数位容量检查，避免非法配置进入管理事务或资金事务；二是结算赔付金额的计算
//! 口径，赢单按 `本金 × (1 + 赔率)` 并按质押资产精度向零截断，输单固定为零；三是幂等重放的一致性
//! 判定与开仓、结算事件的 payload 拼装。
//! 本文件不持有数据库连接、不开启事务、不写钱包，也不主动读取行情价格；所有函数都是同步纯计算或
//! 单纯的广播投递，调用方必须在资金事务提交成功之后才允许调用这里的事件发布函数。

use crate::{
    error::{AppError, AppResult},
    modules::seconds_contract::{
        presentation::{
            CreateSecondsContractProductRequest, OpenSecondsContractOrderResponse,
            SecondsContractOrderResponse, SecondsContractProductCycleInput,
            SecondsContractProductResponse, SettleSecondsContractOrderResponse,
            UpdateSecondsContractProductRequest,
        },
        repository::{SecondsContractProductRuleRow, SecondsContractSettlementPriceRow},
    },
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        wallet::{amount_fits_asset_precision, truncate_amount_to_asset_precision},
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use std::collections::HashSet;

/// 结算价格事件窗口长度；可选历史范围为 `[expires_at, expires_at + 5s)`。
pub(crate) const SETTLEMENT_PRICE_WINDOW_SECONDS: i64 = 5;

/// 对选中的结算行情做完整证据校验，防止损坏的历史行进入资金结算。
/// 交易对去除常见分隔符后比较；价格必须为正，来源必须是已知 provider，
/// generation 和 source_version 必须可追溯，observed_at 必须落在左闭右开窗口。
/// 任一字段不合法都 fail closed，调用方不得跳过该行回退到其他价格。
pub(crate) fn validate_settlement_price_snapshot(
    snapshot: &SecondsContractSettlementPriceRow,
    expected_symbol: &str,
    expires_at: DateTime<Utc>,
) -> AppResult<()> {
    let normalize_symbol = |value: &str| {
        value
            .trim()
            .chars()
            .filter(|character| !matches!(character, '-' | '/' | '_'))
            .flat_map(char::to_uppercase)
            .collect::<String>()
    };
    let expected_symbol = normalize_symbol(expected_symbol);
    if expected_symbol.is_empty() || normalize_symbol(&snapshot.symbol) != expected_symbol {
        return Err(AppError::Validation(
            "seconds contract settlement price symbol is invalid".to_owned(),
        ));
    }
    if snapshot.price <= 0 {
        return Err(AppError::Validation(
            "seconds contract settlement price must be positive".to_owned(),
        ));
    }
    if !matches!(
        snapshot.source.as_str(),
        "bitget" | "htx" | "coinbase" | "strategy"
    ) {
        return Err(AppError::Validation(
            "seconds contract settlement price source is invalid".to_owned(),
        ));
    }
    if snapshot.generation == 0 || snapshot.source_version.trim().is_empty() {
        return Err(AppError::Validation(
            "seconds contract settlement price provenance is incomplete".to_owned(),
        ));
    }
    let window_end = expires_at
        .checked_add_signed(TimeDelta::seconds(SETTLEMENT_PRICE_WINDOW_SECONDS))
        .ok_or_else(|| {
            AppError::Validation(
                "seconds contract settlement event window is outside valid range".to_owned(),
            )
        })?;
    if snapshot.observed_at < expires_at || snapshot.observed_at >= window_end {
        return Err(AppError::Validation(
            "seconds contract settlement price is outside the event window".to_owned(),
        ));
    }
    Ok(())
}

/// 校验通过后的单个秒合约周期配置，是产品创建与更新写库前的统一中间形态。
/// 构造该结构不代表已落库，调用方仍需在管理事务内按周期时长整体覆盖写入产品规则行。
#[derive(Debug, Clone)]
pub(crate) struct NormalizedSecondsContractProductCycle {
    /// 该周期的持仓时长，单位为秒，必须为正且在同一产品的周期集合内唯一。
    pub(crate) duration_seconds: u32,
    /// 赢单赔率，为不含本金的净收益倍率，非负且最多 8 位小数、10 位整数位。
    pub(crate) payout_rate: BigDecimal,
    /// 该周期允许的最小单笔投注额，以质押资产计价，必须为正数。
    pub(crate) min_stake: BigDecimal,
    /// 该周期允许的最大单笔投注额；`None` 表示不设上限，有值时不得小于 `min_stake`。
    pub(crate) max_stake: Option<BigDecimal>,
}

/// 将秒合约产品的完整配置摊平为后台审计日志快照，用于记录创建、更新和状态变更的前后镜像。
/// 快照覆盖交易对、质押资产及其符号、图标、默认周期与赔率、投注额上下限、全部周期集合和上架状态；
/// 赔率与金额沿用 `SecondsContractProductResponse` 中的 `BigDecimal` 序列化形态，以字符串保留原始精度，
/// 不做四舍五入也不折算成浮点，保证审计对账时能逐位还原当时的配置。
/// 本函数只读入参并构造 JSON，不访问数据库、不写审计表，落库由调用方在管理事务内自行完成。
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

/// 将一笔秒合约订单连同赔付金额摊平为审计快照，用于后台人工结算与自动结算的留痕对账。
/// 快照同时保留开仓价 `entry_price` 与结算价 `settlement_price`，便于事后复核胜负判定是否与价格一致；
/// `expires_at` 统一转成毫秒时间戳，避免不同时区渲染导致到期时刻歧义。
/// `payout_amount` 由调用方传入既有结算结果，本函数不重新计算赔付、不读取赔率、不触碰钱包余额，
/// 因此快照金额与实际入账金额必然同源；写入审计表由调用方在结算事务内完成。
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
        "settlement_price_tick_id": order.settlement_price_tick_id,
        "settlement_price_source": order.settlement_price_source,
        "settlement_price_observed_at": order
            .settlement_price_observed_at
            .map(|value| value.timestamp_millis()),
        "settlement_price_generation": order.settlement_price_generation,
        "settlement_price_version": order.settlement_price_version,
        "status": order.status,
        "result": order.result,
        "payout_amount": payout_amount,
        "expires_at": order.expires_at.timestamp_millis(),
    })
}

/// 向下单用户的私有频道推送秒合约开仓成功事件，事件体集中在服务层拼装以避免各调用点各写一份。
/// 推送内容包含订单号、产品与交易对、质押资产、方向、投注额、周期时长、赔率、开仓价和到期毫秒时间戳，
/// 前端据此可在不再查接口的情况下直接起倒计时并渲染持仓卡片。
/// 调用时机受严格约束：必须在冻结或扣减投注本金的数据库事务提交成功之后才能调用，否则事务回滚会留下
/// 用户已看到开仓、账上却没有对应订单的不一致。前置校验或资金写入失败时禁止调用本函数。
/// 本函数只做投递，不重试、不落库、不回填订单状态；广播链路本身不可用时按 hub 既有降级语义静默丢弃，
/// 不会因此回滚已提交的资金写入。
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

/// 仅当本次请求真正新建了订单且广播通道已配置时才推送开仓事件，把幂等重放与降级判断收敛到一处。
/// `is_new_order` 为假表示命中幂等键回读了既有订单，此时不得重复推送，否则前端会出现同一订单开仓两次。
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

/// 向持仓用户的私有频道推送秒合约到期结算事件，与开仓事件相比额外携带结算价、胜负结果和实际赔付额。
/// 事件里的 `payout_amount` 取自结算响应而非现算，赢单为含本金的入账总额，输单为零，与钱包流水金额一致。
/// 必须在赔付入账的结算事务提交成功之后调用；若在提交前推送，事务回滚会让用户看到并不存在的中奖金额。
/// 本函数只投递不落库，也不改写订单状态或补记账；广播不可用时沿用 hub 既有降级语义静默丢弃。
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
            "settlement_price_tick_id": response.order.settlement_price_tick_id,
            "settlement_price_source": response.order.settlement_price_source,
            "settlement_price_observed_at": response.order
                .settlement_price_observed_at
                .map(|value| value.timestamp_millis()),
            "settlement_price_generation": response.order.settlement_price_generation,
            "settlement_price_version": response.order.settlement_price_version,
            "payout_amount": response.payout_amount,
            "result": response.order.result,
            "status": response.order.status,
        })
        .to_string(),
    ));
}

/// 仅当本次调用真正完成了一次新结算且广播通道已配置时才推送结算事件，避免重复结算通知。
/// `is_new_settlement` 为假表示订单此前已结算、本次只是同结果回读，重复推送会让用户误以为二次派奖。
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

/// 校验命中幂等键回读出的既有订单与本次下单请求语义等价，防止同一把幂等键被复用到另一笔交易。
/// 比对维度为产品编号、方向和投注金额三项必比，`duration_seconds` 只在本次请求显式指定周期时才参与比对，
/// 以兼容旧版不带周期字段的客户端；金额按 `BigDecimal` 精确相等判断，不做四舍五入或量级归一。
/// 任一维度不一致时返回 `AppError::Conflict`，调用方据此拒绝下单，既不新建订单也不动用户资金。
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

/// 保障结算幂等：已结算订单只允许以完全相同的胜负结果重放，重复请求直接复用既有结算而不二次派奖。
/// 既有结果为空或与本次判定不同都返回 `AppError::Conflict`，避免自动结算与人工补结算给出相反结论时
/// 把同一笔订单先按输单不赔、再按赢单入账，造成资金重复发放。
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

/// 按订单快照上锁定的本金与赔率计算本次结算应入账金额，赔率取下单当时的值而非产品当前配置。
/// 计算委托给 `seconds_contract_payout_amount`，赢单为本金加净收益，其余结果为零，并按质押资产精度截断。
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

/// 按订单方向和事件时点价格判定胜负；平价统一为 loss，非法方向直接拒绝。
pub(crate) fn settlement_result_from_prices(
    direction: &str,
    entry_price: &BigDecimal,
    settlement_price: &BigDecimal,
) -> AppResult<&'static str> {
    match direction {
        "up" if settlement_price > entry_price => Ok("win"),
        "up" => Ok("loss"),
        "down" if settlement_price < entry_price => Ok("win"),
        "down" => Ok("loss"),
        _ => Err(AppError::Validation(
            "seconds contract direction must be up or down".to_owned(),
        )),
    }
}

/// 计算秒合约结算入账额，是整个模块唯一的赔付口径来源。
/// 结果为 `win` 时返回 `本金 + 本金 × 赔率`，即含本金的总入账额，`payout_rate` 按净收益率理解；
/// 其他结果（含 `loss`）返回按同一精度规整后的零，代表本金已在开仓时扣走且不再退回。
/// 两个分支都调用 `truncate_amount_to_asset_precision` 按质押资产的小数位向零截断，向零而非四舍五入，
/// 确保平台不会因尾差多付，也不会写入钱包无法表示的小数位。
/// 本函数为纯计算，不读产品配置、不查订单、不改钱包余额，也不判定胜负。
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

/// 校验后台新建秒合约产品的整份请求，返回按时长升序排列且已去重的周期配置，供管理事务直接写库。
/// 交易对与质押资产编号不得为零；周期既可用新版 `cycles` 数组给出，也可用旧版单周期字段兜底，
/// 两种形态都会走同一套赔率非负、投注额为正、上限不低于下限以及小数位容量的检查。
/// `status` 为可选项，给出时必须是 `active` 或 `disabled`；`reason` 给出时长度不得超过审计原因上限。
/// 任一项不合法立即返回 `AppError::Validation`，因此非法配置不会进入管理事务，也不会写产品表或审计表。
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

/// 校验后台更新秒合约产品的整份请求，与新建的区别在于 `status` 是必填项而非可选项。
/// 更新语义为整体覆盖：返回的周期集合就是产品更新后的全量周期，请求里未出现的旧周期将被视为删除，
/// 因此调用方不能把本函数的结果当作增量补丁使用。
/// 周期集合同样按时长升序去重，并复用与新建一致的赔率、投注额区间和小数位容量校验；原因文本超长即失败。
/// 校验失败返回 `AppError::Validation` 且不进入管理事务，既有产品配置保持原样。
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

/// 把新旧两种产品周期入参统一归一成校验后的周期集合，是产品创建与更新共用的唯一归一入口。
/// 优先采用新版 `cycles` 数组，数组存在但为空视为非法配置；数组缺省时才回落到旧版单周期字段
/// `legacy_duration_seconds`、`legacy_payout_rate`、`legacy_min_stake`、`legacy_max_stake`，
/// 这条兼容路径要求前三项必须齐备，缺任意一项都返回 `AppError::Validation`。
/// 归一后逐条执行字段级校验，并用 `HashSet` 保证同一产品内周期时长唯一，重复时长会被拒绝，
/// 防止下单时按时长选周期出现二义；最后按时长升序排序，使写库顺序和前端展示顺序稳定可预期。
/// 本函数只做纯校验与排序，不访问数据库、不检查交易对与资产是否真实存在，该职责留给基础设施层外键约束。
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

/// 把请求中一条可选字段形态的周期输入收敛为字段齐备的内部周期结构，缺字段即判定为参数错误。
/// 时长、赔率、最小投注额三项必须显式给出，任一为空返回 `AppError::Validation` 并指明缺失字段；
/// 最大投注额允许为空，语义是该周期不设单笔上限，此处原样透传不做补默认值。
/// 本函数只负责字段存在性与搬运，取值范围和小数位容量由 `validate_product_cycle_fields` 后续把关。
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

/// 逐项校验单条周期配置的取值范围，把非法参数挡在写库之前。
/// 时长必须为正秒数，零时长会让订单创建即到期；赔率必须非负并满足数据库 8 位小数、10 位整数位的容量；
/// 最小投注额必须为正且符合金额字段容量；最大投注额存在时同样按金额规则校验，且不得小于最小投注额，
/// 否则该周期将永远无法下单成功。
/// 校验只针对单条周期，同一产品内多条周期之间的时长唯一性由 `normalize_product_cycles` 负责。
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

/// 下单前对照产品规则行校验本次投注额，是用户资金被冻结之前的最后一道业务闸门。
/// 产品状态非 `active` 时按 `AppError::NotFound` 处理而不是返回校验错误，使已下架产品对外表现为不存在，
/// 不向调用方泄露该产品曾经存在及其配置。
/// 金额必须能被质押资产的精度完整表示，多出的小数位会被判为非法而不是静默截断，避免用户实付与记账不符；
/// 随后按规则行上的 `min_stake` 与可选 `max_stake` 检查区间，注意这两个边界来自调用方选定的那条周期规则。
/// 本函数不查询余额、不冻结资金，余额是否充足由后续钱包扣减环节判定。
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

/// 归一化用户看涨或看跌的下单方向，先去首尾空白再转小写，使 `UP`、`Up`、` up ` 都落到同一存储值。
/// 只承认 `up` 与 `down` 两种方向，其余输入返回 `AppError::Validation`，请求不会创建订单也不会冻结资金。
pub(crate) fn normalize_direction(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "up" => Ok("up".to_owned()),
        "down" => Ok("down".to_owned()),
        _ => Err(AppError::Validation(
            "seconds contract direction must be up or down".to_owned(),
        )),
    }
}

/// 归一化结算结果字面量，去空白转小写后只承认 `win` 与 `loss`，非法值返回 `AppError::Validation`。
/// 胜负本身由调用方比对开仓价与结算价后给出，本函数不读取任何价格、方向或订单数据，也不推导结果，
/// 只保证写入订单表和参与幂等比对的结果值形态统一。
pub(crate) fn normalize_settlement_result(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "win" => Ok("win".to_owned()),
        "loss" => Ok("loss".to_owned()),
        _ => Err(AppError::Validation(
            "seconds contract settlement result must be win or loss".to_owned(),
        )),
    }
}

/// 归一化产品上下架状态，先按可选文本规则裁剪空白，纯空白视同未填并返回参数错误。
/// 仅接受 `active` 与 `disabled` 两种状态，其余取值一律拒绝，避免写入无法被下单校验识别的第三种状态。
/// 注意此处不做大小写折叠，状态值要求调用方原样传入小写形态。
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

/// 取出并校验后台变更操作的必填原因文本，保证每次产品或订单干预都有可追责的说明写进审计日志。
/// 缺省、空串或纯空白都按缺失处理并返回 `AppError::Validation`，裁剪后仍超过审计原因长度上限同样拒绝，
/// 校验在开启管理事务之前完成，因此非法原因不会留下半截写入。
pub(crate) fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "seconds contract reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

/// 校验可选审计原因的长度上限，按裁剪空白后的 Unicode 字符数计数而非字节数，使中文说明不被误判超长。
/// 传入 `None` 表示本次操作不带原因，直接放行；超过 `SECONDS_AUDIT_REASON_MAX_LEN` 返回参数错误，
/// 防止超长文本在写入审计表时被数据库静默截断。
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

/// 校验赔率的业务范围与存储容量：允许为零表示该周期只退本金不给净收益，但不允许为负数。
/// 随后按 `SECONDS_RATE_MAX_SCALE` 与 `SECONDS_RATE_MAX_INTEGER_DIGITS` 检查小数位和整数位，
/// 确保配置能被赔率字段原样保存，避免入库截断后实际派奖倍率与后台填写值不符。
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

/// 校验投注类金额的通用规则，同时服务于产品限额配置和用户实际下单额。
/// 金额必须严格大于零，零和负数都被拒绝，杜绝零元开仓与负额倒扣；再按金额字段的 18 位小数、
/// 20 位整数位容量校验，确保数值入库不被截断，否则冻结额与订单记录会出现尾差。
/// 此处只管金额本身的形态，不涉及产品上下限区间和资产精度，那两项由各自的校验函数负责。
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

/// 判断一个 `BigDecimal` 能否被目标 DECIMAL 列无损保存，是赔率与金额校验共用的容量检查。
/// 先取出内部整数部分与十进制指数，指数即小数位数，超过 `max_scale` 直接判失败。
/// 整数位数由有效数字位数推算：去掉负号和前导零后得到有效位数，指数非负时减去小数位数，
/// 指数为负表示数值被放大了对应量级，此时改为加上其绝对值，`saturating` 运算避免边界下溢。
/// 超出 `max_integer_digits` 返回 `AppError::Validation`，错误信息用 `label` 指明是赔率还是金额，
/// 使后台能定位到具体字段。本函数不修改入参，也不做任何舍入或截断，只判定能否原样存储。
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

/// 归一化下单幂等键：裁剪首尾空白后要求非空且不超过 255 字节，与订单表唯一索引的列宽保持一致。
/// 幂等键是秒合约防重复下单的唯一依据，空键无法建立唯一约束，超长键入库会被截断而让两笔不同请求撞键，
/// 因此两种情况都在进入资金事务前返回 `AppError::Validation`，不占用唯一约束也不冻结任何资金。
/// 长度按 `len` 即字节数判定而非字符数，多字节字符的键需要客户端自行控制长度。
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

/// 把可选文本参数收敛成有值即有意义的形态：裁剪首尾空白，裁剪后为空串的一律降级为 `None`。
/// 用于订单与产品列表的状态、关键字等筛选项，避免前端传来的空白串被当成真实筛选条件而查不到数据。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 归一化产品图标一类的可选图片地址：空白串按未设置处理，返回 `None` 表示保持字段为空。
/// 有值时按 Unicode 字符数限制在 2048 以内，超长返回 `AppError::Validation` 并用 `field` 指明是哪个字段，
/// 防止超长地址写库被截断成打不开的残缺链接。本函数不校验协议合法性，也不探测地址可达性。
pub(crate) fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

/// 从鉴权令牌的 subject 中解析下单用户编号，要求形如 `user:{数字}`。
/// 前缀不符或数字部分解析失败都返回 `AppError::Unauthorized` 而非参数错误，
/// 使管理员令牌无法借用户接口操作他人资金，也不向调用方暴露 subject 的具体格式问题。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从鉴权令牌的 subject 中解析后台管理员编号，要求形如 `admin:{数字}`，解析结果用于审计日志的操作人字段。
/// 前缀不符或数字解析失败一律返回 `AppError::Unauthorized`，确保普通用户令牌无法命中产品配置与人工结算接口。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 归一化秒合约列表接口的分页条数，缺省取 50 并强制夹在 1 到 100 之间。
/// 下限为 1 可避免零条查询退化成无意义请求，上限 100 用来阻止单次拉取超大结果集拖垮订单表查询。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 归一化秒合约列表接口的分页偏移，缺省取 0 并截断到 100000。
/// 偏移同样设上限：超大 offset 会让订单历史这类大表退化为全表扫描加文件排序，深翻页应改用按时间或
/// 主键游标的方式，本函数只做防御性截断而不会因超限返回错误。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 后台审计原因允许的最大字符数，按 Unicode 字符计，对齐审计表原因列的存储容量。
const SECONDS_AUDIT_REASON_MAX_LEN: usize = 512;
/// 赔率允许的最大小数位数，对应赔率列的 DECIMAL scale。
const SECONDS_RATE_MAX_SCALE: i64 = 8;
/// 赔率允许的最大整数位数，与小数位共同构成赔率列的 DECIMAL 精度上限。
const SECONDS_RATE_MAX_INTEGER_DIGITS: usize = 10;
/// 投注与赔付金额允许的最大小数位数，对应金额列的 DECIMAL scale。
const SECONDS_AMOUNT_MAX_SCALE: i64 = 18;
/// 投注与赔付金额允许的最大整数位数，超出即判定为无法无损入库。
const SECONDS_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;
