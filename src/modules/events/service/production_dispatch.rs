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

#[derive(Clone, Default)]
pub struct EventInboxProductionHandler {
    wallet_initializer: Option<Arc<dyn UserWalletInitializer>>,
}

impl std::fmt::Debug for EventInboxProductionHandler {
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

#[derive(Debug, Deserialize)]
struct EventInboxDomainEnvelope {
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    routing_key: String,
    idempotency_key: String,
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

    /// 返回稳定的可观测 dispatch 分类键；不等同 RabbitMQ routing key，且不触发业务处理。
    /// 相同枚举值始终返回相同静态字符串，无事务或外部副作用。
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
    /// 钱包端口自行拥有原子事务和重放幂等；缺少端口或适配失败原样返回，不伪造成功。
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

    fn uses_explicit_producer_idempotency(&self) -> bool {
        matches!(
            self.aggregate_type.as_str(),
            "market_ticker" | "market_depth" | "market_kline" | "market_trade"
        )
    }

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

fn parse_u64_strict(field: &str, value: &str) -> AppResult<u64> {
    value
        .parse::<u64>()
        .map_err(|_| AppError::Validation(format!("event envelope {field} must be a valid u64")))
}
