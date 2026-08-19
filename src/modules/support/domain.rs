//! support 限界上下文的纯领域规则。
//!
//! 这里集中会话状态、发送者身份、分页边界、消息文本与客户端幂等键校验，
//! 不依赖 Axum、SQLx 或任何传输 DTO。字符上限按 Unicode scalar value（Rust `char`）计数，
//! 不按 UTF-8 字节数计算，保证中文和 emoji 不会被错误放大或截断。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

pub(crate) const SUPPORT_MESSAGE_MAX_SCALARS: usize = 2_000;
pub(crate) const SUPPORT_MESSAGE_PREVIEW_SCALARS: usize = 200;
pub(crate) const SUPPORT_CLIENT_MESSAGE_ID_MIN_LEN: usize = 8;
pub(crate) const SUPPORT_CLIENT_MESSAGE_ID_MAX_LEN: usize = 64;
pub(crate) const SUPPORT_PAGE_DEFAULT_LIMIT: u32 = 50;
pub(crate) const SUPPORT_PAGE_MAX_LIMIT: u32 = 100;
pub(crate) const SUPPORT_PAGE_MAX_OFFSET: u32 = 100_000;

/// 可持久化的会话状态，只允许开放与关闭两种稳定值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportConversationStatus {
    Open,
    Closed,
}

impl DomainLayer for SupportConversationStatus {}

impl SupportConversationStatus {
    /// 把 HTTP 输入的状态严格解析为领域值；不做大小写或空白容错。
    /// 这保证数据库 CHECK、队列筛选和状态迁移使用完全相同的字面量，
    /// 未知值在任何查询或写入发生前返回可修正的参数错误。
    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            _ => Err(AppError::Validation(
                "status must be open or closed".to_owned(),
            )),
        }
    }

    /// 返回该状态唯一的持久化文本，不产生 I/O 或状态迁移。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

/// 消息发送者身份；其数字 ID 必须在对应身份命名空间中解释。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportActor {
    User(u64),
    Agent(u64),
    Admin(u64),
}

impl DomainLayer for SupportActor {}

impl SupportActor {
    /// 返回写入消息表的发送者类型，与身份 ID 共同构成幂等命名空间。
    pub(crate) const fn sender_type(self) -> &'static str {
        match self {
            Self::User(_) => "user",
            Self::Agent(_) => "agent",
            Self::Admin(_) => "admin",
        }
    }

    /// 返回已由服务端令牌解析的主体 ID，不接受客户端传入的替代值。
    pub(crate) const fn sender_id(self) -> u64 {
        match self {
            Self::User(id) | Self::Agent(id) | Self::Admin(id) => id,
        }
    }
}

/// 锁定会话时的可见范围；代理范围始终是精确 ID，不含子树。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportConversationAccess {
    User(u64),
    Agent(u64),
    Admin,
}

impl DomainLayer for SupportConversationAccess {}

/// 已通过完整校验的发送输入，可直接进入幂等查询与事务写入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedSupportMessage {
    pub(crate) body: String,
    pub(crate) client_message_id: String,
    pub(crate) preview: String,
}

impl DomainLayer for ValidatedSupportMessage {}

/// 校验并归一化一次客服文本发送：正文去除首尾空白后必须非空，
/// 最多两千个 Unicode scalar value；幂等键必须是 8～64 位 ASCII 字母、数字、`_` 或 `-`。
/// 成功时同时生成最多两百个 scalar value 的会话预览，截断永远落在字符边界；
/// 失败时不访问数据库，也不会消费幂等键。
pub(crate) fn validate_support_message(
    body: String,
    client_message_id: String,
) -> AppResult<ValidatedSupportMessage> {
    let body = body.trim().to_owned();
    let scalar_count = body.chars().count();
    if scalar_count == 0 {
        return Err(AppError::Validation(
            "message body must not be empty".to_owned(),
        ));
    }
    if scalar_count > SUPPORT_MESSAGE_MAX_SCALARS {
        return Err(AppError::Validation(format!(
            "message body must contain at most {SUPPORT_MESSAGE_MAX_SCALARS} Unicode scalar values"
        )));
    }

    let client_id_len = client_message_id.len();
    if !(SUPPORT_CLIENT_MESSAGE_ID_MIN_LEN..=SUPPORT_CLIENT_MESSAGE_ID_MAX_LEN)
        .contains(&client_id_len)
        || !client_message_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AppError::Validation(
            "client_message_id must be an 8-64 character safe token".to_owned(),
        ));
    }

    let preview = body.chars().take(SUPPORT_MESSAGE_PREVIEW_SCALARS).collect();
    Ok(ValidatedSupportMessage {
        body,
        client_message_id,
        preview,
    })
}

/// 客服队列的 offset 分页，limit 始终在 1～100，offset 不超过十万。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SupportOffsetPage {
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

impl DomainLayer for SupportOffsetPage {}

/// 将队列分页参数收敛到服务端硬边界，缺省每页五十条，深分页最多十万。
/// 超出范围时夹取而不拒绝，保持与项目现有列表端点一致，同时避免一次请求扫描无界数据。
pub(crate) fn support_offset_page(limit: Option<u32>, offset: Option<u32>) -> SupportOffsetPage {
    SupportOffsetPage {
        limit: limit
            .unwrap_or(SUPPORT_PAGE_DEFAULT_LIMIT)
            .clamp(1, SUPPORT_PAGE_MAX_LIMIT),
        offset: offset.unwrap_or(0).min(SUPPORT_PAGE_MAX_OFFSET),
    }
}

/// 消息历史的 ID 游标分页，`before_id` 为空表示从最新一页开始。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SupportMessagePage {
    pub(crate) limit: u32,
    pub(crate) before_id: Option<u64>,
}

impl DomainLayer for SupportMessagePage {}

/// 收敛消息游标分页：页大小最多一百，显式的 `before_id` 必须大于零。
/// 游标严格表示“只返回 ID 小于它的消息”，因此连续请求不会重复页边界；
/// 零值无法表达有效自增消息位置，会在查库前返回校验错误。
pub(crate) fn support_message_page(
    limit: Option<u32>,
    before_id: Option<u64>,
) -> AppResult<SupportMessagePage> {
    if before_id == Some(0) {
        return Err(AppError::Validation(
            "before_id must be greater than zero".to_owned(),
        ));
    }
    Ok(SupportMessagePage {
        limit: limit
            .unwrap_or(SUPPORT_PAGE_DEFAULT_LIMIT)
            .clamp(1, SUPPORT_PAGE_MAX_LIMIT),
        before_id,
    })
}

/// 解析可选会话状态筛选；缺省表示 open 与 closed 都可见。
/// 显式空串不会被当作“不筛选”，而是返回校验错误，避免前端筛选状态与查询结果脱节。
pub(crate) fn optional_support_status(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| {
            SupportConversationStatus::parse(&value).map(|status| status.as_str().to_owned())
        })
        .transpose()
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_support_domain_tests.rs"]
mod tests;
