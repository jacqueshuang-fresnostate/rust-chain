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

/// 同步风格的现货订单持久化端口，让领域与用例代码不直接依赖 SQLx 或任何具体存储。
/// 四个方法都取 `&mut self`，实现方可以据此持有一个进行中的事务或连接。
pub trait SpotRepository {
    /// 按交易对标识加载下单校验所需的精度、最小下单额和启停状态。
    /// 交易对不存在或不可用时应返回错误而不是给出一份放行一切的默认规则。
    fn load_pair_rule(
        &mut self,
        pair_id: &str,
    ) -> Result<crate::modules::spot::TradingPairRule, crate::modules::spot::SpotServiceError>;

    /// 落库一个已通过领域校验的新订单并回填主键，返回已持久化的订单实体。
    /// `idempotency_key` 非空时实现方须保证同键重复插入不会产生第二笔订单，
    /// 冲突时应回读既有订单或返回错误，绝不能静默新建导致重复冻结资金。
    fn insert_order(
        &mut self,
        new_order: crate::modules::spot::NewOrder,
        idempotency_key: Option<&str>,
    ) -> Result<crate::modules::spot::SpotOrder, crate::modules::spot::SpotServiceError>;

    /// 按订单主键读回完整订单实体，供撤单和成交推进前重新确认当前状态与已成交量。
    /// 该端口不带用户维度约束，越权检查由调用方在拿到实体后自行比对归属。
    /// 订单不存在时返回错误，不得用零值实体代替，否则会让上层误判为可操作订单。
    fn load_order(
        &mut self,
        order_id: &str,
    ) -> Result<crate::modules::spot::SpotOrder, crate::modules::spot::SpotServiceError>;

    /// 把领域层就地修改过的订单状态与已成交量写回存储，用于撤单和成交推进后的持久化。
    /// 实现方应带状态条件更新以抵御并发覆盖，发现受影响行数异常时返回错误而非静默成功。
    /// 该写入必须与对应的钱包资金变动处在同一事务内，否则会出现状态改了但钱没动的裂口。
    fn save_order(
        &mut self,
        order: crate::modules::spot::SpotOrder,
    ) -> Result<(), crate::modules::spot::SpotServiceError>;
}

/// 用户自助撤单指令，用户标识来自 JWT 并参与 SQL 条件，用于阻止越权撤销他人订单。
#[derive(Debug, Clone)]
pub(crate) struct SpotUserCancelCommand {
    pub(crate) order_id: u64,
    pub(crate) user_id: u64,
}

/// 后台强制撤单指令，不带用户约束但要求管理员标识和必填原因，以便写入审计。
#[derive(Debug, Clone)]
pub(crate) struct SpotAdminCancelCommand {
    pub(crate) order_id: u64,
    pub(crate) admin_id: u64,
    /// 强制撤单原因，已由用例层裁剪并校验非空，直接落入审计记录。
    pub(crate) reason: String,
}

/// 撤单仓储的返回结果，用布尔值区分本次是否真的发生了状态迁移。
#[derive(Debug, Clone)]
pub(crate) struct SpotCancelRepositoryResult {
    /// 撤单后的订单快照，无论本次是否发生迁移都返回当前最新状态。
    pub(crate) order: SpotOrder,
    /// 是否由本次调用完成撤销；对已撤销订单重放时为假，调用方据此决定不重复退款和发事件。
    pub(crate) cancelled: bool,
}

/// 幂等键命中时回读的完整下单快照，用于核对重放请求是否与首次完全一致。
#[derive(Debug, Clone)]
pub(crate) struct SpotIdempotentOrderRecord {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    /// 交易对的数据库主键，与对外的业务标识分开保存。
    pub(crate) pair_db_id: u64,
    /// 交易对业务标识，用于与本次请求的 `pair_id` 比对。
    pub(crate) pair_id: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: OrderType,
    pub(crate) price: Option<BigDecimal>,
    pub(crate) trigger_price: Option<BigDecimal>,
    pub(crate) quantity: BigDecimal,
    pub(crate) filled_quantity: BigDecimal,
    pub(crate) status: OrderStatus,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    /// 首次下单实际冻结的金额，重放时原样回显而不重新计算。
    pub(crate) reserved_amount: Option<BigDecimal>,
    /// 首次请求携带的市价参考价，单独留存以便重放核对，它不等于订单的 `price`。
    pub(crate) request_reference_price: Option<BigDecimal>,
    /// 首次请求携带的委托价原值，与订单落库价格分开保存以覆盖服务端做过归一化的情形。
    pub(crate) request_price: Option<BigDecimal>,
}

/// 异步撤单持久化端口，要求可克隆且跨线程安全，便于在 axum 处理器之间共享同一实现。
/// 两个方法都由实现方自己 owning 事务，因此调用方拿到结果时资金与状态已经落定。
#[async_trait]
pub(crate) trait SpotOrderCancelRepository: Clone + Send + Sync + 'static {
    /// 撤销用户自己的订单，实现方须在同一事务内完成加锁、状态迁移和未成交冻结额退回。
    /// 订单不属于该用户时应返回未找到而不是撤销成功，避免越权操作。
    /// 已撤销订单按幂等重放处理：返回当前快照且 `cancelled` 为假，不重复退款。
    async fn cancel_user_order(
        &self,
        command: SpotUserCancelCommand,
    ) -> AppResult<SpotCancelRepositoryResult>;

    /// 后台强制撤销任意用户的订单，资金处理与用户撤单一致，但额外要求写入带原因的审计记录。
    /// 不带用户维度约束，因此实现方必须自行保证只有已鉴权的管理员路径能调用到它。
    /// 同样遵循幂等语义，对已撤销订单重放时不重复退款也不重复记审计。
    async fn cancel_admin_order(
        &self,
        command: SpotAdminCancelCommand,
    ) -> AppResult<SpotCancelRepositoryResult>;
}
