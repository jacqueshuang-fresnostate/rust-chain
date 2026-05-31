use crate::modules::wallet::{
    LockPosition, LockPositionSource, LockSchedule, WalletDomainError, create_lock_positions,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool};

pub mod routes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStatus {
    Preheat,
    Subscription,
    Distribution,
    Listed,
}

impl LifecycleStatus {
    pub fn transition_to(self, to: LifecycleStatus) -> Result<LifecycleStatus, NewCoinDomainError> {
        match (self, to) {
            (LifecycleStatus::Preheat, LifecycleStatus::Subscription)
            | (LifecycleStatus::Subscription, LifecycleStatus::Distribution)
            | (LifecycleStatus::Distribution, LifecycleStatus::Listed) => Ok(to),
            (from, to) => Err(NewCoinDomainError::InvalidLifecycleTransition { from, to }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockType {
    ImmediateOnListing,
    FixedTime,
    RelativePeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockFeeBasis {
    MarketValue,
    Profit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinDomainError {
    InvalidLifecycleTransition {
        from: LifecycleStatus,
        to: LifecycleStatus,
    },
    SubscriptionNotOpen {
        status: LifecycleStatus,
    },
    PostListingPurchaseNotOpen {
        status: LifecycleStatus,
    },
    PostListingPurchaseDisabled,
    NonPositiveUnlockAmount,
    NonPositiveRelativePeriod,
    NegativeUnlockFeeRate,
    NegativeUnlockPrice,
    NegativePurchaseCost,
    MissingUnlockFeePaymentAsset,
    WalletLock(WalletDomainError),
    UnlockFeePaymentRequired {
        payment_asset: String,
        amount: BigDecimal,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCoinOrderKind {
    Subscription,
    Purchase,
}

impl NewCoinOrderKind {
    pub fn chinese_name(self) -> &'static str {
        match self {
            NewCoinOrderKind::Subscription => "申购",
            NewCoinOrderKind::Purchase => "认购",
        }
    }

    pub fn api_action(self) -> &'static str {
        match self {
            NewCoinOrderKind::Subscription => "subscription",
            NewCoinOrderKind::Purchase => "purchase",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockRule {
    ImmediateOnListing { listed_at: DateTime<Utc> },
    FixedTime { unlock_at: DateTime<Utc> },
    RelativePeriod { seconds_after_source: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockSource {
    pub user_id: String,
    pub asset_id: String,
    pub source_id: String,
    pub amount: BigDecimal,
    pub source_time: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct PendingLockSource {
    user_id: String,
    asset_id: String,
    wallet_source: LockPositionSource,
}

#[derive(Debug, Clone)]
pub struct UnlockApplication {
    pub available_amount: BigDecimal,
    pub locked_amount: BigDecimal,
    pub lock_positions: Vec<LockPosition>,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchasePlan {
    pub order_kind: NewCoinOrderKind,
    pub unlock: UnlockApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeRule {
    pub enabled: bool,
    pub rate: BigDecimal,
    pub basis: UnlockFeeBasis,
    pub payment_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeInput {
    pub unlock_quantity: BigDecimal,
    pub unlock_price: BigDecimal,
    pub purchase_cost: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeQuote {
    pub required: bool,
    pub amount: BigDecimal,
    pub basis: UnlockFeeBasis,
    pub payment_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinRepositoryError {
    Storage(String),
    InvalidStatus(String),
}

impl From<sqlx::Error> for NewCoinRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewCoinServiceError {
    Domain(NewCoinDomainError),
    Repository(NewCoinRepositoryError),
}

impl From<NewCoinDomainError> for NewCoinServiceError {
    fn from(error: NewCoinDomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<NewCoinRepositoryError> for NewCoinServiceError {
    fn from(error: NewCoinRepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[derive(Debug, Clone)]
pub struct WalletLockCommandOutput {
    pub user_id: String,
    pub asset_id: String,
    pub available_delta: BigDecimal,
    pub locked_delta: BigDecimal,
    pub lock_positions: Vec<LockPosition>,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseCommand {
    pub project_id: String,
    pub order_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quantity: BigDecimal,
    pub purchased_at: DateTime<Utc>,
    pub lifecycle_status: LifecycleStatus,
    pub post_listing_purchase_enabled: bool,
    pub unlock_rule: UnlockRule,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseRecord {
    pub project_id: String,
    pub order_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quantity: BigDecimal,
    pub order_kind: NewCoinOrderKind,
    pub purchased_at: DateTime<Utc>,
    pub wallet_lock: WalletLockCommandOutput,
}

#[derive(Debug, Clone)]
pub struct PostListingPurchaseResult {
    pub order_kind: NewCoinOrderKind,
    pub wallet_lock: WalletLockCommandOutput,
}

#[derive(Debug, Clone)]
pub struct UnlockFeeQuoteCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub unlock_quantity: BigDecimal,
    pub unlock_price: BigDecimal,
    pub purchase_cost: BigDecimal,
    pub fee_rule: UnlockFeeRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeeQuoteResult {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub quote: UnlockFeeQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayUnlockFeeCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub payment_asset: String,
    pub amount: BigDecimal,
    pub paid_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockFeePaymentRecord {
    pub unlock_id: String,
    pub user_id: String,
    pub payment_asset: String,
    pub amount: BigDecimal,
    pub paid_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseUnlockCommand {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub fee_quote: UnlockFeeQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseRecord {
    pub unlock_id: String,
    pub user_id: String,
    pub asset_id: String,
    pub released_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReleaseResult {
    pub unlock_id: String,
    pub released: bool,
}

pub trait NewCoinPurchaseRepository {
    fn save_post_listing_purchase(
        &mut self,
        record: PostListingPurchaseRecord,
    ) -> Result<(), NewCoinRepositoryError>;
}

pub trait UnlockFeeRepository {
    fn save_unlock_fee_payment(
        &mut self,
        record: UnlockFeePaymentRecord,
    ) -> Result<(), NewCoinRepositoryError>;

    fn unlock_fee_paid(
        &self,
        unlock_id: &str,
        user_id: &str,
    ) -> Result<bool, NewCoinRepositoryError>;

    fn mark_unlock_released(
        &mut self,
        record: UnlockReleaseRecord,
    ) -> Result<(), NewCoinRepositoryError>;
}

#[derive(Debug, Clone)]
pub struct MySqlNewCoinRepository {
    pool: Pool<MySql>,
}

impl MySqlNewCoinRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn insert_purchase_order(
        &self,
        order: NewCoinPurchaseOrderInsert,
    ) -> Result<NewCoinPurchaseOrderInsertResult, NewCoinRepositoryError> {
        let insert_result = sqlx::query(
            r#"INSERT INTO new_coin_purchase_orders
               (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                quote_amount, lock_position_id, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key"#,
        )
        .bind(order.project_id)
        .bind(order.user_id)
        .bind(order.pair_id)
        .bind(order.base_asset_id)
        .bind(order.quote_asset_id)
        .bind(order.price)
        .bind(order.quantity)
        .bind(order.quote_amount)
        .bind(order.lock_position_id)
        .bind(order.status)
        .bind(&order.idempotency_key)
        .execute(&self.pool)
        .await?;

        let order_id = insert_result.last_insert_id();
        Ok(NewCoinPurchaseOrderInsertResult {
            order_id: if order_id == 0 {
                self.purchase_order_id(&order.idempotency_key).await?
            } else {
                order_id
            },
            inserted: order_id != 0,
        })
    }

    pub async fn unlock_fee_paid_status(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> Result<Option<UnlockFeePaidStatus>, NewCoinRepositoryError> {
        let row = sqlx::query_as::<_, (String,)>(
            r#"SELECT fee_paid_status
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ?
               LIMIT 1"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|(status,)| UnlockFeePaidStatus::from_storage(&status))
            .transpose()
    }

    pub async fn mark_unlock_fee_paid(
        &self,
        payment: UnlockFeePaymentUpdate,
    ) -> Result<bool, NewCoinRepositoryError> {
        let result = sqlx::query(
            r#"UPDATE asset_unlock_records
               SET fee_paid_status = 'paid',
                   unlock_fee_asset = ?,
                   unlock_fee_amount = ?
               WHERE idempotency_key = ?
                 AND user_id = ?
                 AND fee_paid_status <> 'paid'"#,
        )
        .bind(payment.payment_asset_id)
        .bind(payment.amount)
        .bind(payment.unlock_idempotency_key)
        .bind(payment.user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    async fn purchase_order_id(
        &self,
        idempotency_key: &str,
    ) -> Result<u64, NewCoinRepositoryError> {
        let row = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM new_coin_purchase_orders WHERE idempotency_key = ? LIMIT 1",
        )
        .bind(idempotency_key)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}

#[derive(Debug, Clone)]
pub struct NewCoinPurchaseOrderInsert {
    pub project_id: u64,
    pub user_id: u64,
    pub pair_id: u64,
    pub base_asset_id: u64,
    pub quote_asset_id: u64,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub quote_amount: BigDecimal,
    pub lock_position_id: Option<u64>,
    pub status: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCoinPurchaseOrderInsertResult {
    pub order_id: u64,
    pub inserted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockFeePaidStatus {
    NotRequired,
    Pending,
    Paid,
}

impl UnlockFeePaidStatus {
    fn from_storage(value: &str) -> Result<Self, NewCoinRepositoryError> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "pending" => Ok(Self::Pending),
            "paid" => Ok(Self::Paid),
            _ => Err(NewCoinRepositoryError::InvalidStatus(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnlockFeePaymentUpdate {
    pub unlock_idempotency_key: String,
    pub user_id: u64,
    pub payment_asset_id: u64,
    pub amount: BigDecimal,
}

#[derive(Debug, Clone)]
pub struct NewCoinService<R> {
    repository: R,
}

impl<R> NewCoinService<R> {
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

impl<R> NewCoinService<R>
where
    R: NewCoinPurchaseRepository,
{
    pub fn create_post_listing_purchase(
        &mut self,
        command: PostListingPurchaseCommand,
    ) -> Result<PostListingPurchaseResult, NewCoinServiceError> {
        let unlock_source = UnlockSource {
            user_id: command.user_id.clone(),
            asset_id: command.asset_id.clone(),
            source_id: command.order_id.clone(),
            amount: command.quantity.clone(),
            source_time: command.purchased_at,
        };
        let plan = plan_post_listing_purchase(
            command.lifecycle_status,
            command.post_listing_purchase_enabled,
            &command.unlock_rule,
            unlock_source,
        )?;
        let wallet_lock = WalletLockCommandOutput {
            user_id: command.user_id.clone(),
            asset_id: command.asset_id.clone(),
            available_delta: plan.unlock.available_amount,
            locked_delta: plan.unlock.locked_amount,
            lock_positions: plan.unlock.lock_positions,
        };
        let record = PostListingPurchaseRecord {
            project_id: command.project_id,
            order_id: command.order_id,
            user_id: command.user_id,
            asset_id: command.asset_id,
            quantity: command.quantity,
            order_kind: plan.order_kind,
            purchased_at: command.purchased_at,
            wallet_lock: wallet_lock.clone(),
        };

        self.repository.save_post_listing_purchase(record)?;

        Ok(PostListingPurchaseResult {
            order_kind: plan.order_kind,
            wallet_lock,
        })
    }
}

impl<R> NewCoinService<R>
where
    R: UnlockFeeRepository,
{
    pub fn quote_unlock_fee(
        &self,
        command: UnlockFeeQuoteCommand,
    ) -> Result<UnlockFeeQuoteResult, NewCoinServiceError> {
        let quote = calculate_unlock_fee(
            &command.fee_rule,
            UnlockFeeInput {
                unlock_quantity: command.unlock_quantity,
                unlock_price: command.unlock_price,
                purchase_cost: command.purchase_cost,
            },
        )?;

        Ok(UnlockFeeQuoteResult {
            unlock_id: command.unlock_id,
            user_id: command.user_id,
            asset_id: command.asset_id,
            quote,
        })
    }

    pub fn pay_unlock_fee(
        &mut self,
        command: PayUnlockFeeCommand,
    ) -> Result<UnlockFeePaymentRecord, NewCoinServiceError> {
        let record = UnlockFeePaymentRecord {
            unlock_id: command.unlock_id,
            user_id: command.user_id,
            payment_asset: command.payment_asset,
            amount: command.amount,
            paid_at: command.paid_at,
        };

        self.repository.save_unlock_fee_payment(record.clone())?;
        Ok(record)
    }

    pub fn release_unlock(
        &mut self,
        command: ReleaseUnlockCommand,
    ) -> Result<UnlockReleaseResult, NewCoinServiceError> {
        let fee_paid = if command.fee_quote.required {
            self.repository
                .unlock_fee_paid(&command.unlock_id, &command.user_id)?
        } else {
            false
        };
        ensure_unlock_release_allowed(&command.fee_quote, fee_paid)?;

        let record = UnlockReleaseRecord {
            unlock_id: command.unlock_id.clone(),
            user_id: command.user_id,
            asset_id: command.asset_id,
            released_at: Utc::now(),
        };
        self.repository.mark_unlock_released(record)?;

        Ok(UnlockReleaseResult {
            unlock_id: command.unlock_id,
            released: true,
        })
    }
}

pub fn ensure_subscription_allowed(status: LifecycleStatus) -> Result<(), NewCoinDomainError> {
    if status == LifecycleStatus::Subscription {
        Ok(())
    } else {
        Err(NewCoinDomainError::SubscriptionNotOpen { status })
    }
}

pub fn plan_post_listing_purchase(
    status: LifecycleStatus,
    enabled: bool,
    unlock_rule: &UnlockRule,
    source: UnlockSource,
) -> Result<PostListingPurchasePlan, NewCoinDomainError> {
    if status != LifecycleStatus::Listed {
        return Err(NewCoinDomainError::PostListingPurchaseNotOpen { status });
    }

    if !enabled {
        return Err(NewCoinDomainError::PostListingPurchaseDisabled);
    }

    Ok(PostListingPurchasePlan {
        order_kind: NewCoinOrderKind::Purchase,
        unlock: apply_unlock_rule(unlock_rule, vec![source])?,
    })
}

pub fn apply_unlock_rule(
    unlock_rule: &UnlockRule,
    sources: Vec<UnlockSource>,
) -> Result<UnlockApplication, NewCoinDomainError> {
    ensure_positive_sources(&sources)?;

    match unlock_rule {
        UnlockRule::ImmediateOnListing { listed_at } => {
            let mut available_amount = BigDecimal::from(0);
            let mut locked_sources = Vec::new();

            for source in sources {
                if source.source_time >= *listed_at {
                    available_amount += source.amount;
                } else {
                    locked_sources.push(to_lock_source(source, *listed_at));
                }
            }

            let lock_positions = if locked_sources.is_empty() {
                Vec::new()
            } else {
                let user_id = locked_sources[0].user_id.clone();
                let asset_id = locked_sources[0].asset_id.clone();
                let wallet_sources = to_wallet_sources(locked_sources);
                create_lock_positions(
                    &user_id,
                    &asset_id,
                    LockSchedule::ImmediateOnListing {
                        listed_at: *listed_at,
                    },
                    wallet_sources,
                )
                .map_err(NewCoinDomainError::WalletLock)?
            };

            let locked_amount = sum_remaining(&lock_positions);
            Ok(UnlockApplication {
                available_amount,
                locked_amount,
                lock_positions,
            })
        }
        UnlockRule::FixedTime { unlock_at } => {
            let lock_sources = sources
                .into_iter()
                .map(|source| to_lock_source(source, *unlock_at))
                .collect::<Vec<_>>();
            let user_id = lock_sources[0].user_id.clone();
            let asset_id = lock_sources[0].asset_id.clone();
            let wallet_sources = to_wallet_sources(lock_sources);
            let lock_positions = create_lock_positions(
                &user_id,
                &asset_id,
                LockSchedule::FixedTime {
                    unlock_at: *unlock_at,
                },
                wallet_sources,
            )
            .map_err(NewCoinDomainError::WalletLock)?;
            let locked_amount = sum_remaining(&lock_positions);

            Ok(UnlockApplication {
                available_amount: BigDecimal::from(0),
                locked_amount,
                lock_positions,
            })
        }
        UnlockRule::RelativePeriod {
            seconds_after_source,
        } => {
            if *seconds_after_source <= 0 {
                return Err(NewCoinDomainError::NonPositiveRelativePeriod);
            }

            let lock_sources = sources
                .into_iter()
                .map(|source| {
                    let unlock_at = source.source_time + Duration::seconds(*seconds_after_source);
                    to_lock_source(source, unlock_at)
                })
                .collect::<Vec<_>>();
            let user_id = lock_sources[0].user_id.clone();
            let asset_id = lock_sources[0].asset_id.clone();
            let wallet_sources = to_wallet_sources(lock_sources);
            let lock_positions = create_lock_positions(
                &user_id,
                &asset_id,
                LockSchedule::RelativePeriod,
                wallet_sources,
            )
            .map_err(NewCoinDomainError::WalletLock)?;
            let locked_amount = sum_remaining(&lock_positions);

            Ok(UnlockApplication {
                available_amount: BigDecimal::from(0),
                locked_amount,
                lock_positions,
            })
        }
    }
}

pub fn calculate_unlock_fee(
    rule: &UnlockFeeRule,
    input: UnlockFeeInput,
) -> Result<UnlockFeeQuote, NewCoinDomainError> {
    let zero = BigDecimal::from(0);

    if input.unlock_quantity <= zero {
        return Err(NewCoinDomainError::NonPositiveUnlockAmount);
    }
    if input.unlock_price < zero {
        return Err(NewCoinDomainError::NegativeUnlockPrice);
    }
    if input.purchase_cost < zero {
        return Err(NewCoinDomainError::NegativePurchaseCost);
    }
    if rule.rate < zero {
        return Err(NewCoinDomainError::NegativeUnlockFeeRate);
    }

    if !rule.enabled {
        return Ok(UnlockFeeQuote {
            required: false,
            amount: BigDecimal::from(0),
            basis: rule.basis,
            payment_asset: rule.payment_asset.clone(),
        });
    }

    let payment_asset = rule
        .payment_asset
        .as_ref()
        .filter(|asset| !asset.trim().is_empty())
        .cloned()
        .ok_or(NewCoinDomainError::MissingUnlockFeePaymentAsset)?;
    let market_value = input.unlock_quantity * input.unlock_price;
    let basis_amount = match rule.basis {
        UnlockFeeBasis::MarketValue => market_value,
        UnlockFeeBasis::Profit => {
            max_decimal(market_value - input.purchase_cost, BigDecimal::from(0))
        }
    };
    let amount = basis_amount * rule.rate.clone();
    let required = amount > 0;

    Ok(UnlockFeeQuote {
        required,
        amount,
        basis: rule.basis,
        payment_asset: Some(payment_asset),
    })
}

pub fn ensure_unlock_release_allowed(
    fee: &UnlockFeeQuote,
    fee_paid: bool,
) -> Result<(), NewCoinDomainError> {
    if fee.required && !fee_paid {
        Err(NewCoinDomainError::UnlockFeePaymentRequired {
            payment_asset: fee.payment_asset.clone().unwrap_or_default(),
            amount: fee.amount.clone(),
        })
    } else {
        Ok(())
    }
}

fn ensure_positive_sources(sources: &[UnlockSource]) -> Result<(), NewCoinDomainError> {
    if sources.is_empty() || sources.iter().any(|source| source.amount <= 0) {
        Err(NewCoinDomainError::NonPositiveUnlockAmount)
    } else {
        Ok(())
    }
}

fn to_lock_source(source: UnlockSource, unlock_at: DateTime<Utc>) -> PendingLockSource {
    PendingLockSource {
        user_id: source.user_id,
        asset_id: source.asset_id,
        wallet_source: LockPositionSource {
            source_id: source.source_id,
            amount: source.amount,
            unlock_at,
        },
    }
}

fn to_wallet_sources(sources: Vec<PendingLockSource>) -> Vec<LockPositionSource> {
    sources
        .into_iter()
        .map(|source| source.wallet_source)
        .collect()
}

fn sum_remaining(lock_positions: &[LockPosition]) -> BigDecimal {
    lock_positions
        .iter()
        .fold(BigDecimal::from(0), |sum, position| {
            sum + position.remaining_amount.clone()
        })
}

fn max_decimal(left: BigDecimal, right: BigDecimal) -> BigDecimal {
    if left >= right { left } else { right }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(seconds, 0).unwrap()
    }

    fn amount(value: i64) -> BigDecimal {
        BigDecimal::from(value)
    }

    fn decimal(value: &str) -> BigDecimal {
        BigDecimal::from_str(value).unwrap()
    }

    fn unlock_source(
        source_id: &str,
        quantity: i64,
        source_time: chrono::DateTime<chrono::Utc>,
    ) -> UnlockSource {
        UnlockSource {
            user_id: "user-1".to_owned(),
            asset_id: "NEW".to_owned(),
            source_id: source_id.to_owned(),
            amount: amount(quantity),
            source_time,
        }
    }

    #[test]
    fn lifecycle_transitions_are_strictly_forward() {
        assert_eq!(
            LifecycleStatus::Preheat.transition_to(LifecycleStatus::Subscription),
            Ok(LifecycleStatus::Subscription)
        );
        assert_eq!(
            LifecycleStatus::Subscription.transition_to(LifecycleStatus::Distribution),
            Ok(LifecycleStatus::Distribution)
        );
        assert_eq!(
            LifecycleStatus::Distribution.transition_to(LifecycleStatus::Listed),
            Ok(LifecycleStatus::Listed)
        );

        assert_eq!(
            LifecycleStatus::Preheat.transition_to(LifecycleStatus::Listed),
            Err(NewCoinDomainError::InvalidLifecycleTransition {
                from: LifecycleStatus::Preheat,
                to: LifecycleStatus::Listed,
            })
        );
        assert_eq!(
            LifecycleStatus::Listed.transition_to(LifecycleStatus::Distribution),
            Err(NewCoinDomainError::InvalidLifecycleTransition {
                from: LifecycleStatus::Listed,
                to: LifecycleStatus::Distribution,
            })
        );
    }

    #[test]
    fn only_subscription_status_accepts_primary_subscription() {
        assert_eq!(
            ensure_subscription_allowed(LifecycleStatus::Preheat),
            Err(NewCoinDomainError::SubscriptionNotOpen {
                status: LifecycleStatus::Preheat,
            })
        );
        assert_eq!(
            ensure_subscription_allowed(LifecycleStatus::Subscription),
            Ok(())
        );
        assert_eq!(
            ensure_subscription_allowed(LifecycleStatus::Listed),
            Err(NewCoinDomainError::SubscriptionNotOpen {
                status: LifecycleStatus::Listed,
            })
        );
    }

    #[test]
    fn listed_post_listing_purchase_is_named_purchase_and_immediate_unlock_is_available() {
        let listed_at = at(1_700_000_000);
        let source = unlock_source("purchase-1", 50, listed_at + chrono::Duration::seconds(60));

        let plan = plan_post_listing_purchase(
            LifecycleStatus::Listed,
            true,
            &UnlockRule::ImmediateOnListing { listed_at },
            source,
        )
        .unwrap();

        assert_eq!(plan.order_kind, NewCoinOrderKind::Purchase);
        assert_eq!(plan.order_kind.chinese_name(), "认购");
        assert_eq!(plan.order_kind.api_action(), "purchase");
        assert_eq!(plan.unlock.available_amount, amount(50));
        assert_eq!(plan.unlock.locked_amount, amount(0));
        assert!(plan.unlock.lock_positions.is_empty());
    }

    #[test]
    fn listed_purchase_with_fixed_time_unlock_creates_locked_position() {
        let source_time = at(1_700_000_000);
        let unlock_at = source_time + chrono::Duration::days(7);

        let plan = plan_post_listing_purchase(
            LifecycleStatus::Listed,
            true,
            &UnlockRule::FixedTime { unlock_at },
            unlock_source("purchase-1", 25, source_time),
        )
        .unwrap();

        assert_eq!(plan.unlock.available_amount, amount(0));
        assert_eq!(plan.unlock.locked_amount, amount(25));
        assert_eq!(plan.unlock.lock_positions.len(), 1);
        assert_eq!(plan.unlock.lock_positions[0].unlock_type, "fixed_time");
        assert_eq!(plan.unlock.lock_positions[0].unlock_at, unlock_at);
        assert_eq!(plan.unlock.lock_positions[0].remaining_amount, amount(25));
        assert_eq!(plan.unlock.lock_positions[0].source_id, None);
    }

    #[test]
    fn relative_period_unlock_splits_by_purchase_source_time() {
        let source_time = at(1_700_000_000);
        let plan = apply_unlock_rule(
            &UnlockRule::RelativePeriod {
                seconds_after_source: 86_400,
            },
            vec![
                unlock_source("purchase-1", 10, source_time),
                unlock_source(
                    "purchase-2",
                    15,
                    source_time + chrono::Duration::seconds(30),
                ),
            ],
        )
        .unwrap();

        assert_eq!(plan.available_amount, amount(0));
        assert_eq!(plan.locked_amount, amount(25));
        assert_eq!(plan.lock_positions.len(), 2);
        assert_eq!(
            plan.lock_positions[0].source_id.as_deref(),
            Some("purchase-1")
        );
        assert_eq!(
            plan.lock_positions[0].unlock_at,
            source_time + chrono::Duration::seconds(86_400)
        );
        assert_eq!(
            plan.lock_positions[1].source_id.as_deref(),
            Some("purchase-2")
        );
        assert_ne!(
            plan.lock_positions[0].merge_key,
            plan.lock_positions[1].merge_key
        );
    }

    #[test]
    fn unlock_fee_supports_market_value_basis_and_blocks_release_until_paid() {
        let fee = calculate_unlock_fee(
            &UnlockFeeRule {
                enabled: true,
                rate: decimal("0.04"),
                basis: UnlockFeeBasis::MarketValue,
                payment_asset: Some("USDT".to_owned()),
            },
            UnlockFeeInput {
                unlock_quantity: amount(10),
                unlock_price: amount(5),
                purchase_cost: amount(30),
            },
        )
        .unwrap();

        assert!(fee.required);
        assert_eq!(fee.payment_asset.as_deref(), Some("USDT"));
        assert_eq!(fee.amount, decimal("2.00"));
        assert_eq!(
            ensure_unlock_release_allowed(&fee, false),
            Err(NewCoinDomainError::UnlockFeePaymentRequired {
                payment_asset: "USDT".to_owned(),
                amount: decimal("2.00"),
            })
        );
        assert_eq!(ensure_unlock_release_allowed(&fee, true), Ok(()));
    }

    #[test]
    fn unlock_fee_supports_profit_basis_and_disabled_fee_releases_without_payment() {
        let profit_fee = calculate_unlock_fee(
            &UnlockFeeRule {
                enabled: true,
                rate: decimal("0.10"),
                basis: UnlockFeeBasis::Profit,
                payment_asset: Some("USDT".to_owned()),
            },
            UnlockFeeInput {
                unlock_quantity: amount(10),
                unlock_price: amount(5),
                purchase_cost: amount(30),
            },
        )
        .unwrap();

        assert_eq!(profit_fee.amount, decimal("2.00"));
        assert_eq!(profit_fee.payment_asset.as_deref(), Some("USDT"));

        let disabled_fee = calculate_unlock_fee(
            &UnlockFeeRule {
                enabled: false,
                rate: decimal("0.99"),
                basis: UnlockFeeBasis::MarketValue,
                payment_asset: Some("USDT".to_owned()),
            },
            UnlockFeeInput {
                unlock_quantity: amount(10),
                unlock_price: amount(5),
                purchase_cost: amount(30),
            },
        )
        .unwrap();

        assert!(!disabled_fee.required);
        assert_eq!(disabled_fee.amount, amount(0));
        assert_eq!(ensure_unlock_release_allowed(&disabled_fee, false), Ok(()));
    }
}
