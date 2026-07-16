use bigdecimal::BigDecimal;
use chrono::{Duration, Utc};
use exchange_api::modules::{
    spot::{MySqlSpotRepository, NewOrder, NewSpotTrade, OrderSide, OrderStatus, OrderType},
    wallet::{
        BalanceChange, LedgerBatch, LedgerMetadata, MySqlWalletRepository, NewAssetLockPosition,
        NewAssetLockPositionSource,
    },
};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::str::FromStr;
use uuid::Uuid;

fn dec(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL integration test because DATABASE_URL is not set");
            return None;
        }
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    Some(pool)
}

fn unique_suffix() -> String {
    Uuid::now_v7().simple().to_string()
}

async fn create_user(pool: &MySqlPool) -> u64 {
    let email = format!("{}@repo-test.example", unique_suffix());
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, prefix: &str) -> (u64, String) {
    let suffix = unique_suffix();
    let symbol = format!("{}{}", prefix, &suffix[..10]);
    let name = format!("repo test {symbol}");
    let asset_id = sqlx::query(
        "INSERT INTO assets (symbol, name, precision_scale, asset_type, status) VALUES (?, ?, 8, 'coin', 'active')",
    )
    .bind(&symbol)
    .bind(name)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    (asset_id, symbol)
}

async fn create_pair(
    pool: &MySqlPool,
    base_asset_id: u64,
    quote_asset_id: u64,
    base_symbol: &str,
    quote_symbol: &str,
) -> String {
    let symbol = format!("{base_symbol}-{quote_symbol}");
    sqlx::query(
        r#"INSERT INTO trading_pairs
           (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, status, market_type)
           VALUES (?, ?, ?, 2, 4, ?, 'active', 'spot')"#,
    )
    .bind(base_asset_id)
    .bind(quote_asset_id)
    .bind(&symbol)
    .bind(dec("10"))
    .execute(pool)
    .await
    .unwrap();

    symbol
}

#[tokio::test]
async fn mysql_wallet_repository_creates_account_ledger_and_lock_sources() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let repo = MySqlWalletRepository::new(pool.clone());
    let user_id = create_user(&pool).await;
    let (asset_id, _symbol) = create_asset(&pool, "WA").await;

    let account = repo
        .get_or_create_account_async(user_id, asset_id)
        .await
        .unwrap();
    assert_eq!(account.user_id, user_id.to_string());
    assert_eq!(account.asset_id, asset_id.to_string());
    assert_eq!(account.available, dec("0"));

    let mut credited = account;
    let change = BalanceChange::new(dec("25.5"), dec("0"), dec("0"));
    credited.apply_balance_change(change.clone()).unwrap();
    let ref_id = format!("wallet-ledger-{}", unique_suffix());
    let metadata = LedgerMetadata::new("deposit_credit", "deposit_record", ref_id.clone()).unwrap();
    let ledger = LedgerBatch::from_account_change(&credited, change, &metadata);

    repo.save_account_with_ledger_async(credited, ledger)
        .await
        .unwrap();

    let reloaded = repo
        .load_account_async(user_id, asset_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.available, dec("25.5"));

    let ledger_entries = repo
        .list_ledger_by_ref_async("deposit_record", &ref_id)
        .await
        .unwrap();
    assert_eq!(ledger_entries.len(), 1);
    assert_eq!(ledger_entries[0].amount, dec("25.5"));
    assert_eq!(ledger_entries[0].available_after, dec("25.5"));

    let unlock_at = Utc::now() + Duration::days(7);
    let source_id = format!("source-{}", unique_suffix());
    let position_ids = repo
        .insert_asset_lock_positions_async(vec![NewAssetLockPosition {
            user_id,
            asset_id,
            unlock_type: "fixed_time".to_owned(),
            unlock_at,
            locked_amount: dec("10"),
            remaining_amount: dec("10"),
            merge_key: format!("merge-{}", unique_suffix()),
            sources: vec![NewAssetLockPositionSource {
                source_type: "new_coin_purchase".to_owned(),
                source_id,
                source_amount: dec("10"),
                source_time: Utc::now(),
            }],
        }])
        .await
        .unwrap();

    assert_eq!(position_ids.len(), 1);
    assert_eq!(
        repo.count_lock_position_sources_async(position_ids[0])
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn mysql_spot_repository_persists_orders_and_trades() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let repo = MySqlSpotRepository::new(pool.clone());
    let buyer_id = create_user(&pool).await;
    let seller_id = create_user(&pool).await;
    let (base_asset_id, base_symbol) = create_asset(&pool, "SB").await;
    let (quote_asset_id, quote_symbol) = create_asset(&pool, "SQ").await;
    let pair_symbol = create_pair(
        &pool,
        base_asset_id,
        quote_asset_id,
        &base_symbol,
        &quote_symbol,
    )
    .await;

    let pair_rule = repo.load_pair_rule_async(&pair_symbol).await.unwrap();
    assert_eq!(pair_rule.pair_id, pair_symbol);
    assert!(pair_rule.enabled);

    let buy_order = repo
        .insert_order_async(
            NewOrder {
                user_id: buyer_id.to_string(),
                pair_id: pair_symbol.clone(),
                side: OrderSide::Buy,
                order_type: OrderType::Limit,
                price: Some(dec("100.12")),
                trigger_price: None,
                quantity: dec("0.2000"),
                filled_quantity: dec("0"),
                status: OrderStatus::Pending,
            },
            Some(&format!("buy-{}", unique_suffix())),
        )
        .await
        .unwrap();

    let loaded_buy = repo.load_order_async(&buy_order.id).await.unwrap();
    assert_eq!(loaded_buy.pair_id, pair_symbol);
    assert_eq!(loaded_buy.status, OrderStatus::Pending);
    assert_eq!(loaded_buy.price, Some(dec("100.12")));

    let mut open_buy = loaded_buy;
    open_buy.status = OrderStatus::Open;
    repo.save_order_async(open_buy.clone()).await.unwrap();
    assert_eq!(
        repo.load_order_async(&open_buy.id).await.unwrap().status,
        OrderStatus::Open
    );

    let sell_order = repo
        .insert_order_async(
            NewOrder {
                user_id: seller_id.to_string(),
                pair_id: pair_symbol.clone(),
                side: OrderSide::Sell,
                order_type: OrderType::Limit,
                price: Some(dec("100.12")),
                trigger_price: None,
                quantity: dec("0.2000"),
                filled_quantity: dec("0"),
                status: OrderStatus::Open,
            },
            Some(&format!("sell-{}", unique_suffix())),
        )
        .await
        .unwrap();

    let trade = repo
        .insert_trade_async(NewSpotTrade {
            pair_id: pair_symbol.clone(),
            buy_order_id: open_buy.id,
            sell_order_id: sell_order.id,
            price: dec("100.12"),
            quantity: dec("0.1000"),
            fee: dec("0.01"),
        })
        .await
        .unwrap();

    let trades = repo
        .list_trades_by_pair_async(&pair_symbol, 10)
        .await
        .unwrap();
    assert!(trades.iter().any(|row| row.id == trade.id));
    assert_eq!(trade.price, dec("100.12"));
    assert_eq!(trade.quantity, dec("0.1000"));
}
