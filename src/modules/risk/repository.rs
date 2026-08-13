//! risk bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 风控当前只有一条写路径，即命中规则后的事件留痕，因此这里仅承载该写入的字段契约。
//! 规则读取一侧直接复用服务层的 `StoredRiskRule`，未在本文件重复建模。

use crate::architecture::RepositoryLayer;
use serde_json::Value;

/// 风控事件写入契约；当前只有用户发起的资金操作会落库，`actor_type` 固定为 `user`。
#[derive(Debug)]
pub struct RiskEventWrite {
    pub user_id: u64,
    /// 事件类型直接取被拦截的业务操作标识，便于按操作维度统计命中分布。
    pub event_type: String,
    /// 风险等级，当前拒绝路径固定为 high，留作后续分级预警扩展。
    pub risk_level: &'static str,
    /// 处置决策，当前只写 reject，因为放行路径不产生事件。
    pub decision: &'static str,
    /// 面向用户的拒绝提示原文，不含具体阈值。
    pub reason: String,
    /// 现场快照，含触发操作、作用域、请求事实与生效阈值，用于事后复盘。
    pub payload: Value,
}

impl RepositoryLayer for RiskEventWrite {}
