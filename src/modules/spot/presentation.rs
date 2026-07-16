//! spot bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 现货接口的 JSON 结构集中放在这里，避免路由层继续承载业务数据形状。

use crate::modules::spot::{OrderSide, OrderStatus, OrderType, SpotOrder, SpotTrade};
use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSpotOrderRequest {
    pub(crate) pair_id: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: OrderType,
    pub(crate) price: Option<BigDecimal>,
    pub(crate) trigger_price: Option<BigDecimal>,
    pub(crate) quantity: BigDecimal,
    pub(crate) reference_price: Option<BigDecimal>,
    pub(crate) idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SpotOrdersQuery {
    pub(crate) pair_id: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CancelAllSpotOrdersQuery {
    pub(crate) pair_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FillSpotOrdersRequest {
    pub(crate) buy_order_id: String,
    pub(crate) sell_order_id: String,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
}

impl FillSpotOrdersRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminCancelSpotOrderRequest {
    pub(crate) reason: Option<String>,
}

impl AdminCancelSpotOrderRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct SpotTradesQuery {
    pub(crate) pair_id: String,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminSpotOrdersQuery {
    pub(crate) pair_id: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminSpotTradesQuery {
    pub(crate) pair_id: Option<String>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotOrderResponse {
    pub(crate) id: String,
    pub(crate) user_id: String,
    pub(crate) user_email: Option<String>,
    pub(crate) pair_id: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: OrderType,
    pub(crate) price: Option<BigDecimal>,
    pub(crate) trigger_price: Option<BigDecimal>,
    pub(crate) quantity: BigDecimal,
    pub(crate) filled_quantity: BigDecimal,
    pub(crate) average_price: Option<BigDecimal>,
    pub(crate) status: OrderStatus,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotOrdersResponse {
    pub(crate) orders: Vec<SpotOrderResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotCancelResponse {
    pub(crate) order: SpotOrderResponse,
    pub(crate) cancelled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotCancelAllResponse {
    pub(crate) orders: Vec<SpotOrderResponse>,
    pub(crate) failures: Vec<SpotBatchActionFailure>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotBatchActionFailure {
    pub(crate) id: String,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotFillResponse {
    pub(crate) buy_order: SpotOrderResponse,
    pub(crate) sell_order: SpotOrderResponse,
    pub(crate) trade: SpotTradeResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotTradeResponse {
    pub(crate) id: String,
    pub(crate) pair_id: String,
    pub(crate) buy_order_id: String,
    pub(crate) sell_order_id: String,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) fee: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpotTradesResponse {
    pub(crate) trades: Vec<SpotTradeResponse>,
}

impl From<SpotOrder> for SpotOrderResponse {
    fn from(order: SpotOrder) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            user_email: None,
            pair_id: order.pair_id,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            trigger_price: order.trigger_price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            average_price: None,
            status: order.status,
            created_at: None,
        }
    }
}

impl From<SpotTrade> for SpotTradeResponse {
    fn from(trade: SpotTrade) -> Self {
        Self {
            id: trade.id,
            pair_id: trade.pair_id,
            buy_order_id: trade.buy_order_id,
            sell_order_id: trade.sell_order_id,
            price: trade.price,
            quantity: trade.quantity,
            fee: trade.fee,
            created_at: trade.created_at,
        }
    }
}

impl From<SpotOrderResponse> for SpotOrder {
    fn from(order: SpotOrderResponse) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            pair_id: order.pair_id,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            trigger_price: order.trigger_price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: order.status,
        }
    }
}
