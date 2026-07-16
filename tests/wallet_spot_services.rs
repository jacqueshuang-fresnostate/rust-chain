use bigdecimal::BigDecimal;
use exchange_api::modules::{
    spot::{
        CancelSpotOrderCommand, CreateSpotOrderCommand, FillSpotOrderCommand, NewOrder, OrderSide,
        OrderStatus, OrderType, SpotOrder, SpotRepository, SpotService, SpotServiceError,
        TradingPairRule,
    },
    wallet::{
        BalanceBucket, FreezeBalanceCommand, LedgerBatch, LedgerMetadata, LockPosition,
        LockPositionCreationCommand, LockPositionSource, LockSchedule, WalletAccount,
        WalletLedgerEntry, WalletRepository, WalletService, WalletServiceError,
    },
};
use std::{collections::HashMap, str::FromStr};

fn dec(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(seconds, 0).unwrap()
}

fn ledger(change_type: &str, ref_type: &str, ref_id: &str) -> LedgerMetadata {
    LedgerMetadata::new(change_type, ref_type, ref_id).unwrap()
}

fn account(user_id: &str, asset_id: &str, available: &str, frozen: &str) -> WalletAccount {
    WalletAccount {
        user_id: user_id.to_owned(),
        asset_id: asset_id.to_owned(),
        available: dec(available),
        frozen: dec(frozen),
        locked: dec("0"),
    }
}

#[derive(Default)]
struct FakeWalletRepository {
    accounts: HashMap<(String, String), WalletAccount>,
    ledger: Vec<WalletLedgerEntry>,
    lock_positions: Vec<LockPosition>,
}

impl FakeWalletRepository {
    fn with_account(mut self, account: WalletAccount) -> Self {
        self.accounts
            .insert((account.user_id.clone(), account.asset_id.clone()), account);
        self
    }

    fn account(&self, user_id: &str, asset_id: &str) -> &WalletAccount {
        self.accounts
            .get(&(user_id.to_owned(), asset_id.to_owned()))
            .unwrap()
    }
}

impl WalletRepository for FakeWalletRepository {
    fn load_account(
        &mut self,
        user_id: &str,
        asset_id: &str,
    ) -> Result<WalletAccount, WalletServiceError> {
        self.accounts
            .get(&(user_id.to_owned(), asset_id.to_owned()))
            .cloned()
            .ok_or_else(|| WalletServiceError::Repository("missing wallet account".to_owned()))
    }

    fn save_account_with_ledger(
        &mut self,
        account: WalletAccount,
        ledger: LedgerBatch,
    ) -> Result<(), WalletServiceError> {
        self.ledger.extend(ledger.into_entries());
        self.accounts
            .insert((account.user_id.clone(), account.asset_id.clone()), account);
        Ok(())
    }

    fn insert_lock_positions(
        &mut self,
        positions: Vec<LockPosition>,
    ) -> Result<(), WalletServiceError> {
        self.lock_positions.extend(positions);
        Ok(())
    }
}

#[derive(Clone)]
struct FakeSpotRepository {
    pair: TradingPairRule,
    orders: HashMap<String, SpotOrder>,
    next_order_id: String,
}

impl FakeSpotRepository {
    fn new() -> Self {
        Self {
            pair: TradingPairRule {
                pair_id: "BTC-USDT".to_owned(),
                price_precision: 2,
                quantity_precision: 4,
                min_order_value: dec("10"),
                enabled: true,
            },
            orders: HashMap::new(),
            next_order_id: "order-1".to_owned(),
        }
    }

    fn with_order(mut self, order: SpotOrder) -> Self {
        self.orders.insert(order.id.clone(), order);
        self
    }
}

impl SpotRepository for FakeSpotRepository {
    fn load_pair_rule(&mut self, pair_id: &str) -> Result<TradingPairRule, SpotServiceError> {
        if self.pair.pair_id == pair_id {
            Ok(self.pair.clone())
        } else {
            Err(SpotServiceError::Repository(
                "missing trading pair".to_owned(),
            ))
        }
    }

    fn insert_order(
        &mut self,
        new_order: NewOrder,
        _idempotency_key: Option<&str>,
    ) -> Result<SpotOrder, SpotServiceError> {
        let order = SpotOrder {
            id: self.next_order_id.clone(),
            user_id: new_order.user_id,
            pair_id: new_order.pair_id,
            side: new_order.side,
            order_type: new_order.order_type,
            price: new_order.price,
            trigger_price: new_order.trigger_price,
            quantity: new_order.quantity,
            filled_quantity: new_order.filled_quantity,
            status: new_order.status,
        };
        self.orders.insert(order.id.clone(), order.clone());
        Ok(order)
    }

    fn load_order(&mut self, order_id: &str) -> Result<SpotOrder, SpotServiceError> {
        self.orders
            .get(order_id)
            .cloned()
            .ok_or_else(|| SpotServiceError::Repository("missing spot order".to_owned()))
    }

    fn save_order(&mut self, order: SpotOrder) -> Result<(), SpotServiceError> {
        self.orders.insert(order.id.clone(), order);
        Ok(())
    }
}

#[test]
fn ledger_metadata_rejects_empty_required_fields() {
    assert!(LedgerMetadata::new("", "spot_order", "order-1").is_err());
    assert!(LedgerMetadata::new("spot_freeze", "", "order-1").is_err());
    assert!(LedgerMetadata::new("spot_freeze", "spot_order", "").is_err());
}

#[test]
fn wallet_freeze_updates_balances_and_records_required_ledger() {
    let repo = FakeWalletRepository::default().with_account(account("user-1", "USDT", "100", "0"));
    let mut service = WalletService::new(repo);

    service
        .freeze(FreezeBalanceCommand {
            user_id: "user-1".to_owned(),
            asset_id: "USDT".to_owned(),
            amount: dec("25"),
            ledger: ledger("spot_freeze", "spot_order", "order-1"),
        })
        .unwrap();

    let repo = service.into_repository();
    let account = repo.account("user-1", "USDT");
    assert_eq!(account.available, dec("75"));
    assert_eq!(account.frozen, dec("25"));
    assert_eq!(repo.ledger.len(), 2);
    assert!(repo.ledger.iter().all(|entry| {
        entry.change_type == "spot_freeze"
            && entry.ref_type == "spot_order"
            && entry.ref_id == "order-1"
    }));
    assert!(repo.ledger.iter().any(|entry| {
        entry.balance_type == BalanceBucket::Available && entry.amount == dec("-25")
    }));
    assert!(
        repo.ledger.iter().any(|entry| {
            entry.balance_type == BalanceBucket::Frozen && entry.amount == dec("25")
        })
    );
}

#[test]
fn lock_position_creation_locks_available_balance_and_persists_positions() {
    let repo = FakeWalletRepository::default().with_account(account("user-1", "NEW", "50", "0"));
    let mut service = WalletService::new(repo);
    let unlock_at = at(1_700_000_000);

    let positions = service
        .create_lock_positions(LockPositionCreationCommand {
            user_id: "user-1".to_owned(),
            asset_id: "NEW".to_owned(),
            schedule: LockSchedule::FixedTime { unlock_at },
            sources: vec![
                LockPositionSource {
                    source_id: "purchase-1".to_owned(),
                    amount: dec("10"),
                    unlock_at,
                },
                LockPositionSource {
                    source_id: "purchase-2".to_owned(),
                    amount: dec("20"),
                    unlock_at,
                },
            ],
            ledger: ledger("asset_lock", "new_coin_purchase", "batch-1"),
        })
        .unwrap();

    let repo = service.into_repository();
    let account = repo.account("user-1", "NEW");
    assert_eq!(positions.len(), 1);
    assert_eq!(repo.lock_positions.len(), 1);
    assert_eq!(positions[0].remaining_amount, dec("30"));
    assert_eq!(account.available, dec("20"));
    assert_eq!(account.locked, dec("30"));
    assert_eq!(repo.ledger.len(), 2);
}

#[test]
fn spot_create_limit_buy_order_freezes_quote_asset_before_insert() {
    let spot_repo = FakeSpotRepository::new();
    let wallet_repo =
        FakeWalletRepository::default().with_account(account("user-1", "USDT", "100", "0"));
    let mut service = SpotService::new(spot_repo, wallet_repo);

    let order = service
        .create_order(CreateSpotOrderCommand {
            user_id: "user-1".to_owned(),
            pair_id: "BTC-USDT".to_owned(),
            base_asset_id: "BTC".to_owned(),
            quote_asset_id: "USDT".to_owned(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            price: Some(dec("10")),
            trigger_price: None,
            quantity: dec("2"),
            reference_price: None,
            idempotency_key: Some("client-order-1".to_owned()),
            wallet_ledger: ledger("spot_freeze", "spot_order", "client-order-1"),
        })
        .unwrap();

    let (_spot_repo, wallet_repo) = service.into_repositories();
    let quote = wallet_repo.account("user-1", "USDT");
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(quote.available, dec("80"));
    assert_eq!(quote.frozen, dec("20"));
}

#[test]
fn spot_cancel_is_idempotent_and_unfreezes_remaining_balance_once() {
    let spot_repo = FakeSpotRepository::new().with_order(SpotOrder {
        id: "order-1".to_owned(),
        user_id: "user-1".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(dec("10")),
        trigger_price: None,
        quantity: dec("2"),
        filled_quantity: dec("0"),
        status: OrderStatus::Open,
    });
    let wallet_repo =
        FakeWalletRepository::default().with_account(account("user-1", "USDT", "80", "20"));
    let mut service = SpotService::new(spot_repo, wallet_repo);

    let first_cancelled = service
        .cancel_order(CancelSpotOrderCommand {
            order_id: "order-1".to_owned(),
            base_asset_id: "BTC".to_owned(),
            quote_asset_id: "USDT".to_owned(),
            wallet_ledger: ledger("spot_unfreeze", "spot_order", "order-1"),
        })
        .unwrap();
    let second_cancelled = service
        .cancel_order(CancelSpotOrderCommand {
            order_id: "order-1".to_owned(),
            base_asset_id: "BTC".to_owned(),
            quote_asset_id: "USDT".to_owned(),
            wallet_ledger: ledger("spot_unfreeze", "spot_order", "order-1"),
        })
        .unwrap();

    let (spot_repo, wallet_repo) = service.into_repositories();
    let quote = wallet_repo.account("user-1", "USDT");
    assert!(first_cancelled);
    assert!(!second_cancelled);
    assert_eq!(spot_repo.orders["order-1"].status, OrderStatus::Cancelled);
    assert_eq!(quote.available, dec("100"));
    assert_eq!(quote.frozen, dec("0"));
    assert_eq!(wallet_repo.ledger.len(), 2);
}

#[test]
fn spot_fill_settles_frozen_quote_and_credits_base_for_buy_order() {
    let spot_repo = FakeSpotRepository::new().with_order(SpotOrder {
        id: "order-1".to_owned(),
        user_id: "user-1".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(dec("10")),
        trigger_price: None,
        quantity: dec("2"),
        filled_quantity: dec("0"),
        status: OrderStatus::Open,
    });
    let wallet_repo = FakeWalletRepository::default()
        .with_account(account("user-1", "USDT", "80", "20"))
        .with_account(account("user-1", "BTC", "0", "0"));
    let mut service = SpotService::new(spot_repo, wallet_repo);

    let order = service
        .fill_order(FillSpotOrderCommand {
            order_id: "order-1".to_owned(),
            base_asset_id: "BTC".to_owned(),
            quote_asset_id: "USDT".to_owned(),
            fill_price: dec("10"),
            fill_quantity: dec("1"),
            wallet_ledger: ledger("spot_fill", "spot_trade", "trade-1"),
        })
        .unwrap();

    let (_spot_repo, wallet_repo) = service.into_repositories();
    assert_eq!(order.status, OrderStatus::PartiallyFilled);
    assert_eq!(wallet_repo.account("user-1", "USDT").frozen, dec("10"));
    assert_eq!(wallet_repo.account("user-1", "BTC").available, dec("1"));
    assert_eq!(wallet_repo.ledger.len(), 2);
}
