//! spot bounded context.
//!
//! 按 DDD 结构划分为：domain、repository、service、application、infrastructure、presentation、routes。

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
pub use service::{
    CancelSpotOrderCommand, CreateSpotOrderCommand, FillSpotOrderCommand, SpotService,
};
