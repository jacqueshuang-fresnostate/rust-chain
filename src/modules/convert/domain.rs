//! convert bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::architecture::DomainLayer;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug)]
pub struct DomainLayerMarker;

impl DomainLayer for DomainLayerMarker {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteTtl {
    pub quote_id: QuoteId,
    #[serde(with = "crate::time::unix_millis")]
    pub expires_at: DateTime<Utc>,
}

impl QuoteTtl {
    pub(crate) fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertQuote {
    ttl: QuoteTtl,
    idempotency_key: String,
}

impl ConvertQuote {
    /// 从报价信息构建幂等 key 和过期时间，避免重复提交重复落库。
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

    pub fn quote_id(&self) -> &QuoteId {
        &self.ttl.quote_id
    }

    pub fn ttl(&self) -> &QuoteTtl {
        &self.ttl
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// 任何时间点都只允许在未过期前继续确认报价。
    pub fn ensure_not_expired(&self, now: DateTime<Utc>) -> Result<(), ConvertQuoteError> {
        if self.ttl.is_expired(now) {
            Err(ConvertQuoteError::Expired)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ConvertQuoteError {
    #[error("convert quote is expired")]
    Expired,
    #[error("convert quote ttl must be positive")]
    InvalidTtl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertServiceError {
    Quote(ConvertQuoteError),
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
    InsufficientAvailableBalance {
        asset_id: String,
        requested: Box<bigdecimal::BigDecimal>,
        available: Box<bigdecimal::BigDecimal>,
        locked: Box<bigdecimal::BigDecimal>,
    },
}

impl From<ConvertQuoteError> for ConvertServiceError {
    fn from(error: ConvertQuoteError) -> Self {
        Self::Quote(error)
    }
}

impl From<ConvertRepositoryError> for ConvertServiceError {
    fn from(error: ConvertRepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertBalanceSnapshot {
    pub available: bigdecimal::BigDecimal,
    pub locked: bigdecimal::BigDecimal,
}

#[derive(Debug, Clone)]
pub struct ConvertQuoteCommand {
    pub quote_id: QuoteId,
    pub user_id: String,
    pub from_asset: String,
    pub to_asset: String,
    pub from_amount: bigdecimal::BigDecimal,
    pub to_amount: bigdecimal::BigDecimal,
    pub balance: ConvertBalanceSnapshot,
    pub created_at: DateTime<Utc>,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvertQuoteCacheEntry {
    pub quote_id: QuoteId,
    pub user_id: String,
    pub from_asset: String,
    pub to_asset: String,
    pub from_amount: bigdecimal::BigDecimal,
    pub to_amount: bigdecimal::BigDecimal,
    pub fee_rate: bigdecimal::BigDecimal,
    pub fee_amount: bigdecimal::BigDecimal,
    #[serde(with = "crate::time::unix_millis")]
    pub expires_at: DateTime<Utc>,
    pub redis_key: String,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteCreated {
    pub quote_id: QuoteId,
    pub expires_at: DateTime<Utc>,
    pub redis_key: String,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmConvertQuoteCommand {
    pub quote_id: QuoteId,
    pub user_id: String,
    pub confirmed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteConfirmationRecord {
    pub quote_id: QuoteId,
    pub user_id: String,
    pub from_asset: String,
    pub to_asset: String,
    pub from_amount: bigdecimal::BigDecimal,
    pub to_amount: bigdecimal::BigDecimal,
    pub confirmed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertConfirmationInsert {
    Inserted,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertConfirmationResult {
    pub quote_id: QuoteId,
    pub confirmed: bool,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConvertRepositoryError {
    #[error("convert repository storage error: {0}")]
    Storage(String),
    #[error("convert repository serialization error: {0}")]
    Serialization(String),
}

impl From<sqlx::Error> for ConvertRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<redis::RedisError> for ConvertRepositoryError {
    fn from(error: redis::RedisError) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<serde_json::Error> for ConvertRepositoryError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error.to_string())
    }
}
