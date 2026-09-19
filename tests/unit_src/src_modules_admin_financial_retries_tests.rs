use super::*;
use crate::{
    modules::{
        admin::{infrastructure::FinancialRetryRecord, service::required_admin_permission},
        auth::{TokenScope, issue_token},
    },
    state::AppState,
    workers::financial_retry::{RetryOutcome, claim, finish},
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::Duration;
use futures_util::FutureExt;
use serde_json::{Value, json};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use tower::ServiceExt;

fn request(reason: &str) -> RequeueFinancialRetryRequest {
    RequeueFinancialRetryRequest {
        reason: reason.to_owned(),
        expected_version: "0".repeat(64),
    }
}

fn state() -> AppState {
    AppState::new(
        serde_json::from_value(json!({
            "app_env": "test", "database_url": "mysql://unused",
            "mongodb_uri": "mongodb://unused", "mongodb_database": "unused",
            "redis_url": "redis://unused", "rabbitmq_url": "amqp://unused",
            "jwt_secret": "financial-retries-unit-secret",
            "bitget_rest_base_url": "https://unused.test", "bitget_ws_url": "wss://unused.test",
            "htx_rest_base_url": "https://unused.test", "htx_ws_url": "wss://unused.test"
        }))
        .unwrap(),
    )
}

#[test]
fn financial_retries_permissions_are_exact_and_method_aware() {
    assert_eq!(
        required_admin_permission("GET", "/financial-reconciliation").as_deref(),
        Some("governance.financial.read")
    );
    assert_eq!(
        required_admin_permission("POST", "/financial-reconciliation").as_deref(),
        Some("admin.unmapped.write")
    );
    assert_eq!(
        required_admin_permission("GET", "/financial-reconciliation/extra").as_deref(),
        Some("admin.unmapped.read")
    );
    assert_eq!(
        required_admin_permission("POST", "/admin/api/v1/agent-commissions/7/reversal").as_deref(),
        Some("agents.commissions.settle")
    );
    for path in [
        "/agent-commissions/0/reversal",
        "/agent-commissions/01/reversal",
        "/agent-commissions/+1/reversal",
        "/agent-commissions/7/reversal/extra",
        "/agent-commissions/18446744073709551616/reversal",
    ] {
        assert_eq!(
            required_admin_permission("POST", path).as_deref(),
            Some("admin.unmapped.write")
        );
    }
    assert_eq!(
        required_admin_permission("GET", "/agent-commissions/7/reversal").as_deref(),
        Some("admin.unmapped.read")
    );
    assert_eq!(
        required_admin_permission("PATCH", "/agent-commissions/7/reversal").as_deref(),
        Some("admin.unmapped.write")
    );
    let root = "/admin/api/v1/governance/financial-retries";
    assert_eq!(
        required_admin_permission("GET", root).as_deref(),
        Some("governance.financial.read")
    );
    for kind in ["earn", "loan", "commission"] {
        assert_eq!(
            required_admin_permission("POST", &format!("{root}/{kind}/1/requeue")).as_deref(),
            Some("governance.financial.operate")
        );
    }
    for kind in ["earn", "loan", "commission", "seconds"] {
        assert_eq!(
            required_admin_permission("PATCH", &format!("{root}/{kind}/1/incident")).as_deref(),
            Some("governance.financial.operate")
        );
        assert_eq!(
            required_admin_permission("POST", &format!("{root}/{kind}/1/incident")).as_deref(),
            Some("admin.unmapped.write")
        );
    }
    assert_eq!(
        required_admin_permission("POST", &format!("{root}/seconds/1/requeue")).as_deref(),
        Some("admin.unmapped.write")
    );
    for path in [
        format!("{root}/seconds/0/incident"),
        format!("{root}/seconds/01/incident"),
        format!("{root}/seconds/1/incident/extra"),
    ] {
        assert_eq!(
            required_admin_permission("PATCH", &path).as_deref(),
            Some("admin.unmapped.write")
        );
    }
    for path in [
        format!("{root}/earn/0/requeue"),
        format!("{root}/earn/+1/requeue"),
        format!("{root}/earn/01/requeue"),
        format!("{root}/earn/18446744073709551616/requeue"),
        format!("{root}/unknown/1/requeue"),
        format!("{root}/earn/1/requeue/extra"),
        format!("{root}/earn//1/requeue"),
        format!("{root}-export"),
    ] {
        assert_eq!(
            required_admin_permission("POST", &path).as_deref(),
            Some("admin.unmapped.write"),
            "{path}"
        );
    }
    assert_eq!(
        required_admin_permission("GET", &format!("{root}/earn/1/requeue")).as_deref(),
        Some("admin.unmapped.read")
    );
    assert_eq!(
        required_admin_permission("POST", root).as_deref(),
        Some("admin.unmapped.write")
    );
}

#[tokio::test]
async fn financial_retries_validate_before_database_and_reject_extra_command_fields() {
    for query in [
        FinancialRetriesQuery {
            task_kind: Some("wallet".into()),
            ..Default::default()
        },
        FinancialRetriesQuery {
            outcome: Some("refunded".into()),
            ..Default::default()
        },
        FinancialRetriesQuery {
            limit: Some(0),
            ..Default::default()
        },
        FinancialRetriesQuery {
            limit: Some(101),
            ..Default::default()
        },
        FinancialRetriesQuery {
            offset: Some(100_001),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            get_admin_financial_retries(None, query).await,
            Err(AppError::Validation(_))
        ));
    }
    for reason in ["".to_owned(), " \n ".to_owned(), "字".repeat(501)] {
        assert!(matches!(
            requeue_admin_financial_retry(None, 1, "earn".into(), 1, request(&reason)).await,
            Err(AppError::Validation(_))
        ));
    }
    assert!(matches!(
        requeue_admin_financial_retry(None, 0, "earn".into(), 1, request("核查")).await,
        Err(AppError::Unauthorized)
    ));
    assert!(matches!(
        requeue_admin_financial_retry(None, 1, "earn".into(), 0, request("核查")).await,
        Err(AppError::Validation(_))
    ));
    assert!(
        serde_json::from_value::<RequeueFinancialRetryRequest>(
            json!({"reason":"核查","amount":"100"})
        )
        .is_err()
    );
    for value in [
        json!({"expected_version":0,"reason":"核查","owner_admin_id":null}),
        json!({"expected_version":0,"reason":"核查","due_at":null}),
        json!({"expected_version":0,"reason":"核查","owner_admin_id":null,"due_at":null,"result":"win"}),
    ] {
        assert!(serde_json::from_value::<UpdateFinancialRetryIncidentRequest>(value).is_err());
    }
    for (admin_id, kind, id, owner, due, reason) in [
        (0, "seconds", 1, None, None, "核查"),
        (1, "wallet", 1, None, None, "核查"),
        (1, "seconds", 0, None, None, "核查"),
        (1, "seconds", 1, Some(0), None, "核查"),
        (1, "seconds", 1, None, Some(i64::MAX), "核查"),
        (1, "seconds", 1, None, None, " "),
    ] {
        let result = update_admin_financial_incident(
            None,
            admin_id,
            kind.into(),
            id,
            UpdateFinancialRetryIncidentRequest {
                expected_version: 0,
                owner_admin_id: owner,
                due_at: due,
                reason: reason.into(),
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(AppError::Validation(_)) | Err(AppError::Unauthorized)
        ));
    }
    assert!(matches!(
        requeue_admin_financial_retry(None, 1, "seconds".into(), 1, request("核查")).await,
        Err(AppError::Validation(_))
    ));
    assert!(
        serde_json::from_value::<RequeueFinancialRetryRequest>(
            json!({"reason":"核查","admin_id":7})
        )
        .is_err()
    );
}

#[test]
fn financial_retries_lease_boundary_and_token_redaction() {
    let now = Utc::now();
    let mut row = FinancialRetryRecord {
        task_kind: "earn".into(),
        item_id: 1,
        attempt_count: 7,
        last_attempt_at: Some(now - Duration::minutes(5)),
        next_attempt_at: now + Duration::microseconds(1),
        outcome: "running".into(),
        lease_token: Some("secret-lease".into()),
    };
    assert_eq!(row.lease_status(now), "active");
    row.next_attempt_at = now;
    assert_eq!(row.lease_status(now), "expired");
    row.lease_token = None;
    row.next_attempt_at = now + Duration::seconds(1);
    assert_eq!(
        row.lease_status(now),
        "active",
        "inconsistent running row must fail closed"
    );
    row.outcome = "failed".into();
    assert_eq!(row.lease_status(now), "none");
    let response = serde_json::to_value(row.response(now)).unwrap();
    assert!(response.get("lease_token").is_none());
    assert_eq!(response["attempt_count"], 7);
}

#[tokio::test]
async fn financial_retries_openapi_documents_readonly_requeue_and_incident_contracts() {
    let response = crate::build_router(state())
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let doc: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let root = "/admin/api/v1/governance/financial-retries";
    for (path, method) in [
        (root.to_owned(), "get"),
        (format!("{root}/{{kind}}/{{id}}/requeue"), "post"),
        (format!("{root}/{{kind}}/{{id}}/incident"), "patch"),
    ] {
        assert!(doc["paths"][&path][method]["security"].is_array());
    }
    let schemas = &doc["components"]["schemas"];
    assert!(
        schemas["RequeueFinancialRetryRequest"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("expected_version"))
    );
    assert!(schemas["UpdateFinancialRetryIncidentRequest"]["properties"]["due_at"].is_object());
    let incident_required = schemas["UpdateFinancialRetryIncidentRequest"]["required"]
        .as_array()
        .unwrap();
    assert!(incident_required.contains(&json!("due_at")));
    assert!(incident_required.contains(&json!("owner_admin_id")));
    assert!(schemas["FinancialRetryIncident"]["properties"]["version"].is_object());
}

#[tokio::test]
async fn financial_retries_http_requires_authenticated_admin() {
    let state = state();
    let user = issue_token(&state.settings, "user:1", TokenScope::User, 900).unwrap();
    let app = crate::build_router(state);
    for (method, path) in [
        ("GET", "/admin/api/v1/governance/financial-retries"),
        (
            "POST",
            "/admin/api/v1/governance/financial-retries/earn/1/requeue",
        ),
        (
            "PATCH",
            "/admin/api/v1/governance/financial-retries/seconds/1/incident",
        ),
    ] {
        for (token, status) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(&user), StatusCode::FORBIDDEN),
        ] {
            let mut builder = Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json");
            if let Some(token) = token {
                builder = builder.header("authorization", format!("Bearer {token}"));
            }
            let response = app
                .clone()
                .oneshot(builder.body(Body::from(r#"{"reason":"核查"}"#)).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
    }
}

#[tokio::test]
async fn financial_retries_mysql_readonly_filters_audit_leases_and_money_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    // Never inherit a production/default DATABASE_URL. This opt-in creates its own local schema.
    let Ok(url) = std::env::var("FINANCIAL_RETRIES_TEST_DATABASE_URL") else {
        eprintln!(
            "SKIP financial retries MySQL: set FINANCIAL_RETRIES_TEST_DATABASE_URL to a local disposable database server"
        );
        return Ok(());
    };
    let options: MySqlConnectOptions = url.parse()?;
    assert!(
        matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1"),
        "local database only"
    );
    let admin = MySqlPoolOptions::new()
        .max_connections(2)
        .connect_with(options.clone())
        .await?;
    let name = format!("financial_retry_e05_{}", uuid::Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE DATABASE `{name}`"))
        .execute(&admin)
        .await?;
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect_with(options.database(&name))
        .await?;
    let result = std::panic::AssertUnwindSafe(exercise_mysql(&pool))
        .catch_unwind()
        .await;
    pool.close().await;
    sqlx::query(&format!("DROP DATABASE `{name}`"))
        .execute(&admin)
        .await?;
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn exercise_mysql(pool: &MySqlPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::migrate!("./migrations").run(pool).await?;
    let role = sqlx::query("INSERT INTO admin_roles(name, permissions) VALUES ('e05', JSON_ARRAY('governance.financial.read'))").execute(pool).await?.last_insert_id();
    let admin_id = sqlx::query(
        "INSERT INTO admin_users(username,password_hash,role_id) VALUES ('e05','unused',?)",
    )
    .bind(role)
    .execute(pool)
    .await?
    .last_insert_id();
    let user =
        sqlx::query("INSERT INTO users(email,password_hash) VALUES ('e05@example.test','unused')")
            .execute(pool)
            .await?
            .last_insert_id();
    let asset = sqlx::query(
        "INSERT INTO assets(symbol,name,precision_scale,status) VALUES ('E05','E05',18,'active')",
    )
    .execute(pool)
    .await?
    .last_insert_id();
    let product = sqlx::query("INSERT INTO earn_products(asset_id,name,term_days,apr_rate,min_subscribe,introduction_json) VALUES (?,'e05',1,0.1,1,JSON_OBJECT())").bind(asset).execute(pool).await?.last_insert_id();
    let item = sqlx::query("INSERT INTO earn_subscriptions(user_id,product_id,asset_id,amount,apr_rate,term_days,idempotency_key,matures_at) VALUES (?,?,?,123.123456789012345678,0.1,1,'e05',UTC_TIMESTAMP())")
        .bind(user).bind(product).bind(asset).execute(pool).await?.last_insert_id();
    sqlx::query(
        "INSERT INTO wallet_accounts(user_id,asset_id,available,frozen) VALUES (?,?,10,20)",
    )
    .bind(user)
    .bind(asset)
    .execute(pool)
    .await?;
    let old = Utc::now() - Duration::minutes(6);
    let stale = claim(pool, "earn", item, old).await?.unwrap();
    let active = claim(pool, "loan", item, Utc::now()).await?.unwrap();
    let loan_product = sqlx::query("INSERT INTO loan_products(loan_type,asset_id,name,name_json,term_days,interest_rate,min_amount) VALUES ('credit',?,'e05',JSON_OBJECT(),1,0.1,1)")
        .bind(asset).execute(pool).await?.last_insert_id();
    sqlx::query("INSERT INTO loan_orders(id,user_id,product_id,loan_type,asset_id,amount,interest_rate,interest_calculation_mode,term_days,idempotency_key,request_fingerprint) VALUES (?,?,?,'credit',?,10,0.1,'full_term',1,'e05',REPEAT('a',64))")
        .bind(item).bind(user).bind(loan_product).bind(asset).execute(pool).await?;
    let agent =
        sqlx::query("INSERT INTO agents(user_id,agent_code,path) VALUES (?,'e05','/pending')")
            .bind(user)
            .execute(pool)
            .await?
            .last_insert_id();
    sqlx::query("UPDATE agents SET root_agent_id=id,path=CONCAT('/agent:',id) WHERE id=?")
        .bind(agent)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO agent_commission_records(id,agent_id,user_id,source_type,source_id,source_amount,payout_asset_id,commission_amount) VALUES (?,?,?,'convert_order','quote-e05-opaque',100,?,0.123456789012345678)")
        .bind(item).bind(agent).bind(user).bind(asset).execute(pool).await?;
    let stale_request = current_request(pool, "earn", item, "核查").await?;
    let active_request = current_request(pool, "loan", item, "核查").await?;
    let stale_version = stale_request.expected_version.clone();
    sqlx::query("INSERT INTO financial_worker_retries(task_kind,item_id,next_attempt_at,outcome) VALUES ('commission',?,UTC_TIMESTAMP(),'failed'),('earn',?,UTC_TIMESTAMP(),'failed')")
        .bind(item).bind(item + 1).execute(pool).await?;
    let pair = sqlx::query("INSERT INTO trading_pairs(base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,market_type) VALUES (?,?,'E05/E05',2,2,1,'external')")
        .bind(asset).bind(asset).execute(pool).await?.last_insert_id();
    let seconds_product = sqlx::query("INSERT INTO seconds_contract_products(pair_id,stake_asset,duration_seconds,payout_rate,min_stake) VALUES (?,?,60,0.5,1)")
        .bind(pair).bind(asset).execute(pool).await?.last_insert_id();
    let seconds = sqlx::query("INSERT INTO seconds_contract_orders(user_id,product_id,pair_id,stake_asset,direction,stake_amount,payout_rate,status,idempotency_key,expires_at,settlement_failure_code,settlement_failed_at,settlement_window_start,settlement_window_end)
        VALUES (?,?,?,?,'up',3.123456789012345678,0.5,'manual_review','e05-seconds',UTC_TIMESTAMP(),'price_missing',UTC_TIMESTAMP(),UTC_TIMESTAMP()-INTERVAL 1 MINUTE,UTC_TIMESTAMP())")
        .bind(user).bind(seconds_product).bind(pair).bind(asset).execute(pool).await?.last_insert_id();
    let before_money = money_snapshot(pool).await?;
    let before_schedule: Value = sqlx::query_scalar("SELECT JSON_ARRAYAGG(JSON_OBJECT('kind',task_kind,'id',item_id,'attempt',attempt_count,'outcome',outcome,'lease',lease_token,'next',next_attempt_at)) FROM financial_worker_retries").fetch_one(pool).await?;
    let state = state().with_mysql(pool.clone());
    let token = issue_token(
        &state.settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = crate::build_router(state);
    let root = "/admin/api/v1/governance/financial-retries";
    let get = || {
        Request::builder()
            .uri(format!("{root}?task_kind=earn&limit=1"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    get_admin_financial_retries(
        Some(pool.clone()),
        FinancialRetriesQuery {
            task_kind: Some("earn".into()),
            limit: Some(1),
            ..Default::default()
        },
    )
    .await?;
    for _ in 0..2 {
        let response = app.clone().oneshot(get()).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
        assert_eq!(body["total"], 2);
        assert_eq!(body["retries"].as_array().unwrap().len(), 1);
        assert_eq!(body["retries"][0]["amount"], "123.123456789012345678");
        assert_eq!(body["retries"][0]["source_order_id"], item.to_string());
        assert_eq!(body["retries"][0]["user_id"], user);
        assert_eq!(body["retries"][0]["lease_status"], "expired");
    }
    let after_schedule: Value = sqlx::query_scalar("SELECT JSON_ARRAYAGG(JSON_OBJECT('kind',task_kind,'id',item_id,'attempt',attempt_count,'outcome',outcome,'lease',lease_token,'next',next_attempt_at)) FROM financial_worker_retries").fetch_one(pool).await?;
    assert_eq!(
        before_schedule, after_schedule,
        "GET must not mutate schedule"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_audit_logs")
            .fetch_one(pool)
            .await?,
        0
    );
    let filtered = get_admin_financial_retries(
        Some(pool.clone()),
        FinancialRetriesQuery {
            task_kind: Some("earn".into()),
            outcome: Some("failed".into()),
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await?;
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.counts[0].count, 1);
    assert!(filtered.retries.is_empty());
    for (kind, source, amount) in [
        ("loan", item.to_string(), "10.000000000000000000"),
        (
            "commission",
            "quote-e05-opaque".to_owned(),
            "0.123456789012345678",
        ),
    ] {
        let result = get_admin_financial_retries(
            Some(pool.clone()),
            FinancialRetriesQuery {
                task_kind: Some(kind.into()),
                ..Default::default()
            },
        )
        .await?;
        assert_eq!(result.total, 1);
        assert_eq!(
            result.retries[0].source_order_id.as_deref(),
            Some(source.as_str())
        );
        assert_eq!(result.retries[0].amount.as_deref(), Some(amount));
        assert_eq!(result.retries[0].user_id, Some(user));
    }
    let post = || {
        Request::builder()
            .method("POST")
            .uri(format!("{root}/earn/{item}/requeue"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"reason":"核对来源后重新排期","expected_version":stale_version}).to_string(),
            ))
            .unwrap()
    };
    assert_eq!(
        app.clone().oneshot(post()).await?.status(),
        StatusCode::FORBIDDEN
    );
    let metadata = Request::builder()
        .method("PATCH")
        .uri(format!("{root}/seconds/{seconds}/incident"))
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"expected_version":0,"owner_admin_id":null,"due_at":null,"reason":"核查"})
                .to_string(),
        ))?;
    assert_eq!(
        app.clone().oneshot(metadata).await?.status(),
        StatusCode::FORBIDDEN
    );
    assert!(matches!(
        requeue_admin_financial_retry(
            Some(pool.clone()),
            admin_id,
            "loan".into(),
            item,
            active_request
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    // A nonexistent audit actor must roll the schedule change back atomically.
    assert!(
        requeue_admin_financial_retry(
            Some(pool.clone()),
            u64::MAX,
            "earn".into(),
            item,
            stale_request
        )
        .await
        .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT outcome FROM financial_worker_retries WHERE task_kind='earn' AND item_id=?"
        )
        .bind(item)
        .fetch_one(pool)
        .await?,
        "running"
    );
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY('governance.financial.read','governance.financial.operate') WHERE id=?").bind(role).execute(pool).await?;
    let response = app.clone().oneshot(post()).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let receipt: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_ne!(receipt["version"], stale_version);
    assert_eq!(receipt["lease_status"], "none");
    assert_eq!(
        app.oneshot(post()).await?.status(),
        StatusCode::CONFLICT,
        "replay cannot move the new schedule"
    );
    finish(pool, stale, Utc::now(), RetryOutcome::Complete).await?;
    let (outcome, attempt, lease): (String,u64,Option<String>) = sqlx::query_as("SELECT outcome,attempt_count,lease_token FROM financial_worker_retries WHERE task_kind='earn' AND item_id=?").bind(item).fetch_one(pool).await?;
    assert_eq!((outcome.as_str(), attempt, lease), ("ready", 1, None));
    let (actor, reason, before, after): (u64,String,Value,Value) = sqlx::query_as("SELECT admin_id,reason,before_json,after_json FROM admin_audit_logs WHERE action='financial_retry.requeue'").fetch_one(pool).await?;
    assert_eq!(actor, admin_id);
    assert_eq!(reason, "核对来源后重新排期");
    assert_eq!(before["outcome"], "running");
    assert_eq!(after["outcome"], "ready");
    assert_eq!(
        receipt, after,
        "HTTP receipt is the atomically audited snapshot"
    );
    assert_eq!(before["attempt_count"], after["attempt_count"]);
    assert_eq!(before["last_attempt_at"], after["last_attempt_at"]);
    assert!(before.get("lease_token").is_none());
    assert_eq!(
        money_snapshot(pool).await?,
        before_money,
        "no business state, wallet or ledger changes"
    );
    finish(pool, active, Utc::now(), RetryOutcome::Complete).await?;
    // A worker may claim and finish between display and confirmation. Even after its
    // active lease is released, a stale command must not shorten the new backoff.
    let obsolete = current_request(pool, "earn", item, "旧页面").await?;
    let replacement = claim(pool, "earn", item, Utc::now()).await?.unwrap();
    finish(pool, replacement, Utc::now(), RetryOutcome::Failed).await?;
    assert!(matches!(
        requeue_admin_financial_retry(Some(pool.clone()), admin_id, "earn".into(), item, obsolete)
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT outcome FROM financial_worker_retries WHERE task_kind='earn' AND item_id=?"
        )
        .bind(item)
        .fetch_one(pool)
        .await?,
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_audit_logs")
            .fetch_one(pool)
            .await?,
        1
    );
    let a = current_request(pool, "commission", item, "并发操作甲").await?;
    let b = current_request(pool, "commission", item, "并发操作乙").await?;
    let (a, b) = tokio::join!(
        requeue_admin_financial_retry(Some(pool.clone()), admin_id, "commission".into(), item, a),
        requeue_admin_financial_retry(Some(pool.clone()), admin_id, "commission".into(), item, b)
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "one snapshot admits exactly one concurrent command"
    );
    assert_eq!(money_snapshot(pool).await?, before_money);
    exercise_incidents(pool, admin_id, role, seconds, item).await?;
    assert_eq!(money_snapshot(pool).await?, before_money);
    Ok(())
}

async fn exercise_incidents(
    pool: &MySqlPool,
    admin_id: u64,
    role: u64,
    seconds: u64,
    worker_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = || FinancialRetriesQuery {
        task_kind: Some("seconds".into()),
        outcome: Some("manual_review".into()),
        ..Default::default()
    };
    for _ in 0..2 {
        let response = get_admin_financial_retries(Some(pool.clone()), query()).await?;
        assert_eq!(response.total, 1);
        assert_eq!(response.counts[0].outcome, "manual_review");
        let row = &response.retries[0];
        assert_eq!(row.schedule.item_id, seconds);
        assert_eq!(row.schedule.attempt_count, None);
        assert_eq!(row.schedule.next_attempt_at, None);
        assert_eq!(row.schedule.lease_status, "none");
        assert_eq!(row.amount.as_deref(), Some("3.123456789012345678"));
        assert_eq!(row.failure_code.as_deref(), Some("price_missing"));
        assert!(row.review_window_start < row.review_window_end);
        assert_eq!(row.incident.version, 0);
        assert_eq!(row.incident.owner_admin_id, None);
        assert_eq!(row.incident.due_at, None);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM financial_retry_incidents")
            .fetch_one(pool)
            .await?,
        0,
        "GET cannot invent assignment or deadline"
    );
    let inactive = sqlx::query("INSERT INTO admin_users(username,password_hash,role_id,status) VALUES ('e05-disabled','unused',?,'disabled')").bind(role).execute(pool).await?.last_insert_id();
    let req = |version, owner, due| UpdateFinancialRetryIncidentRequest {
        expected_version: version,
        owner_admin_id: owner,
        due_at: due,
        reason: "核查跟进".into(),
    };
    for owner in [u64::MAX, inactive] {
        assert!(matches!(
            update_admin_financial_incident(
                Some(pool.clone()),
                admin_id,
                "seconds".into(),
                seconds,
                req(0, Some(owner), None)
            )
            .await,
            Err(AppError::Validation(_))
        ));
    }
    // A foreign-key/audit actor failure must not create partial metadata.
    assert!(
        update_admin_financial_incident(
            Some(pool.clone()),
            u64::MAX,
            "seconds".into(),
            seconds,
            req(0, None, None)
        )
        .await
        .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM financial_retry_incidents")
            .fetch_one(pool)
            .await?,
        0
    );
    let due = (Utc::now() - Duration::hours(1)).timestamp_millis();
    let result = update_admin_financial_incident(
        Some(pool.clone()),
        admin_id,
        "seconds".into(),
        seconds,
        req(0, Some(admin_id), Some(due)),
    )
    .await?;
    assert_eq!(result.version, 1);
    assert_eq!(result.owner_admin_id, Some(admin_id));
    assert_eq!(
        result.due_at,
        Some(due),
        "past or future deadline is the operator's explicit choice"
    );
    assert!(matches!(
        update_admin_financial_incident(
            Some(pool.clone()),
            admin_id,
            "seconds".into(),
            seconds,
            req(0, None, None)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let cleared = update_admin_financial_incident(
        Some(pool.clone()),
        admin_id,
        "seconds".into(),
        seconds,
        req(1, None, None),
    )
    .await?;
    assert_eq!(cleared.version, 2);
    assert_eq!(cleared.owner_admin_id, None);
    assert_eq!(cleared.due_at, None);
    let (a, b) = tokio::join!(
        update_admin_financial_incident(
            Some(pool.clone()),
            admin_id,
            "seconds".into(),
            seconds,
            req(2, Some(admin_id), None)
        ),
        update_admin_financial_incident(
            Some(pool.clone()),
            admin_id,
            "seconds".into(),
            seconds,
            req(2, None, Some(due))
        )
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "version gates simultaneous assignment"
    );
    assert!(matches!(
        update_admin_financial_incident(
            Some(pool.clone()),
            admin_id,
            "seconds".into(),
            seconds + 999,
            req(0, None, None)
        )
        .await,
        Err(AppError::NotFound)
    ));
    // Assignment is allowed while a worker owns its lease, but must preserve every schedule field.
    let lease = claim(pool, "loan", worker_id, Utc::now()).await?.unwrap();
    let before = current_request(pool, "loan", worker_id, "核查")
        .await?
        .expected_version;
    update_admin_financial_incident(
        Some(pool.clone()),
        admin_id,
        "loan".into(),
        worker_id,
        req(0, Some(admin_id), None),
    )
    .await?;
    assert_eq!(
        current_request(pool, "loan", worker_id, "核查")
            .await?
            .expected_version,
        before
    );
    assert!(matches!(
        requeue_admin_financial_retry(
            Some(pool.clone()),
            admin_id,
            "loan".into(),
            worker_id,
            current_request(pool, "loan", worker_id, "核查").await?
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    finish(pool, lease, Utc::now(), RetryOutcome::Complete).await?;
    let audits: Vec<(u64,String,Value,Value)> = sqlx::query_as("SELECT admin_id,reason,before_json,after_json FROM admin_audit_logs WHERE action='financial_retry.incident_update'").fetch_all(pool).await?;
    assert_eq!(audits.len(), 4);
    assert!(
        audits
            .iter()
            .all(|(actor, reason, before, after)| *actor == admin_id
                && reason == "核查跟进"
                && before["task_kind"] == after["task_kind"])
    );
    Ok(())
}

async fn current_request(
    pool: &MySqlPool,
    kind: &str,
    id: u64,
    reason: &str,
) -> Result<RequeueFinancialRetryRequest, sqlx::Error> {
    let row: FinancialRetryRecord =
        sqlx::query_as("SELECT * FROM financial_worker_retries WHERE task_kind=? AND item_id=?")
            .bind(kind)
            .bind(id)
            .fetch_one(pool)
            .await?;
    Ok(RequeueFinancialRetryRequest {
        expected_version: row.version(),
        ..request(reason)
    })
}

async fn money_snapshot(pool: &MySqlPool) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(r#"SELECT JSON_OBJECT(
      'subscriptions',(SELECT JSON_ARRAYAGG(JSON_OBJECT('id',id,'amount',CAST(amount AS CHAR),'status',status)) FROM earn_subscriptions),
      'wallet',(SELECT JSON_ARRAYAGG(JSON_OBJECT('available',CAST(available AS CHAR),'frozen',CAST(frozen AS CHAR))) FROM wallet_accounts),
      'ledger',(SELECT COUNT(*) FROM wallet_ledger),
      'loans',(SELECT JSON_ARRAYAGG(JSON_OBJECT('id',id,'amount',CAST(amount AS CHAR),'status',status)) FROM loan_orders),
      'seconds',(SELECT JSON_ARRAYAGG(JSON_OBJECT('id',id,'stake',CAST(stake_amount AS CHAR),'status',status,'result',result,'settled_at',settled_at)) FROM seconds_contract_orders),
      'platform_entries',(SELECT COUNT(*) FROM platform_financial_journal),
      'commissions',(SELECT JSON_ARRAYAGG(JSON_OBJECT('id',id,'amount',CAST(commission_amount AS CHAR),'status',status)) FROM agent_commission_records))"#).fetch_one(pool).await
}
