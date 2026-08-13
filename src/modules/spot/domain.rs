//! spot bounded context domain layer.
//!
//! 领域层：放置领域实体、值对象、错误和纯业务规则。
//! 这部分代码不依赖数据库/网络/HTTP，便于被应用层直接复用和独立测试。
//! 三类规则集中在此：按订单类型构造订单实体并校验交易对约束、计算下单预留的资产与金额、
//! 以及订单状态机的合法迁移判定，包括撤单与成交推进。
//! 校验口径统一为：交易对必须启用、数量与价格严格为正且不超过交易对精度、成交额不低于最小下单额。
//! 全部函数都是纯计算，不查库、不读行情、不加锁、不发事件，调用方须自行在事务内完成落库。

use crate::modules::wallet::WalletServiceError;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 订单方向，决定预留哪种资产以及成交时资金往哪个方向流。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    /// 买入，预留报价资产，成交后收到基础资产。
    Buy,
    /// 卖出，预留基础资产，成交后收到报价资产。
    Sell,
}

/// 订单类型，决定价格字段的必填组合与是否需要触发价。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    /// 限价单，必须带价格，按该价格预留资金并等待行情触发。
    Limit,
    /// 市价单，不接受价格，用服务端参考价折算预留额。
    Market,
    /// 止损限价单，同时需要触发价和限价，触发后按限价成交。
    StopLimit,
}

/// 订单状态机的全部取值，合法迁移关系由 `can_transition` 定义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// 刚创建、资金已冻结但尚未挂上盘口的初始态。
    Pending,
    /// 已挂单等待成交。
    Open,
    /// 部分成交，剩余数量仍占用冻结资金。
    PartiallyFilled,
    /// 完全成交，是终态。
    Filled,
    /// 已撤销，未成交部分的冻结资金已退回，是终态。
    Cancelled,
    /// 被拒绝，是终态且不可再迁移到任何其他状态。
    Rejected,
}

/// 交易对的下单约束快照，所有校验规则都以它为依据。
#[derive(Debug, Clone)]
pub struct TradingPairRule {
    /// 交易对业务标识，会被写进新建订单实体。
    pub pair_id: String,
    /// 价格允许的最大小数位，超出即报精度错误。
    pub price_precision: u32,
    /// 数量允许的最大小数位，超出即报精度错误。
    pub quantity_precision: u32,
    /// 最小下单额，以价格乘数量的成交额口径比较，低于它拒绝下单。
    pub min_order_value: BigDecimal,
    /// 交易对是否启用，停用时任何类型的下单都会被拒绝。
    pub enabled: bool,
}

/// 尚未持久化的订单实体，由三个构造函数产出，`status` 恒为 Pending 且成交量为零。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrder {
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    /// 限价与止损限价的委托价；市价单恒为 None，其参考价不落到订单上。
    pub price: Option<BigDecimal>,
    /// 止损限价的触发价，其余类型恒为 None。
    pub trigger_price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub filled_quantity: BigDecimal,
    pub status: OrderStatus,
}

/// 已持久化的订单实体，比 `NewOrder` 多一个主键标识，其余字段语义相同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotOrder {
    pub id: String,
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub trigger_price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    /// 已成交数量，与 `quantity` 的差值即剩余量，决定还有多少冻结资金未释放。
    pub filled_quantity: BigDecimal,
    pub status: OrderStatus,
}

/// 尚未持久化的成交记录，买卖双方订单标识成对出现。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSpotTrade {
    pub pair_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    /// 实际成交价，介于买单限价与卖单限价之间。
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    /// 本笔成交收取的手续费。
    pub fee: BigDecimal,
}

/// 已持久化的成交记录，比 `NewSpotTrade` 多主键和成交时间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotTrade {
    pub id: String,
    pub pair_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub fee: BigDecimal,
    pub created_at: DateTime<Utc>,
}

/// 领域层校验失败的全部原因，都是纯规则冲突，不含任何 I/O 或存储错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotDomainError {
    TradingPairDisabled,
    LimitOrderRequiresPrice,
    StopLimitOrderRequiresPrice,
    StopLimitOrderRequiresTriggerPrice,
    MarketOrderRejectsPrice,
    NonPositivePrice,
    NonPositiveQuantity,
    PricePrecisionExceeded {
        allowed: u32,
    },
    QuantityPrecisionExceeded {
        allowed: u32,
    },
    MinOrderValueNotMet {
        actual: BigDecimal,
        minimum: BigDecimal,
    },
    InvalidStatusTransition {
        from: OrderStatus,
        to: OrderStatus,
    },
    FillQuantityExceedsRemaining {
        remaining: BigDecimal,
        fill: BigDecimal,
    },
}

/// 服务层错误，聚合领域规则冲突、钱包错误、缺失价格输入和仓储故障四类来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotServiceError {
    /// 领域校验未通过，属于请求本身不合法。
    Domain(SpotDomainError),
    /// 钱包侧错误，例如余额不足或账户缺失。
    Wallet(WalletServiceError),
    /// 计算剩余冻结额时买单缺少价格，无法折算报价资产数量。
    MissingPriceForWalletReservation,
    /// 市价单缺少服务端参考价，无法确定预留金额。
    MissingReferencePriceForMarketOrder,
    /// 止损限价单缺少触发价，无法判定何时进入撮合。
    MissingTriggerPriceForStopLimitOrder,
    /// 持久化层故障，携带原始错误描述。
    Repository(String),
}

/// 把领域校验错误提升为服务层错误，使用例代码能用问号运算符统一向上传播。
impl From<SpotDomainError> for SpotServiceError {
    fn from(error: SpotDomainError) -> Self {
        Self::Domain(error)
    }
}

/// 把钱包错误提升为服务层错误，保留原始变体以便调用方区分余额不足与账户异常。
impl From<WalletServiceError> for SpotServiceError {
    fn from(error: WalletServiceError) -> Self {
        Self::Wallet(error)
    }
}

/// 构造限价订单实体，依次校验交易对已启用、数量为正且精度合规、价格为正且精度合规，
/// 最后按价格乘数量得到的委托额不得低于交易对的最小下单额。
/// 产出的实体状态恒为 Pending、成交量为零、触发价为空，价格原样保留供后续冻结与撮合使用。
/// 纯计算不落库，任一校验失败即返回对应领域错误，调用方此时不得产生任何资金副作用。
pub fn create_limit_order(
    user_id: impl Into<String>,
    side: OrderSide,
    price: BigDecimal,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&price, pair.price_precision)?;
    validate_min_order_value(price.clone() * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::Limit,
        price: Some(price),
        trigger_price: None,
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

/// 构造市价订单实体，用调用方给定的服务端参考价参与精度和最小下单额校验。
/// 与限价单的关键差别是参考价只用于校验和折算预留额，不会写进订单的 `price` 字段，
/// 该字段保持为 None，表示这笔单不承诺任何成交价格。
/// 参考价本身仍须为正且满足交易对价格精度，避免用异常行情算出错误的冻结金额。
/// 纯计算不落库，校验失败返回领域错误且不产生任何资金副作用。
pub fn create_market_order(
    user_id: impl Into<String>,
    side: OrderSide,
    quantity: BigDecimal,
    reference_price: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&reference_price, pair.price_precision)?;
    validate_min_order_value(reference_price * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::Market,
        price: None,
        trigger_price: None,
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

/// 构造止损限价订单实体，触发价和委托价都要独立通过为正与精度校验，两者可以不相等。
/// 最小下单额只按委托价乘数量判定，不看触发价，因为触发价只决定何时进入撮合而非成交金额。
/// 产出的实体同时带 `price` 和 `trigger_price`，是三种类型里唯一两者皆非空的。
/// 不校验触发价与委托价的相对大小，也不判断触发方向，那属于触发用例的职责。
/// 纯计算不落库，校验失败返回领域错误且不产生任何资金副作用。
pub fn create_stop_limit_order(
    user_id: impl Into<String>,
    side: OrderSide,
    trigger_price: BigDecimal,
    price: BigDecimal,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&trigger_price, pair.price_precision)?;
    validate_price(&price, pair.price_precision)?;
    validate_min_order_value(price.clone() * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::StopLimit,
        price: Some(price),
        trigger_price: Some(trigger_price),
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

/// 计算下单需要冻结的金额：买单为价格乘数量的报价资产额，卖单直接就是基础资产数量。
/// 这个不对称来自现货的资金语义，买方付出报价资产换基础资产，卖方反之。
/// 返回值不是 Result，因为它只做乘法不做校验，正负与精度由调用方在构造订单时保证。
/// 市价单调用时传入的是服务端参考价，因此冻结额是估算值，成交后多余部分由结算路径退回。
pub fn spot_reservation_amount(
    side: OrderSide,
    price: &BigDecimal,
    quantity: &BigDecimal,
) -> BigDecimal {
    reservation_amount(side, price, quantity)
}

/// 选出下单要冻结哪一种资产：买单冻结报价资产，卖单冻结基础资产。
/// 返回的是输入引用之一而非新分配的字符串，因此结果的生命周期与两个入参绑定。
/// 它与 `spot_reservation_amount` 必须成对使用，一个给出币种一个给出数量，二者错配会冻错账户。
pub fn spot_reserve_asset_id<'a>(
    side: OrderSide,
    base_asset_id: &'a str,
    quote_asset_id: &'a str,
) -> &'a str {
    reserve_asset_id(side, base_asset_id, quote_asset_id)
}

/// 计算撤单时还应退回多少冻结资金，口径是「未成交数量」而非原始下单数量。
/// 买单返回报价资产标识和价格乘剩余量的金额，卖单返回基础资产标识和剩余量本身。
/// 部分成交的订单因此只退回未成交那部分，已成交部分的冻结额在结算时就已被扣走。
/// 买单没有价格时返回 `MissingPriceForWalletReservation`，因为无从折算报价资产金额；
/// 卖单不依赖价格，所以任何情况下都能算出结果。
pub fn spot_remaining_reserved_amount(
    order: &SpotOrder,
    base_asset_id: &str,
    quote_asset_id: &str,
) -> Result<(String, BigDecimal), SpotServiceError> {
    remaining_reserved_amount(order, base_asset_id, quote_asset_id)
        .map(|reserved| (reserved.asset_id, reserved.amount))
}

/// 只做下单请求的合法性预检而不产出订单实体，按类型与价格是否存在的六种组合分派。
/// 限价带价时借道 `create_limit_order` 复用全套校验并丢弃结果，因此会用占位用户和买方向，
/// 这不影响判定，因为方向和用户标识都不参与任何校验规则。
/// 限价缺价、市价带价分别报对应的必填与冲突错误；市价缺价只校验数量和交易对，因为没有价格可验。
/// 止损限价在这里一律报错：带价提示缺触发价、缺价提示缺价格，说明本入口不支持该类型的完整校验，
/// 调用方必须改用 `create_stop_limit_order` 并同时提供触发价与委托价。
pub fn validate_order_request(
    order_type: OrderType,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<(), SpotDomainError> {
    match (order_type, price) {
        (OrderType::Limit, Some(price)) => {
            create_limit_order("validation", OrderSide::Buy, price, quantity, pair).map(|_| ())
        }
        (OrderType::Limit, None) => Err(SpotDomainError::LimitOrderRequiresPrice),
        (OrderType::Market, Some(_)) => Err(SpotDomainError::MarketOrderRejectsPrice),
        (OrderType::Market, None) => validate_common(&quantity, pair),
        (OrderType::StopLimit, Some(_)) => Err(SpotDomainError::StopLimitOrderRequiresTriggerPrice),
        (OrderType::StopLimit, None) => Err(SpotDomainError::StopLimitOrderRequiresPrice),
    }
}

/// 按订单状态机判定一次迁移是否合法，合法则回显目标状态，非法则返回带前后状态的错误。
/// 只做判定不修改任何订单，调用方拿到结果后需自行赋值，`apply_fill` 就是这样用的。
/// 允许 PartiallyFilled 迁移到自身以支持多次部分成交，也允许 Cancelled 到自身以支持撤单重放；
/// Filled 和 Rejected 是绝对终态，不能迁往任何状态，包括自身。
pub fn transition_status(
    current: OrderStatus,
    next: OrderStatus,
) -> Result<OrderStatus, SpotDomainError> {
    if can_transition(current, next) {
        Ok(next)
    } else {
        Err(SpotDomainError::InvalidStatusTransition {
            from: current,
            to: next,
        })
    }
}

/// 就地把订单迁移为 Cancelled，返回值表示本次调用是否真的改变了状态。
/// 已是 Cancelled 时返回 false 且不做任何修改，这正是撤单幂等的依据：
/// 调用方据此判断该不该退还冻结资金和发布撤单事件，避免重复退款或重复通知。
/// Pending、Open、PartiallyFilled 三种活跃态都可撤销并返回 true。
/// Filled 与 Rejected 是终态，返回带前后状态的迁移错误，不会静默成功。
pub fn cancel_order(order: &mut SpotOrder) -> Result<bool, SpotDomainError> {
    match order.status {
        OrderStatus::Cancelled => Ok(false),
        OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled => {
            order.status = OrderStatus::Cancelled;
            Ok(true)
        }
        status => Err(SpotDomainError::InvalidStatusTransition {
            from: status,
            to: OrderStatus::Cancelled,
        }),
    }
}

/// 就地把一笔成交量累加到订单上，并按累加后是否等于下单量迁移到 Filled 或 PartiallyFilled。
/// 成交量必须为正，且不得超过 `quantity` 减 `filled_quantity` 的剩余量，超量返回带剩余量的错误，
/// 这是防止同一订单被超额成交、进而多扣冻结资金的领域级兜底。
/// 状态迁移先判定后落值：迁移非法时直接返回错误，`filled_quantity` 不会被改写，
/// 因此对终态订单调用本函数不会留下「成交量变了但状态没变」的中间态。
/// 相等判定用十进制精确比较，不做容差，因此累加结果必须与下单量完全一致才判为完全成交。
pub fn apply_fill(order: &mut SpotOrder, fill_quantity: BigDecimal) -> Result<(), SpotDomainError> {
    if fill_quantity <= 0 {
        return Err(SpotDomainError::NonPositiveQuantity);
    }

    let remaining = order.quantity.clone() - order.filled_quantity.clone();
    if fill_quantity > remaining {
        return Err(SpotDomainError::FillQuantityExceedsRemaining {
            remaining,
            fill: fill_quantity,
        });
    }

    let next_filled = order.filled_quantity.clone() + fill_quantity;
    let next_status = if next_filled == order.quantity {
        OrderStatus::Filled
    } else {
        OrderStatus::PartiallyFilled
    };

    transition_status(order.status, next_status)?;
    order.filled_quantity = next_filled;
    order.status = next_status;
    Ok(())
}

/// 冻结资产与金额的配对结果，只在领域层内部流转，对外由公开函数拆成元组返回。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReservedAmount {
    asset_id: String,
    amount: BigDecimal,
}

/// 计算订单未成交部分仍占用的冻结资产与金额，是撤单退款额的唯一口径来源。
/// 剩余量取下单量减已成交量；买单折算成报价资产金额，卖单直接以基础资产数量表示。
/// 买单缺少价格时返回服务层错误而非领域错误，因为这属于数据完整性问题而不是请求非法。
/// 资产标识会被复制成新的 String，因为返回值不借用入参的生命周期。
fn remaining_reserved_amount(
    order: &SpotOrder,
    base_asset_id: &str,
    quote_asset_id: &str,
) -> Result<ReservedAmount, SpotServiceError> {
    let remaining_quantity = order.quantity.clone() - order.filled_quantity.clone();
    match order.side {
        OrderSide::Buy => {
            let price = order
                .price
                .clone()
                .ok_or(SpotServiceError::MissingPriceForWalletReservation)?;
            Ok(ReservedAmount {
                asset_id: quote_asset_id.to_owned(),
                amount: price * remaining_quantity,
            })
        }
        OrderSide::Sell => Ok(ReservedAmount {
            asset_id: base_asset_id.to_owned(),
            amount: remaining_quantity,
        }),
    }
}

/// 按方向在基础资产与报价资产之间二选一，买单选报价、卖单选基础。
/// 独立成私有函数是为了让公开入口和内部剩余额计算共用同一份映射，杜绝两处判断写反。
fn reserve_asset_id<'a>(
    side: OrderSide,
    base_asset_id: &'a str,
    quote_asset_id: &'a str,
) -> &'a str {
    match side {
        OrderSide::Buy => quote_asset_id,
        OrderSide::Sell => base_asset_id,
    }
}

/// 按方向决定冻结金额的计算式，买单为价格乘数量，卖单忽略价格直接取数量。
/// 卖单不看价格是因为它冻结的就是要卖出的基础资产本身，与成交价无关。
fn reservation_amount(side: OrderSide, price: &BigDecimal, quantity: &BigDecimal) -> BigDecimal {
    match side {
        OrderSide::Buy => price.clone() * quantity.clone(),
        OrderSide::Sell => quantity.clone(),
    }
}

/// 三种订单类型都要过的公共校验：交易对必须启用、数量严格为正、数量精度不超过交易对上限。
/// 顺序刻意如此，停用交易对优先报错，避免对已下架交易对返回数量相关的误导性提示。
/// 精度判定内部返回单位错误，这里统一映射成带允许位数的领域错误便于客户端修正。
fn validate_common(quantity: &BigDecimal, pair: &TradingPairRule) -> Result<(), SpotDomainError> {
    if !pair.enabled {
        return Err(SpotDomainError::TradingPairDisabled);
    }
    if quantity <= &BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositiveQuantity);
    }
    validate_precision(quantity, pair.quantity_precision).map_err(|()| {
        SpotDomainError::QuantityPrecisionExceeded {
            allowed: pair.quantity_precision,
        }
    })
}

/// 校验单个价格严格为正且小数位不超过交易对的价格精度，零和负数一律拒绝。
/// 限价单的委托价、市价单的参考价、止损限价单的触发价与委托价都各调用一次，规则完全一致。
fn validate_price(price: &BigDecimal, precision: u32) -> Result<(), SpotDomainError> {
    if price <= &BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositivePrice);
    }
    validate_precision(price, precision)
        .map_err(|()| SpotDomainError::PricePrecisionExceeded { allowed: precision })
}

/// 校验委托额不低于交易对的最小下单额，用不小于而非大于，因此恰好等于最小额是允许的。
/// 传入的 `actual` 已由调用方按各自口径算好：限价用委托价、市价用参考价、止损限价用委托价。
/// 错误里同时带上实际值与门槛值，方便客户端直接提示还差多少。
fn validate_min_order_value(
    actual: BigDecimal,
    pair: &TradingPairRule,
) -> Result<(), SpotDomainError> {
    if actual < pair.min_order_value {
        Err(SpotDomainError::MinOrderValueNotMet {
            actual,
            minimum: pair.min_order_value.clone(),
        })
    } else {
        Ok(())
    }
}

/// 判定十进制数的有效小数位是否在允许精度内，先做 `normalized` 去掉尾随零。
/// 因此 1.500 在精度为一时也算合规，用户多写的零不会被判超精度。
/// 负标度表示科学计数形式的整数，用 `max(0)` 归零，整数不占小数位。
/// 只回答是否合规，具体报价格还是数量精度错误由调用方决定。
fn validate_precision(amount: &BigDecimal, precision: u32) -> Result<(), ()> {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    if scale.max(0) as u32 <= precision {
        Ok(())
    } else {
        Err(())
    }
}

/// 用白名单穷举订单状态机的全部合法迁移，未列出的组合一律视为非法。
/// Pending 可以直接跳到任意后继状态，涵盖「下单即成交」和「下单即被拒」两类快路径。
/// Open 不能回退到 Pending，PartiallyFilled 允许自迁移以支持同一订单多次部分成交。
/// Cancelled 允许自迁移是为了让重复撤单表现为幂等成功而不是报错。
/// Filled 与 Rejected 完全不出现在左侧，是不可再迁出的绝对终态。
fn can_transition(current: OrderStatus, next: OrderStatus) -> bool {
    use OrderStatus::*;
    matches!(
        (current, next),
        (Pending, Open)
            | (Pending, PartiallyFilled)
            | (Pending, Filled)
            | (Pending, Cancelled)
            | (Pending, Rejected)
            | (Open, PartiallyFilled)
            | (Open, Filled)
            | (Open, Cancelled)
            | (PartiallyFilled, PartiallyFilled)
            | (PartiallyFilled, Filled)
            | (PartiallyFilled, Cancelled)
            | (Cancelled, Cancelled)
    )
}
