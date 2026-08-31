//! spot bounded context.
//!
//! 按 DDD 结构划分为：domain、repository、service、application、infrastructure、presentation、routes。
//! 该上下文覆盖限价单、市价单与止损限价单的创建与余额冻结、撮合成交的双边结算与佣金、
//! 用户撤单与后台强制撤单的解冻、限价单按真实行情触发成交，以及订单与成交的用户端和后台读模型。
//! 资金语义统一为 available 与 frozen 双桶：下单把预留额从 available 转入 frozen，
//! 成交只从 frozen 扣减并把对手资产计入 available，撤单把未成交部分的 frozen 退回 available。
//! 买单预留报价资产、卖单预留基础资产；金额一律按十八位小数处理，与资金列精度一致。
//! 所有 WebSocket 私有事件都只在对应资金事务提交成功之后才发布，失败或幂等重放路径不广播。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub mod routes;

pub use domain::{
    NewOrder, NewSpotTrade, OrderSide, OrderStatus, OrderType, SpotDomainError, SpotOrder,
    SpotServiceError, SpotTrade, TradingPairRule, apply_fill, cancel_order, create_limit_order,
    create_market_order, create_stop_limit_order, spot_remaining_reserved_amount,
    spot_reservation_amount, spot_reserve_asset_id, transition_status, validate_order_request,
};
pub use infrastructure::MySqlSpotRepository;
pub use repository::SpotRepository;
pub use service::{CancelSpotOrderCommand, FillSpotOrderCommand, SpotService};
