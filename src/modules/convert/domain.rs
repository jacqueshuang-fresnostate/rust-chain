//! convert bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//!
//! 闪兑领域只表达「报价在一段有限时间内有效」这一条核心约束：报价由 UUID 唯一标识，
//! 派生出稳定的 Redis 缓存键，并携带一个绝对到期时刻。所有过期判定都以调用方传入的时刻为准，
//! 领域层自身不读时钟、不访问缓存或数据库，也不冻结任何钱包余额。
//! 资金结算、订单唯一性和费率快照落库全部由基础设施层的 MySQL 事务负责。

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 报价的全局唯一标识，取 UUIDv7 以保证按创建时间单调递增。
/// 它同时是 Redis 缓存键后缀、`convert_quotes.quote_id` 列值，以及 `convert_orders` 的幂等唯一键。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteId(pub Uuid);

/// 报价的有效期快照：标识加上一个绝对到期时刻。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteTtl {
    /// 该有效期归属的报价标识，与缓存键和订单幂等键同源。
    pub quote_id: QuoteId,
    /// 绝对到期 UTC 时刻，序列化为 Unix 毫秒；到达该时刻即失效，不做宽限。
    #[serde(with = "crate::time::unix_millis")]
    pub expires_at: DateTime<Utc>,
}

impl QuoteTtl {
    /// 判定给定时刻是否已越过报价到期边界，边界取闭区间：`now == expires_at` 也算过期。
    /// 只做两个 UTC 时刻的比较，不读系统时钟，因此调用方必须自行传入统一时间源。
    /// 判定为过期不会删除 Redis 键、不改订单状态，也不释放任何余额。
    pub(crate) fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }
}

/// 报价值对象，只承载有效期与派生缓存键两项不变量，金额与汇率由上层快照持有。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertQuote {
    /// 由创建时刻加 TTL 秒数算出的到期边界。
    ttl: QuoteTtl,
    /// 由报价 UUID 派生的 `convert:quote:{uuid}` 缓存键，构造后不再变化。
    idempotency_key: String,
}

impl ConvertQuote {
    /// 用报价 UUID 构造 `convert:quote:{uuid}` 缓存键，并从创建时间计算严格为正的有效期。
    /// 到期时刻等于 `created_at + ttl_seconds`，创建时刻由调用方给定，本构造器不读系统时钟。
    /// `ttl_seconds` 必须为正，零或负数返回 `InvalidTtl` 并且不产生任何报价状态。
    /// 该值对象不检查用户、交易对或余额，也不冻结 available，更不会写入 Redis 或数据库。
    pub fn new(
        quote_id: QuoteId,
        created_at: DateTime<Utc>,
        ttl_seconds: i64,
    ) -> Result<Self, ConvertQuoteError> {
        if ttl_seconds <= 0 {
            return Err(ConvertQuoteError::InvalidTtl);
        }

        let idempotency_key = format!("convert:quote:{}", quote_id.0);
        Ok(Self {
            ttl: QuoteTtl {
                quote_id,
                expires_at: created_at + TimeDelta::seconds(ttl_seconds),
            },
            idempotency_key,
        })
    }

    /// 借用报价 UUID，用于回填响应体或作为订单与流水的引用键，不读取缓存或数据库。
    pub fn quote_id(&self) -> &QuoteId {
        &self.ttl.quote_id
    }

    /// 返回报价 UUID 与到期时刻组成的只读有效期快照，供上层写入缓存 TTL 或回传给客户端。
    pub fn ttl(&self) -> &QuoteTtl {
        &self.ttl
    }

    /// 返回由报价 UUID 唯一派生的 Redis 键；它标识报价缓存，不是用户请求幂等键。
    /// 客户端重复提交同一笔兑换仍会生成新报价，真正的资金幂等由订单表唯一约束保证。
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// 按调用方提供的时刻校验报价未过期；等于到期时刻即视为失效。
    /// 校验只读领域快照，不删除 Redis、不更新数据库状态，也不触碰钱包。
    pub fn ensure_not_expired(&self, now: DateTime<Utc>) -> Result<(), ConvertQuoteError> {
        if self.ttl.is_expired(now) {
            Err(ConvertQuoteError::Expired)
        } else {
            Ok(())
        }
    }
}

/// 报价值对象自身可能违反的两条不变量，属于纯领域错误，不含存储或鉴权语义。
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ConvertQuoteError {
    /// 校验时刻已达到或越过到期边界。
    #[error("convert quote is expired")]
    Expired,
    /// 构造报价时给出的 TTL 秒数不是正数。
    #[error("convert quote ttl must be positive")]
    InvalidTtl,
}

/// 通用闪兑服务在报价与确认流程中可能返回的失败分类，供内存适配器和测试断言区分原因。
/// `QuoteNotFound` 表示仓储里查不到该快照，通常是缓存已按 TTL 自然淘汰；
/// `QuoteExpired` 表示快照仍在但确认时刻不早于到期时刻；
/// `DuplicateQuoteConfirmation` 表示唯一键拒绝了第二次写入，本次调用不产生任何资金副作用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertServiceError {
    /// 领域不变量被破坏，例如 TTL 非正或报价已过期。
    Quote(ConvertQuoteError),
    /// 底层仓储读写或序列化失败，不代表报价本身不存在。
    Repository(ConvertRepositoryError),
    QuoteNotFound {
        quote_id: QuoteId,
    },
    QuoteExpired {
        quote_id: QuoteId,
    },
    DuplicateQuoteConfirmation {
        quote_id: QuoteId,
    },
    /// 源资产 available 不足以覆盖请求金额，附带当时的可用与锁定余额便于排查。
    InsufficientAvailableBalance {
        asset_id: String,
        requested: Box<bigdecimal::BigDecimal>,
        available: Box<bigdecimal::BigDecimal>,
        locked: Box<bigdecimal::BigDecimal>,
    },
}

impl From<ConvertQuoteError> for ConvertServiceError {
    /// 保留报价过期或 TTL 非法事实并包装为服务错误，不执行仓储或资金操作。
    fn from(error: ConvertQuoteError) -> Self {
        Self::Quote(error)
    }
}

impl From<ConvertRepositoryError> for ConvertServiceError {
    /// 保留仓储错误类别并包装为服务错误，不把失败转换为报价未命中。
    fn from(error: ConvertRepositoryError) -> Self {
        Self::Repository(error)
    }
}

/// 调用方在报价时刻抓取的源资产余额快照，仅用于提示性校验，不代表资金已被预留。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertBalanceSnapshot {
    /// 可自由动用的余额，报价校验只看这一项。
    pub available: bigdecimal::BigDecimal,
    /// 被其他业务占用的余额，仅在余额不足错误里作为诊断上下文回显。
    pub locked: bigdecimal::BigDecimal,
}

/// 通用闪兑服务创建报价的入参，金额与汇率已由调用方算好，服务层只负责有效期与落库。
#[derive(Debug, Clone)]
pub struct ConvertQuoteCommand {
    /// 调用方预先生成的报价标识，服务层不再重新分配。
    pub quote_id: QuoteId,
    /// 报价归属用户，以字符串形式透传给缓存快照。
    pub user_id: String,
    /// 源资产标识，扣款方向。
    pub from_asset: String,
    /// 目标资产标识，入账方向。
    pub to_asset: String,
    /// 用户提交的源资产数量，需被 available 完全覆盖。
    pub from_amount: bigdecimal::BigDecimal,
    /// 调用方算好的目标资产到账数量，服务层原样写入缓存不再复核。
    pub to_amount: bigdecimal::BigDecimal,
    /// 报价时刻的源资产余额快照。
    pub balance: ConvertBalanceSnapshot,
    /// 报价创建时刻，同时作为过期判定与 TTL 计算的基准。
    pub created_at: DateTime<Utc>,
    /// 有效期秒数，必须为正。
    pub ttl_seconds: i64,
}

/// 写入 Redis 的报价完整快照，确认阶段据此判定归属与有效期，因此字段一经写入即固化。
/// 费率与费用是报价时刻复制的值，后续修改交易对配置不会回溯影响已发出的报价。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvertQuoteCacheEntry {
    /// 报价标识，同时决定 `redis_key` 的取值。
    pub quote_id: QuoteId,
    /// 报价归属用户，确认时与调用者身份逐字比对，不一致按未找到处理。
    pub user_id: String,
    /// 源资产标识。
    pub from_asset: String,
    /// 目标资产标识。
    pub to_asset: String,
    /// 报价锁定的源资产扣减数量。
    pub from_amount: bigdecimal::BigDecimal,
    /// 报价锁定的目标资产到账数量，已按目标资产精度截断。
    pub to_amount: bigdecimal::BigDecimal,
    /// 报价时刻复制的手续费率快照。
    pub fee_rate: bigdecimal::BigDecimal,
    /// 报价时刻算出的手续费金额，已折进 to_amount，不再另行扣钱包。
    pub fee_amount: bigdecimal::BigDecimal,
    /// 绝对到期时刻，序列化为 Unix 毫秒，与 Redis 键 TTL 表达同一边界。
    #[serde(with = "crate::time::unix_millis")]
    pub expires_at: DateTime<Utc>,
    /// 实际使用的 Redis 键名，形如 `convert:quote:{uuid}`。
    pub redis_key: String,
    /// 写入 Redis 时设置的过期秒数。
    pub ttl_seconds: i64,
}

/// 报价创建成功后回传给调用方的最小结果集，不含金额，金额以缓存快照为准。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteCreated {
    /// 新建报价的标识。
    pub quote_id: QuoteId,
    /// 绝对到期时刻。
    pub expires_at: DateTime<Utc>,
    /// 缓存该报价所用的 Redis 键。
    pub redis_key: String,
    /// 缓存 TTL 秒数。
    pub ttl_seconds: i64,
}

/// 确认报价的入参：由谁、在哪个时刻兑现哪一笔报价。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmConvertQuoteCommand {
    /// 待确认的报价标识。
    pub quote_id: QuoteId,
    /// 发起确认的用户。
    pub user_id: String,
    /// 确认时刻，与报价 `expires_at` 比较判定是否过期。
    pub confirmed_at: DateTime<Utc>,
}

/// 落入订单仓储的确认记录，字段全部来自报价缓存快照而非重新计算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteConfirmationRecord {
    /// 报价标识，同时充当订单侧的幂等唯一键。
    pub quote_id: QuoteId,
    /// 确认人。
    pub user_id: String,
    /// 源资产标识。
    pub from_asset: String,
    /// 目标资产标识。
    pub to_asset: String,
    /// 报价固化的源资产扣减数量。
    pub from_amount: bigdecimal::BigDecimal,
    /// 报价固化的目标资产到账数量。
    pub to_amount: bigdecimal::BigDecimal,
    /// 确认发生的时刻。
    pub confirmed_at: DateTime<Utc>,
}

/// 订单仓储写入确认记录后的判定结果，用于把重放与首次确认区分开。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertConfirmationInsert {
    /// 首次写入成功，可继续后续结算。
    Inserted,
    /// 唯一键命中已有记录，本次调用不得再产生资金副作用。
    Duplicate,
}

/// 确认流程对外的成功结果，`confirmed` 为真表示本次调用完成了首次确认。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertConfirmationResult {
    /// 被确认的报价标识。
    pub quote_id: QuoteId,
    /// 是否由本次调用完成确认。
    pub confirmed: bool,
}

/// 闪兑仓储层错误，刻意把存储故障与数据格式故障分开，避免把缓存不可用误判为报价不存在。
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConvertRepositoryError {
    /// MySQL 或 Redis 的连接、命令层面故障，原始文本保留在负载中。
    #[error("convert repository storage error: {0}")]
    Storage(String),
    /// 报价快照的 JSON 序列化或反序列化失败，通常意味着缓存内容与当前结构不兼容。
    #[error("convert repository serialization error: {0}")]
    Serialization(String),
}
