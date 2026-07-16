//! admin bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::{architecture::RepositoryLayer, infra::email::VerificationCodeTemplate};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone)]
pub(crate) struct AgentCommissionPayoutTarget {
    // 佣金结算只需要知道入账用户和资产，避免应用层依赖 agents/convert_orders 表结构。
    pub(crate) agent_user_id: u64,
    pub(crate) asset_id: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct RiskRuleWrite {
    // 风控规则写入契约只暴露业务字段，隐藏 risk_rules 表的具体 INSERT 细节。
    pub(crate) rule_type: String,
    pub(crate) target_type: String,
    pub(crate) target_id: Option<String>,
    pub(crate) config_json: Value,
    pub(crate) enabled: bool,
    pub(crate) created_by: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminAgentWrite {
    // 代理主表写入只暴露领域字段，避免应用层依赖 agents 表默认状态和 INSERT 细节。
    pub(crate) user_id: u64,
    pub(crate) parent_agent_id: Option<u64>,
    pub(crate) root_agent_id: Option<u64>,
    pub(crate) agent_code: String,
    pub(crate) level: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminAgentAdminUserWrite {
    // 代理后台账号与代理主表同事务创建，应用层只关心账号归属和密码散列。
    pub(crate) agent_id: u64,
    pub(crate) username: String,
    pub(crate) password_hash: String,
}

#[derive(Debug, Clone)]
pub(crate) struct UserAgentReferralWrite {
    // 用户归属代理写入契约封装邀请树的直接上级、root agent 和 path 字段。
    pub(crate) user_id: u64,
    pub(crate) agent_id: u64,
    pub(crate) path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminSmtpConfigRecord {
    // SMTP 配置记录保留密文字段给应用层做“空值保留/替换”判断，响应和审计只展示 mask。
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) security: String,
    pub(crate) username_ciphertext: Option<String>,
    pub(crate) password_ciphertext: Option<String>,
    pub(crate) username_mask: Option<String>,
    pub(crate) from_email: String,
    pub(crate) from_name: Option<String>,
    pub(crate) verification_code_template_html: Option<String>,
    pub(crate) verification_code_templates: Vec<VerificationCodeTemplate>,
    pub(crate) enabled: bool,
    pub(crate) priority: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminSmtpConfigWrite {
    // 应用层完成校验和密钥处理后，基础设施只负责写入规范化 SMTP 字段。
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) security: String,
    pub(crate) username_ciphertext: Option<String>,
    pub(crate) password_ciphertext: Option<String>,
    pub(crate) username_mask: Option<String>,
    pub(crate) from_email: String,
    pub(crate) from_name: Option<String>,
    pub(crate) verification_code_template_html: Option<String>,
    pub(crate) verification_code_templates: Vec<VerificationCodeTemplate>,
    pub(crate) enabled: bool,
    pub(crate) priority: u32,
    pub(crate) updated_by: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminSmtpDeliverySettingsRecord {
    // 发信策略需要和轮询游标一起锁定，避免并发发送时重复选择同一配置。
    pub(crate) strategy: String,
    pub(crate) round_robin_cursor: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminMarketFeedConfigRecord {
    // 行情订阅配置记录保留版本字段，应用层用它决定 reload 和审计前后状态。
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) symbols: Vec<String>,
    pub(crate) intervals: Vec<String>,
    pub(crate) providers: Vec<String>,
    pub(crate) enabled: bool,
    pub(crate) version: u64,
    pub(crate) applied_version: Option<u64>,
    pub(crate) last_reload_status: Option<String>,
    pub(crate) last_reload_error: Option<String>,
    pub(crate) last_reloaded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminMarketFeedConfigWrite {
    // 配置写入只包含已校验和规范化的订阅字段，version 由应用层在锁行后推进。
    pub(crate) symbols: Vec<String>,
    pub(crate) intervals: Vec<String>,
    pub(crate) providers: Vec<String>,
    pub(crate) enabled: bool,
    pub(crate) version: u64,
    pub(crate) updated_by: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminMarketSourceCredentialRecord {
    // 凭据记录保留密文字段给应用层做“空值保留/替换”，响应和审计只展示 mask。
    pub(crate) provider: String,
    pub(crate) auth_type: String,
    pub(crate) api_key_ciphertext: Option<String>,
    pub(crate) api_secret_ciphertext: Option<String>,
    pub(crate) passphrase_ciphertext: Option<String>,
    pub(crate) api_key_mask: Option<String>,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminMarketSourceCredentialWrite {
    // 应用层完成 provider/auth_type 校验和密钥处理后，基础设施只负责持久化。
    pub(crate) provider: String,
    pub(crate) auth_type: String,
    pub(crate) api_key_ciphertext: Option<String>,
    pub(crate) api_secret_ciphertext: Option<String>,
    pub(crate) passphrase_ciphertext: Option<String>,
    pub(crate) api_key_mask: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) updated_by: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UploadObjectOwner {
    // 上传对象归属只区分后台管理员和前台用户，避免落库时把上传入口耦合到 HTTP route。
    Admin(u64),
    User(u64),
}

impl UploadObjectOwner {
    pub(crate) const fn admin_id(self) -> Option<u64> {
        match self {
            Self::Admin(id) => Some(id),
            Self::User(_) => None,
        }
    }

    pub(crate) const fn user_id(self) -> Option<u64> {
        match self {
            Self::Admin(_) => None,
            Self::User(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AdminUploadConfigRecord {
    // 上传配置记录隐藏密文字段，只把“是否已配置”和 mask 暴露给应用/审计层。
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) provider: String,
    pub(crate) endpoint: Option<String>,
    pub(crate) file_field: Option<String>,
    pub(crate) bearer_token_ciphertext: Option<String>,
    pub(crate) bearer_token_mask: Option<String>,
    pub(crate) access_key_ciphertext: Option<String>,
    pub(crate) access_key_mask: Option<String>,
    pub(crate) secret_key_ciphertext: Option<String>,
    pub(crate) bucket: Option<String>,
    pub(crate) region: Option<String>,
    pub(crate) public_base_url: Option<String>,
    pub(crate) local_root: Option<String>,
    pub(crate) key_prefix: Option<String>,
    pub(crate) max_file_size_bytes: u64,
    pub(crate) allowed_mime_types: Vec<String>,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminUploadConfigWrite {
    // 应用层完成业务校验和密钥处理后，基础设施只负责把规范化字段持久化。
    pub(crate) provider: String,
    pub(crate) endpoint: Option<String>,
    pub(crate) file_field: Option<String>,
    pub(crate) bearer_token_ciphertext: Option<String>,
    pub(crate) bearer_token_mask: Option<String>,
    pub(crate) access_key_ciphertext: Option<String>,
    pub(crate) access_key_mask: Option<String>,
    pub(crate) secret_key_ciphertext: Option<String>,
    pub(crate) bucket: Option<String>,
    pub(crate) region: Option<String>,
    pub(crate) public_base_url: Option<String>,
    pub(crate) local_root: Option<String>,
    pub(crate) key_prefix: Option<String>,
    pub(crate) max_file_size_bytes: u64,
    pub(crate) allowed_mime_types: Vec<String>,
    pub(crate) enabled: bool,
    pub(crate) updated_by: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminUploadObjectWrite {
    // 上传成功后必须落库对象记录，用户头像和后台素材都复用同一持久化契约。
    pub(crate) provider: String,
    pub(crate) object_key: String,
    pub(crate) public_url: String,
    pub(crate) share_url: Option<String>,
    pub(crate) delete_url: Option<String>,
    pub(crate) mime_type: String,
    pub(crate) size_bytes: u64,
    pub(crate) original_filename: String,
    pub(crate) owner: UploadObjectOwner,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminNewCoinLockPositionWrite {
    // 后台新币派发可能直接入账，也可能生成锁仓计划；应用层只传递锁仓业务字段。
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) unlock_type: String,
    pub(crate) unlock_at: DateTime<Utc>,
    pub(crate) amount: BigDecimal,
    pub(crate) merge_key: String,
    pub(crate) source_time: DateTime<Utc>,
    pub(crate) source_type: String,
    pub(crate) source_id: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AdminNewCoinLedgerWrite<'a> {
    // 钱包流水元数据由应用层决定，基础设施只负责同事务写余额和流水。
    pub(crate) change_type: &'a str,
    pub(crate) ref_type: &'a str,
    pub(crate) ref_id: &'a str,
}
