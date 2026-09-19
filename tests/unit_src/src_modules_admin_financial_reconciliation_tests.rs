use super::*;

#[tokio::test]
async fn reconciliation_rejects_invalid_query_before_database() {
    for query in [
        FinancialReconciliationQuery {
            asset_id: Some(0),
            ..Default::default()
        },
        FinancialReconciliationQuery {
            limit: Some(0),
            ..Default::default()
        },
        FinancialReconciliationQuery {
            limit: Some(101),
            ..Default::default()
        },
        FinancialReconciliationQuery {
            offset: Some(100_001),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            get_financial_reconciliation(None, query).await,
            Err(AppError::Validation(_))
        ));
    }
    assert_eq!(
        validate(&FinancialReconciliationQuery::default()).unwrap(),
        (20, 0)
    );
}

#[tokio::test]
async fn reconciliation_mysql_evidence_is_asset_scoped_read_only_and_historically_partial()
-> Result<(), Box<dyn std::error::Error>> {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use bigdecimal::BigDecimal;
    use serde_json::{Value, json};
    use sqlx::mysql::MySqlPoolOptions;
    use std::str::FromStr;
    use tower::ServiceExt;

    let Ok(url) = std::env::var("RECONCILIATION_DATABASE_URL") else {
        eprintln!(
            "SKIP: set RECONCILIATION_DATABASE_URL to isolated hardening_reconciliation_test"
        );
        return Ok(());
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        database, "hardening_reconciliation_test",
        "never run financial fixtures in another database"
    );
    sqlx::migrate!("./migrations").run(&pool).await?;
    let suffix = &uuid::Uuid::now_v7().simple().to_string()[..20];
    let mut assets = Vec::new();
    for name in ["A", "B", "C"] {
        assets.push(sqlx::query("INSERT INTO assets (symbol, name, precision_scale) VALUES (?, 'reconciliation fixture', 2)")
            .bind(format!("RC{name}{suffix}")).execute(&pool).await?.last_insert_id());
    }
    let (a, b, empty) = (assets[0], assets[1], assets[2]);
    let mut users = Vec::new();
    for _ in 0..4 {
        users.push(
            sqlx::query("INSERT INTO users (password_hash) VALUES ('!fixture')")
                .execute(&pool)
                .await?
                .last_insert_id(),
        );
    }
    for (user, current) in [(users[0], "11"), (users[1], "9"), (users[2], "0")] {
        sqlx::query("INSERT INTO wallet_accounts (user_id, asset_id, available, frozen, locked) VALUES (?, ?, ?, 2, 3)")
            .bind(user).bind(a).bind(current).execute(&pool).await?;
    }
    let mut latest_id = 0;
    for (user, value) in [
        (users[0], "999"),
        (users[0], "10"),
        (users[1], "10"),
        (users[3], "5"),
    ] {
        latest_id = sqlx::query("INSERT INTO wallet_ledger (user_id, asset_id, change_type, amount, balance_type, balance_after, available_after, frozen_after, locked_after, ref_type, ref_id, created_at) VALUES (?, ?, 'test', 1, 'available', ?, ?, 2, 3, 'test', 'test', '2026-01-01')")
            .bind(user).bind(a).bind(value).bind(value).execute(&pool).await?.last_insert_id();
    }
    sqlx::query("INSERT INTO margin_wallet_accounts (user_id, asset_id, available, frozen, locked) VALUES (?, ?, 12, 4, 6)")
        .bind(users[0]).bind(a).execute(&pool).await?;
    sqlx::query("INSERT INTO margin_wallet_ledger (user_id, asset_id, change_type, amount, balance_type, balance_after, available_after, frozen_after, locked_after, ref_type, ref_id) VALUES (?, ?, 'test', 12, 'available', 12, 12, 4, 6, 'test', 'test')")
        .bind(users[0]).bind(a).execute(&pool).await?;
    for (key, asset, code, amount) in [
        ("one", a, "income", "1"),
        ("two", a, "expense", "-1"),
        ("cross", a, "inventory", "3"),
        ("cross", b, "inventory", "-3"),
        ("balanced", a, "positive", "2"),
        ("balanced", a, "negative", "-2"),
    ] {
        sqlx::query("INSERT INTO platform_financial_journal (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id) VALUES (?, 'test', ?, ?, ?, 'test', 'test')")
            .bind(format!("{suffix}:{key}")).bind(code).bind(asset).bind(amount).execute(&pool).await?;
    }
    let earn_product = sqlx::query("INSERT INTO earn_products (asset_id, name, introduction_json, term_days, apr_rate, min_subscribe) VALUES (?, 'fixture', JSON_OBJECT(), 30, 0.1, 1)")
        .bind(a).execute(&pool).await?.last_insert_id();
    for (status, amount) in [("subscribed", "123.45"), ("redeemed", "999")] {
        sqlx::query("INSERT INTO earn_subscriptions (user_id, product_id, asset_id, amount, apr_rate, term_days, status, idempotency_key, matures_at) VALUES (?, ?, ?, ?, 0.1, 30, ?, ?, UTC_TIMESTAMP() + INTERVAL 1 DAY)")
            .bind(users[0]).bind(earn_product).bind(a).bind(amount).bind(status)
            .bind(format!("{suffix}:{status}")).execute(&pool).await?;
    }
    let pair = sqlx::query("INSERT INTO trading_pairs (base_asset, quote_asset, symbol, price_precision, qty_precision, min_order_value, market_type) VALUES (?, ?, ?, 2, 2, 1, 'external')")
        .bind(a).bind(b).bind(format!("RCP{suffix}")).execute(&pool).await?.last_insert_id();
    let product = sqlx::query("INSERT INTO seconds_contract_products (pair_id, stake_asset, duration_seconds, payout_rate, min_stake) VALUES (?, ?, 60, 0.333, 1)")
        .bind(pair).bind(a).execute(&pool).await?.last_insert_id();
    for status in ["opened", "settled"] {
        sqlx::query("INSERT INTO seconds_contract_orders (user_id, product_id, pair_id, stake_asset, direction, stake_amount, payout_rate, status, idempotency_key, expires_at) VALUES (?, ?, ?, ?, 'up', 1, 0.333, ?, ?, UTC_TIMESTAMP() + INTERVAL 1 DAY)")
            .bind(users[0]).bind(product).bind(pair).bind(a).bind(status)
            .bind(format!("{suffix}:seconds:{status}")).execute(&pool).await?;
    }
    for status in ["pending", "open", "partially_filled", "filled", "cancelled"] {
        sqlx::query("INSERT INTO spot_orders (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, idempotency_key) VALUES (?, ?, 'buy', 'limit', 1, 4, 1, ?, ?)")
            .bind(users[0]).bind(pair).bind(status).bind(format!("{suffix}:spot:{status}"))
            .execute(&pool).await?;
    }
    let loan_product = sqlx::query("INSERT INTO loan_products (loan_type, asset_id, name, name_json, term_days, interest_rate, min_amount, initial_ltv, maintenance_ltv, liquidation_ltv) VALUES ('collateralized', ?, 'fixture', JSON_OBJECT(), 30, 0.1, 1, 0.5, 0.7, 0.8)")
        .bind(a).execute(&pool).await?.last_insert_id();
    for status in ["pending", "disbursed", "overdue", "repaid"] {
        sqlx::query("INSERT INTO loan_orders (user_id, product_id, loan_type, asset_id, amount, interest_rate, interest_calculation_mode, term_days, collateral_asset_id, collateral_amount, status, idempotency_key, request_fingerprint) VALUES (?, ?, 'collateralized', ?, 10, 0.1, 'full_term', 30, ?, 4, ?, ?, ?)")
            .bind(users[0]).bind(loan_product).bind(a).bind(b).bind(status)
            .bind(format!("{suffix}:loan:{status}")).bind("a".repeat(64)).execute(&pool).await?;
    }
    let margin_product = sqlx::query("INSERT INTO margin_products (pair_id, margin_asset, margin_modes, leverage_levels, max_leverage, min_margin, maintenance_margin_rate) VALUES (?, ?, JSON_ARRAY('isolated'), JSON_ARRAY(2), 2, 1, 0.05)")
        .bind(pair).bind(a).execute(&pool).await?.last_insert_id();
    for status in ["opened", "closed"] {
        sqlx::query("INSERT INTO margin_positions (user_id, product_id, pair_id, margin_asset, direction, margin_amount, leverage, notional_amount, interest_amount, status, idempotency_key) VALUES (?, ?, ?, ?, 'long', 5, 2, 10, 0.5, ?, ?)")
            .bind(users[0]).bind(margin_product).bind(pair).bind(a).bind(status)
            .bind(format!("{suffix}:margin:{status}")).execute(&pool).await?;
    }
    let market = sqlx::query("INSERT INTO prediction_markets (external_market_id, title, tags_json) VALUES (?, 'fixture', JSON_ARRAY())")
        .bind(format!("{suffix}:market")).execute(&pool).await?.last_insert_id();
    for (status, cap) in [("open", "0"), ("open", "2.123"), ("settled", "0")] {
        let key = format!("{suffix}:prediction:{status}:{cap}");
        sqlx::query("INSERT INTO prediction_orders (user_id, market_id, quote_id, idempotency_key, outcome, asset_id, stake_amount, fee_amount, accepted_price, shares, theoretical_payout, effective_payout_cap, status) VALUES (?, ?, ?, ?, 'yes', ?, 1, 0, 0.4, 2.5, 2.5, ?, ?)")
            .bind(users[0]).bind(market).bind(&key).bind(&key).bind(a).bind(cap).bind(status)
            .execute(&pool).await?;
    }
    for status in [
        "pending_review",
        "unknown_broadcast",
        "manual_review",
        "confirmed",
        "failed",
    ] {
        sqlx::query("INSERT INTO wallet_withdrawal_requests (user_id, asset_id, asset_symbol, address, amount, fee, total_reserved, security_method, idempotency_key, gateway_request_id, status, tx_hash, acceptance_evidence_at) VALUES (?, ?, ?, 'fixture-address', 10, 1, 11, 'totp', ?, ?, ?, ?, ?)")
            .bind(users[0]).bind(a).bind(format!("RCA{suffix}")).bind(format!("{suffix}:withdraw:{status}"))
            .bind(uuid::Uuid::now_v7().to_string()).bind(status)
            .bind((status == "confirmed").then(|| format!("fixture-{suffix}")))
            .bind((status == "confirmed").then(|| Utc::now().naive_utc()))
            .execute(&pool).await?;
    }
    let project = sqlx::query(
        "INSERT INTO new_coin_projects
        (asset_id,symbol,total_supply,remaining_supply,issue_price,unlock_type) VALUES (?,?,1000,1000,1,'immediate')",
    )
    .bind(b)
    .bind(format!("NC{suffix}"))
    .execute(&pool)
    .await?
    .last_insert_id();
    for (mode, status, quote, frozen, refunded) in [
        ("manual_distribution", "pending", "7.25", "7.25", "0"),
        ("manual_distribution", "refunded", "10", "0", "10"),
        ("legacy_instant", "pending", "10", "0", "0"),
    ] {
        sqlx::query("INSERT INTO new_coin_subscriptions
            (project_id,user_id,quote_asset,quote_amount,requested_quantity,settlement_mode,frozen_quote_amount,
             settled_quote_amount,refunded_quote_amount,status,idempotency_key)
            VALUES (?,?,?,?,10,?,?,0,?,?,?)")
            .bind(project).bind(users[0]).bind(a).bind(quote).bind(mode).bind(frozen).bind(refunded).bind(status)
            .bind(format!("{suffix}:newcoin:{mode}:{frozen}")).execute(&pool).await?;
    }
    let agent = sqlx::query("INSERT INTO agents(user_id,agent_code,path) VALUES (?,?,'')")
        .bind(users[3])
        .bind(format!("rc-{}", &suffix[..16]))
        .execute(&pool)
        .await?
        .last_insert_id();
    for (status, payout_asset) in [
        ("pending", Some(a)),
        ("settled", Some(a)),
        ("pending", None),
        ("pending", Some(b)),
    ] {
        sqlx::query(
            "INSERT INTO agent_commission_records
            (agent_id,user_id,source_type,source_id,source_amount,commission_amount,status,payout_asset_id)
            VALUES (?,?,'seconds_contract',?,10,0.75,?,?)",
        )
        .bind(agent)
        .bind(users[0])
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(status)
        .bind(payout_asset)
        .execute(&pool)
        .await?;
    }
    let read = |asset_id| {
        get_financial_reconciliation(
            Some(pool.clone()),
            FinancialReconciliationQuery {
                asset_id: Some(asset_id),
                ..Default::default()
            },
        )
    };
    let response = read(a).await?;
    let report = response.report.unwrap();
    assert_eq!(report.coverage, "partial");
    assert_eq!(report.journal.entry_count, 5);
    assert_eq!(report.journal.transaction_count, 4);
    assert_eq!(report.journal.imbalanced_transaction_count, 3);
    assert_eq!(
        report.journal_differences.len(),
        3,
        "opposite transactions cannot cancel"
    );
    assert_eq!(report.wallet_difference_count, 4);
    let spot = &report.wallets[0];
    assert_eq!(
        (
            spot.wallet_count,
            spot.mismatch_count,
            spot.missing_ledger_count,
            spot.missing_wallet_count
        ),
        (3, 2, 1, 1)
    );
    assert_eq!(
        BigDecimal::from_str(&spot.comparable_available_delta)?,
        BigDecimal::from(0)
    );
    assert_eq!(
        report.wallets[1].mismatch_count, 0,
        "margin scope never consumes spot snapshots"
    );
    assert!(
        report
            .wallet_differences
            .iter()
            .any(|row| row.ledger_id == Some(latest_id) && row.issue == "missing_wallet")
    );
    for (kind, count, amount) in [
        ("earn_principal", 1, "123.45"),
        ("seconds_stake", 1, "1"),
        ("seconds_conditional_payout", 1, "1.33"),
        ("spot_unfilled_base", 3, "9"),
        ("loan_principal_receivable", 2, "20"),
        ("margin_collateral", 1, "5"),
        ("margin_recorded_interest", 1, "0.5"),
        ("prediction_stake", 2, "2"),
        ("prediction_conditional_payout", 2, "4.62"),
        ("withdrawal_reserved", 3, "33"),
        ("new_coin_frozen_quote", 1, "7.25"),
        ("commission_pending", 1, "0.75"),
    ] {
        let row = report
            .obligations
            .iter()
            .find(|row| row.kind == kind)
            .unwrap();
        assert_eq!(row.record_count, count, "{kind}");
        assert_eq!(
            BigDecimal::from_str(&row.amount)?,
            BigDecimal::from_str(amount)?,
            "{kind}"
        );
    }
    let other = read(b).await?.report.unwrap();
    assert_eq!(
        other.journal.imbalanced_transaction_count, 1,
        "same key across assets never balances"
    );
    assert_eq!(other.wallets[0].wallet_count, 0);
    let collateral = other
        .obligations
        .iter()
        .find(|row| row.kind == "loan_collateral")
        .unwrap();
    assert_eq!(collateral.record_count, 3);
    assert_eq!(
        BigDecimal::from_str(&collateral.amount)?,
        BigDecimal::from(12)
    );
    let empty_report = read(empty).await?.report.unwrap();
    assert_eq!(empty_report.journal.entry_count, 0);
    assert_eq!(empty_report.journal.first_entry_at, None);
    assert_eq!(empty_report.coverage, "partial");
    assert_eq!(empty_report.obligations.len(), 13);
    assert!(
        empty_report
            .obligations
            .iter()
            .all(|row| row.record_count == 0)
    );
    assert!(
        get_financial_reconciliation(Some(pool.clone()), FinancialReconciliationQuery::default())
            .await?
            .report
            .is_none()
    );
    let again = read(a).await?.report.unwrap();
    assert_eq!(
        serde_json::to_value(&report)?,
        serde_json::to_value(&again)?,
        "repeat reads do not mutate financial evidence"
    );
    let mut connection = pool.acquire().await?;
    store::prepare_snapshot(&mut connection).await?;
    let mut snapshot = connection
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let before = store::report(&mut snapshot, a).await?;
    sqlx::query("UPDATE wallet_accounts SET available=available+1 WHERE user_id=? AND asset_id=?")
        .bind(users[0])
        .bind(a)
        .execute(&pool)
        .await?;
    let during = store::report(&mut snapshot, a).await?;
    assert_eq!(
        serde_json::to_value(before)?,
        serde_json::to_value(during)?,
        "concurrent commit cannot split one report snapshot"
    );
    let forbidden = sqlx::query("UPDATE wallet_accounts SET available=available WHERE asset_id=?")
        .bind(a)
        .execute(&mut *snapshot)
        .await;
    assert!(
        forbidden.is_err(),
        "MySQL enforces the read-only transaction"
    );
    snapshot.rollback().await?;

    let settings: crate::config::Settings = serde_json::from_value(json!({
        "app_env":"test", "database_url":url, "mongodb_uri":"mongodb://127.0.0.1:27017",
        "mongodb_database":"unused", "redis_url":"redis://127.0.0.1:6379",
        "rabbitmq_url":"amqp://127.0.0.1", "jwt_secret":"reconciliation-test-secret",
        "bitget_rest_base_url":"http://127.0.0.1", "bitget_ws_url":"ws://127.0.0.1",
        "htx_rest_base_url":"http://127.0.0.1", "htx_ws_url":"ws://127.0.0.1"
    }))?;
    let role = sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('governance.financial.read'))")
        .bind(format!("rc-{suffix}")).execute(&pool).await?.last_insert_id();
    let admin = sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, '!fixture', ?)",
    )
    .bind(format!("rc-{suffix}"))
    .bind(role)
    .execute(&pool)
    .await?
    .last_insert_id();
    let token = crate::modules::auth::issue_token(
        &settings,
        format!("admin:{admin}"),
        crate::modules::auth::TokenScope::Admin,
        900,
    )?;
    let app = crate::build_router(crate::state::AppState::new(settings).with_mysql(pool.clone()));
    let request = |method: &str, path: &str, auth: bool| {
        let mut builder = Request::builder().method(method).uri(path);
        if auth {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        builder.body(Body::empty()).unwrap()
    };
    let path = format!("/admin/api/v1/financial-reconciliation?asset_id={a}");
    let result = app.clone().oneshot(request("GET", &path, true)).await?;
    assert_eq!(result.status(), 200);
    let body: Value = serde_json::from_slice(&to_bytes(result.into_body(), usize::MAX).await?)?;
    assert_eq!(body["report"]["coverage"], "partial");
    assert_eq!(
        app.clone()
            .oneshot(request("GET", &path, false))
            .await?
            .status(),
        401
    );
    assert_eq!(
        app.clone()
            .oneshot(request("POST", &path, true))
            .await?
            .status(),
        403,
        "read-only role cannot inherit a write permission on this endpoint"
    );
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY() WHERE id=?")
        .bind(role)
        .execute(&pool)
        .await?;
    assert_eq!(
        app.oneshot(request("GET", &path, true)).await?.status(),
        403
    );
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id=?")
        .bind(admin)
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        audits, 0,
        "readonly reporting has no audit/mutation side effects"
    );

    // A large discrepancy set must stay bounded without hiding its actual count.
    for index in 0..105 {
        sqlx::query("INSERT INTO platform_financial_journal (transaction_key, context, account_code, asset_id, amount, ref_type, ref_id) VALUES (?, 'test', 'test', ?, 1, 'test', 'test')")
            .bind(format!("{suffix}:bounded:{index}")).bind(empty).execute(&pool).await?;
    }
    let bounded = read(empty).await?.report.unwrap();
    assert_eq!(bounded.journal.imbalanced_transaction_count, 105);
    assert_eq!(bounded.journal_differences.len(), 100);
    Ok(())
}
