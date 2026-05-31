use crate::time::unix_millis;
use chrono::{DateTime, TimeDelta, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool};
use thiserror::Error;
use uuid::Uuid;

pub mod routes;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteTtl {
    pub quote_id: QuoteId,
    #[serde(with = "unix_millis")]
    pub expires_at: DateTime<Utc>,
}

impl QuoteTtl {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertQuote {
    ttl: QuoteTtl,
    idempotency_key: String,
}

impl ConvertQuote {
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

    pub fn ensure_not_expired(&self, now: DateTime<Utc>) -> Result<(), ConvertQuoteError> {
        if self.ttl.is_expired(now) {
            Err(ConvertQuoteError::Expired)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ConvertQuoteError {
    #[error("convert quote is expired")]
    Expired,
    #[error("convert quote ttl must be positive")]
    InvalidTtl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertRepositoryError {
    Storage(String),
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
    #[serde(with = "unix_millis")]
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

pub trait ConvertQuoteRepository {
    fn save_quote_ttl(
        &mut self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError>;

    fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError>;
}

pub trait ConvertOrderRepository {
    fn insert_quote_confirmation(
        &mut self,
        record: ConvertQuoteConfirmationRecord,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError>;
}

#[derive(Clone)]
pub struct RedisConvertQuoteCache {
    manager: ConnectionManager,
}

impl RedisConvertQuoteCache {
    pub fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    pub fn manager(&self) -> &ConnectionManager {
        &self.manager
    }

    pub async fn save_quote_ttl(
        &self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError> {
        let payload = serde_json::to_string(&entry)?;
        let mut connection = self.manager.clone();
        let _: () = connection
            .set_ex(&entry.redis_key, payload, entry.ttl_seconds as u64)
            .await?;
        Ok(())
    }

    pub async fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError> {
        let key = quote_redis_key(quote_id);
        let mut connection = self.manager.clone();
        let payload: Option<String> = connection.get(key).await?;
        payload
            .map(|value| serde_json::from_str::<ConvertQuoteCacheEntry>(&value))
            .transpose()
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct MySqlConvertRepository {
    pool: Pool<MySql>,
}

impl MySqlConvertRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn insert_quote(
        &self,
        quote: ConvertQuoteInsert,
    ) -> Result<ConvertQuoteInsertResult, ConvertRepositoryError> {
        let insert_result = sqlx::query(
            r#"INSERT INTO convert_quotes
               (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
                to_amount, rate, spread_rate, expires_at, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'quoted')
               ON DUPLICATE KEY UPDATE quote_id = quote_id"#,
        )
        .bind(quote.quote_id.0.to_string())
        .bind(quote.convert_pair_id)
        .bind(quote.user_id)
        .bind(quote.from_asset_id)
        .bind(quote.to_asset_id)
        .bind(quote.from_amount)
        .bind(quote.to_amount)
        .bind(quote.rate)
        .bind(quote.spread_rate)
        .bind(quote.expires_at.naive_utc())
        .execute(&self.pool)
        .await?;

        let quote_id = insert_result.last_insert_id();
        Ok(ConvertQuoteInsertResult {
            quote_row_id: if quote_id == 0 {
                self.quote_row_id(&quote.quote_id).await?
            } else {
                quote_id
            },
            inserted: insert_result.rows_affected() == 1,
        })
    }

    pub async fn insert_order_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError> {
        let result = sqlx::query(
            r#"INSERT INTO convert_orders
               (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount,
                to_amount, rate, status)
               SELECT quotes.quote_id, quotes.convert_pair_id, quotes.user_id, quotes.from_asset,
                      quotes.to_asset, quotes.from_amount, quotes.to_amount, quotes.rate, 'pending'
               FROM convert_quotes quotes
               WHERE quotes.quote_id = ?
               ON DUPLICATE KEY UPDATE quote_id = convert_orders.quote_id"#,
        )
        .bind(quote_id.0.to_string())
        .execute(&self.pool)
        .await?;

        if result.last_insert_id() == 0 {
            Ok(ConvertConfirmationInsert::Duplicate)
        } else {
            Ok(ConvertConfirmationInsert::Inserted)
        }
    }

    async fn quote_row_id(&self, quote_id: &QuoteId) -> Result<u64, ConvertRepositoryError> {
        let row =
            sqlx::query_as::<_, (u64,)>("SELECT id FROM convert_quotes WHERE quote_id = ? LIMIT 1")
                .bind(quote_id.0.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}

#[derive(Debug, Clone)]
pub struct ConvertQuoteInsert {
    pub quote_id: QuoteId,
    pub convert_pair_id: u64,
    pub user_id: u64,
    pub from_asset_id: u64,
    pub to_asset_id: u64,
    pub from_amount: bigdecimal::BigDecimal,
    pub to_amount: bigdecimal::BigDecimal,
    pub rate: bigdecimal::BigDecimal,
    pub spread_rate: bigdecimal::BigDecimal,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteInsertResult {
    pub quote_row_id: u64,
    pub inserted: bool,
}

fn quote_redis_key(quote_id: &QuoteId) -> String {
    format!("convert:quote:{}", quote_id.0)
}

#[derive(Debug, Clone)]
pub struct ConvertService<R> {
    repository: R,
}

impl<R> ConvertService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }
}

impl<R> ConvertService<R>
where
    R: ConvertQuoteRepository,
{
    pub fn create_quote(
        &mut self,
        command: ConvertQuoteCommand,
    ) -> Result<ConvertQuoteCreated, ConvertServiceError> {
        if command.balance.available < command.from_amount {
            return Err(ConvertServiceError::InsufficientAvailableBalance {
                asset_id: command.from_asset,
                requested: Box::new(command.from_amount),
                available: Box::new(command.balance.available),
                locked: Box::new(command.balance.locked),
            });
        }

        let quote = ConvertQuote::new(
            command.quote_id.clone(),
            command.created_at,
            command.ttl_seconds,
        )?;
        let entry = ConvertQuoteCacheEntry {
            quote_id: command.quote_id.clone(),
            user_id: command.user_id,
            from_asset: command.from_asset,
            to_asset: command.to_asset,
            from_amount: command.from_amount,
            to_amount: command.to_amount,
            expires_at: quote.ttl().expires_at,
            redis_key: quote.idempotency_key().to_owned(),
            ttl_seconds: command.ttl_seconds,
        };

        self.repository.save_quote_ttl(entry)?;

        Ok(ConvertQuoteCreated {
            quote_id: command.quote_id,
            expires_at: quote.ttl().expires_at,
            redis_key: quote.idempotency_key().to_owned(),
            ttl_seconds: command.ttl_seconds,
        })
    }
}

impl<R> ConvertService<R>
where
    R: ConvertQuoteRepository + ConvertOrderRepository,
{
    pub fn confirm_quote(
        &mut self,
        command: ConfirmConvertQuoteCommand,
    ) -> Result<ConvertConfirmationResult, ConvertServiceError> {
        let entry = self
            .repository
            .get_quote_ttl(&command.quote_id)?
            .ok_or_else(|| ConvertServiceError::QuoteNotFound {
                quote_id: command.quote_id.clone(),
            })?;

        if command.confirmed_at >= entry.expires_at {
            return Err(ConvertServiceError::QuoteExpired {
                quote_id: command.quote_id,
            });
        }

        let record = ConvertQuoteConfirmationRecord {
            quote_id: command.quote_id.clone(),
            user_id: command.user_id,
            from_asset: entry.from_asset,
            to_asset: entry.to_asset,
            from_amount: entry.from_amount,
            to_amount: entry.to_amount,
            confirmed_at: command.confirmed_at,
        };

        match self.repository.insert_quote_confirmation(record)? {
            ConvertConfirmationInsert::Inserted => Ok(ConvertConfirmationResult {
                quote_id: command.quote_id,
                confirmed: true,
            }),
            ConvertConfirmationInsert::Duplicate => {
                Err(ConvertServiceError::DuplicateQuoteConfirmation {
                    quote_id: command.quote_id,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn quote_ttl_accepts_before_expiry_and_rejects_at_expiry() {
        let quote_id = QuoteId(Uuid::nil());
        let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();
        let quote = ConvertQuote::new(quote_id.clone(), now, 10).unwrap();

        assert_eq!(quote.quote_id(), &quote_id);
        assert_eq!(
            quote.idempotency_key(),
            "convert:quote:00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(quote.ttl().expires_at, now + TimeDelta::seconds(10));
        assert_eq!(
            quote.ensure_not_expired(now + TimeDelta::seconds(9)),
            Ok(())
        );
        assert_eq!(
            quote.ensure_not_expired(now + TimeDelta::seconds(10)),
            Err(ConvertQuoteError::Expired)
        );
    }

    #[test]
    fn quote_ttl_requires_positive_ttl() {
        let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();

        assert_eq!(
            ConvertQuote::new(QuoteId(Uuid::nil()), now, 0).unwrap_err(),
            ConvertQuoteError::InvalidTtl
        );
    }
}
