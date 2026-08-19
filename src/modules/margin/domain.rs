//! margin bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件只保存杠杆的资金口径计算：逐仓返还额、全仓账户组合风险评估和账户权益的分仓分摊。
//! 所有函数都是纯计算，不访问数据库、Redis 或行情，也不发布事件；调用方需自行保证输入取自同一时点快照。
//! 金额一律以 `with_scale(18)` 归一到十八位小数，与 `DECIMAL(38,18)` 资金列精度保持一致。

use bigdecimal::BigDecimal;

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

/// 全仓账户组合风险快照，同一用户同一保证金币种下所有全仓仓位共享这组指标。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossMarginRiskState {
    /// 账户总权益，含杠杆钱包可用余额、已占用仓位保证金、浮盈与应付利息。
    pub(crate) equity: BigDecimal,
    /// 组合权益，仅统计这批仓位自身净值，是强平时回写共享钱包的有符号增量。
    pub(crate) portfolio_equity: BigDecimal,
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
/// 组合权益剔除钱包部分，只表示这批仓位的净值，两者与各项汇总均归一到十八位小数。
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
    let portfolio_equity =
        (position_margin.clone() + unrealized_pnl.clone() - interest_amount.clone()).with_scale(18);
    let margin_ratio = if maintenance_margin > 0 {
        Some((equity.clone() / maintenance_margin.clone()).with_scale(18))
    } else {
        None
    };
    CrossMarginRiskState {
        should_liquidate: !positions.is_empty() && equity <= maintenance_margin,
        equity,
        portfolio_equity,
        unrealized_pnl,
        interest_amount,
        maintenance_margin,
        margin_ratio,
    }
}

/// 将账户级可返还权益按各仓位正权益的占比拆分到每个仓位，仅用于强平记录和事件展示。
///
/// 钱包只按组合权益整体变更一次；这里的分摊结果之和被严格约束为不超过组合正权益，
/// 避免盈利仓按自身权益返还、亏损仓被截零后凭空多分出资产。
/// 组合权益非正或所有仓位权益都非正时，整批返回十八位精度的零，长度与输入一一对应。
/// 权益非正的仓位一律分到零；最后一个正权益仓位领取剩余尾差，吸收逐笔按比例相除产生的十八位截断误差。
pub(crate) fn allocate_cross_margin_payouts(
    position_equities: &[BigDecimal],
    portfolio_equity: &BigDecimal,
) -> Vec<BigDecimal> {
    let zero = BigDecimal::from(0).with_scale(18);
    let payout_total = if portfolio_equity > &zero {
        portfolio_equity.clone().with_scale(18)
    } else {
        zero.clone()
    };
    let positive_total = position_equities
        .iter()
        .filter(|equity| *equity > &zero)
        .fold(zero.clone(), |total, equity| total + equity.clone())
        .with_scale(18);
    if payout_total == zero || positive_total == zero {
        return vec![zero; position_equities.len()];
    }

    let mut allocated = zero.clone();
    let last_positive_index = position_equities
        .iter()
        .rposition(|equity| equity > &zero)
        .expect("positive total requires one positive position");
    position_equities
        .iter()
        .enumerate()
        .map(|(index, equity)| {
            if equity <= &zero {
                return zero.clone();
            }
            if index == last_positive_index {
                return (payout_total.clone() - allocated.clone()).with_scale(18);
            }
            let amount =
                (payout_total.clone() * equity.clone() / positive_total.clone()).with_scale(18);
            allocated += amount.clone();
            amount
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_domain_tests.rs"]
mod tests;
