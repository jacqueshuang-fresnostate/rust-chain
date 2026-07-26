use axum::async_trait;
use bigdecimal::BigDecimal;
use exchange_api::{
    error::AppResult,
    modules::wallet::{
        infrastructure::list_wallet_chain_event_dead_letters,
        repository::{
            WalletChainBroadcastCommand, WalletChainBroadcastResult, WalletChainDepositObservation,
            WalletChainGateway, WalletChainPollPage, WalletChainWithdrawalObservation,
        },
    },
    workers::wallet_chain::{WalletChainWorkerConfig, run_once_with_gateway},
};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use uuid::Uuid;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

async fn mysql_pool_or_skip() -> Result<Option<MySqlPool>, Box<dyn Error>> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping wallet chain worker test because DATABASE_URL is not set");
            return Ok(None);
        }
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(Some(pool))
}

#[derive(Debug)]
struct ConfirmingGateway {
    request_id: String,
    tx_hash: String,
}

#[async_trait]
impl WalletChainGateway for ConfirmingGateway {
    async fn broadcast_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        command: &WalletChainBroadcastCommand,
    ) -> AppResult<WalletChainBroadcastResult> {
        assert_eq!(command.request_id, self.request_id);
        Ok(WalletChainBroadcastResult {
            tx_hash: self.tx_hash.clone(),
            block_height: Some(100),
            confirmations: 0,
        })
    }

    async fn poll_chain_events(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> AppResult<WalletChainPollPage> {
        Ok(WalletChainPollPage {
            next_cursor: Some("wallet-chain-test-cursor".to_owned()),
            deposits: Vec::new(),
            withdrawals: vec![WalletChainWithdrawalObservation {
                request_id: self.request_id.clone(),
                network: "base".to_owned(),
                tx_hash: Some(self.tx_hash.clone()),
                block_height: Some(100),
                confirmations: 12,
                status: "confirmed".to_owned(),
                failure_reason: None,
            }],
        })
    }
}

#[tokio::test]
async fn wallet_chain_worker_broadcasts_and_confirms_withdrawal_once() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("WC{}", &suffix[20..32]).to_ascii_uppercase();
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("wallet-chain-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let asset_id = sqlx::query(
        r#"INSERT INTO assets
           (symbol, name, precision_scale, asset_type, status, withdraw_enabled, withdraw_fee)
           VALUES (?, ?, 8, 'coin', 'active', TRUE, 1)"#,
    )
    .bind(&symbol)
    .bind(format!("{symbol} asset"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 11, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&pool)
    .await?;

    let request_id = Uuid::now_v7().to_string();
    let withdrawal_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'base', '0xabc', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("wallet-chain-{suffix}"))
    .bind(&request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO wallet_chain_gateways
           (network, broadcast_url, event_poll_url, status)
           VALUES ('base', 'http://gateway.test/broadcast', 'http://gateway.test/events', 'active')
           ON DUPLICATE KEY UPDATE
             broadcast_url = VALUES(broadcast_url),
             event_poll_url = VALUES(event_poll_url),
             auth_token_encrypted = NULL,
             last_deposit_cursor = NULL,
             status = 'active'"#,
    )
    .execute(&pool)
    .await?;

    let tx_hash = format!("0x{}", Uuid::now_v7().simple());
    let gateway = ConfirmingGateway {
        request_id,
        tx_hash: tx_hash.clone(),
    };
    let config = WalletChainWorkerConfig {
        enabled: true,
        interval_seconds: 1,
        batch_limit: 10,
        max_attempts: 3,
    };
    let first = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(first.withdrawal_broadcasted, 1);
    assert_eq!(first.withdrawal_confirmed, 1);

    let (status, stored_tx_hash, confirmations): (String, Option<String>, u32) = sqlx::query_as(
        "SELECT status, tx_hash, confirmations FROM wallet_withdrawal_requests WHERE id = ?",
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "confirmed");
    assert_eq!(stored_tx_hash.as_deref(), Some(tx_hash.as_str()));
    assert_eq!(confirmations, 12);
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("0"));
    assert_eq!(frozen, decimal("0"));

    let second = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(second.withdrawal_broadcasted, 0);
    assert_eq!(second.withdrawal_confirmed, 0);
    let confirm_ledger_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ?
             AND change_type = 'withdrawal_confirm'"#,
    )
    .bind(withdrawal_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(confirm_ledger_count, 1);

    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_withdrawal_requests WHERE id = ?")
        .bind(withdrawal_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_chain_gateways WHERE network = 'base'")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_accounts WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[derive(Debug)]
struct DepositPageGateway {
    page: WalletChainPollPage,
}

#[async_trait]
impl WalletChainGateway for DepositPageGateway {
    async fn broadcast_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _command: &WalletChainBroadcastCommand,
    ) -> AppResult<WalletChainBroadcastResult> {
        unreachable!("deposit page gateway does not broadcast withdrawals")
    }

    async fn poll_chain_events(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> AppResult<WalletChainPollPage> {
        Ok(self.page.clone())
    }
}

#[tokio::test]
async fn wallet_chain_worker_dead_letters_poison_deposit_but_halts_on_transient_failure()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("DL{}", &suffix[20..32]).to_ascii_uppercase();
    let group_code = format!("DLG-{}", &suffix[24..32]);
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("wallet-chain-dl-{suffix}@example.test"))
        .bind("not-a-real-hash")
        .execute(&pool)
        .await?
        .last_insert_id();
    let asset_id = sqlx::query(
        r#"INSERT INTO assets
           (symbol, name, precision_scale, asset_type, status, deposit_enabled, min_deposit_amount)
           VALUES (?, ?, 8, 'coin', 'active', TRUE, 1)"#,
    )
    .bind(&symbol)
    .bind(format!("{symbol} asset"))
    .execute(&pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"INSERT INTO deposit_network_configs
           (network, display_name, address_group_code, address_group_name, asset_symbols_json,
            status, sort_order, required_confirmations)
           VALUES ('base', 'Base', ?, ?, NULL, 'active', 0, 12)
           ON DUPLICATE KEY UPDATE
             address_group_code = VALUES(address_group_code),
             address_group_name = VALUES(address_group_name),
             asset_symbols_json = NULL,
             status = 'active',
             required_confirmations = 12"#,
    )
    .bind(&group_code)
    .bind(&group_code)
    .execute(&pool)
    .await?;
    let allocated_address = format!("0xdl{}", &suffix[..20]);
    sqlx::query(
        r#"INSERT INTO deposit_address_pool
           (network, address_group_code, address, status, assigned_user_id,
            assigned_asset_symbol, assigned_at)
           VALUES ('base', ?, ?, 'assigned', ?, ?, CURRENT_TIMESTAMP(6))"#,
    )
    .bind(&group_code)
    .bind(&allocated_address)
    .bind(user_id)
    .bind(&symbol)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO wallet_chain_gateways (network, event_poll_url, status)
           VALUES ('base', 'http://gateway.test/events', 'active')
           ON DUPLICATE KEY UPDATE
             broadcast_url = NULL,
             event_poll_url = VALUES(event_poll_url),
             auth_token_encrypted = NULL,
             last_deposit_cursor = NULL,
             status = 'active'"#,
    )
    .execute(&pool)
    .await?;

    let poison_tx_hash = format!("0xpoison{}", Uuid::now_v7().simple());
    let good_tx_hash = format!("0xgood{}", Uuid::now_v7().simple());
    let config = WalletChainWorkerConfig {
        enabled: true,
        interval_seconds: 1,
        batch_limit: 10,
        max_attempts: 3,
    };
    let deposit =
        |address: &str, tx_hash: &str, memo: Option<String>| WalletChainDepositObservation {
            asset_symbol: symbol.clone(),
            network: "base".to_owned(),
            address: address.to_owned(),
            memo,
            tx_hash: tx_hash.to_owned(),
            event_index: 0,
            amount: "5".to_owned(),
            block_height: Some(100),
            confirmations: 0,
        };

    // 未分配地址的毒性事件进入死信，后续正常事件与游标不再被阻塞。
    let gateway = DepositPageGateway {
        page: WalletChainPollPage {
            next_cursor: Some("dl-cursor-1".to_owned()),
            deposits: vec![
                deposit(
                    &format!("0xunallocated{}", &suffix[..16]),
                    &poison_tx_hash,
                    None,
                ),
                deposit(&allocated_address, &good_tx_hash, None),
            ],
            withdrawals: Vec::new(),
        },
    };
    let first = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(first.deposit_observed, 1);
    assert_eq!(first.event_dead_lettered, 1);
    assert_eq!(first.gateway_failed, 0);
    let (cursor,): (Option<String>,) = sqlx::query_as(
        "SELECT last_deposit_cursor FROM wallet_chain_gateways WHERE network = 'base'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor.as_deref(), Some("dl-cursor-1"));
    let (good_status,): (String,) =
        sqlx::query_as("SELECT status FROM wallet_deposit_events WHERE tx_hash = ?")
            .bind(&good_tx_hash)
            .fetch_one(&pool)
            .await?;
    assert_eq!(good_status, "observed");
    let dead_letters = list_wallet_chain_event_dead_letters(&pool, Some("base"), 50).await?;
    let record = dead_letters
        .iter()
        .find(|record| record.tx_hash.as_deref() == Some(poison_tx_hash.as_str()))
        .expect("poison deposit should be dead lettered");
    assert_eq!(record.event_kind, "deposit");
    assert_eq!(record.event_index, Some(0));
    assert!(record.failure_reason.contains("not found"));
    assert_eq!(record.payload_json.0["asset_symbol"], symbol.as_str());

    // 数据库层拒绝（memo 超长）按瞬态失败处理：停页、不推进游标、不写死信。
    let transient_tx_hash = format!("0xtransient{}", Uuid::now_v7().simple());
    let gateway = DepositPageGateway {
        page: WalletChainPollPage {
            next_cursor: Some("dl-cursor-2".to_owned()),
            deposits: vec![deposit(
                &allocated_address,
                &transient_tx_hash,
                Some("m".repeat(300)),
            )],
            withdrawals: Vec::new(),
        },
    };
    let second = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(second.deposit_observed, 0);
    assert_eq!(second.event_dead_lettered, 0);
    assert_eq!(second.gateway_failed, 1);
    let (cursor,): (Option<String>,) = sqlx::query_as(
        "SELECT last_deposit_cursor FROM wallet_chain_gateways WHERE network = 'base'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor.as_deref(), Some("dl-cursor-1"));
    let dead_letters = list_wallet_chain_event_dead_letters(&pool, Some("base"), 50).await?;
    assert!(
        dead_letters
            .iter()
            .all(|record| record.tx_hash.as_deref() != Some(transient_tx_hash.as_str()))
    );

    sqlx::query("DELETE FROM wallet_chain_event_dead_letters WHERE tx_hash = ?")
        .bind(&poison_tx_hash)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_deposit_events WHERE tx_hash = ?")
        .bind(&good_tx_hash)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM deposit_address_pool WHERE address = ?")
        .bind(&allocated_address)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_chain_gateways WHERE network = 'base'")
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"UPDATE deposit_network_configs
           SET address_group_code = 'A', address_group_name = 'EVM',
               asset_symbols_json = JSON_ARRAY('ETH', 'USDT', 'USDC')
           WHERE network = 'base'"#,
    )
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await?;
    Ok(())
}
