//! agent bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::architecture::RepositoryLayer;
use bigdecimal::BigDecimal;

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentAccessScope {
    pub(crate) agent_id: u64,
    pub(crate) root_agent_id: u64,
    pub(crate) path: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentConvertStatsRecord {
    pub(crate) agent_id: u64,
    pub(crate) total_orders: i64,
    pub(crate) pending_orders: BigDecimal,
    pub(crate) completed_orders: BigDecimal,
    pub(crate) total_from_amount: BigDecimal,
    pub(crate) total_to_amount: BigDecimal,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentInviteCodeWrite {
    pub(crate) agent_id: u64,
    pub(crate) code: String,
    pub(crate) usage_limit: Option<i32>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentCommissionRuleRecord {
    pub(crate) agent_id: u64,
    pub(crate) commission_rate: BigDecimal,
}

#[derive(Debug)]
pub(crate) struct AgentBusinessCommissionWrite<'a> {
    // 统一的返佣写入契约，使各业务不再各自复制归属、规则和幂等 SQL。
    pub(crate) user_id: u64,
    pub(crate) product_type: &'a str,
    pub(crate) source_type: &'a str,
    pub(crate) source_id: &'a str,
    pub(crate) source_amount: &'a BigDecimal,
    pub(crate) payout_asset_id: u64,
}
