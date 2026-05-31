use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use exchange_api::{
    build_router,
    config::Settings,
    modules::auth::{TokenScope, issue_token},
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{error::Error, str::FromStr};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct AgentFixture {
    agent_user_id: u64,
    agent_id: u64,
    admin_user_id: u64,
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
    }
}

async fn mysql_pool() -> Option<MySqlPool> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL agent route test because DATABASE_URL is not set");
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

async fn create_user(pool: &MySqlPool, label: &str) -> u64 {
    let email = format!(
        "agent-route-{label}-{}@example.test",
        Uuid::now_v7().simple()
    );
    sqlx::query("INSERT INTO users (email, password_hash) VALUES (?, ?)")
        .bind(email)
        .bind("not-a-real-hash")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_agent(pool: &MySqlPool, label: &str) -> AgentFixture {
    let agent_user_id = create_user(pool, &format!("agent-owner-{label}")).await;
    let agent_code = format!("agent-{}-{}", label, Uuid::now_v7().simple());
    let agent_id = sqlx::query("INSERT INTO agents (user_id, agent_code) VALUES (?, ?)")
        .bind(agent_user_id)
        .bind(agent_code)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id();
    let username = format!("agent-admin-{}-{}", label, Uuid::now_v7().simple());
    let admin_user_id = sqlx::query(
        "INSERT INTO agent_admin_users (agent_id, username, password_hash) VALUES (?, ?, ?)",
    )
    .bind(agent_id)
    .bind(username)
    .bind("not-a-real-hash")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id();

    AgentFixture {
        agent_user_id,
        agent_id,
        admin_user_id,
    }
}

async fn refer_user_to_agent(pool: &MySqlPool, user_id: u64, agent_id: u64, depth: u32) {
    refer_user_with_inviter(pool, user_id, agent_id, agent_id, "agent", depth).await;
}

async fn refer_user_with_inviter(
    pool: &MySqlPool,
    user_id: u64,
    root_agent_id: u64,
    inviter_id: u64,
    inviter_type: &str,
    depth: u32,
) {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(inviter_id)
    .bind(inviter_type)
    .bind(root_agent_id)
    .bind(depth)
    .bind(format!("/{root_agent_id}/{inviter_id}/{user_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn create_unassigned_referral(pool: &MySqlPool, user_id: u64) {
    sqlx::query(
        r#"INSERT INTO user_referrals
           (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, NULL, NULL, NULL, 0, ?)"#,
    )
    .bind(user_id)
    .bind(format!("/{user_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn cleanup_agent_fixture(
    pool: &MySqlPool,
    agents: &[AgentFixture],
    team_user_ids: &[u64],
) -> Result<(), sqlx::Error> {
    for agent in agents {
        sqlx::query("DELETE FROM invite_codes WHERE owner_type = 'agent' AND owner_id = ?")
            .bind(agent.agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in team_user_ids {
        sqlx::query("DELETE FROM user_referrals WHERE user_id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents {
        sqlx::query("DELETE FROM agent_admin_users WHERE id = ?")
            .bind(agent.admin_user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents {
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(agent.agent_id)
            .execute(pool)
            .await?;
    }
    for user_id in team_user_ids {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    for agent in agents {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(agent.agent_user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn create_invite_code(pool: &MySqlPool, agent_id: u64, status: &str) -> u64 {
    let code = format!("invite-{}", Uuid::now_v7().simple());
    sqlx::query(
        r#"INSERT INTO invite_codes (owner_type, owner_id, code, usage_limit, status)
           VALUES ('agent', ?, ?, 10, ?)"#,
    )
    .bind(agent_id)
    .bind(code)
    .bind(status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

struct CommissionSeed<'a> {
    agent_id: u64,
    user_id: u64,
    source_type: &'a str,
    source_id: &'a str,
    source_amount: &'a str,
    commission_amount: &'a str,
    status: &'a str,
}

async fn create_commission_record(
    pool: &MySqlPool,
    agent_id: u64,
    user_id: u64,
    source_type: &str,
    source_amount: &str,
    commission_amount: &str,
    status: &str,
) -> u64 {
    let source_id = format!("agent-seeded-{}", Uuid::now_v7());
    create_commission_record_with_source_id(
        pool,
        CommissionSeed {
            agent_id,
            user_id,
            source_type,
            source_id: &source_id,
            source_amount,
            commission_amount,
            status,
        },
    )
    .await
}

async fn create_commission_record_with_source_id(
    pool: &MySqlPool,
    seed: CommissionSeed<'_>,
) -> u64 {
    sqlx::query(
        r#"INSERT INTO agent_commission_records
           (agent_id, user_id, source_type, source_id, source_amount, commission_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(seed.agent_id)
    .bind(seed.user_id)
    .bind(seed.source_type)
    .bind(seed.source_id)
    .bind(BigDecimal::from_str(seed.source_amount).unwrap())
    .bind(BigDecimal::from_str(seed.commission_amount).unwrap())
    .bind(seed.status)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_commission_payout_ledger(
    pool: &MySqlPool,
    user_id: u64,
    asset_id: u64,
    commission_id: u64,
    amount: &str,
    balance_after: &str,
) -> u64 {
    let amount = BigDecimal::from_str(amount).unwrap();
    let balance_after = BigDecimal::from_str(balance_after).unwrap();
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'agent_commission_payout', ?, 'available', ?, ?, 0, 0,
                   'agent_commission', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(amount)
    .bind(balance_after.clone())
    .bind(balance_after)
    .bind(commission_id.to_string())
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_asset(pool: &MySqlPool, label: &str) -> u64 {
    let symbol =
        format!("{}{}", label, &Uuid::now_v7().simple().to_string()[..8]).to_ascii_uppercase();
    sqlx::query("INSERT INTO assets (symbol, name, precision_scale) VALUES (?, ?, 18)")
        .bind(&symbol)
        .bind(&symbol)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_id()
}

async fn create_convert_pair(pool: &MySqlPool, from_asset: u64, to_asset: u64) -> u64 {
    sqlx::query(
        r#"INSERT INTO convert_pairs
           (from_asset, to_asset, pricing_mode, spread_rate, min_amount, enabled)
           VALUES (?, ?, 'fixed', 0, 1, TRUE)"#,
    )
    .bind(from_asset)
    .bind(to_asset)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_id()
}

async fn create_convert_order(
    pool: &MySqlPool,
    pair: (u64, u64, u64),
    user_id: u64,
    amounts: (&str, &str),
    status: &str,
) -> String {
    let quote_id = Uuid::now_v7().to_string();
    sqlx::query(
        r#"INSERT INTO convert_orders
           (quote_id, convert_pair_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, 2, ?)"#,
    )
    .bind(&quote_id)
    .bind(pair.0)
    .bind(user_id)
    .bind(pair.1)
    .bind(pair.2)
    .bind(BigDecimal::from_str(amounts.0).unwrap())
    .bind(BigDecimal::from_str(amounts.1).unwrap())
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    quote_id
}

#[tokio::test]
async fn agent_users_route_rejects_non_agent_scopes() -> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user_token = issue_token(&settings, "user:1", TokenScope::User, 900).unwrap();
    let admin_token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();
    let app = build_router(AppState::new(settings));

    for path in [
        "/agent/api/v1/dashboard",
        "/agent/api/v1/users",
        "/agent/api/v1/invite-codes",
        "/agent/api/v1/team-tree",
        "/agent/api/v1/commissions",
        "/agent/api/v1/convert/stats",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {user_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {admin_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    Ok(())
}

#[tokio::test]
async fn agent_convert_stats_only_summarize_authenticated_agent_team() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "convert-a").await;
    let agent_b = create_agent(&pool, "convert-b").await;
    let direct_user = create_user(&pool, "convert-direct").await;
    let nested_user = create_user(&pool, "convert-nested").await;
    let other_agent_user = create_user(&pool, "convert-other").await;
    let unassigned_user = create_user(&pool, "convert-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;
    let from_asset = create_asset(&pool, "acfsfrom").await;
    let to_asset = create_asset(&pool, "acfsoto").await;
    let pair_id = create_convert_pair(&pool, from_asset, to_asset).await;
    let direct_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        direct_user,
        ("10.000000000000000000", "20.000000000000000000"),
        "pending",
    )
    .await;
    let nested_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        nested_user,
        ("30.000000000000000000", "60.000000000000000000"),
        "completed",
    )
    .await;
    let other_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        other_agent_user,
        ("50.000000000000000000", "100.000000000000000000"),
        "completed",
    )
    .await;
    let unassigned_order = create_convert_order(
        &pool,
        (pair_id, from_asset, to_asset),
        unassigned_user,
        ("70.000000000000000000", "140.000000000000000000"),
        "pending",
    )
    .await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/convert/stats")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let stats: Value = serde_json::from_slice(&body)?;
    assert_eq!(stats["agent_id"], agent_a.agent_id);
    assert_eq!(stats["total_orders"], 2);
    assert_eq!(stats["pending_orders"], 1);
    assert_eq!(stats["completed_orders"], 1);
    assert_eq!(stats["total_from_amount"], "40.000000000000000000");
    assert_eq!(stats["total_to_amount"], "80.000000000000000000");

    for quote_id in [
        &direct_order,
        &nested_order,
        &other_order,
        &unassigned_order,
    ] {
        sqlx::query("DELETE FROM convert_orders WHERE quote_id = ?")
            .bind(quote_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM convert_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(&pool)
        .await?;
    for asset_id in [from_asset, to_asset] {
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(asset_id)
            .execute(&pool)
            .await?;
    }
    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_dashboard_only_summarizes_authenticated_agent_team() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "dashboard-a").await;
    let agent_b = create_agent(&pool, "dashboard-b").await;
    let direct_user = create_user(&pool, "dashboard-direct").await;
    let nested_user = create_user(&pool, "dashboard-nested").await;
    let other_agent_user = create_user(&pool, "dashboard-other").await;
    let unassigned_user = create_user(&pool, "dashboard-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;
    let active_invite_code = create_invite_code(&pool, agent_a.agent_id, "active").await;
    let disabled_invite_code = create_invite_code(&pool, agent_a.agent_id, "disabled").await;
    let other_invite_code = create_invite_code(&pool, agent_b.agent_id, "active").await;
    let direct_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        direct_user,
        "spot_trade",
        "100.000000000000000000",
        "5.000000000000000000",
        "pending",
    )
    .await;
    let nested_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        nested_user,
        "convert_order",
        "200.000000000000000000",
        "8.000000000000000000",
        "settled",
    )
    .await;
    let other_commission = create_commission_record(
        &pool,
        agent_b.agent_id,
        other_agent_user,
        "spot_trade",
        "300.000000000000000000",
        "15.000000000000000000",
        "pending",
    )
    .await;
    let unassigned_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        unassigned_user,
        "spot_trade",
        "400.000000000000000000",
        "20.000000000000000000",
        "pending",
    )
    .await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/dashboard")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let dashboard: Value = serde_json::from_slice(&body)?;
    assert_eq!(dashboard["agent_id"], agent_a.agent_id);
    assert_eq!(dashboard["team_user_count"], 2);
    assert_eq!(dashboard["active_invite_code_count"], 1);
    assert_eq!(dashboard["commission_record_count"], 2);
    assert_eq!(
        dashboard["pending_commission_amount"],
        "5.000000000000000000"
    );
    assert_eq!(
        dashboard["settled_commission_amount"],
        "8.000000000000000000"
    );
    assert_eq!(
        dashboard["total_commission_amount"],
        "13.000000000000000000"
    );

    for record_id in [
        direct_commission,
        nested_commission,
        other_commission,
        unassigned_commission,
    ] {
        sqlx::query("DELETE FROM agent_commission_records WHERE id = ?")
            .bind(record_id)
            .execute(&pool)
            .await?;
    }
    for code_id in [active_invite_code, disabled_invite_code, other_invite_code] {
        sqlx::query("DELETE FROM invite_codes WHERE id = ?")
            .bind(code_id)
            .execute(&pool)
            .await?;
    }
    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_commissions_only_return_authenticated_agent_team_records()
-> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "commission-a").await;
    let agent_b = create_agent(&pool, "commission-b").await;
    let direct_user = create_user(&pool, "commission-direct").await;
    let nested_user = create_user(&pool, "commission-nested").await;
    let other_agent_user = create_user(&pool, "commission-other").await;
    let unassigned_user = create_user(&pool, "commission-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let direct_source_id = format!("direct-{}", Uuid::now_v7());
    let nested_source_id = format!("nested-{}", Uuid::now_v7());
    let direct_commission = create_commission_record_with_source_id(
        &pool,
        CommissionSeed {
            agent_id: agent_a.agent_id,
            user_id: direct_user,
            source_type: "spot_trade",
            source_id: &direct_source_id,
            source_amount: "100.500000000000000000",
            commission_amount: "5.025000000000000000",
            status: "pending",
        },
    )
    .await;
    let nested_commission = create_commission_record_with_source_id(
        &pool,
        CommissionSeed {
            agent_id: agent_a.agent_id,
            user_id: nested_user,
            source_type: "convert_order",
            source_id: &nested_source_id,
            source_amount: "200.000000000000000000",
            commission_amount: "8.000000000000000000",
            status: "settled",
        },
    )
    .await;
    let payout_asset = create_asset(&pool, "acpayout").await;
    let payout_ledger = create_commission_payout_ledger(
        &pool,
        agent_a.agent_user_id,
        payout_asset,
        nested_commission,
        "8.000000000000000000",
        "18.000000000000000000",
    )
    .await;
    let other_agent_commission = create_commission_record(
        &pool,
        agent_b.agent_id,
        other_agent_user,
        "spot_trade",
        "300.000000000000000000",
        "15.000000000000000000",
        "pending",
    )
    .await;
    let unassigned_commission = create_commission_record(
        &pool,
        agent_a.agent_id,
        unassigned_user,
        "spot_trade",
        "400.000000000000000000",
        "20.000000000000000000",
        "pending",
    )
    .await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/commissions")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let commissions: Value = serde_json::from_slice(&body)?;
    assert_eq!(commissions["agent_id"], agent_a.agent_id);
    assert_eq!(commissions["total_records"], 2);
    assert_eq!(
        commissions["total_commission_amount"],
        "13.025000000000000000"
    );
    let records = commissions["commissions"].as_array().unwrap();
    let listed_ids = records
        .iter()
        .map(|record| record["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(listed_ids, vec![direct_commission, nested_commission]);
    assert_eq!(records[0]["user_id"], direct_user);
    assert_eq!(records[0]["source_type"], "spot_trade");
    assert_eq!(records[0]["source_id"], direct_source_id);
    assert_eq!(records[0]["source_amount"], "100.500000000000000000");
    assert_eq!(records[0]["commission_amount"], "5.025000000000000000");
    assert_eq!(records[0]["status"], "pending");
    assert_eq!(records[0]["depth"], 1);
    assert_eq!(records[0]["payout_ledger_id"], Value::Null);
    assert_eq!(records[0]["payout_asset_id"], Value::Null);
    assert_eq!(records[0]["payout_amount"], Value::Null);
    assert_eq!(records[0]["payout_balance_after"], Value::Null);
    assert_eq!(records[0]["payout_created_at"], Value::Null);
    assert_eq!(records[1]["user_id"], nested_user);
    assert_eq!(records[1]["source_id"], nested_source_id);
    assert_eq!(records[1]["status"], "settled");
    assert_eq!(records[1]["depth"], 2);
    assert_eq!(records[1]["payout_ledger_id"], payout_ledger);
    assert_eq!(records[1]["payout_asset_id"], payout_asset);
    assert_eq!(records[1]["payout_amount"], "8.000000000000000000");
    assert_eq!(records[1]["payout_balance_after"], "18.000000000000000000");
    assert!(records[1]["payout_created_at"].as_i64().unwrap() > 0);
    assert!(!listed_ids.contains(&other_agent_commission));
    assert!(!listed_ids.contains(&unassigned_commission));

    sqlx::query("DELETE FROM wallet_ledger WHERE id = ?")
        .bind(payout_ledger)
        .execute(&pool)
        .await?;
    for record_id in [
        direct_commission,
        nested_commission,
        other_agent_commission,
        unassigned_commission,
    ] {
        sqlx::query("DELETE FROM agent_commission_records WHERE id = ?")
            .bind(record_id)
            .execute(&pool)
            .await?;
    }
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(payout_asset)
        .execute(&pool)
        .await?;
    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_invite_codes_are_scoped_to_authenticated_agent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "invite-a").await;
    let agent_b = create_agent(&pool, "invite-b").await;
    let owned_code = create_invite_code(&pool, agent_a.agent_id, "active").await;
    let other_code = create_invite_code(&pool, agent_b.agent_id, "active").await;
    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/api/v1/invite-codes")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "usage_limit": 25 }).to_string()))
                .unwrap(),
        )
        .await?;
    let create_status = create_response.status();
    let create_body = axum::body::to_bytes(create_response.into_body(), 8192).await?;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&create_body)
    );
    let created: Value = serde_json::from_slice(&create_body)?;
    assert_eq!(created["owner_id"], agent_a.agent_id);
    assert_eq!(created["usage_limit"], 25);
    assert_eq!(created["status"], "active");
    let created_code_id = created["id"].as_u64().unwrap();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/invite-codes")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let list_status = list_response.status();
    let list_body = axum::body::to_bytes(list_response.into_body(), 8192).await?;
    assert_eq!(
        list_status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&list_body)
    );
    let invite_codes: Value = serde_json::from_slice(&list_body)?;
    let listed_ids = invite_codes["invite_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert!(listed_ids.contains(&owned_code));
    assert!(listed_ids.contains(&created_code_id));
    assert!(!listed_ids.contains(&other_code));

    let deactivate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/agent/api/v1/invite-codes/{owned_code}/status"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "disabled" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(deactivate_response.status(), StatusCode::OK);
    let (owned_status,): (String,) = sqlx::query_as("SELECT status FROM invite_codes WHERE id = ?")
        .bind(owned_code)
        .fetch_one(&pool)
        .await?;
    assert_eq!(owned_status, "disabled");

    let other_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/agent/api/v1/invite-codes/{other_code}/status"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "disabled" }).to_string()))
                .unwrap(),
        )
        .await?;
    assert_eq!(other_response.status(), StatusCode::NOT_FOUND);
    let (other_status,): (String,) = sqlx::query_as("SELECT status FROM invite_codes WHERE id = ?")
        .bind(other_code)
        .fetch_one(&pool)
        .await?;
    assert_eq!(other_status, "active");

    cleanup_agent_fixture(&pool, &[agent_a, agent_b], &[]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_team_tree_only_returns_authenticated_agent_referrals() -> Result<(), Box<dyn Error>>
{
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "tree-a").await;
    let agent_b = create_agent(&pool, "tree-b").await;
    let direct_user = create_user(&pool, "tree-direct").await;
    let nested_user = create_user(&pool, "tree-nested").await;
    let other_agent_user = create_user(&pool, "tree-other").await;
    let unassigned_user = create_user(&pool, "tree-unassigned").await;
    refer_user_to_agent(&pool, direct_user, agent_a.agent_id, 1).await;
    refer_user_with_inviter(&pool, nested_user, agent_a.agent_id, direct_user, "user", 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/team-tree")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let tree: Value = serde_json::from_slice(&body)?;
    assert_eq!(tree["root_agent_id"], agent_a.agent_id);
    let nodes = tree["nodes"].as_array().unwrap();
    let listed_ids = nodes
        .iter()
        .map(|node| node["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(listed_ids, vec![direct_user, nested_user]);
    assert_eq!(nodes[0]["direct_inviter_type"], "agent");
    assert_eq!(nodes[0]["direct_inviter_id"], agent_a.agent_id);
    assert_eq!(nodes[0]["depth"], 1);
    assert_eq!(nodes[1]["direct_inviter_type"], "user");
    assert_eq!(nodes[1]["direct_inviter_id"], direct_user);
    assert_eq!(nodes[1]["depth"], 2);
    assert!(!listed_ids.contains(&other_agent_user));
    assert!(!listed_ids.contains(&unassigned_user));

    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[direct_user, nested_user, other_agent_user, unassigned_user],
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn agent_users_route_rejects_suspended_agent_root() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent = create_agent(&pool, "suspended").await;
    let team_user = create_user(&pool, "suspended-team").await;
    refer_user_to_agent(&pool, team_user, agent.agent_id, 1).await;
    sqlx::query("UPDATE agents SET status = 'suspended' WHERE id = ?")
        .bind(agent.agent_id)
        .execute(&pool)
        .await?;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_agent_fixture(&pool, &[agent], &[team_user]).await?;
    Ok(())
}

#[tokio::test]
async fn agent_users_route_only_returns_authenticated_agent_team() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let settings = test_settings();
    let agent_a = create_agent(&pool, "a").await;
    let agent_b = create_agent(&pool, "b").await;
    let direct_team_user = create_user(&pool, "direct").await;
    let nested_team_user = create_user(&pool, "nested").await;
    let other_agent_user = create_user(&pool, "other-agent").await;
    let unassigned_user = create_user(&pool, "unassigned").await;
    refer_user_to_agent(&pool, direct_team_user, agent_a.agent_id, 1).await;
    refer_user_to_agent(&pool, nested_team_user, agent_a.agent_id, 2).await;
    refer_user_to_agent(&pool, other_agent_user, agent_b.agent_id, 1).await;
    create_unassigned_referral(&pool, unassigned_user).await;

    let token = issue_token(
        &settings,
        format!("agent:{}", agent_a.admin_user_id),
        TokenScope::Agent,
        900,
    )
    .unwrap();
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/agent/api/v1/users")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "payload: {}",
        String::from_utf8_lossy(&body)
    );
    let users: Value = serde_json::from_slice(&body)?;
    let listed_ids = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["user_id"].as_u64().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(listed_ids, vec![direct_team_user, nested_team_user]);
    assert!(!listed_ids.contains(&other_agent_user));
    assert!(!listed_ids.contains(&unassigned_user));
    assert_eq!(users["users"][0]["root_agent_id"], agent_a.agent_id);
    assert_eq!(users["users"][1]["root_agent_id"], agent_a.agent_id);

    cleanup_agent_fixture(
        &pool,
        &[agent_a, agent_b],
        &[
            direct_team_user,
            nested_team_user,
            other_agent_user,
            unassigned_user,
        ],
    )
    .await?;
    Ok(())
}
