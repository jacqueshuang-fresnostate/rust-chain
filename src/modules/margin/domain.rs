//! margin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件只保存杠杆的资金口径计算：逐仓返还额、全仓账户组合风险、条件强平价与强平清算政策。
//! 所有函数都是纯计算，不访问数据库、Redis 或行情，也不发布事件；调用方需自行保证输入取自同一时点快照。
//! 金额一律以 `with_scale(18)` 归一到十八位小数，与 `DECIMAL(38,18)` 资金列精度保持一致。

use bigdecimal::{BigDecimal, RoundingMode};
use std::str::FromStr;

/// 全仓条件强平价的净数量稳定性阈值：净数量不高于同 pair 总数量的百万分之一时不展示价格。
/// 该常量只用于展示估算的数值稳定性，绝不参与 `equity <= maintenance_margin` 的实际强平判定。
pub(crate) const CROSS_NET_DELTA_EPSILON: &str = "0.000001";

/// 杠杆开仓的唯一订单类型值对象，传输层文本只能在这里归一化为市价或限价。
/// 枚举本身不持有客户端价格；实际成交价仍由应用层传入的服务端权威 ticker 决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarginOrderType {
    Market,
    Limit,
}

impl MarginOrderType {
    /// 将缺省值按历史兼容语义解析为市价，显式值裁剪空白并忽略大小写。
    /// 任何其他文本都返回稳定错误，调用方必须在资金事务开始前拒绝。
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, &'static str> {
        let Some(value) = value else {
            return Ok(Self::Market);
        };
        if value.trim().eq_ignore_ascii_case("market") {
            Ok(Self::Market)
        } else if value.trim().eq_ignore_ascii_case("limit") {
            Ok(Self::Limit)
        } else {
            Err("margin order_type must be market or limit")
        }
    }

    /// 返回数据库、幂等比对与 API 共用的规范小写文本，不做额外分配或 I/O。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Limit => "limit",
        }
    }
}

/// 校验杠杆限价严格为正且有效小数位不超过交易对价格精度。
/// 先用 `normalized` 去掉尾随零，因此 `1.2300` 在两位精度下仍合法；非法的负精度配置一律拒绝。
/// 本函数只做纯计算，不会圆整或改写用户的限价，避免下单意图在边界内被静默改变。
pub(crate) fn validate_margin_limit_price(
    price: &BigDecimal,
    price_precision: i32,
) -> Result<(), &'static str> {
    if price <= &BigDecimal::from(0) {
        return Err("margin limit price must be positive");
    }
    if price_precision < 0 {
        return Err("margin pair price precision is invalid");
    }
    let (_, scale) = price.normalized().as_bigint_and_exponent();
    if scale.max(0) > i64::from(price_precision) {
        return Err("margin limit price exceeds pair price precision");
    }
    Ok(())
}

/// 用一笔权威市场价判定未成交杠杆限价单是否触发整笔成交。
/// 做多与买入限价同向，市场价不高于限价时触发；做空与卖出同向，市场价不低于限价时触发。
/// 等价边界也会成交；两个价格必须严格为正，方向非 long/short 则返回错误而不默认为某一边。
/// 本函数不读 Redis 也不写仓位，调用方必须确保 `market_price` 来自已被行情 CAS 接受的服务端 ticker。
pub(crate) fn margin_limit_order_is_triggered(
    direction: &str,
    limit_price: &BigDecimal,
    market_price: &BigDecimal,
) -> Result<bool, &'static str> {
    if limit_price <= &BigDecimal::from(0) {
        return Err("margin limit price must be positive");
    }
    if market_price <= &BigDecimal::from(0) {
        return Err("margin market price must be positive");
    }
    match direction {
        "long" => Ok(market_price <= limit_price),
        "short" => Ok(market_price >= limit_price),
        _ => Err("margin direction must be long or short"),
    }
}

/// 计算逐仓仓位平仓时可返还用户的金额，口径为保证金加已实现盈亏再扣除累计利息。
/// `realized_pnl` 为 None 表示仓位尚未成交也没有盈亏结果，直接返回十八位精度的零而非退回本金。
/// 结果按非负截断：亏损吃穿保证金时只返还零，穿仓缺口不在这里体现，由全仓账户结算或坏账登记承担。
/// 纯计算函数，不读写钱包余额、流水与仓位状态，调用方仍须在结算事务内自行完成入账。
pub(crate) fn margin_position_payout_amount(
    margin_amount: &BigDecimal,
    realized_pnl: Option<&BigDecimal>,
    interest_amount: &BigDecimal,
) -> BigDecimal {
    realized_pnl
        .map(|pnl| {
            let payout_amount = margin_amount + pnl - interest_amount;
            if payout_amount > 0 {
                payout_amount.with_scale(18)
            } else {
                BigDecimal::from(0).with_scale(18)
            }
        })
        .unwrap_or_else(|| BigDecimal::from(0).with_scale(18))
}

/// 把本次平仓或强平切片的已实现盈亏累加到仓位历史值，并统一落为十八位小数。
///
/// `previous` 为 NULL 代表该仓位此前没有已实现盈亏；它不等价于“本次盈亏缺失”，
/// 调用方仍必须提供通过服务端标记价计算出的当前切片结果。主动部分平仓、最终平仓和后续强平
/// 必须共用本函数，避免终态处理覆盖已经结算并写入钱包的历史切片盈亏。
pub(crate) fn accumulate_margin_realized_pnl(
    previous: Option<&BigDecimal>,
    current: &BigDecimal,
) -> BigDecimal {
    (previous
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0).with_scale(18))
        + current.clone())
    .with_scale(18)
}

/// 一次主动平仓从当前剩余仓位分配出的结算切片与落库后剩余金额。
/// 关闭金额和剩余金额成对保存，调用方不得再用浮点比例二次推导钱包或仓位写入值。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarginCloseSlice {
    /// 用户本次选择的整数百分比，作用于加锁后的当前剩余仓位。
    pub(crate) close_percentage: u16,
    /// 是否消费当前仓位的全部剩余敞口；只有该状态允许迁移为 closed。
    pub(crate) fully_closed: bool,
    /// 本次释放并参与权益结算的保证金。
    pub(crate) close_margin_amount: BigDecimal,
    /// 本次按权威标记价计算已实现盈亏的名义价值。
    pub(crate) close_notional_amount: BigDecimal,
    /// 本次同比例释放的借款本金快照，仅用于审计和剩余敞口维护。
    pub(crate) close_borrowed_amount: BigDecimal,
    /// 本次结算并从权益扣除的累计利息份额。
    pub(crate) close_interest_amount: BigDecimal,
    /// 部分平仓后继续留在 opened 仓位中的保证金。
    pub(crate) remaining_margin_amount: BigDecimal,
    /// 部分平仓后继续承担市场风险的名义价值。
    pub(crate) remaining_notional_amount: BigDecimal,
    /// 部分平仓后继续计息的借款本金。
    pub(crate) remaining_borrowed_amount: BigDecimal,
    /// 部分平仓后尚未结算的已计提利息。
    pub(crate) remaining_interest_amount: BigDecimal,
}

/// 按整数百分比从已锁定仓位分配一次平仓切片，并以十八位小数向下截取关闭份额。
/// 剩余金额始终用「原金额减关闭金额」得到，因此四类金额分别严格守恒；100% 直接消费原值，
/// 避免先乘除再圆整造成末位残留。1..99% 若把正保证金或正名义价值截成零，或使剩余值为零，
/// 则拒绝该请求，调用方必须在任何钱包、流水或仓位写入之前返回参数错误。
pub(crate) fn allocate_margin_close_slice(
    margin_amount: &BigDecimal,
    notional_amount: &BigDecimal,
    borrowed_amount: &BigDecimal,
    interest_amount: &BigDecimal,
    close_percentage: u16,
) -> Result<MarginCloseSlice, &'static str> {
    let zero = BigDecimal::from(0).with_scale(18);
    if !(1..=100).contains(&close_percentage) {
        return Err("margin close percentage must be between 1 and 100");
    }
    if margin_amount <= &zero || notional_amount <= &zero {
        return Err("margin close source margin and notional must be positive");
    }
    if borrowed_amount < &zero || interest_amount < &zero {
        return Err("margin close borrowed amount and interest must be non-negative");
    }

    let fully_closed = close_percentage == 100;
    let allocate = |amount: &BigDecimal| {
        if fully_closed {
            amount.clone().with_scale(18)
        } else {
            (amount.clone() * BigDecimal::from(close_percentage) / BigDecimal::from(100))
                .with_scale_round(18, RoundingMode::Down)
        }
    };
    let close_margin_amount = allocate(margin_amount);
    let close_notional_amount = allocate(notional_amount);
    let close_borrowed_amount = allocate(borrowed_amount);
    let close_interest_amount = allocate(interest_amount);
    let remaining_margin_amount =
        (margin_amount.clone() - close_margin_amount.clone()).with_scale(18);
    let remaining_notional_amount =
        (notional_amount.clone() - close_notional_amount.clone()).with_scale(18);
    let remaining_borrowed_amount =
        (borrowed_amount.clone() - close_borrowed_amount.clone()).with_scale(18);
    let remaining_interest_amount =
        (interest_amount.clone() - close_interest_amount.clone()).with_scale(18);

    if close_margin_amount <= zero || close_notional_amount <= zero {
        return Err("margin close percentage is below the representable amount");
    }
    if !fully_closed && (remaining_margin_amount <= zero || remaining_notional_amount <= zero) {
        return Err("margin partial close must preserve a positive remaining position");
    }

    Ok(MarginCloseSlice {
        close_percentage,
        fully_closed,
        close_margin_amount,
        close_notional_amount,
        close_borrowed_amount,
        close_interest_amount,
        remaining_margin_amount,
        remaining_notional_amount,
        remaining_borrowed_amount,
        remaining_interest_amount,
    })
}

/// 单仓在一笔服务端标记价下的统一风险结果。
///
/// 查询、主动平仓和强平都必须经由同一计算入口，避免方向盈亏、权益或维持保证金出现多套口径。
#[derive(Debug, Clone, PartialEq)]
pub struct MarginPositionRiskState {
    /// 权益是否已经不高于维持保证金。
    pub should_liquidate: bool,
    /// 保证金加标记盈亏再减累计利息。
    pub equity: BigDecimal,
    /// 名义价值乘当前产品维持保证金率。
    pub maintenance_margin: BigDecimal,
    /// 按标记价折算的未实现盈亏；保留历史字段语义供强平记录复用。
    pub realized_pnl: BigDecimal,
}

/// 按方向和同一标记价计算单仓盈亏，是主动平仓与账户风险评估共用的唯一价差公式。
pub(crate) fn margin_mark_pnl(
    direction: &str,
    notional_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
) -> Result<BigDecimal, &'static str> {
    if entry_price <= &BigDecimal::from(0) {
        return Err("margin entry price must be positive");
    }
    if mark_price <= &BigDecimal::from(0) {
        return Err("margin mark price must be positive");
    }
    let price_delta = match direction {
        "long" => mark_price.clone() - entry_price.clone(),
        "short" => entry_price.clone() - mark_price.clone(),
        _ => return Err("margin direction must be long or short"),
    };
    Ok((notional_amount.clone() * price_delta / entry_price.clone()).with_scale(18))
}

/// 计算逐仓风险；账户级查询和强平会复用其中的盈亏与维持保证金结果再做组合聚合。
pub fn evaluate_margin_position_risk(
    direction: &str,
    margin_amount: &BigDecimal,
    notional_amount: &BigDecimal,
    interest_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
    maintenance_margin_rate: &BigDecimal,
) -> Result<MarginPositionRiskState, &'static str> {
    if margin_amount < &BigDecimal::from(0)
        || notional_amount < &BigDecimal::from(0)
        || interest_amount < &BigDecimal::from(0)
        || maintenance_margin_rate < &BigDecimal::from(0)
    {
        return Err("margin risk amounts and rate must be non-negative");
    }
    let realized_pnl = margin_mark_pnl(direction, notional_amount, entry_price, mark_price)?;
    let equity =
        (margin_amount.clone() + realized_pnl.clone() - interest_amount.clone()).with_scale(18);
    let maintenance_margin =
        (notional_amount.clone() * maintenance_margin_rate.clone()).with_scale(18);
    Ok(MarginPositionRiskState {
        should_liquidate: equity <= maintenance_margin,
        equity,
        maintenance_margin,
        realized_pnl,
    })
}

/// 手机端持仓卡所需的派生风险指标，所有值均基于同一个服务端风险快照计算。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarginPositionDisplayMetrics {
    /// 名义价值折算的基础资产数量。
    pub(crate) position_quantity: BigDecimal,
    /// 浮动盈亏相对投入保证金的比例。
    pub(crate) return_rate: Option<BigDecimal>,
    /// 当前权益相对维持保证金的比例。
    pub(crate) margin_ratio: Option<BigDecimal>,
    /// 逐仓模式下的预估强平价格；全仓没有独立单仓强平价。
    pub(crate) estimated_liquidation_price: Option<BigDecimal>,
    /// 标记价到预估强平价格的绝对距离比例。
    pub(crate) liquidation_distance_rate: Option<BigDecimal>,
}

/// 单仓展示指标的同一时点输入，调用方必须从一份服务端风险快照中组装，禁止混用不同行情时刻。
pub(crate) struct MarginPositionDisplayInput<'a> {
    /// 仓位保证金模式，只接受 isolated 或 cross。
    pub(crate) margin_mode: &'a str,
    /// 仓位方向，只接受 long 或 short。
    pub(crate) direction: &'a str,
    /// 用户实际投入并被占用的保证金。
    pub(crate) margin_amount: &'a BigDecimal,
    /// 开仓时锁定的名义价值。
    pub(crate) notional_amount: &'a BigDecimal,
    /// 当前尚未结算的累计借款利息。
    pub(crate) interest_amount: &'a BigDecimal,
    /// 仓位真实成交均价。
    pub(crate) entry_price: &'a BigDecimal,
    /// 同一风险快照使用的服务端标记价。
    pub(crate) mark_price: &'a BigDecimal,
    /// 按标记价计算的未实现盈亏。
    pub(crate) unrealized_pnl: &'a BigDecimal,
    /// 保证金、未实现盈亏与利息合成后的当前权益。
    pub(crate) equity: &'a BigDecimal,
    /// 当前产品维持保证金率折算出的维持保证金金额。
    pub(crate) maintenance_margin: &'a BigDecimal,
}

/// 从单仓风险快照派生页面展示指标，不读取存储且不改变强平判定。
///
/// 数量、收益率和保证金率分别按入场价、投入保证金和维持保证金作为分母；分母为零时相应
/// 比率返回 `None`。预估强平价只对逐仓有单仓意义：做多和做空分别解出权益等于维持保证金
/// 时的标记价。算出的价格非正时视为没有有效强平价，距离也随之返回 `None`。
pub(crate) fn margin_position_display_metrics(
    input: MarginPositionDisplayInput<'_>,
) -> Result<MarginPositionDisplayMetrics, &'static str> {
    let zero = BigDecimal::from(0);
    if input.entry_price <= &zero || input.mark_price <= &zero {
        return Err("margin display prices must be positive");
    }
    if input.direction != "long" && input.direction != "short" {
        return Err("margin direction must be long or short");
    }
    if input.margin_mode != "isolated" && input.margin_mode != "cross" {
        return Err("margin mode must be isolated or cross");
    }

    let position_quantity = if input.notional_amount > &zero {
        (input.notional_amount.clone() / input.entry_price.clone()).with_scale(18)
    } else {
        zero.clone().with_scale(18)
    };
    let return_rate = (input.margin_amount > &zero)
        .then(|| (input.unrealized_pnl.clone() / input.margin_amount.clone()).with_scale(18));
    let margin_ratio = (input.maintenance_margin > &zero)
        .then(|| (input.equity.clone() / input.maintenance_margin.clone()).with_scale(18));

    let estimated_liquidation_price =
        if input.margin_mode == "isolated" && input.notional_amount > &zero {
            let adjustment = (input.maintenance_margin.clone() - input.margin_amount.clone()
                + input.interest_amount.clone())
                / input.notional_amount.clone();
            let multiplier = if input.direction == "long" {
                BigDecimal::from(1) + adjustment
            } else {
                BigDecimal::from(1) - adjustment
            };
            let price = (input.entry_price.clone() * multiplier).with_scale(18);
            (price > zero).then_some(price)
        } else {
            None
        };
    let liquidation_distance_rate = estimated_liquidation_price.as_ref().map(|price| {
        let delta = if input.mark_price >= price {
            input.mark_price.clone() - price.clone()
        } else {
            price.clone() - input.mark_price.clone()
        };
        (delta / input.mark_price.clone()).with_scale(18)
    });

    Ok(MarginPositionDisplayMetrics {
        position_quantity,
        return_rate,
        margin_ratio,
        estimated_liquidation_price,
        liquidation_distance_rate,
    })
}

/// 一个全仓账户中的单仓风险输入，三项都以保证金币种计价并已按当前标记价估过值。
#[derive(Debug, Clone)]
pub(crate) struct CrossMarginPositionRisk {
    /// 该仓位按最新标记价计算的浮动盈亏，做多做空均可正可负。
    pub(crate) unrealized_pnl: BigDecimal,
    /// 该仓位截至当前已计提但尚未结算的借款利息，恒为非负并直接抵减权益。
    pub(crate) interest_amount: BigDecimal,
    /// 该仓位的维持保证金要求，等于名义价值乘以产品维持保证金率。
    pub(crate) maintenance_margin: BigDecimal,
}

/// 一笔已成交全仓仓位和它在本批次使用的标记价。
pub(crate) struct MarkedCrossMarginPosition<'a> {
    pub(crate) direction: &'a str,
    pub(crate) margin_amount: &'a BigDecimal,
    pub(crate) notional_amount: &'a BigDecimal,
    pub(crate) interest_amount: &'a BigDecimal,
    pub(crate) entry_price: &'a BigDecimal,
    pub(crate) mark_price: &'a BigDecimal,
    pub(crate) maintenance_margin_rate: &'a BigDecimal,
}

/// 账户风险与按输入顺序返回的各仓统一估值结果。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EvaluatedCrossMarginRisk {
    pub(crate) account: CrossMarginRiskState,
    pub(crate) positions: Vec<MarginPositionRiskState>,
}

/// 全仓账户组合风险快照，同一用户同一保证金币种下所有全仓仓位共享这组指标。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossMarginRiskState {
    /// 账户总权益，含杠杆钱包可用余额、已占用仓位保证金、浮盈与应付利息。
    pub(crate) equity: BigDecimal,
    /// 各仓位浮动盈亏之和。
    pub(crate) unrealized_pnl: BigDecimal,
    /// 各仓位应付利息之和。
    pub(crate) interest_amount: BigDecimal,
    /// 各仓位维持保证金之和，是判定强平线的分母。
    pub(crate) maintenance_margin: BigDecimal,
    /// 账户权益与维持保证金之比；无仓位或维持保证金为零时为 None，表示该比率无意义。
    pub(crate) margin_ratio: Option<BigDecimal>,
    /// 是否已触发账户级强平，由存在仓位且权益不高于维持保证金共同决定。
    pub(crate) should_liquidate: bool,
}

/// 汇总同一用户、同一保证金币种下全部全仓仓位的估值，产出账户级共享风险快照。
/// 浮盈、利息与维持保证金按仓位逐项求和；账户权益为钱包权益加已占用仓位保证金加浮盈再减利息，
/// 权益与各项汇总均归一到十八位小数；强平后钱包如何处置由独立的账户清算政策决定，不能把仓位净值当作返款。
/// 维持保证金为零时保证金率返回 None 以避免除零；仓位非空且权益不高于维持保证金即置强平标记。
/// 纯计算函数，不查询行情、不加行锁、不落库，输入必须由调用方取自同一时点的仓位与钱包快照。
pub(crate) fn evaluate_cross_margin(
    wallet_equity: &BigDecimal,
    position_margin: &BigDecimal,
    positions: &[CrossMarginPositionRisk],
) -> CrossMarginRiskState {
    let unrealized_pnl = positions
        .iter()
        .fold(BigDecimal::from(0), |total, position| {
            total + position.unrealized_pnl.clone()
        })
        .with_scale(18);
    let interest_amount = positions
        .iter()
        .fold(BigDecimal::from(0), |total, position| {
            total + position.interest_amount.clone()
        })
        .with_scale(18);
    let maintenance_margin = positions
        .iter()
        .fold(BigDecimal::from(0), |total, position| {
            total + position.maintenance_margin.clone()
        })
        .with_scale(18);
    let equity = (wallet_equity.clone() + position_margin.clone() + unrealized_pnl.clone()
        - interest_amount.clone())
    .with_scale(18);
    let margin_ratio = if maintenance_margin > 0 {
        Some((equity.clone() / maintenance_margin.clone()).with_scale(18))
    } else {
        None
    };
    CrossMarginRiskState {
        should_liquidate: !positions.is_empty() && equity <= maintenance_margin,
        equity,
        unrealized_pnl,
        interest_amount,
        maintenance_margin,
        margin_ratio,
    }
}

/// 用同一批标记价统一评估全仓账户。
///
/// 本函数先逐仓调用唯一单仓风险公式，再调用账户聚合公式；查询、强平和资金转出均复用该入口，
/// 因而不会各自复制方向盈亏或维持保证金算法。输入顺序会被保留，方便调用方关联仓位主键。
pub(crate) fn evaluate_marked_cross_margin(
    wallet_equity: &BigDecimal,
    positions: &[MarkedCrossMarginPosition<'_>],
) -> Result<EvaluatedCrossMarginRisk, &'static str> {
    let mut position_margin = BigDecimal::from(0);
    let mut position_states = Vec::with_capacity(positions.len());
    let mut account_positions = Vec::with_capacity(positions.len());
    for position in positions {
        let state = evaluate_margin_position_risk(
            position.direction,
            position.margin_amount,
            position.notional_amount,
            position.interest_amount,
            position.entry_price,
            position.mark_price,
            position.maintenance_margin_rate,
        )?;
        position_margin += position.margin_amount.clone();
        account_positions.push(CrossMarginPositionRisk {
            unrealized_pnl: state.realized_pnl.clone(),
            interest_amount: position.interest_amount.clone(),
            maintenance_margin: state.maintenance_margin.clone(),
        });
        position_states.push(state);
    }
    Ok(EvaluatedCrossMarginRisk {
        account: evaluate_cross_margin(
            wallet_equity,
            &position_margin.with_scale(18),
            &account_positions,
        ),
        positions: position_states,
    })
}

/// 从账户风险缓冲和实际 available 共同得出可转回现货的上限。
/// 上限恰为 `min(available, max(equity - maintenance, 0))`，调用方仍须用转后快照复核不低于维持线。
pub(crate) fn cross_margin_max_transferable(
    available: &BigDecimal,
    risk: &CrossMarginRiskState,
) -> Result<BigDecimal, &'static str> {
    if available < &BigDecimal::from(0) {
        return Err("cross margin available balance must be non-negative");
    }
    let zero = BigDecimal::from(0).with_scale(18);
    let buffer = (risk.equity.clone() - risk.maintenance_margin.clone()).with_scale(18);
    if buffer <= zero {
        Ok(zero)
    } else if buffer < *available {
        Ok(buffer)
    } else {
        Ok(available.clone().with_scale(18))
    }
}

/// 全仓条件强平价估算中的一笔已成交仓位，数量由名义价值除以入场价得到。
pub(crate) struct CrossMarginReferencePosition<'a> {
    /// 交易对主键，只有与参考仓位同 pair 的仓位才改变条件价格斜率。
    pub(crate) pair_id: u64,
    /// long 记为正数量、short 记为负数量，非法值会使账户快照失败。
    pub(crate) direction: &'a str,
    /// 开仓时锁定的名义价值。
    pub(crate) notional_amount: &'a BigDecimal,
    /// 已成交的正入场价，是基础资产数量的分母。
    pub(crate) entry_price: &'a BigDecimal,
}

/// 条件强平价的稳定状态，传输层直接使用对应的 snake_case 值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrossMarginEstimateStatus {
    /// 当前账户尚未触发，且存在正的不利方向价格边界。
    Estimated,
    /// 当前权益已不高于维持保证金，无需再展示未来触发价。
    AlreadyLiquidatable,
    /// 同 pair 多空净数量精确为零，共享标记价变化不改变账户权益。
    NetDeltaZero,
    /// 净数量占总数量比例落入命名阈值，计算结果对小额资金变动过度敏感。
    NetDeltaNearZero,
    /// 目标 pair 没有正的可估值数量，数据不足以建立价格边界。
    InvalidExposure,
    /// 线性求根得到零或负价，正价轴上不存在该条件边界。
    NoPositiveBoundary,
    /// 按交易对精度保守圆整后不再位于净数量的不利方向。
    WrongAdverseDirection,
}

impl CrossMarginEstimateStatus {
    /// 返回 API 和手机端共用的稳定状态码，不做本地化或分配。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Estimated => "estimated",
            Self::AlreadyLiquidatable => "already_liquidatable",
            Self::NetDeltaZero => "net_delta_zero",
            Self::NetDeltaNearZero => "net_delta_near_zero",
            Self::InvalidExposure => "invalid_exposure",
            Self::NoPositiveBoundary => "no_positive_boundary",
            Self::WrongAdverseDirection => "wrong_adverse_direction",
        }
    }
}

/// 指定 pair 在「其他 pair 标记价不变」假设下的账户级条件强平价结果。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossMarginConditionalPrice {
    /// 同 pair 按 long 正、short 负汇总的基础资产数量。
    pub(crate) net_quantity: BigDecimal,
    /// 同 pair 不抵消方向的基础资产总数量。
    pub(crate) gross_quantity: BigDecimal,
    /// 估算状态；只有 Estimated 同时携带价格和距离。
    pub(crate) status: CrossMarginEstimateStatus,
    /// 保守圆整到 pair 价格精度的条件强平价。
    pub(crate) price: Option<BigDecimal>,
    /// 条件价与当前共享标记价的绝对距离比例。
    pub(crate) distance_rate: Option<BigDecimal>,
}

/// 按 `P*=P0-Buffer/D` 估算指定 pair 的账户级条件强平价。
/// 这里只让参考 pair 的所有多空腿共用一个变量价，其他 pair、钱包、利息和维持保证金保持输入快照不变。
/// `liquidation_buffer` 必须是同一账户快照的 `equity-maintenance_margin`；非正表示已触发，先于对冲稳定性判定。
/// 净数量精确为零或占总数量不高于 `CROSS_NET_DELTA_EPSILON` 时返回明确状态和空价格，不伪造无穷大。
/// 有效根按净多头向上取 tick、净空头向下取 tick，保证展示价不比真实边界更乐观；圆整后方向失真则返回空值。
/// 本函数只做 BigDecimal 纯计算，不更改实际强平触发精度，行情完整性和新鲜度由调用方在组装输入前保证。
pub(crate) fn estimate_cross_margin_conditional_price(
    reference_pair_id: u64,
    current_mark: &BigDecimal,
    liquidation_buffer: &BigDecimal,
    price_precision: i32,
    positions: &[CrossMarginReferencePosition<'_>],
) -> Result<CrossMarginConditionalPrice, &'static str> {
    let zero = BigDecimal::from(0);
    if current_mark <= &zero {
        return Err("cross margin reference mark must be positive");
    }
    if price_precision < 0 {
        return Err("cross margin price precision is invalid");
    }

    let mut net_quantity = zero.clone();
    let mut gross_quantity = zero.clone();
    let mut invalid_exposure = false;
    for position in positions
        .iter()
        .filter(|position| position.pair_id == reference_pair_id)
    {
        if position.entry_price <= &zero || position.notional_amount <= &zero {
            invalid_exposure = true;
            continue;
        }
        let quantity = position.notional_amount.clone() / position.entry_price.clone();
        gross_quantity += quantity.clone();
        match position.direction {
            "long" => net_quantity += quantity,
            "short" => net_quantity -= quantity,
            _ => return Err("margin direction must be long or short"),
        }
    }
    let display_net_quantity = net_quantity.clone().with_scale(18);
    let display_gross_quantity = gross_quantity.clone().with_scale(18);
    let empty_result = |status| CrossMarginConditionalPrice {
        net_quantity: display_net_quantity.clone(),
        gross_quantity: display_gross_quantity.clone(),
        status,
        price: None,
        distance_rate: None,
    };

    if liquidation_buffer <= &zero {
        return Ok(empty_result(CrossMarginEstimateStatus::AlreadyLiquidatable));
    }
    if invalid_exposure || gross_quantity <= zero {
        return Ok(empty_result(CrossMarginEstimateStatus::InvalidExposure));
    }
    if net_quantity == 0 {
        return Ok(empty_result(CrossMarginEstimateStatus::NetDeltaZero));
    }
    let absolute_net_quantity = decimal_absolute(&net_quantity);
    let net_delta_ratio = absolute_net_quantity / gross_quantity;
    let epsilon = BigDecimal::from_str(CROSS_NET_DELTA_EPSILON)
        .expect("CROSS_NET_DELTA_EPSILON must be a valid decimal");
    if net_delta_ratio <= epsilon {
        return Ok(empty_result(CrossMarginEstimateStatus::NetDeltaNearZero));
    }

    let exact_price = current_mark.clone() - liquidation_buffer.clone() / net_quantity.clone();
    if exact_price <= zero {
        return Ok(empty_result(CrossMarginEstimateStatus::NoPositiveBoundary));
    }
    let rounding_mode = if net_quantity > 0 {
        RoundingMode::Ceiling
    } else {
        RoundingMode::Floor
    };
    let price = exact_price
        .with_scale_round(i64::from(price_precision), rounding_mode)
        .with_scale(18);
    let wrong_adverse_direction = if net_quantity > 0 {
        &price >= current_mark
    } else {
        &price <= current_mark
    };
    if wrong_adverse_direction {
        return Ok(empty_result(
            CrossMarginEstimateStatus::WrongAdverseDirection,
        ));
    }
    let distance_rate = (decimal_absolute(&(price.clone() - current_mark.clone()))
        / current_mark.clone())
    .with_scale(18);
    Ok(CrossMarginConditionalPrice {
        net_quantity: display_net_quantity,
        gross_quantity: display_gross_quantity,
        status: CrossMarginEstimateStatus::Estimated,
        price: Some(price),
        distance_rate: Some(distance_rate),
    })
}

/// 全仓强平的唯一账户级钱包政策结果，不包含 frozen/locked 因为这两桶在本次清算中不变。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossMarginLiquidationSettlement {
    /// 强平后 available 恒为十八位精度的零。
    pub(crate) available_after: BigDecimal,
    /// 账户级流水增量，精确等于负的事务锁定前 available。
    pub(crate) wallet_delta: BigDecimal,
    /// 穿仓坏账，精确等于强平前账户总权益负值的正部分。
    pub(crate) bad_debt: BigDecimal,
}

/// 对一份已锁定的全仓账户快照应用「可用抵押全部消耗」清算政策。
/// `available_before` 来自 `margin_wallet_accounts` 的 FOR UPDATE 行，必须非负；结果只把该桶一次性归零，流水为其负值。
/// 仓位保证金、浮盈与利息不再作为钱包返款；正剩余权益被强平政策消耗，仅负账户权益的绝对值记为坏账。
/// 纯计算不访问钱包；调用方必须将余额更新、唯一账户流水、仓位终态与坏账写入同一事务。
pub(crate) fn cross_margin_liquidation_settlement(
    available_before: &BigDecimal,
    account_equity: &BigDecimal,
) -> Result<CrossMarginLiquidationSettlement, &'static str> {
    if available_before < &BigDecimal::from(0) {
        return Err("cross margin wallet available must be non-negative");
    }
    let zero = BigDecimal::from(0).with_scale(18);
    let bad_debt = if account_equity < &zero {
        (-account_equity.clone()).with_scale(18)
    } else {
        zero.clone()
    };
    Ok(CrossMarginLiquidationSettlement {
        available_after: zero,
        wallet_delta: (-available_before.clone()).with_scale(18),
        bad_debt,
    })
}

fn decimal_absolute(value: &BigDecimal) -> BigDecimal {
    if value < &BigDecimal::from(0) {
        -value.clone()
    } else {
        value.clone()
    }
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_domain_tests.rs"]
mod tests;
