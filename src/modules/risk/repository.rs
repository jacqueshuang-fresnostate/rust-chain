//! risk bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。

use crate::architecture::RepositoryLayer;
use serde_json::Value;

/// 风控事件写入契约；当前只有用户发起的资金操作会落库，`actor_type` 固定为 `user`。
#[derive(Debug)]
pub struct RiskEventWrite {
    pub user_id: u64,
    pub event_type: String,
    pub risk_level: &'static str,
    pub decision: &'static str,
    pub reason: String,
    pub payload: Value,
}

impl RepositoryLayer for RiskEventWrite {}
