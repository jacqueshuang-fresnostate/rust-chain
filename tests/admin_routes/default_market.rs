use super::*;

fn request(method: &str, uri: &str, token: Option<&str>, payload: Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    builder.body(Body::from(payload.to_string())).unwrap()
}

fn config(version: u32, enabled: bool, initial_price: Value) -> Value {
    json!({"expected_version":version,"enabled":enabled,"initial_price":initial_price,"config":{},"reason":"配置常驻行情"})
}

struct Fixture {
    pool: MySqlPool,
    app: axum::Router,
    token: String,
    role: u64,
    admin: u64,
    pair: u64,
    base: u64,
    quote: u64,
    uri: String,
}

impl Fixture {
    async fn new() -> Result<Option<Self>, Box<dyn Error>> {
        let Some(pool) = mysql_pool().await else {
            return Ok(None);
        };
        let settings = test_settings();
        let (role, admin) = create_admin_user(&pool).await;
        let token = issue_token(&settings, format!("admin:{admin}"), TokenScope::Admin, 900)?;
        // UUIDv7 的前十位主要是时间；使用随机尾部隔离同秒并发和失败后重跑的样本。
        let suffix = Uuid::now_v7().simple().to_string();
        let base = create_asset(&pool, &format!("DMB{}", &suffix[16..24])).await;
        let quote = create_asset(&pool, &format!("DMQ{}", &suffix[24..32])).await;
        let symbol = format!("DM{}-USDT", &Uuid::now_v7().simple().to_string()[12..]);
        let pair = sqlx::query("INSERT INTO trading_pairs (base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES (?,?,?,4,4,1,'active','strategy')")
            .bind(base).bind(quote).bind(symbol).execute(&pool).await?.last_insert_id();
        let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
        Ok(Some(Self {
            pool,
            app,
            token,
            role,
            admin,
            pair,
            base,
            quote,
            uri: format!("/admin/api/v1/market-pairs/{pair}/default-generator"),
        }))
    }
    async fn call(&self, method: &str, suffix: &str, payload: Value) -> (StatusCode, Value) {
        let response = self
            .app
            .clone()
            .oneshot(request(
                method,
                &format!("{}{suffix}", self.uri),
                Some(&self.token),
                payload,
            ))
            .await
            .unwrap();
        (response.status(), body_json(response).await.unwrap())
    }
    async fn counts(&self) -> (i64, i64, i64) {
        sqlx::query_as("SELECT (SELECT COUNT(*) FROM market_default_generators WHERE pair_id=?),(SELECT COUNT(*) FROM market_default_generator_versions WHERE pair_id=?),(SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id=? AND target_type='default_market')")
            .bind(self.pair).bind(self.pair).bind(self.admin).fetch_one(&self.pool).await.unwrap()
    }
    async fn cleanup(self) {
        sqlx::query("DELETE FROM market_price_ticks WHERE symbol = REPLACE((SELECT symbol FROM trading_pairs WHERE id=?), '-', '')")
            .bind(self.pair).execute(&self.pool).await.unwrap();

        for table in [
            "market_pair_generation_runs",
            "market_default_generator_versions",
            "market_default_generators",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE pair_id=?"))
                .bind(self.pair)
                .execute(&self.pool)
                .await
                .unwrap();
        }
        sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id=?")
            .bind(self.admin)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM trading_pairs WHERE id=?")
            .bind(self.pair)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM assets WHERE id IN (?,?)")
            .bind(self.base)
            .bind(self.quote)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM admin_users WHERE id=?")
            .bind(self.admin)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM admin_roles WHERE id=?")
            .bind(self.role)
            .execute(&self.pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn default_market_routes_require_admin_scope_and_explicit_enable_field()
-> Result<(), Box<dyn Error>> {
    let settings = test_settings();
    let user = issue_token(&settings, "user:1", TokenScope::User, 900)?;
    let admin = issue_token(&settings, "admin:1", TokenScope::Admin, 900)?;
    let app = build_router(AppState::new(settings));
    for (method, suffix, payload) in [
        ("GET", "", Value::Null),
        ("PATCH", "", config(0, false, Value::Null)),
        (
            "POST",
            "/preview",
            json!({"expected_version":0,"config":{}}),
        ),
        (
            "PATCH",
            "/pause-all",
            json!({"expected_version":0,"all_market_paused":true,"reason":"暂停"}),
        ),
    ] {
        let uri = format!("/admin/api/v1/market-pairs/1/default-generator{suffix}");
        assert_eq!(
            app.clone()
                .oneshot(request(method, &uri, None, payload.clone()))
                .await?
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.clone()
                .oneshot(request(method, &uri, Some(&user), payload))
                .await?
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let response = app
        .oneshot(request(
            "PATCH",
            "/admin/api/v1/market-pairs/1/default-generator",
            Some(&admin),
            json!({"expected_version":0,"config":{},"reason":"保存"}),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    Ok(())
}

#[tokio::test]
async fn default_market_read_preview_save_and_conflict_are_versioned_without_implicit_enable()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let (status, initial) = f.call("GET", "", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{initial}");
    assert_eq!(initial["configured"], false);
    assert_eq!(initial["version"], 0);
    assert_eq!(initial["enabled"], false);
    assert_eq!(initial["runtime"]["active_source"], "none");
    assert_eq!(f.counts().await, (0, 0, 0));
    let (status, preview) = f
        .call(
            "POST",
            "/preview",
            json!({"expected_version":0,"initial_price":"1.2345","config":{}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let samples = preview["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 60);
    for rows in samples.windows(2) {
        assert_eq!(
            decimal(rows[0]["close"].as_str().unwrap()),
            decimal(rows[1]["open"].as_str().unwrap())
        );
    }
    assert_eq!(f.counts().await, (0, 0, 0));
    let (status, saved) = f.call("PATCH", "", config(0, false, json!("1.2345"))).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["enabled"], false);
    assert_eq!(saved["version"], 1);
    assert_eq!(saved["seed"], preview["seed"]);
    assert_eq!(f.counts().await, (1, 1, 1));
    let runtime_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM market_pair_generation_runs WHERE pair_id=?")
            .bind(f.pair)
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(runtime_count, 0);
    let (status, _) = f.call("PATCH", "", config(0, true, json!("2"))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(f.counts().await, (1, 1, 1));
    let (status, enabled) = f.call("PATCH", "", config(1, true, json!("1.2345"))).await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    assert_eq!(enabled["enabled"], true);
    assert_eq!(f.counts().await, (1, 2, 2));
    let old: SqlxJson<Value> = sqlx::query_scalar(
        "SELECT config_json FROM market_default_generator_versions WHERE pair_id=? AND version=1",
    )
    .bind(f.pair)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(old.0, saved["config"]);
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_initial_precision_limits_and_no_bootstrap_are_rejected_without_writes()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    for price in [
        Value::Null,
        json!("0"),
        json!("-1"),
        json!("1.23456"),
        json!("100000000000000000000"),
    ] {
        let (status, body) = f.call("PATCH", "", config(0, true, price)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(f.counts().await, (0, 0, 0));
    }
    for params in [
        json!({"volatility":"-0.1"}),
        json!({"price_min":"1.00001"}),
        json!({"volume_min":"3","volume_max":"2"}),
        json!({"depth_levels":0}),
    ] {
        let mut payload = config(0, false, json!("1"));
        payload["config"] = params;
        let (status, body) = f.call("PATCH", "", payload).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(f.counts().await, (0, 0, 0));
    }
    let (status, saved) = f.call("PATCH", "", config(0, false, Value::Null)).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["initial_price"], Value::Null);
    assert_eq!(saved["enabled"], false);
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_pause_all_preserves_price_pair_status_and_never_enables_default()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    sqlx::query("INSERT INTO market_pair_generation_runs (pair_id,generation,active_source,last_price,last_tick_at,state_json,lease_owner,lease_expires_at) VALUES (?,7,'strategy',1.2345,UTC_TIMESTAMP(6),JSON_OBJECT('checkpoint','preserved'),'old',DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 30 SECOND))").bind(f.pair).execute(&f.pool).await?;
    let (status, paused) = f
        .call(
            "PATCH",
            "/pause-all",
            json!({"expected_version":0,"all_market_paused":true,"reason":"暂停全部"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{paused}");
    assert_eq!(paused["all_market_paused"], true);
    assert_eq!(paused["enabled"], false);
    assert_eq!(paused["runtime"]["generation"], 8);
    assert_eq!(paused["runtime"]["active_source"], "none");
    assert_eq!(
        decimal(paused["runtime"]["last_price"].as_str().unwrap()),
        decimal("1.2345")
    );
    let row:(String,Option<String>,SqlxJson<Value>)=sqlx::query_as("SELECT p.status,r.lease_owner,r.state_json FROM trading_pairs p JOIN market_pair_generation_runs r ON r.pair_id=p.id WHERE p.id=?").bind(f.pair).fetch_one(&f.pool).await?;
    assert_eq!(row.0, "active");
    assert_eq!(row.1, None);
    assert_eq!(row.2.0, json!({"checkpoint":"preserved"}));
    let (status, _) = f
        .call(
            "PATCH",
            "/pause-all",
            json!({"expected_version":0,"all_market_paused":false,"reason":"旧页面恢复"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, resumed) = f
        .call(
            "PATCH",
            "/pause-all",
            json!({"expected_version":1,"all_market_paused":false,"reason":"恢复全部"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resumed}");
    assert_eq!(resumed["enabled"], false);
    assert_eq!(resumed["all_market_paused"], false);
    let (status, preview) = f
        .call(
            "POST",
            "/preview",
            json!({"expected_version":2,"initial_price":"8","config":{}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        decimal(preview["start_price"].as_str().unwrap()),
        decimal("1.2345")
    );
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_permissions_cover_read_save_preview_and_pause_all()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY('market.strategies') WHERE id=?")
        .bind(f.role)
        .execute(&f.pool)
        .await?;
    for (method, suffix, payload) in [
        ("GET", "", Value::Null),
        ("PATCH", "", config(0, false, json!("1"))),
        (
            "POST",
            "/preview",
            json!({"expected_version":0,"initial_price":"1","config":{}}),
        ),
        (
            "PATCH",
            "/pause-all",
            json!({"expected_version":0,"all_market_paused":true,"reason":"暂停"}),
        ),
    ] {
        let (status, body) = f.call(method, suffix, payload).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    assert_eq!(f.counts().await, (0, 0, 0));
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_audit_failure_rolls_back_config_versions_and_pause_generation()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let (status, body) = f.call("PATCH", "", config(0, false, json!("1"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query("INSERT INTO market_pair_generation_runs (pair_id,generation,active_source,last_price) VALUES (?,9,'default',1)").bind(f.pair).execute(&f.pool).await?;
    let trigger = format!("default_market_audit_failure_{}", f.admin);
    sqlx::raw_sql(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON admin_audit_logs FOR EACH ROW BEGIN IF NEW.admin_id = {} AND NEW.target_type='default_market' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'default market audit fixture'; END IF; END",f.admin)).execute(&f.pool).await?;
    let pause = f
        .call(
            "PATCH",
            "/pause-all",
            json!({"expected_version":1,"all_market_paused":true,"reason":"应回滚暂停"}),
        )
        .await;
    let save = f.call("PATCH", "", config(1, true, json!("2"))).await;
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
        .execute(&f.pool)
        .await?;
    assert_eq!(pause.0, StatusCode::INTERNAL_SERVER_ERROR, "{}", pause.1);
    assert_eq!(save.0, StatusCode::INTERNAL_SERVER_ERROR, "{}", save.1);
    assert_eq!(f.counts().await, (1, 1, 1));
    let (_, saved) = f.call("GET", "", Value::Null).await;
    assert_eq!(saved["enabled"], false);
    assert_eq!(saved["all_market_paused"], false);
    assert_eq!(saved["runtime"]["generation"], 9);
    assert_eq!(saved["runtime"]["active_source"], "default");
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_concurrent_create_has_one_version_and_one_audit()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let (first, second) = tokio::join!(
        f.call("PATCH", "", config(0, false, json!("1"))),
        f.call("PATCH", "", config(0, false, json!("2")))
    );
    let mut statuses = vec![first.0, second.0];
    statuses.sort();
    assert_eq!(
        statuses,
        vec![StatusCode::OK, StatusCode::CONFLICT],
        "{first:?} {second:?}"
    );
    assert_eq!(f.counts().await, (1, 1, 1));
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_pause_waits_for_inflight_owner_before_mutating_state()
-> Result<(), Box<dyn Error>> {
    use exchange_api::modules::market::infrastructure::default_runtime::PairGenerationLock;
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    {
        let owner = PairGenerationLock::acquire(&f.pool, f.pair, 0)
            .await?
            .unwrap();
        let pending = f.call(
            "PATCH",
            "/pause-all",
            json!({"expected_version":0,"all_market_paused":true,"reason":"排空旧推送后暂停"}),
        );
        tokio::pin!(pending);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut pending)
                .await
                .is_err()
        );
        assert_eq!(f.counts().await, (0, 0, 0));
        owner.release().await?;
        let (status, body) = pending.await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["all_market_paused"], true);
    }
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_bootstrap_uses_normalized_committed_history_and_ignores_future_ticks()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    for (price, seconds) in [("1.1111", -60), ("1.2345", -5), ("9.9999", 3600)] {
        let event_key = Uuid::now_v7().simple().to_string().repeat(2);
        sqlx::query("INSERT INTO market_price_ticks (event_key,symbol,price,source,observed_at,generation,source_version) SELECT ?,REPLACE(symbol,'-',''),?,'default',TIMESTAMPADD(SECOND,?,UTC_TIMESTAMP(6)),1,'default:fixture' FROM trading_pairs WHERE id=?")
            .bind(event_key).bind(decimal(price)).bind(seconds).bind(f.pair).execute(&f.pool).await?;
    }
    let (status, preview) = f
        .call(
            "POST",
            "/preview",
            json!({"expected_version":0,"initial_price":"8","config":{}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        decimal(preview["start_price"].as_str().unwrap()),
        decimal("1.2345")
    );
    let (status, saved) = f.call("PATCH", "", config(0, true, Value::Null)).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["initial_price"], Value::Null);
    assert_eq!(saved["enabled"], true);
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_seconds_capability_requires_current_owned_archived_generation()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let (status, body) = f.call("PATCH", "", config(0, true, json!("1"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query("INSERT INTO market_pair_generation_runs (pair_id,generation,active_source,default_version,lease_owner,lease_expires_at,last_price,last_tick_at) VALUES (?,5,'default',1,'fixture-owner',DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 1 HOUR),1,CURRENT_TIMESTAMP(6))")
        .bind(f.pair).execute(&f.pool).await?;
    let source_version = format!("default:{}:g5:v1", f.pair);
    let tick=sqlx::query("INSERT INTO market_price_ticks (event_key,symbol,price,source,observed_at,generation,source_version) SELECT ?,REPLACE(UPPER(symbol),'-',''),1,'default',CURRENT_TIMESTAMP(6),5,? FROM trading_pairs WHERE id=?")
        .bind(Uuid::now_v7().simple().to_string().repeat(2)).bind(&source_version).bind(f.pair).execute(&f.pool).await?.last_insert_id();
    let product=sqlx::query("INSERT INTO seconds_contract_products (pair_id,stake_asset,duration_seconds,payout_rate,min_stake,max_stake,status) VALUES (?,?,60,0.8,5,100,'disabled')")
        .bind(f.pair).bind(f.quote).execute(&f.pool).await?.last_insert_id();
    let uri = format!("/admin/api/v1/seconds-contracts/products/{product}/status");
    let activate = || {
        request(
            "PATCH",
            &uri,
            Some(&f.token),
            json!({"status":"active","reason":"验证默认行情结算能力"}),
        )
    };
    let response = f.app.clone().oneshot(activate()).await?;
    let status = response.status();
    let body = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    for (table, assignment, key) in [
        (
            "market_pair_generation_runs",
            "error_message='generation failed'",
            f.pair,
        ),
        ("market_default_generators", "enabled=FALSE", f.pair),
        (
            "market_default_generators",
            "all_market_paused=TRUE",
            f.pair,
        ),
        (
            "market_pair_generation_runs",
            "active_source='none'",
            f.pair,
        ),
        ("market_pair_generation_runs", "lease_owner=''", f.pair),
        (
            "market_pair_generation_runs",
            "lease_expires_at=DATE_SUB(CURRENT_TIMESTAMP(6),INTERVAL 1 SECOND)",
            f.pair,
        ),
        ("market_pair_generation_runs", "default_version=2", f.pair),
        (
            "market_pair_generation_runs",
            "last_tick_at=DATE_SUB(CURRENT_TIMESTAMP(6),INTERVAL 61 SECOND)",
            f.pair,
        ),
        (
            "market_pair_generation_runs",
            "last_tick_at=DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 60 SECOND)",
            f.pair,
        ),
        ("market_price_ticks", "generation=6", tick),
        (
            "market_price_ticks",
            "source_version='default:wrong-version'",
            tick,
        ),
        (
            "market_price_ticks",
            "observed_at=DATE_SUB(CURRENT_TIMESTAMP(6),INTERVAL 61 SECOND)",
            tick,
        ),
        (
            "market_price_ticks",
            "observed_at=DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 60 SECOND)",
            tick,
        ),
    ] {
        sqlx::query("UPDATE seconds_contract_products SET status='disabled' WHERE id=?")
            .bind(product)
            .execute(&f.pool)
            .await?;
        sqlx::query("UPDATE market_default_generators SET enabled=TRUE,all_market_paused=FALSE WHERE pair_id=?").bind(f.pair).execute(&f.pool).await?;
        sqlx::query("UPDATE market_pair_generation_runs SET active_source='default',error_message=NULL,lease_owner='fixture-owner',lease_expires_at=DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 1 HOUR),default_version=1,last_tick_at=CURRENT_TIMESTAMP(6) WHERE pair_id=?").bind(f.pair).execute(&f.pool).await?;
        sqlx::query("UPDATE market_price_ticks SET generation=5,source_version=?,observed_at=CURRENT_TIMESTAMP(6) WHERE id=?").bind(&source_version).bind(tick).execute(&f.pool).await?;
        let key_column = if table == "market_price_ticks" {
            "id"
        } else {
            "pair_id"
        };
        sqlx::query(&format!(
            "UPDATE {table} SET {assignment} WHERE {key_column}=?"
        ))
        .bind(key)
        .execute(&f.pool)
        .await?;
        let response = f.app.clone().oneshot(activate()).await?;
        let status = response.status();
        let body = body_json(response).await?;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{table} {assignment}: {body}"
        );
        let actual: String =
            sqlx::query_scalar("SELECT status FROM seconds_contract_products WHERE id=?")
                .bind(product)
                .fetch_one(&f.pool)
                .await?;
        assert_eq!(actual, "disabled");
    }
    // 人工策略沿用原能力标准，但交易对“全部暂停”必须覆盖它，不能由人工来源绕过。
    let strategy = sqlx::query("INSERT INTO market_strategies (pair_id,strategy_type,start_price,target_price,start_time,end_time,volatility,volume_min,volume_max,status) VALUES (?,'price_path',1,1,DATE_SUB(CURRENT_TIMESTAMP(6),INTERVAL 1 HOUR),DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 1 HOUR),0.01,1,10,'active')")
        .bind(f.pair).execute(&f.pool).await?.last_insert_id();
    sqlx::query("INSERT INTO strategy_versions (strategy_id,version,effective_time,config_json,seed) VALUES (?,1,CURRENT_TIMESTAMP(6),JSON_OBJECT(),'manual-capability-fixture')")
        .bind(strategy).execute(&f.pool).await?;
    sqlx::query("INSERT INTO strategy_runs (strategy_id,active_version,run_status,lease_owner,lease_expires_at) VALUES (?,1,'running','manual-fixture',DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 1 HOUR))")
        .bind(strategy).execute(&f.pool).await?;
    sqlx::query("UPDATE market_default_generators SET all_market_paused=TRUE WHERE pair_id=?")
        .bind(f.pair)
        .execute(&f.pool)
        .await?;
    let response = f.app.clone().oneshot(activate()).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    sqlx::query("UPDATE market_default_generators SET all_market_paused=FALSE WHERE pair_id=?")
        .bind(f.pair)
        .execute(&f.pool)
        .await?;
    let response = f.app.clone().oneshot(activate()).await?;
    let status = response.status();
    let body = body_json(response).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query("DELETE FROM strategy_runs WHERE strategy_id=?")
        .bind(strategy)
        .execute(&f.pool)
        .await?;
    sqlx::query("DELETE FROM strategy_versions WHERE strategy_id=?")
        .bind(strategy)
        .execute(&f.pool)
        .await?;
    sqlx::query("DELETE FROM market_strategies WHERE id=?")
        .bind(strategy)
        .execute(&f.pool)
        .await?;
    sqlx::query("DELETE FROM seconds_contract_product_cycles WHERE product_id=?")
        .bind(product)
        .execute(&f.pool)
        .await?;
    sqlx::query("DELETE FROM seconds_contract_products WHERE id=?")
        .bind(product)
        .execute(&f.pool)
        .await?;
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_preview_and_enable_prefer_newer_archive_to_older_checkpoint()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    sqlx::query("INSERT INTO market_pair_generation_runs(pair_id,generation,active_source,last_price,last_tick_at) VALUES(?,1,'default',1,DATE_SUB(CURRENT_TIMESTAMP(6),INTERVAL 30 SECOND))")
        .bind(f.pair).execute(&f.pool).await?;
    sqlx::query("INSERT INTO market_price_ticks(event_key,symbol,price,source,observed_at,generation,source_version) SELECT ?,REPLACE(UPPER(symbol),'-',''),2,'default',DATE_SUB(CURRENT_TIMESTAMP(6),INTERVAL 5 SECOND),1,'default:preview-current' FROM trading_pairs WHERE id=?")
        .bind(Uuid::now_v7().simple().to_string().repeat(2)).bind(f.pair).execute(&f.pool).await?;
    let parameters = json!({"price_min":"1.5","price_max":"2.5"});
    let (status, preview) = f
        .call(
            "POST",
            "/preview",
            json!({"expected_version":0,"initial_price":null,"config":parameters}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        decimal(preview["start_price"].as_str().unwrap()),
        decimal("2")
    );
    let mut save = config(0, true, Value::Null);
    save["config"] = parameters;
    let (status, saved) = f.call("PATCH", "", save).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["enabled"], true);
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_preview_ignores_future_or_undated_checkpoint() -> Result<(), Box<dyn Error>>
{
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    sqlx::query("INSERT INTO market_pair_generation_runs(pair_id,generation,active_source,last_price,last_tick_at) VALUES(?,1,'default',9,DATE_ADD(CURRENT_TIMESTAMP(6),INTERVAL 1 HOUR))")
        .bind(f.pair).execute(&f.pool).await?;
    for undated in [false, true] {
        if undated {
            sqlx::query("UPDATE market_pair_generation_runs SET last_tick_at=NULL WHERE pair_id=?")
                .bind(f.pair)
                .execute(&f.pool)
                .await?;
        }
        let (status, preview) = f
            .call(
                "POST",
                "/preview",
                json!({"expected_version":0,"initial_price":"1","config":{"price_max":"2"}}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(
            decimal(preview["start_price"].as_str().unwrap()),
            decimal("1")
        );
        let (status, body) = f
            .call(
                "POST",
                "/preview",
                json!({"expected_version":0,"initial_price":null,"config":{}}),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, body) = f.call("PATCH", "", config(0, true, Value::Null)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(f.counts().await, (0, 0, 0));
    }
    f.cleanup().await;
    Ok(())
}

async fn reference_pair(f: &Fixture, market_type: &str, status: &str) -> u64 {
    sqlx::query("INSERT INTO trading_pairs(base_asset,quote_asset,symbol,price_precision,qty_precision,min_order_value,status,market_type) VALUES(?,?,?,4,4,1,?,?)")
        .bind(f.base).bind(f.quote).bind(format!("REF{}-USDT",&Uuid::now_v7().simple().to_string()[16..]))
        .bind(status).bind(market_type).execute(&f.pool).await.unwrap().last_insert_id()
}

fn follow_configuration(reference: u64) -> Value {
    json!({"mode":"follow","follow":{"reference_pair_id":reference}})
}

#[tokio::test]
async fn default_market_follow_requires_an_active_external_reference_and_preserves_old_configs()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let external = reference_pair(&f, "external", "active").await;
    let inactive = reference_pair(&f, "external", "disabled").await;
    let internal = reference_pair(&f, "internal", "active").await;
    for reference in [f.pair, inactive, internal, u64::MAX] {
        let mut body = config(0, false, json!("1"));
        body["config"] = follow_configuration(reference);
        let (status, result) = f.call("PATCH", "", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
        assert_eq!(f.counts().await, (0, 0, 0));
        let (status,result)=f.call("POST","/preview",json!({"expected_version":0,"initial_price":"1","config":follow_configuration(reference)})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
        assert_eq!(f.counts().await, (0, 0, 0));
    }
    let (_, old) = f.call("GET", "", Value::Null).await;
    assert_eq!(old["config"]["mode"], "independent");
    assert_eq!(old["config"]["follow"], Value::Null);
    assert_eq!(old["reference_pair"], Value::Null);
    assert_eq!(old["runtime"]["follow"], Value::Null);
    let mut body = config(0, false, json!("1"));
    body["config"] = follow_configuration(external);
    let (status, saved) = f.call("PATCH", "", body).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["enabled"], false);
    assert_eq!(saved["config"]["mode"], "follow");
    assert_eq!(saved["reference_pair"]["id"], external);
    assert_eq!(saved["reference_pair"]["market_type"], "external");
    assert_eq!(saved["config"]["follow"]["multiplier"], "1");
    let (status, _) = f.call("PATCH", "", config(0, false, json!("1"))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    // 已保存的引用后来停用时，GET 仍展示真实元数据，全部暂停不被引用健康状况阻挡。
    sqlx::query("UPDATE trading_pairs SET status='disabled' WHERE id=?")
        .bind(external)
        .execute(&f.pool)
        .await?;
    let (status, current) = f.call("GET", "", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["reference_pair"]["status"], "disabled");
    let (status, paused) = f
        .call(
            "PATCH",
            "/pause-all",
            json!({"expected_version":1,"all_market_paused":true,"reason":"暂停跟随行情"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{paused}");
    assert_eq!(paused["all_market_paused"], true);
    sqlx::query("DELETE FROM trading_pairs WHERE id IN (?,?,?)")
        .bind(external)
        .bind(inactive)
        .bind(internal)
        .execute(&f.pool)
        .await?;
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_follow_status_uses_persisted_evidence_and_actual_reference_metadata()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let reference = reference_pair(&f, "external", "active").await;
    let actual_symbol: String = sqlx::query_scalar("SELECT symbol FROM trading_pairs WHERE id=?")
        .bind(reference)
        .fetch_one(&f.pool)
        .await?;
    let mut body = config(0, false, json!("1"));
    body["config"] = follow_configuration(reference);
    let (status, saved) = f.call("PATCH", "", body).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let switched = chrono::Utc::now().timestamp_millis();
    let mut state = json!({"follow":{"status":{"mode":"fallback","reference_pair_id":reference,"reference_symbol":"UNTRUSTED-STALE-NAME","reference_price":"123.45","reference_observed_at":switched-120000,"fallback_reason":"reference_stale","switched_at":switched}}});
    sqlx::query("INSERT INTO market_pair_generation_runs(pair_id,generation,active_source,state_json) VALUES(?,1,'default',?)").bind(f.pair).bind(SqlxJson(state.clone())).execute(&f.pool).await?;
    let (status, current) = f.call("GET", "", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["runtime"]["follow"]["mode"], "fallback");
    assert_eq!(
        current["runtime"]["follow"]["reference_symbol"],
        actual_symbol
    );
    assert_eq!(
        current["runtime"]["follow"]["fallback_reason"],
        "reference_stale"
    );
    assert_eq!(current["runtime"]["follow"]["switched_at"], switched);
    assert_eq!(current["reference_pair"]["symbol"], actual_symbol);
    state["follow"]["status"]["mode"] = json!("following");
    state["follow"]["status"]["fallback_reason"] = Value::Null;
    sqlx::query("UPDATE market_pair_generation_runs SET state_json=? WHERE pair_id=?")
        .bind(SqlxJson(state))
        .bind(f.pair)
        .execute(&f.pool)
        .await?;
    let (status, current) = f.call("GET", "", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["runtime"]["follow"]["mode"], "following");
    sqlx::query("DELETE FROM trading_pairs WHERE id=?")
        .bind(reference)
        .execute(&f.pool)
        .await?;
    f.cleanup().await;
    Ok(())
}

async fn reference_tick(
    f: &Fixture,
    reference: u64,
    source: &str,
    at: chrono::DateTime<chrono::Utc>,
    price: &str,
) {
    sqlx::query("INSERT INTO market_price_ticks(event_key,symbol,price,source,observed_at,generation,source_version) SELECT ?,REPLACE(UPPER(symbol),'-',''),?,?,?,1,'follow-preview-fixture' FROM trading_pairs WHERE id=?")
        .bind(Uuid::now_v7().simple().to_string().repeat(2)).bind(decimal(price)).bind(source).bind(at.naive_utc()).bind(reference).execute(&f.pool).await.unwrap();
}

async fn cleanup_reference(f: &Fixture, reference: u64) {
    sqlx::query("DELETE FROM market_price_ticks WHERE symbol=REPLACE((SELECT symbol FROM trading_pairs WHERE id=?),'-','')")
        .bind(reference).execute(&f.pool).await.unwrap();
    sqlx::query("DELETE FROM trading_pairs WHERE id=?")
        .bind(reference)
        .execute(&f.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn default_market_follow_preview_replays_real_percent_changes_without_writes_or_forecasting()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let reference = reference_pair(&f, "external", "active").await;
    let boundary =
        chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp().div_euclid(60) * 60, 0)
            .unwrap();
    let minute = boundary - chrono::Duration::minutes(2);
    reference_tick(&f, reference, "bitget", minute, "100").await;
    reference_tick(
        &f,
        reference,
        "bitget",
        minute + chrono::Duration::seconds(20),
        "101",
    )
    .await;
    reference_tick(
        &f,
        reference,
        "bitget",
        minute + chrono::Duration::seconds(35),
        "99.2",
    )
    .await;
    reference_tick(
        &f,
        reference,
        "default",
        minute + chrono::Duration::seconds(36),
        "900000",
    )
    .await;
    reference_tick(
        &f,
        reference,
        "bitget",
        boundary + chrono::Duration::hours(1),
        "900000",
    )
    .await;
    let mut parameters = follow_configuration(reference);
    parameters["volatility"] = json!("0");
    parameters["mean_reversion"] = json!("0");
    let (status, preview) = f
        .call(
            "POST",
            "/preview",
            json!({"expected_version":0,"initial_price":"1","config":parameters}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let metadata = &preview["follow_preview"];
    assert_eq!(metadata["kind"], "historical_replay");
    assert_eq!(metadata["reference_pair_id"], reference);
    assert_eq!(metadata["reference_sample_count"], 3);
    assert!(
        metadata["warning"]
            .as_str()
            .unwrap()
            .contains("不是实时逐笔记录或未来预测")
    );
    assert!(metadata["range_end"].as_i64().unwrap() <= chrono::Utc::now().timestamp_millis());
    assert_eq!(
        metadata["range_end"].as_i64().unwrap() - metadata["range_start"].as_i64().unwrap(),
        3_600_000
    );
    let samples = preview["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 60);
    let candle = samples
        .iter()
        .find(|sample| sample["open_time"] == minute.timestamp_millis())
        .unwrap();
    assert_eq!(decimal(candle["open"].as_str().unwrap()), decimal("1"));
    assert_eq!(decimal(candle["high"].as_str().unwrap()), decimal("1.01"));
    assert_eq!(decimal(candle["low"].as_str().unwrap()), decimal("0.992"));
    assert_eq!(decimal(candle["close"].as_str().unwrap()), decimal("0.992"));
    for window in samples.windows(2) {
        assert_eq!(window[0]["close"], window[1]["open"]);
    }
    assert_eq!(f.counts().await, (0, 0, 0));
    let runtimes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM market_pair_generation_runs WHERE pair_id=?")
            .bind(f.pair)
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(runtimes, 0);
    cleanup_reference(&f, reference).await;
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_follow_preview_marks_missing_stale_and_conflicting_evidence_as_fallback()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let reference = reference_pair(&f, "external", "active").await;
    let boundary =
        chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp().div_euclid(60) * 60, 0)
            .unwrap();
    reference_tick(
        &f,
        reference,
        "bitget",
        boundary - chrono::Duration::hours(2),
        "100",
    )
    .await;
    reference_tick(
        &f,
        reference,
        "bitget",
        boundary + chrono::Duration::hours(1),
        "100",
    )
    .await;
    reference_tick(
        &f,
        reference,
        "default",
        boundary - chrono::Duration::minutes(2),
        "100",
    )
    .await;
    for conflict in [false, true] {
        if conflict {
            for price in ["100", "101"] {
                reference_tick(
                    &f,
                    reference,
                    "bitget",
                    boundary - chrono::Duration::minutes(2),
                    price,
                )
                .await;
            }
        }
        let (status,preview)=f.call("POST","/preview",json!({"expected_version":0,"initial_price":"1","config":follow_configuration(reference)})).await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["follow_preview"]["kind"], "independent_fallback");
        assert_eq!(preview["follow_preview"]["reference_sample_count"], 0);
        assert!(
            preview["follow_preview"]["warning"]
                .as_str()
                .unwrap()
                .contains("独立震荡样本")
        );
        assert_eq!(f.counts().await, (0, 0, 0));
    }
    cleanup_reference(&f, reference).await;
    f.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn default_market_follow_preview_does_not_present_truncated_archive_as_complete_replay()
-> Result<(), Box<dyn Error>> {
    let Some(f) = Fixture::new().await? else {
        return Ok(());
    };
    let reference = reference_pair(&f, "external", "active").await;
    let symbol: String =
        sqlx::query_scalar("SELECT REPLACE(symbol,'-','') FROM trading_pairs WHERE id=?")
            .bind(reference)
            .fetch_one(&f.pool)
            .await?;
    let at = chrono::Utc::now() - chrono::Duration::minutes(2);
    sqlx::query("INSERT INTO market_price_ticks(event_key,symbol,price,source,observed_at,generation,source_version) SELECT SHA2(CONCAT(?,numbers.n),256),?,100,'bitget',?,1,'preview-overflow-fixture' FROM JSON_TABLE(?, '$[*]' COLUMNS(n INT PATH '$')) AS numbers")
        .bind(Uuid::now_v7().to_string()).bind(symbol).bind(at.naive_utc())
        .bind(SqlxJson(json!((0..20_001).collect::<Vec<_>>()))).execute(&f.pool).await?;
    let (status, preview) = f.call("POST", "/preview", json!({"expected_version":0,"initial_price":"1","config":follow_configuration(reference)})).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["follow_preview"]["kind"], "independent_fallback");
    assert_eq!(preview["follow_preview"]["reference_sample_count"], 0);
    assert!(
        preview["follow_preview"]["warning"]
            .as_str()
            .unwrap()
            .contains("超过 20000 条")
    );
    assert_eq!(f.counts().await, (0, 0, 0));
    cleanup_reference(&f, reference).await;
    f.cleanup().await;
    Ok(())
}
