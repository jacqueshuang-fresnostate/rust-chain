//! 生产事件 envelope 校验与业务 dispatch。
//!
//! `user.created` 通过 adapter-neutral port 触发钱包初始化，service 不感知具体数据库或事务。

use super::{EventIdempotency, EventInboxHandler, InboundEventMessage};
use crate::{
    error::{AppError, AppResult},
    modules::events::repository::UserWalletInitializer,
};
use axum::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

/// 生产环境的 inbox 业务处理器，先做 envelope 一致性校验再按事件类型分派。
/// 当前只有用户创建事件会产生真实副作用，其余事件属于兼容性消费：确认收到但不做任何处理，
/// 这样上游可以先行发布事件，消费侧按需逐步补齐实现而不必让消息堆在死信里。
#[derive(Clone, Default)]
pub struct EventInboxProductionHandler {
    /// 钱包初始化端口，为空表示本实例不处理会建账的事件；遇到用户创建事件时会返回配置错误。
    wallet_initializer: Option<Arc<dyn UserWalletInitializer>>,
}

impl std::fmt::Debug for EventInboxProductionHandler {
    /// 手写调试输出，只暴露钱包端口是否已配置这一布尔量。
    /// 之所以不派生实现，是因为端口是 trait 对象无法自动派生调试格式；
    /// 只打布尔量也避免把适配器内部持有的连接信息带进日志。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventInboxProductionHandler")
            .field(
                "wallet_initializer_configured",
                &self.wallet_initializer.is_some(),
            )
            .finish()
    }
}

impl EventInboxProductionHandler {
    /// 注入用户钱包初始化端口；None 仅适用于不产生该副作用的事件或轻量测试。
    /// 构造不执行 I/O；收到 `user.created` 且端口缺失时保持既有明确配置错误并进入重试语义。
    pub fn new(wallet_initializer: Option<Arc<dyn UserWalletInitializer>>) -> Self {
        Self { wallet_initializer }
    }
}

/// 已通过校验的事件分派目标，是消息类型的白名单。
/// 只有列在这里的聚合与事件类型组合才会被接受，未知组合一律判为校验错误，
/// 因此新增事件类型必须同步扩充本枚举、分派逻辑与预期路由键三处。
/// 除用户创建携带用户编号外，其余分支不带数据，因为它们当前不产生副作用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionEventDispatch {
    WalletAccountBalanceChanged,
    WalletLedgerEntryCreated,
    SpotOrderCreated,
    SpotOrderCancelled,
    SpotOrderFilled,
    SpotTradeCreated,
    ConvertOrderConfirmed,
    ConvertOrderCompleted,
    NewCoinPurchaseSubscribed,
    NewCoinPurchasePurchased,
    NewCoinPurchaseReleased,
    StrategyMarketEventGenerated,
    MarketTickerUpdated,
    MarketDepthUpdated,
    MarketKlineUpdated,
    MarketTradeCreated,
    UserCreated(u64),
}

/// 消息载荷中的事件 envelope 结构，与发布侧写入 outbox 的字段一一对应。
/// 五个标量字段都会参与一致性校验，任何一项与消息头或彼此不符都会导致该消息被判为非法。
#[derive(Debug, Deserialize)]
struct EventInboxDomainEnvelope {
    /// 聚合类型，与事件类型共同决定分派目标。
    aggregate_type: String,
    /// 聚合实例标识，用户创建事件中它就是用户编号的字符串形式。
    aggregate_id: String,
    /// 事件类型名。
    event_type: String,
    /// 发布方声明的路由键，会与按分派目标推导出的预期值逐字比对。
    routing_key: String,
    /// 幂等键，须同时与消息头一致、并与三段拼接结果一致（行情类事件除外）。
    idempotency_key: String,
    /// 业务载荷，不得为 JSON null。
    payload: Value,
}

#[async_trait]
impl EventInboxHandler for EventInboxProductionHandler {
    /// 校验生产 envelope 并执行对应 dispatch；普通通知事件为无副作用成功。
    /// `user.created` 委托幂等钱包端口，失败返回消费服务，由其记录 retry/dead-letter；本层不 ACK 消息。
    async fn handle(&self, message: &InboundEventMessage) -> AppResult<()> {
        ProductionEventDispatch::from_inbound(message)?
            .dispatch(self.wallet_initializer.as_deref())
            .await
    }
}

impl ProductionEventDispatch {
    /// 将生产 envelope 限制到钱包余额/账本、现货订单/成交、兑换、新币、策略行情、ticker/depth/kline/trade 与 `user.created` 白名单。
    /// 同时核对外层幂等键、聚合派生幂等键（行情 producer 显式键除外）、各事件的精确 routing key，以及 user payload/aggregate ID 一致性。
    /// 这里只做纯解析；格式、未知事件或任一字段不一致返回 validation error，不执行钱包初始化或确认 RabbitMQ delivery。
    pub fn from_inbound(message: &InboundEventMessage) -> AppResult<Self> {
        let envelope: EventInboxDomainEnvelope = serde_json::from_value(message.payload.clone())
            .map_err(|error| AppError::Validation(format!("invalid event envelope: {error}")))?;
        envelope.dispatch(message)
    }

    /// 返回稳定的可观测 dispatch 分类键，供日志与指标按事件类型聚合。
    /// 该键刻意与 broker 路由键分开：路由键含交易对、用户编号等可变段，基数极高不适合做指标维度，
    /// 这里返回的则是「聚合.事件」形式的固定枚举值，基数与本枚举分支数相同。
    /// 用户创建分支丢弃所携带的用户编号，同样是为了避免把编号带进指标维度。
    /// 相同枚举值始终返回相同静态字符串，不分配内存，也不触发任何业务处理或外部副作用。
    pub fn dispatch_key(&self) -> &'static str {
        match self {
            Self::WalletAccountBalanceChanged => "wallet_account.balance_changed",
            Self::WalletLedgerEntryCreated => "wallet_ledger.entry_created",
            Self::SpotOrderCreated => "spot_order.created",
            Self::SpotOrderCancelled => "spot_order.cancelled",
            Self::SpotOrderFilled => "spot_order.filled",
            Self::SpotTradeCreated => "spot_trade.created",
            Self::ConvertOrderConfirmed => "convert_order.confirmed",
            Self::ConvertOrderCompleted => "convert_order.completed",
            Self::NewCoinPurchaseSubscribed => "new_coin_purchase.subscribed",
            Self::NewCoinPurchasePurchased => "new_coin_purchase.purchased",
            Self::NewCoinPurchaseReleased => "new_coin_purchase.released",
            Self::StrategyMarketEventGenerated => "strategy_market_event.generated",
            Self::MarketTickerUpdated => "market_ticker.ticker_updated",
            Self::MarketDepthUpdated => "market_depth.depth_updated",
            Self::MarketKlineUpdated => "market_kline.kline_updated",
            Self::MarketTradeCreated => "market_trade.trade_created",
            Self::UserCreated(_) => "user.created",
        }
    }

    /// 执行已验证 dispatch；当前仅 `user.created` 产生钱包初始化副作用，其余事件确认兼容消费。
    /// 其余十六类事件统一返回成功且不做任何事，属于「已接收但暂不处理」的兼容消费，
    /// 使上游可以先行发布事件而消费侧按需补齐实现，消息不会因此堆进死信。
    /// 钱包端口自行拥有原子事务并保证重放幂等，因此同一用户事件被重复处理不会产生重复账户。
    /// 端口未注入时返回内部错误而非静默跳过，避免用户建号后钱包缺失却无人察觉；
    /// 该错误会让消息进入正常的重试与死信流程。适配器错误原样上抛，绝不伪造成功。
    async fn dispatch(
        &self,
        wallet_initializer: Option<&dyn UserWalletInitializer>,
    ) -> AppResult<()> {
        match self {
            Self::WalletAccountBalanceChanged
            | Self::WalletLedgerEntryCreated
            | Self::SpotOrderCreated
            | Self::SpotOrderCancelled
            | Self::SpotOrderFilled
            | Self::SpotTradeCreated
            | Self::ConvertOrderConfirmed
            | Self::ConvertOrderCompleted
            | Self::NewCoinPurchaseSubscribed
            | Self::NewCoinPurchasePurchased
            | Self::NewCoinPurchaseReleased
            | Self::StrategyMarketEventGenerated
            | Self::MarketTickerUpdated
            | Self::MarketDepthUpdated
            | Self::MarketKlineUpdated
            | Self::MarketTradeCreated => Ok(()),
            Self::UserCreated(user_id) => {
                let wallet_initializer = wallet_initializer.ok_or_else(|| {
                    AppError::Internal(
                        "mysql pool is not configured for user-created event handling".to_owned(),
                    )
                })?;
                wallet_initializer.initialize_user_wallets(*user_id).await
            }
        }
    }
}

impl EventInboxDomainEnvelope {
    /// 逐层校验 envelope 并解析出分派目标，任一项不符都返回校验错误且不执行任何副作用。
    /// 校验按五步推进：先要求四个标量字段去空白后非空；
    /// 再核对 envelope 内的幂等键与消息头携带的幂等键一致，防止载荷被替换成另一条事件；
    /// 接着除行情类事件外还要求幂等键等于三段拼接结果，杜绝发布方自造不符合规则的键；
    /// 然后要求载荷非 null；最后按聚合与事件类型查白名单得到分派目标。
    /// 用户创建事件额外比对载荷中的用户编号与聚合标识是否相同，兼容数字与字符串两种写法，
    /// 不一致即拒绝，避免按错误的用户建钱包账户。
    /// 收尾再把发布方给出的路由键与按分派目标推导的预期值逐字比对。
    /// 本函数为纯解析与校验，不建账、不确认消息、不写任何存储。
    fn dispatch(&self, message: &InboundEventMessage) -> AppResult<ProductionEventDispatch> {
        if self.aggregate_id.trim().is_empty()
            || self.event_type.trim().is_empty()
            || self.routing_key.trim().is_empty()
            || self.idempotency_key.trim().is_empty()
        {
            return Err(AppError::Validation("invalid event envelope".to_owned()));
        }
        if self.idempotency_key != message.idempotency_key {
            return Err(AppError::Validation(
                "event envelope idempotency key mismatch".to_owned(),
            ));
        }
        if !self.uses_explicit_producer_idempotency()
            && self.idempotency_key
                != EventIdempotency::new(
                    self.aggregate_type.clone(),
                    self.aggregate_id.clone(),
                    self.event_type.clone(),
                )
                .into_key()
        {
            return Err(AppError::Validation(
                "event envelope idempotency key is inconsistent".to_owned(),
            ));
        }
        if self.payload.is_null() {
            return Err(AppError::Validation(
                "event envelope payload is required".to_owned(),
            ));
        }

        let dispatch = self.to_dispatch()?;
        if let ProductionEventDispatch::UserCreated(user_id) = dispatch
            && let Some(payload_user_id) = self.payload.get("user_id")
        {
            let payload_user_id = payload_user_id
                .as_u64()
                .or_else(|| {
                    payload_user_id
                        .as_str()
                        .and_then(|value| value.parse::<u64>().ok())
                })
                .ok_or_else(|| {
                    AppError::Validation(
                        "event envelope payload user_id must be a valid u64".to_owned(),
                    )
                })?;

            if payload_user_id != user_id {
                return Err(AppError::Validation(
                    "event envelope payload user_id mismatch with aggregate id".to_owned(),
                ));
            }
        }
        if self.routing_key != self.expected_routing_key(&dispatch) {
            return Err(AppError::Validation(
                "event envelope routing key mismatch".to_owned(),
            ));
        }
        Ok(dispatch)
    }

    /// 判断该事件是否允许使用生产方自带的幂等键，而不必等于三段拼接结果。
    /// 四类行情事件属于此列：它们的幂等键由上游数据源按自身规则生成，
    /// 强行套用聚合拼接规则会让同一行情的重复推送无法被识别为同一条消息。
    /// 其余事件一律要求幂等键与聚合信息严格对应。
    fn uses_explicit_producer_idempotency(&self) -> bool {
        matches!(
            self.aggregate_type.as_str(),
            "market_ticker" | "market_depth" | "market_kline" | "market_trade"
        )
    }

    /// 按「聚合类型加事件类型」二元组查白名单，映射到具体分派目标。
    /// 白名单之外的组合返回校验错误并带上实际取值，便于排查是上游发了新事件还是拼错了名字；
    /// 该错误不属于报文格式错误，因此消息会走重试与死信流程而不是被直接确认跳过。
    /// 用户创建事件在此把聚合标识严格解析成用户编号，解析失败即判非法。
    fn to_dispatch(&self) -> AppResult<ProductionEventDispatch> {
        match (self.aggregate_type.as_str(), self.event_type.as_str()) {
            ("wallet_account", "balance_changed") => {
                Ok(ProductionEventDispatch::WalletAccountBalanceChanged)
            }
            ("wallet_ledger", "entry_created") => {
                Ok(ProductionEventDispatch::WalletLedgerEntryCreated)
            }
            ("spot_order", "created") => Ok(ProductionEventDispatch::SpotOrderCreated),
            ("spot_order", "cancelled") => Ok(ProductionEventDispatch::SpotOrderCancelled),
            ("spot_order", "filled") => Ok(ProductionEventDispatch::SpotOrderFilled),
            ("spot_trade", "created") => Ok(ProductionEventDispatch::SpotTradeCreated),
            ("convert_order", "confirmed") => Ok(ProductionEventDispatch::ConvertOrderConfirmed),
            ("convert_order", "completed") => Ok(ProductionEventDispatch::ConvertOrderCompleted),
            ("new_coin_purchase", "subscribed") => {
                Ok(ProductionEventDispatch::NewCoinPurchaseSubscribed)
            }
            ("new_coin_purchase", "purchased") => {
                Ok(ProductionEventDispatch::NewCoinPurchasePurchased)
            }
            ("new_coin_purchase", "released") => {
                Ok(ProductionEventDispatch::NewCoinPurchaseReleased)
            }
            ("strategy_market_event", "generated") => {
                Ok(ProductionEventDispatch::StrategyMarketEventGenerated)
            }
            ("market_ticker", "ticker_updated") => Ok(ProductionEventDispatch::MarketTickerUpdated),
            ("market_depth", "depth_updated") => Ok(ProductionEventDispatch::MarketDepthUpdated),
            ("market_kline", "kline_updated") => Ok(ProductionEventDispatch::MarketKlineUpdated),
            ("market_trade", "trade_created") => Ok(ProductionEventDispatch::MarketTradeCreated),
            ("user", "created") => Ok(ProductionEventDispatch::UserCreated(parse_u64_strict(
                "user_id",
                &self.aggregate_id,
            )?)),
            _ => Err(AppError::Validation(format!(
                "unsupported event type {}:{}",
                self.aggregate_type, self.event_type
            ))),
        }
    }

    /// 按分派目标推导该事件应有的路由键，用于与发布方声明的值比对。
    /// 各类事件的拼接规则并不统一：钱包与现货把聚合标识嵌在中间，兑换与新币只用事件类型结尾，
    /// 行情类则把交易对放在中间。这些差异是历史约定，改动任一条都会使存量消息校验失败。
    /// K 线的聚合标识形如「交易对:周期」，此处按首个冒号切分，缺少冒号时周期段留空；
    /// 成交事件优先取载荷中的交易对，缺失时才回落到聚合标识。
    /// 本函数只做字符串拼接，不校验取值合法性。
    fn expected_routing_key(&self, dispatch: &ProductionEventDispatch) -> String {
        match dispatch {
            ProductionEventDispatch::WalletAccountBalanceChanged => {
                format!("wallet.{}.balance_changed", self.aggregate_id)
            }
            ProductionEventDispatch::WalletLedgerEntryCreated => {
                format!("wallet.{}.ledger.entry_created", self.aggregate_id)
            }
            ProductionEventDispatch::SpotOrderCreated => {
                format!("spot.{}.order.created", self.aggregate_id)
            }
            ProductionEventDispatch::SpotOrderCancelled => {
                format!("spot.{}.order.cancelled", self.aggregate_id)
            }
            ProductionEventDispatch::SpotOrderFilled => {
                format!("spot.{}.order.filled", self.aggregate_id)
            }
            ProductionEventDispatch::SpotTradeCreated => {
                format!("spot.{}.trade.created", self.aggregate_id)
            }
            ProductionEventDispatch::ConvertOrderConfirmed => {
                format!("convert.order.{}", self.event_type)
            }
            ProductionEventDispatch::ConvertOrderCompleted => {
                format!("convert.order.{}", self.event_type)
            }
            ProductionEventDispatch::NewCoinPurchaseSubscribed => {
                format!("new_coin.purchase.{}", self.event_type)
            }
            ProductionEventDispatch::NewCoinPurchasePurchased => {
                format!("new_coin.purchase.{}", self.event_type)
            }
            ProductionEventDispatch::NewCoinPurchaseReleased => {
                format!("new_coin.purchase.{}", self.event_type)
            }
            ProductionEventDispatch::StrategyMarketEventGenerated => {
                format!("strategy.market.{}", self.aggregate_id)
            }
            ProductionEventDispatch::MarketTickerUpdated => {
                format!("market.{}.ticker", self.aggregate_id)
            }
            ProductionEventDispatch::MarketDepthUpdated => {
                format!("market.{}.depth", self.aggregate_id)
            }
            ProductionEventDispatch::MarketKlineUpdated => {
                let (symbol, interval) = self
                    .aggregate_id
                    .split_once(':')
                    .unwrap_or((&self.aggregate_id, ""));
                format!("market.{symbol}.kline.{interval}")
            }
            ProductionEventDispatch::MarketTradeCreated => {
                let symbol = self
                    .payload
                    .get("symbol")
                    .and_then(Value::as_str)
                    .unwrap_or(&self.aggregate_id);
                format!("market.{symbol}.trade")
            }
            ProductionEventDispatch::UserCreated(user_id) => {
                format!("user.{user_id}.created")
            }
        }
    }
}

/// 严格把文本解析为 64 位无符号整数，不接受空白、正负号以外的任何宽松写法。
/// 用于从聚合标识还原用户编号，解析结果会直接决定给谁创建钱包账户，因此不容许任何容错解释。
/// 失败时返回带字段名的校验错误，便于定位是哪个字段格式不对。
fn parse_u64_strict(field: &str, value: &str) -> AppResult<u64> {
    value
        .parse::<u64>()
        .map_err(|_| AppError::Validation(format!("event envelope {field} must be a valid u64")))
}
