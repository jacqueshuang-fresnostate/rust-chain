use bigdecimal::BigDecimal;
use chrono::{Duration, TimeDelta, TimeZone, Utc};
use exchange_api::modules::{
    convert::{
        ConfirmConvertQuoteCommand, ConvertBalanceSnapshot, ConvertConfirmationInsert,
        ConvertOrderRepository, ConvertQuoteCacheEntry, ConvertQuoteCommand,
        ConvertQuoteRepository, ConvertRepositoryError, ConvertService, ConvertServiceError,
        QuoteId,
    },
    new_coin::{
        LifecycleStatus, NewCoinDomainError, NewCoinPurchaseRepository, NewCoinRepositoryError,
        NewCoinService, NewCoinServiceError, PostListingPurchaseCommand, PostListingPurchaseRecord,
        ReleaseUnlockCommand, UnlockFeeBasis, UnlockFeePaymentRecord, UnlockFeeQuoteCommand,
        UnlockFeeRepository, UnlockFeeRule, UnlockReleaseRecord, UnlockRule,
    },
};
use std::{collections::HashMap, str::FromStr};
use uuid::Uuid;

#[derive(Default)]
struct FakeNewCoinRepository {
    purchases: Vec<PostListingPurchaseRecord>,
    payments: Vec<UnlockFeePaymentRecord>,
    releases: Vec<UnlockReleaseRecord>,
}

impl NewCoinPurchaseRepository for FakeNewCoinRepository {
    fn save_post_listing_purchase(
        &mut self,
        record: PostListingPurchaseRecord,
    ) -> Result<(), NewCoinRepositoryError> {
        self.purchases.push(record);
        Ok(())
    }
}

impl UnlockFeeRepository for FakeNewCoinRepository {
    fn save_unlock_fee_payment(
        &mut self,
        record: UnlockFeePaymentRecord,
    ) -> Result<(), NewCoinRepositoryError> {
        self.payments.push(record);
        Ok(())
    }

    fn unlock_fee_paid(
        &self,
        unlock_id: &str,
        user_id: &str,
    ) -> Result<bool, NewCoinRepositoryError> {
        Ok(self
            .payments
            .iter()
            .any(|payment| payment.unlock_id == unlock_id && payment.user_id == user_id))
    }

    fn mark_unlock_released(
        &mut self,
        record: UnlockReleaseRecord,
    ) -> Result<(), NewCoinRepositoryError> {
        self.releases.push(record);
        Ok(())
    }
}

#[derive(Default)]
struct FakeConvertRepository {
    quotes: HashMap<Uuid, ConvertQuoteCacheEntry>,
    confirmations: Vec<exchange_api::modules::convert::ConvertQuoteConfirmationRecord>,
}

impl ConvertQuoteRepository for FakeConvertRepository {
    fn save_quote_ttl(
        &mut self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError> {
        self.quotes.insert(entry.quote_id.0, entry);
        Ok(())
    }

    fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError> {
        Ok(self.quotes.get(&quote_id.0).cloned())
    }
}

impl ConvertOrderRepository for FakeConvertRepository {
    fn insert_quote_confirmation(
        &mut self,
        record: exchange_api::modules::convert::ConvertQuoteConfirmationRecord,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError> {
        if self
            .confirmations
            .iter()
            .any(|existing| existing.quote_id == record.quote_id)
        {
            return Ok(ConvertConfirmationInsert::Duplicate);
        }

        self.confirmations.push(record);
        Ok(ConvertConfirmationInsert::Inserted)
    }
}

fn amount(value: i64) -> BigDecimal {
    BigDecimal::from(value)
}

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn at(seconds: i64) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(seconds, 0).unwrap()
}

#[test]
fn post_listing_purchase_returns_wallet_lock_command_for_fixed_time_unlock() {
    let purchased_at = at(1_700_000_000);
    let unlock_at = purchased_at + Duration::days(7);
    let mut service = NewCoinService::new(FakeNewCoinRepository::default());

    let result = service
        .create_post_listing_purchase(PostListingPurchaseCommand {
            project_id: "launch-1".to_owned(),
            order_id: "purchase-1".to_owned(),
            user_id: "user-1".to_owned(),
            asset_id: "NEW".to_owned(),
            quantity: amount(25),
            purchased_at,
            lifecycle_status: LifecycleStatus::Listed,
            post_listing_purchase_enabled: true,
            unlock_rule: UnlockRule::FixedTime { unlock_at },
        })
        .unwrap();

    assert_eq!(result.order_kind.chinese_name(), "认购");
    assert_eq!(result.wallet_lock.available_delta, amount(0));
    assert_eq!(result.wallet_lock.locked_delta, amount(25));
    assert_eq!(result.wallet_lock.lock_positions.len(), 1);
    assert_eq!(
        result.wallet_lock.lock_positions[0].unlock_type,
        "fixed_time"
    );
    assert_eq!(result.wallet_lock.lock_positions[0].unlock_at, unlock_at);
    assert_eq!(
        result.wallet_lock.lock_positions[0].remaining_amount,
        amount(25)
    );

    let saved = service.repository().purchases.first().unwrap();
    assert_eq!(saved.order_id, "purchase-1");
    assert_eq!(saved.wallet_lock.locked_delta, amount(25));
}

#[test]
fn fee_required_unlock_release_is_blocked_until_payment_exists() {
    let mut service = NewCoinService::new(FakeNewCoinRepository::default());
    let fee = service
        .quote_unlock_fee(UnlockFeeQuoteCommand {
            unlock_id: "unlock-1".to_owned(),
            user_id: "user-1".to_owned(),
            asset_id: "NEW".to_owned(),
            unlock_quantity: amount(10),
            unlock_price: amount(5),
            purchase_cost: amount(30),
            fee_rule: UnlockFeeRule {
                enabled: true,
                rate: decimal("0.04"),
                basis: UnlockFeeBasis::MarketValue,
                payment_asset: Some("USDT".to_owned()),
            },
        })
        .unwrap();

    let error = service
        .release_unlock(ReleaseUnlockCommand {
            unlock_id: "unlock-1".to_owned(),
            user_id: "user-1".to_owned(),
            asset_id: "NEW".to_owned(),
            fee_quote: fee.quote,
        })
        .unwrap_err();

    assert_eq!(
        error,
        NewCoinServiceError::Domain(NewCoinDomainError::UnlockFeePaymentRequired {
            payment_asset: "USDT".to_owned(),
            amount: decimal("2.00"),
        })
    );
    assert!(service.repository().releases.is_empty());
}

#[test]
fn expired_convert_quote_is_rejected_before_mysql_confirmation_insert() {
    let quote_id = QuoteId(Uuid::from_u128(1));
    let created_at = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
    let mut service = ConvertService::new(FakeConvertRepository::default());

    service
        .create_quote(ConvertQuoteCommand {
            quote_id: quote_id.clone(),
            user_id: "user-1".to_owned(),
            from_asset: "USDT".to_owned(),
            to_asset: "BTC".to_owned(),
            from_amount: amount(100),
            to_amount: decimal("0.001"),
            balance: ConvertBalanceSnapshot {
                available: amount(100),
                locked: amount(0),
            },
            created_at,
            ttl_seconds: 10,
        })
        .unwrap();

    let error = service
        .confirm_quote(ConfirmConvertQuoteCommand {
            quote_id: quote_id.clone(),
            user_id: "user-1".to_owned(),
            confirmed_at: created_at + TimeDelta::seconds(10),
        })
        .unwrap_err();

    assert_eq!(error, ConvertServiceError::QuoteExpired { quote_id });
    assert!(service.repository().confirmations.is_empty());
}

#[test]
fn duplicate_convert_quote_confirmation_is_rejected_by_quote_id_idempotency() {
    let quote_id = QuoteId(Uuid::from_u128(2));
    let created_at = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
    let mut service = ConvertService::new(FakeConvertRepository::default());

    service
        .create_quote(ConvertQuoteCommand {
            quote_id: quote_id.clone(),
            user_id: "user-1".to_owned(),
            from_asset: "USDT".to_owned(),
            to_asset: "ETH".to_owned(),
            from_amount: amount(50),
            to_amount: decimal("0.02"),
            balance: ConvertBalanceSnapshot {
                available: amount(50),
                locked: amount(0),
            },
            created_at,
            ttl_seconds: 30,
        })
        .unwrap();

    service
        .confirm_quote(ConfirmConvertQuoteCommand {
            quote_id: quote_id.clone(),
            user_id: "user-1".to_owned(),
            confirmed_at: created_at + TimeDelta::seconds(1),
        })
        .unwrap();

    let error = service
        .confirm_quote(ConfirmConvertQuoteCommand {
            quote_id: quote_id.clone(),
            user_id: "user-1".to_owned(),
            confirmed_at: created_at + TimeDelta::seconds(2),
        })
        .unwrap_err();

    assert_eq!(
        error,
        ConvertServiceError::DuplicateQuoteConfirmation { quote_id }
    );
    assert_eq!(service.repository().confirmations.len(), 1);
}

#[test]
fn locked_balance_is_not_convertible_when_available_balance_is_too_low() {
    let quote_id = QuoteId(Uuid::from_u128(3));
    let created_at = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
    let mut service = ConvertService::new(FakeConvertRepository::default());

    let error = service
        .create_quote(ConvertQuoteCommand {
            quote_id: quote_id.clone(),
            user_id: "user-1".to_owned(),
            from_asset: "NEW".to_owned(),
            to_asset: "USDT".to_owned(),
            from_amount: amount(10),
            to_amount: amount(20),
            balance: ConvertBalanceSnapshot {
                available: amount(5),
                locked: amount(100),
            },
            created_at,
            ttl_seconds: 30,
        })
        .unwrap_err();

    assert_eq!(
        error,
        ConvertServiceError::InsufficientAvailableBalance {
            asset_id: "NEW".to_owned(),
            requested: Box::new(amount(10)),
            available: Box::new(amount(5)),
            locked: Box::new(amount(100)),
        }
    );
    assert!(service.repository().quotes.is_empty());
}
