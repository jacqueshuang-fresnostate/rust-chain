//! spot bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::{
    error::AppResult,
    modules::spot::{OrderSide, OrderStatus, OrderType, SpotOrder},
};
use axum::async_trait;
use bigdecimal::BigDecimal;

pub trait SpotRepository {
    /// 定义交易对规则的领域持久化端口，使上层不依赖具体数据库、缓存或网络客户端。
    /// 实现方必须保持现货既有事务与幂等语义，失败应返回错误而非伪造成功结果。
    fn load_pair_rule(
        &mut self,
        pair_id: &str,
    ) -> Result<crate::modules::spot::TradingPairRule, crate::modules::spot::SpotServiceError>;

    /// 定义现货订单写入的领域持久化端口，使上层不依赖具体数据库、缓存或网络客户端。
    /// 实现方必须保持现货既有事务与幂等语义，失败应返回错误而非伪造成功结果。
    fn insert_order(
        &mut self,
        new_order: crate::modules::spot::NewOrder,
        idempotency_key: Option<&str>,
    ) -> Result<crate::modules::spot::SpotOrder, crate::modules::spot::SpotServiceError>;

    /// 定义现货订单读取的领域持久化端口，使上层不依赖具体数据库、缓存或网络客户端。
    /// 实现方必须保持现货既有事务与幂等语义，失败应返回错误而非伪造成功结果。
    fn load_order(
        &mut self,
        order_id: &str,
    ) -> Result<crate::modules::spot::SpotOrder, crate::modules::spot::SpotServiceError>;

    /// 定义现货订单状态保存的领域持久化端口，使上层不依赖具体数据库、缓存或网络客户端。
    /// 实现方必须保持现货既有事务与幂等语义，失败应返回错误而非伪造成功结果。
    fn save_order(
        &mut self,
        order: crate::modules::spot::SpotOrder,
    ) -> Result<(), crate::modules::spot::SpotServiceError>;
}

#[derive(Debug, Clone)]
pub(crate) struct SpotUserCancelCommand {
    pub(crate) order_id: u64,
    pub(crate) user_id: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct SpotAdminCancelCommand {
    pub(crate) order_id: u64,
    pub(crate) admin_id: u64,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SpotCancelRepositoryResult {
    pub(crate) order: SpotOrder,
    pub(crate) cancelled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SpotIdempotentOrderRecord {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) pair_db_id: u64,
    pub(crate) pair_id: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: OrderType,
    pub(crate) price: Option<BigDecimal>,
    pub(crate) trigger_price: Option<BigDecimal>,
    pub(crate) quantity: BigDecimal,
    pub(crate) filled_quantity: BigDecimal,
    pub(crate) status: OrderStatus,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) reserved_amount: Option<BigDecimal>,
    pub(crate) request_reference_price: Option<BigDecimal>,
    pub(crate) request_price: Option<BigDecimal>,
}

#[async_trait]
pub(crate) trait SpotOrderCancelRepository: Clone + Send + Sync + 'static {
    /// 定义用户现货撤单的领域持久化端口，使上层不依赖具体数据库、缓存或网络客户端。
    /// 实现方必须保持现货既有事务与幂等语义，失败应返回错误而非伪造成功结果。
    async fn cancel_user_order(
        &self,
        command: SpotUserCancelCommand,
    ) -> AppResult<SpotCancelRepositoryResult>;

    /// 定义后台订单详情的领域持久化端口，使上层不依赖具体数据库、缓存或网络客户端。
    /// 实现方必须保持现货既有事务与幂等语义，失败应返回错误而非伪造成功结果。
    async fn cancel_admin_order(
        &self,
        command: SpotAdminCancelCommand,
    ) -> AppResult<SpotCancelRepositoryResult>;
}
