//! user bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::architecture::RepositoryLayer;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

/// 邮箱验证码仓储记录：只暴露业务判断需要的字段，隐藏具体 SQL 查询形态。
#[derive(Debug)]
pub(crate) struct EmailVerificationRecord {
    pub(crate) id: u64,
    pub(crate) code_hash: String,
    pub(crate) attempt_count: i32,
    pub(crate) expires_at: DateTime<Utc>,
}

/// 用户登录密码记录：应用层只关心身份、hash 和状态，不暴露 users 表细节。
#[derive(Debug)]
pub(crate) struct UserPasswordRecord {
    pub(crate) id: u64,
    pub(crate) password_hash: String,
    pub(crate) status: String,
}

/// 邀请码持久化记录：应用层只依赖归属、用量和状态判断，不关心 invite_codes 表结构。
#[derive(Debug)]
pub(crate) struct InviteCodeRecord {
    pub(crate) id: u64,
    pub(crate) owner_type: String,
    pub(crate) owner_id: u64,
    pub(crate) usage_limit: Option<i32>,
    pub(crate) used_count: i32,
}

/// 推荐链路记录：用于计算用户邀请下级时的新深度和路径。
#[derive(Debug)]
pub(crate) struct ReferralLinkRecord {
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) depth: i32,
    pub(crate) path: String,
}
