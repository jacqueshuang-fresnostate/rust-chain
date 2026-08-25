use axum::async_trait;
use bigdecimal::BigDecimal;
use exchange_api::{
    error::AppResult,
    modules::wallet::{
        infrastructure::list_wallet_chain_event_dead_letters,
        repository::{
            WalletChainBroadcastCommand, WalletChainBroadcastResult, WalletChainDepositObservation,
            WalletChainGateway, WalletChainGatewayError, WalletChainGatewayErrorClass,
            WalletChainPollPage, WalletChainWithdrawalObservation,
            WalletChainWithdrawalQueryResult, WalletChainWithdrawalQueryStatus,
        },
    },
    workers::wallet_chain::{WalletChainWorkerConfig, run_once_with_gateway},
};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{
    error::Error,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};
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
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError> {
        assert_eq!(command.request_id, self.request_id);
        Ok(WalletChainBroadcastResult {
            tx_hash: self.tx_hash.clone(),
            block_height: Some(100),
            confirmations: 0,
        })
    }

    async fn query_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        request_id: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError> {
        assert_eq!(request_id, self.request_id);
        Ok(WalletChainWithdrawalQueryResult {
            status: WalletChainWithdrawalQueryStatus::Confirmed,
            tx_hash: Some(self.tx_hash.clone()),
            block_height: Some(100),
            confirmations: 12,
            failure_reason: None,
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
                // 乱序的旧回执不得把广播时已记录的区块高度 100 回退。
                block_height: Some(90),
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

    let (status, stored_tx_hash, block_height, confirmations): (
        String,
        Option<String>,
        Option<u64>,
        u32,
    ) = sqlx::query_as(
        "SELECT status, tx_hash, block_height, confirmations FROM wallet_withdrawal_requests WHERE id = ?",
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "confirmed");
    assert_eq!(stored_tx_hash.as_deref(), Some(tx_hash.as_str()));
    assert_eq!(block_height, Some(100));
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

#[tokio::test]
async fn ambiguous_broadcast_keeps_funds_frozen_and_reconciles_by_stable_request_id()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("WA{}", &suffix[20..32]).to_ascii_uppercase();
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("wallet-ambiguous-{suffix}@example.test"))
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
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked) VALUES (?, ?, 0, 11, 0)",
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO wallet_chain_gateways
           (network, broadcast_url, withdrawal_status_url, event_poll_url, status)
           VALUES ('solana', 'http://gateway.test/broadcast', 'http://gateway.test/status',
                   'http://gateway.test/events', 'active')
           ON DUPLICATE KEY UPDATE broadcast_url = VALUES(broadcast_url),
             withdrawal_status_url = VALUES(withdrawal_status_url),
             event_poll_url = VALUES(event_poll_url), auth_token_encrypted = NULL,
             last_deposit_cursor = NULL, status = 'active'"#,
    )
    .execute(&pool)
    .await?;

    let request_id = Uuid::now_v7().to_string();
    let withdrawal_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'solana', 'recipient', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("ambiguous-confirm-{suffix}"))
    .bind(&request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let gateway = AmbiguousGateway {
        request_id: request_id.clone(),
        tx_hash: format!("0x{}", Uuid::now_v7().simple()),
        broadcast_error_class: WalletChainGatewayErrorClass::Unknown,
        query_status: WalletChainWithdrawalQueryStatus::Confirmed,
        query_acceptance_evidence: true,
        query_tx_hash_evidence: true,
        broadcast_calls: AtomicUsize::new(0),
        query_calls: AtomicUsize::new(0),
    };
    let config = WalletChainWorkerConfig {
        enabled: true,
        interval_seconds: 1,
        batch_limit: 10,
        max_attempts: 3,
    };

    let first = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(first.withdrawal_retried, 1);
    let (status, available, frozen): (String, BigDecimal, BigDecimal) = sqlx::query_as(
        r#"SELECT requests.status, accounts.available, accounts.frozen
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "unknown_broadcast");
    assert_eq!(available, decimal("0"));
    assert_eq!(frozen, decimal("11"));
    assert_eq!(gateway.broadcast_calls.load(Ordering::SeqCst), 1);

    sqlx::query(
        "UPDATE wallet_withdrawal_requests SET next_attempt_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(withdrawal_id)
    .execute(&pool)
    .await?;
    let reconciled = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(reconciled.withdrawal_confirmed, 1);
    assert_eq!(gateway.broadcast_calls.load(Ordering::SeqCst), 1);
    assert_eq!(gateway.query_calls.load(Ordering::SeqCst), 1);
    let (status, frozen): (String, BigDecimal) = sqlx::query_as(
        r#"SELECT requests.status, accounts.frozen
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "confirmed");
    assert_eq!(frozen, decimal("0"));
    let confirm_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ? AND change_type = 'withdrawal_confirm'",
    )
    .bind(withdrawal_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(confirm_count, 1);

    // 另一笔始终无法查明的广播在查询预算耗尽后转人工，不会退冻。
    sqlx::query("UPDATE wallet_accounts SET frozen = 11 WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let unknown_request_id = Uuid::now_v7().to_string();
    let unknown_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'solana', 'recipient-2', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("ambiguous-manual-{suffix}"))
    .bind(&unknown_request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let unknown_gateway = AmbiguousGateway {
        request_id: unknown_request_id,
        tx_hash: "unused".to_owned(),
        broadcast_error_class: WalletChainGatewayErrorClass::Unknown,
        query_status: WalletChainWithdrawalQueryStatus::Unknown,
        query_acceptance_evidence: false,
        query_tx_hash_evidence: false,
        broadcast_calls: AtomicUsize::new(0),
        query_calls: AtomicUsize::new(0),
    };
    run_once_with_gateway(&pool, None, &unknown_gateway, config).await?;
    for _ in 0..config.max_attempts {
        sqlx::query(
            "UPDATE wallet_withdrawal_requests SET next_attempt_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
        )
        .bind(unknown_id)
        .execute(&pool)
        .await?;
        run_once_with_gateway(&pool, None, &unknown_gateway, config).await?;
    }
    let (unknown_status, available, frozen, released_at): (
        String,
        BigDecimal,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        r#"SELECT requests.status, accounts.available, accounts.frozen, requests.released_at
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(unknown_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unknown_status, "manual_review");
    assert_eq!(available, decimal("0"));
    assert_eq!(frozen, decimal("11"));
    assert!(released_at.is_none());
    assert_eq!(unknown_gateway.broadcast_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        unknown_gateway.query_calls.load(Ordering::SeqCst),
        config.max_attempts as usize
    );
    let unknown_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_withdrawal_broadcast_audits WHERE withdrawal_id = ?",
    )
    .bind(unknown_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unknown_audit_count, i64::from(config.max_attempts + 1));

    // “未受理”响应若同时携带链上证据属于自相矛盾的结果，只能转人工，不能退冻。
    sqlx::query("UPDATE wallet_accounts SET frozen = 22 WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let contradictory_request_id = Uuid::now_v7().to_string();
    let contradictory_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'solana', 'recipient-3', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("ambiguous-contradictory-{suffix}"))
    .bind(&contradictory_request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let contradictory_gateway = AmbiguousGateway {
        request_id: contradictory_request_id.clone(),
        tx_hash: format!("0x{}", Uuid::now_v7().simple()),
        broadcast_error_class: WalletChainGatewayErrorClass::Unknown,
        query_status: WalletChainWithdrawalQueryStatus::NotAccepted,
        query_acceptance_evidence: true,
        query_tx_hash_evidence: false,
        broadcast_calls: AtomicUsize::new(0),
        query_calls: AtomicUsize::new(0),
    };
    run_once_with_gateway(&pool, None, &contradictory_gateway, config).await?;
    sqlx::query(
        "UPDATE wallet_withdrawal_requests SET next_attempt_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(contradictory_id)
    .execute(&pool)
    .await?;
    let contradictory_summary =
        run_once_with_gateway(&pool, None, &contradictory_gateway, config).await?;
    assert_eq!(contradictory_summary.withdrawal_manual_review, 1);
    let (contradictory_status, available, frozen, released_at): (
        String,
        BigDecimal,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        r#"SELECT requests.status, accounts.available, accounts.frozen, requests.released_at
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(contradictory_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(contradictory_status, "manual_review");
    assert_eq!(available, decimal("0"));
    assert_eq!(frozen, decimal("22"));
    assert!(released_at.is_none());
    assert_eq!(
        contradictory_gateway.broadcast_calls.load(Ordering::SeqCst),
        1
    );
    assert_eq!(contradictory_gateway.query_calls.load(Ordering::SeqCst), 1);

    // 只观测到区块高度而没有可用 tx_hash 时，仍要永久记住“曾有受理证据”。
    let (contradictory_tx_hash, acceptance_evidence_at): (
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT tx_hash, acceptance_evidence_at FROM wallet_withdrawal_requests WHERE id = ?",
    )
    .bind(contradictory_id)
    .fetch_one(&pool)
    .await?;
    assert!(contradictory_tx_hash.is_none());
    assert!(acceptance_evidence_at.is_some());

    // 后来一条不带证据的 not_accepted 回执不得覆盖先前证据并退冻。
    let contradictory_receipt_gateway = DepositPageGateway {
        page: WalletChainPollPage {
            next_cursor: None,
            deposits: Vec::new(),
            withdrawals: vec![WalletChainWithdrawalObservation {
                request_id: contradictory_request_id.clone(),
                network: "solana".to_owned(),
                tx_hash: None,
                block_height: None,
                confirmations: 0,
                status: "not_accepted".to_owned(),
                failure_reason: Some("late contradictory receipt".to_owned()),
            }],
        },
    };
    run_once_with_gateway(&pool, None, &contradictory_receipt_gateway, config).await?;
    let (contradictory_status, available, frozen, released_at): (
        String,
        BigDecimal,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        r#"SELECT requests.status, accounts.available, accounts.frozen, requests.released_at
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(contradictory_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(contradictory_status, "manual_review");
    assert_eq!(available, decimal("0"));
    assert_eq!(frozen, decimal("22"));
    assert!(released_at.is_none());

    // 即使 accepted 查询漏了 tx_hash/区块/确认数，远端状态本身也是受理证据。
    // 必须持久化证据闸门，后到的干净 not_accepted 回执不得退冻。
    sqlx::query("UPDATE wallet_accounts SET frozen = 33 WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let status_only_request_id = Uuid::now_v7().to_string();
    let status_only_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'solana', 'recipient-status-only', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("ambiguous-status-only-{suffix}"))
    .bind(&status_only_request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let status_only_gateway = AmbiguousGateway {
        request_id: status_only_request_id.clone(),
        tx_hash: "unused".to_owned(),
        broadcast_error_class: WalletChainGatewayErrorClass::Unknown,
        query_status: WalletChainWithdrawalQueryStatus::Accepted,
        query_acceptance_evidence: false,
        query_tx_hash_evidence: false,
        broadcast_calls: AtomicUsize::new(0),
        query_calls: AtomicUsize::new(0),
    };
    run_once_with_gateway(&pool, None, &status_only_gateway, config).await?;
    sqlx::query(
        "UPDATE wallet_withdrawal_requests SET next_attempt_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(status_only_id)
    .execute(&pool)
    .await?;
    let status_only_summary =
        run_once_with_gateway(&pool, None, &status_only_gateway, config).await?;
    assert_eq!(status_only_summary.withdrawal_manual_review, 1);
    let (status_only_status, status_only_evidence): (
        String,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT status, acceptance_evidence_at FROM wallet_withdrawal_requests WHERE id = ?",
    )
    .bind(status_only_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status_only_status, "manual_review");
    assert!(status_only_evidence.is_some());
    run_once_with_gateway(
        &pool,
        None,
        &DepositPageGateway {
            page: WalletChainPollPage {
                next_cursor: None,
                deposits: Vec::new(),
                withdrawals: vec![WalletChainWithdrawalObservation {
                    request_id: status_only_request_id,
                    network: "solana".to_owned(),
                    tx_hash: None,
                    block_height: None,
                    confirmations: 0,
                    status: "not_accepted".to_owned(),
                    failure_reason: None,
                }],
            },
        },
        config,
    )
    .await?;

    // 2xx 广播结果已解析为成功结构但 tx_hash 格式损坏时，也是受理不确定性。
    // 不得把它当普通解析错误重试或释放。
    sqlx::query("UPDATE wallet_accounts SET frozen = 44 WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let invalid_success_request_id = Uuid::now_v7().to_string();
    let invalid_success_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'solana', 'recipient-invalid-success', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("ambiguous-invalid-success-{suffix}"))
    .bind(&invalid_success_request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let invalid_success_summary = run_once_with_gateway(
        &pool,
        None,
        &InvalidBroadcastSuccessGateway {
            request_id: invalid_success_request_id.clone(),
        },
        config,
    )
    .await?;
    assert_eq!(invalid_success_summary.withdrawal_manual_review, 1);
    let (invalid_success_status, invalid_success_evidence): (
        String,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT status, acceptance_evidence_at FROM wallet_withdrawal_requests WHERE id = ?",
    )
    .bind(invalid_success_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(invalid_success_status, "manual_review");
    assert!(invalid_success_evidence.is_some());

    // 干净、权威的未受理查询则应立即只释放一次，不再退回 approved 盲重播。
    sqlx::query("UPDATE wallet_accounts SET frozen = 55 WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    let rejected_request_id = Uuid::now_v7().to_string();
    let rejected_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, 'solana', 'recipient-4', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(format!("ambiguous-authoritative-rejection-{suffix}"))
    .bind(&rejected_request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let rejected_gateway = AmbiguousGateway {
        request_id: rejected_request_id.clone(),
        tx_hash: "unused".to_owned(),
        broadcast_error_class: WalletChainGatewayErrorClass::Unknown,
        query_status: WalletChainWithdrawalQueryStatus::NotAccepted,
        query_acceptance_evidence: false,
        query_tx_hash_evidence: false,
        broadcast_calls: AtomicUsize::new(0),
        query_calls: AtomicUsize::new(0),
    };
    run_once_with_gateway(&pool, None, &rejected_gateway, config).await?;
    sqlx::query(
        "UPDATE wallet_withdrawal_requests SET next_attempt_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(rejected_id)
    .execute(&pool)
    .await?;
    let rejected_summary = run_once_with_gateway(&pool, None, &rejected_gateway, config).await?;
    assert_eq!(rejected_summary.withdrawal_failed, 1);
    assert_eq!(rejected_gateway.broadcast_calls.load(Ordering::SeqCst), 1);
    assert_eq!(rejected_gateway.query_calls.load(Ordering::SeqCst), 1);
    let (rejected_status, available, frozen, released_at): (
        String,
        BigDecimal,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        r#"SELECT requests.status, accounts.available, accounts.frozen, requests.released_at
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(rejected_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(rejected_status, "failed");
    assert_eq!(available, decimal("11"));
    assert_eq!(frozen, decimal("44"));
    assert!(released_at.is_some());

    let repeated_receipts = DepositPageGateway {
        page: WalletChainPollPage {
            next_cursor: None,
            deposits: Vec::new(),
            withdrawals: vec![
                WalletChainWithdrawalObservation {
                    request_id: contradictory_request_id,
                    network: "solana".to_owned(),
                    tx_hash: None,
                    block_height: None,
                    confirmations: 0,
                    status: "not_accepted".to_owned(),
                    failure_reason: None,
                },
                WalletChainWithdrawalObservation {
                    request_id: rejected_request_id,
                    network: "solana".to_owned(),
                    tx_hash: None,
                    block_height: None,
                    confirmations: 0,
                    status: "not_accepted".to_owned(),
                    failure_reason: None,
                },
            ],
        },
    };
    run_once_with_gateway(&pool, None, &repeated_receipts, config).await?;
    let release_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_ledger WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ? AND change_type = 'withdrawal_release'",
    )
    .bind(rejected_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(release_count, 1);
    let (available, frozen): (BigDecimal, BigDecimal) = sqlx::query_as(
        "SELECT available, frozen FROM wallet_accounts WHERE user_id = ? AND asset_id = ?",
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(available, decimal("11"));
    assert_eq!(frozen, decimal("44"));

    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_withdrawal_requests WHERE id IN (?, ?, ?, ?, ?, ?)")
        .bind(withdrawal_id)
        .bind(unknown_id)
        .bind(contradictory_id)
        .bind(status_only_id)
        .bind(invalid_success_id)
        .bind(rejected_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_chain_gateways WHERE network = 'solana'")
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

#[tokio::test]
async fn deterministic_broadcast_rejection_releases_reservation_exactly_once()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool_or_skip().await? else {
        return Ok(());
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let symbol = format!("WR{}", &suffix[20..32]).to_ascii_uppercase();
    let network = format!("reject-{}", &suffix[20..32]);
    let user_id = sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(format!("wallet-rejected-{suffix}@example.test"))
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
        "INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked) VALUES (?, ?, 0, 11, 0)",
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO wallet_chain_gateways (network, broadcast_url, status)
           VALUES (?, 'http://gateway.test/broadcast', 'active')"#,
    )
    .bind(&network)
    .execute(&pool)
    .await?;

    let request_id = Uuid::now_v7().to_string();
    let withdrawal_id = sqlx::query(
        r#"INSERT INTO wallet_withdrawal_requests
           (user_id, asset_id, asset_symbol, network, address, amount, fee, total_reserved,
            status, security_method, idempotency_key, gateway_request_id)
           VALUES (?, ?, ?, ?, 'recipient', 10, 1, 11, 'approved',
                   'fund_password', ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(&symbol)
    .bind(&network)
    .bind(format!("deterministic-rejection-{suffix}"))
    .bind(&request_id)
    .execute(&pool)
    .await?
    .last_insert_id();
    let gateway = AmbiguousGateway {
        request_id,
        tx_hash: "unused".to_owned(),
        broadcast_error_class: WalletChainGatewayErrorClass::DeterministicRejected,
        query_status: WalletChainWithdrawalQueryStatus::Unknown,
        query_acceptance_evidence: false,
        query_tx_hash_evidence: false,
        broadcast_calls: AtomicUsize::new(0),
        query_calls: AtomicUsize::new(0),
    };
    let config = WalletChainWorkerConfig {
        enabled: true,
        interval_seconds: 1,
        batch_limit: 10,
        max_attempts: 3,
    };

    let first = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(first.withdrawal_failed, 1);
    assert_eq!(gateway.broadcast_calls.load(Ordering::SeqCst), 1);
    let (status, resolution, available, frozen, released_at): (
        String,
        Option<String>,
        BigDecimal,
        BigDecimal,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        r#"SELECT requests.status, requests.broadcast_resolution,
                  accounts.available, accounts.frozen, requests.released_at
           FROM wallet_withdrawal_requests requests
           INNER JOIN wallet_accounts accounts
                   ON accounts.user_id = requests.user_id AND accounts.asset_id = requests.asset_id
           WHERE requests.id = ?"#,
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "failed");
    assert_eq!(resolution.as_deref(), Some("authoritative_not_accepted"));
    assert_eq!(available, decimal("11"));
    assert_eq!(frozen, decimal("0"));
    assert!(released_at.is_some());

    let second = run_once_with_gateway(&pool, None, &gateway, config).await?;
    assert_eq!(second.withdrawal_failed, 0);
    assert_eq!(gateway.broadcast_calls.load(Ordering::SeqCst), 1);
    let release_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM wallet_ledger
           WHERE ref_type = 'wallet_withdrawal_request' AND ref_id = ?
             AND change_type = 'withdrawal_release'"#,
    )
    .bind(withdrawal_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(release_count, 1);
    let audit_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM wallet_withdrawal_broadcast_audits
           WHERE withdrawal_id = ? AND outcome_class = 'deterministic_rejected'"#,
    )
    .bind(withdrawal_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 1);

    sqlx::query("DELETE FROM wallet_ledger WHERE user_id = ? AND asset_id = ?")
        .bind(user_id)
        .bind(asset_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_withdrawal_requests WHERE id = ?")
        .bind(withdrawal_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM wallet_chain_gateways WHERE network = ?")
        .bind(&network)
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

#[derive(Debug)]
struct InvalidBroadcastSuccessGateway {
    request_id: String,
}

#[derive(Debug)]
struct AmbiguousGateway {
    request_id: String,
    tx_hash: String,
    broadcast_error_class: WalletChainGatewayErrorClass,
    query_status: WalletChainWithdrawalQueryStatus,
    query_acceptance_evidence: bool,
    query_tx_hash_evidence: bool,
    broadcast_calls: AtomicUsize,
    query_calls: AtomicUsize,
}

#[async_trait]
impl WalletChainGateway for AmbiguousGateway {
    async fn broadcast_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        command: &WalletChainBroadcastCommand,
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError> {
        assert_eq!(command.request_id, self.request_id);
        self.broadcast_calls.fetch_add(1, Ordering::SeqCst);
        Err(WalletChainGatewayError::new(
            self.broadcast_error_class,
            "simulated timeout after remote acceptance",
        ))
    }

    async fn query_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        request_id: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError> {
        assert_eq!(request_id, self.request_id);
        self.query_calls.fetch_add(1, Ordering::SeqCst);
        Ok(WalletChainWithdrawalQueryResult {
            status: self.query_status,
            tx_hash: self.query_tx_hash_evidence.then(|| self.tx_hash.clone()),
            block_height: self.query_acceptance_evidence.then_some(101),
            confirmations: if self.query_status == WalletChainWithdrawalQueryStatus::Confirmed {
                12
            } else {
                0
            },
            failure_reason: None,
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
            next_cursor: None,
            deposits: Vec::new(),
            withdrawals: Vec::new(),
        })
    }
}

#[async_trait]
impl WalletChainGateway for InvalidBroadcastSuccessGateway {
    async fn broadcast_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        command: &WalletChainBroadcastCommand,
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError> {
        assert_eq!(command.request_id, self.request_id);
        Ok(WalletChainBroadcastResult {
            tx_hash: "invalid hash with spaces".to_owned(),
            block_height: Some(202),
            confirmations: 1,
        })
    }

    async fn query_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _request_id: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError> {
        panic!("an invalid accepted response must stop automatic reconciliation")
    }

    async fn poll_chain_events(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> AppResult<WalletChainPollPage> {
        Ok(WalletChainPollPage {
            next_cursor: None,
            deposits: Vec::new(),
            withdrawals: Vec::new(),
        })
    }
}

#[async_trait]
impl WalletChainGateway for DepositPageGateway {
    async fn broadcast_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _command: &WalletChainBroadcastCommand,
    ) -> Result<WalletChainBroadcastResult, WalletChainGatewayError> {
        unreachable!("deposit page gateway does not broadcast withdrawals")
    }

    async fn query_withdrawal(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _request_id: &str,
    ) -> Result<WalletChainWithdrawalQueryResult, WalletChainGatewayError> {
        unreachable!("deposit page gateway does not query withdrawals")
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
