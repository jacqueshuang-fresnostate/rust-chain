//! events bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

/// 出站消息状态。
///
/// 需要被仓储查询或状态机分流时引用，集中在领域层可减少基础设施层的散落常量。
pub const OUTBOX_PENDING: &str = "pending";
pub const OUTBOX_PUBLISHED: &str = "published";
pub const OUTBOX_RETRY: &str = "retry";
pub const OUTBOX_DEAD_LETTER: &str = "dead_letter";

/// 入站消息状态。
///
/// 入站与消费重试流程使用同一组状态语义，集中定义有利于一致性。
pub const INBOX_PROCESSING: &str = "processing";
pub const INBOX_CONSUMED: &str = "consumed";
pub const INBOX_RETRY: &str = "retry";
pub const INBOX_DEAD_LETTER: &str = "dead_letter";

/// 入站消息处理时长（秒）。
///
/// 与仓储 SQL 的过期判断相关，使用领域常量可以避免 SQL 构建层重复硬编码。
pub const INBOX_PROCESSING_LEASE_SECONDS: i64 = 300;

/// 入站消费进度时间戳格式。
pub const INBOX_PROCESSING_TOKEN_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.6f";
