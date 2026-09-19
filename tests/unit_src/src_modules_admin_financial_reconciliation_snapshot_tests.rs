use super::*;

#[tokio::test]
async fn reconciliation_snapshot_inputs_fail_before_database() {
    for (asset_id, key, reason) in [
        (0, "valid", "capture"),
        (1, "bad key", "capture"),
        (1, "valid", " "),
    ] {
        assert!(matches!(
            capture(
                None,
                1,
                CaptureRequest {
                    asset_id,
                    reason: reason.into(),
                    idempotency_key: key.into(),
                }
            )
            .await,
            Err(AppError::Validation(_))
        ));
    }
    assert!(
        serde_json::from_value::<FollowupRequest>(json!({
            "expected_version":0, "notes":"inspect", "reason":"review", "idempotency_key":"key",
            "owner_admin_id":null
        }))
        .is_err(),
        "missing explicit deadline is not an implicit clear"
    );
}

#[tokio::test]
async fn reconciliation_snapshot_openapi_aliases_and_contracts()
-> Result<(), Box<dyn std::error::Error>> {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    let settings: crate::config::Settings = serde_json::from_value(json!({
        "app_env":"test","database_url":"mysql://unused","mongodb_uri":"mongodb://127.0.0.1:27017",
        "mongodb_database":"unused","redis_url":"redis://127.0.0.1:6379",
        "rabbitmq_url":"amqp://127.0.0.1","jwt_secret":"reconciliation-test-secret",
        "bitget_rest_base_url":"http://127.0.0.1","bitget_ws_url":"ws://127.0.0.1",
        "htx_rest_base_url":"http://127.0.0.1","htx_ws_url":"ws://127.0.0.1"
    }))?;
    let app = crate::build_router(crate::state::AppState::new(settings));
    let mut documents = Vec::new();
    for alias in ["/openapi.json", "/api/openapi.json"] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(alias).body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), 200);
        documents.push(serde_json::from_slice::<serde_json::Value>(
            &to_bytes(response.into_body(), usize::MAX).await?,
        )?);
    }
    assert_eq!(documents[0], documents[1]);
    let doc = &documents[0];
    let base = "/admin/api/v1/financial-reconciliation";
    for (path, methods) in [
        (base.to_owned(), vec!["get"]),
        (format!("{base}/snapshots"), vec!["get", "post"]),
        (format!("{base}/snapshots/{{id}}"), vec!["get"]),
        (
            format!("{base}/snapshots/{{id}}/follow-ups"),
            vec!["get", "post"],
        ),
    ] {
        for method in methods {
            let operation = &doc["paths"][&path][method];
            assert!(operation["security"].is_array(), "{path} {method}");
            assert!(
                operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
                    .is_string()
            );
        }
    }
    let schemas = &doc["components"]["schemas"];
    assert!(
        schemas["SnapshotDetail"]["allOf"]
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["properties"]["report"]["$ref"]
                == "#/components/schemas/FinancialReconciliationReport"),
        "flattened summary must retain the actual report schema"
    );
    for field in [
        "owner_admin_id",
        "due_at",
        "expected_version",
        "idempotency_key",
        "notes",
        "reason",
    ] {
        assert!(
            schemas["FollowupRequest"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field)),
            "{field}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn reconciliation_snapshot_mysql_atomic_immutable_replay_and_followup()
-> Result<(), Box<dyn std::error::Error>> {
    use sqlx::mysql::MySqlPoolOptions;
    let Ok(url) = std::env::var("RECONCILIATION_DATABASE_URL") else {
        eprintln!("SKIP: set RECONCILIATION_DATABASE_URL to hardening_reconciliation_test");
        return Ok(());
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await?;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        database, "hardening_reconciliation_test",
        "isolated database required"
    );
    sqlx::migrate!("./migrations").run(&pool).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let suffix = uuid::Uuid::now_v7().simple().to_string();
    let asset = sqlx::query(
        "INSERT INTO assets (symbol,name,precision_scale) VALUES (?,'capture fixture',8)",
    )
    .bind(format!("SC{}", &suffix[..20]))
    .execute(&pool)
    .await?
    .last_insert_id();
    let role = sqlx::query("INSERT INTO admin_roles (name,permissions) VALUES (?,JSON_ARRAY('governance.financial.read'))")
        .bind(format!("snap-{}", &suffix[..20])).execute(&pool).await?.last_insert_id();
    let admin = sqlx::query(
        "INSERT INTO admin_users(username,password_hash,role_id) VALUES (?,'!fixture',?)",
    )
    .bind(format!("snap-{}", &suffix[..20]))
    .bind(role)
    .execute(&pool)
    .await?
    .last_insert_id();
    let user = sqlx::query("INSERT INTO users(password_hash) VALUES ('!fixture')")
        .execute(&pool)
        .await?
        .last_insert_id();
    sqlx::query("INSERT INTO wallet_accounts(user_id,asset_id,available) VALUES (?,?,10)")
        .bind(user)
        .bind(asset)
        .execute(&pool)
        .await?;
    for index in 0..105 {
        sqlx::query("INSERT INTO platform_financial_journal(transaction_key,context,account_code,asset_id,amount,ref_type,ref_id)
            VALUES (?,'capture_fixture','inventory',?,1,'fixture','fixture')")
            .bind(format!("{suffix}:{index}")).bind(asset).execute(&pool).await?;
    }
    let key = format!("capture-{suffix}");
    let command = || CaptureRequest {
        asset_id: asset,
        reason: "manual observation".into(),
        idempotency_key: key.clone(),
    };
    let (one, two, three) = tokio::join!(
        capture(Some(pool.clone()), admin, command()),
        capture(Some(pool.clone()), admin, command()),
        capture(Some(pool.clone()), admin, command())
    );
    let original = one?;
    assert_eq!(json!(&original), json!(two?));
    assert_eq!(json!(&original), json!(three?));
    let id = original.summary.id;
    assert_eq!(original.report["coverage"], "partial");
    assert_eq!(
        original.report["journal"]["imbalanced_transaction_count"],
        105
    );
    assert_eq!(
        original.report["journal_differences"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
    assert_eq!(original.report["wallets"][0]["missing_ledger_count"], 1);
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id=?")
        .bind(admin)
        .fetch_one(&pool)
        .await?;
    assert_eq!(audits, 1, "concurrent capture audits exactly once");
    let mut changed = command();
    changed.reason = "different reason".into();
    assert!(matches!(
        capture(Some(pool.clone()), admin, changed).await,
        Err(AppError::Conflict(_))
    ));

    // Simulate a digest collision without breaking the immutable-row trigger.
    let collision_key = format!("collision-{suffix}");
    sqlx::query("INSERT INTO financial_reconciliation_snapshots
        (asset_id,asset_symbol,precision_scale,schema_version,captured_at,admin_id,reason,idempotency_key,key_hash,request_hash,report_hash,report_json)
        SELECT asset_id,asset_symbol,precision_scale,schema_version,captured_at,admin_id,reason,
        'different-raw-key',?,request_hash,report_hash,report_json FROM financial_reconciliation_snapshots WHERE id=?")
        .bind(store::digest(collision_key.as_bytes())).bind(id).execute(&pool).await?;
    let mut collision = command();
    collision.idempotency_key = collision_key;
    assert!(matches!(
        capture(Some(pool.clone()), admin, collision).await,
        Err(AppError::Conflict(_))
    ));

    // Only the fixture changes the wallet. Replays and history keep the saved JSON exactly.
    sqlx::query("UPDATE wallet_accounts SET available=11 WHERE user_id=? AND asset_id=?")
        .bind(user)
        .bind(asset)
        .execute(&pool)
        .await?;
    assert_eq!(
        json!(&original),
        json!(capture(Some(pool.clone()), admin, command()).await?)
    );
    assert_eq!(
        json!(&original),
        json!(detail(Some(pool.clone()), id).await?)
    );
    let live = super::super::get_financial_reconciliation(Some(pool.clone()),
        crate::modules::admin::presentation::financial_reconciliation::FinancialReconciliationQuery {
            asset_id:Some(asset), ..Default::default()
        }).await?;
    assert_ne!(
        json!(live.report.unwrap())["wallets"],
        original.report["wallets"]
    );
    for sql in [
        "UPDATE financial_reconciliation_snapshots SET reason='rewrite' WHERE id=?",
        "DELETE FROM financial_reconciliation_snapshots WHERE id=?",
    ] {
        assert!(
            sqlx::query(sql).bind(id).execute(&pool).await.is_err(),
            "immutable evidence"
        );
    }
    let follow_key = format!("follow-{suffix}");
    let follow = || FollowupRequest {
        expected_version: 0,
        owner_admin_id: Some(admin),
        due_at: Some(1_900_000_000_123),
        notes: "verify missing ledger and custody evidence".into(),
        reason: "assign review".into(),
        idempotency_key: follow_key.clone(),
    };
    let (first, retry) = tokio::join!(
        append_followup(Some(pool.clone()), admin, id, follow()),
        append_followup(Some(pool.clone()), admin, id, follow())
    );
    let first = first?;
    assert_eq!(json!(&first), json!(retry?));
    let mut stale = follow();
    stale.idempotency_key = format!("stale-{suffix}");
    assert!(matches!(
        append_followup(Some(pool.clone()), admin, id, stale).await,
        Err(AppError::Conflict(_))
    ));
    let mut altered = follow();
    altered.notes = "altered notes".into();
    assert!(matches!(
        append_followup(Some(pool.clone()), admin, id, altered).await,
        Err(AppError::Conflict(_))
    ));
    let follow_collision = format!("follow-collision-{suffix}");
    sqlx::query("INSERT INTO financial_reconciliation_followups
        (snapshot_id,version,owner_admin_id,due_at,notes,admin_id,reason,recorded_at,idempotency_key,key_hash,request_hash)
        SELECT snapshot_id,2,owner_admin_id,due_at,notes,admin_id,reason,recorded_at,'other-raw-follow-key',?,request_hash
        FROM financial_reconciliation_followups WHERE id=?")
        .bind(store::digest(follow_collision.as_bytes())).bind(first.id).execute(&pool).await?;
    let mut colliding_follow = follow();
    colliding_follow.idempotency_key = follow_collision;
    assert!(matches!(
        append_followup(Some(pool.clone()), admin, id, colliding_follow).await,
        Err(AppError::Conflict(_))
    ));
    let mut clear = follow();
    clear.expected_version = 2;
    clear.owner_admin_id = None;
    clear.due_at = None;
    clear.notes = "explicitly remove assignment; evidence unresolved".into();
    clear.idempotency_key = format!("clear-{suffix}");
    let cleared = append_followup(Some(pool.clone()), admin, id, clear).await?;
    assert_eq!(cleared.version, 3);
    assert_eq!(cleared.owner_admin_id, None);
    assert_eq!(cleared.due_at, None);
    assert_eq!(
        json!(&first),
        json!(append_followup(Some(pool.clone()), admin, id, follow()).await?),
        "replay returns original receipt after later follow-up"
    );
    let list = followups(
        Some(pool.clone()),
        id,
        FollowupQuery {
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await?;
    assert_eq!(
        (
            list.total,
            list.latest_version,
            list.records.len(),
            list.records[0].version
        ),
        (3, 3, 1, 2)
    );
    for sql in [
        "UPDATE financial_reconciliation_followups SET notes='rewrite' WHERE id=?",
        "DELETE FROM financial_reconciliation_followups WHERE id=?",
    ] {
        assert!(
            sqlx::query(sql)
                .bind(first.id)
                .execute(&pool)
                .await
                .is_err(),
            "append-only follow-up"
        );
    }
    let mut invalid_owner = follow();
    invalid_owner.expected_version = 3;
    invalid_owner.owner_admin_id = Some(u64::MAX);
    invalid_owner.idempotency_key = format!("bad-owner-{suffix}");
    assert!(matches!(
        append_followup(Some(pool.clone()), admin, id, invalid_owner).await,
        Err(AppError::Validation(_))
    ));

    // Scope an audit failure to this test actor/reason, then remove it before any assertion.
    let trigger = format!("e02_audit_fail_{}", &suffix[..16]);
    sqlx::raw_sql(&format!(
        "CREATE TRIGGER {trigger} BEFORE INSERT ON admin_audit_logs FOR EACH ROW
        BEGIN IF NEW.admin_id={admin} AND NEW.reason='force audit failure' THEN
        SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture audit failure'; END IF; END"
    ))
    .execute(&pool)
    .await?;
    let mut fail_capture = command();
    fail_capture.reason = "force audit failure".into();
    fail_capture.idempotency_key = format!("fail-capture-{suffix}");
    let fail_key = fail_capture.idempotency_key.clone();
    let result_capture = capture(Some(pool.clone()), admin, fail_capture).await;
    let mut fail_follow = follow();
    fail_follow.expected_version = 3;
    fail_follow.reason = "force audit failure".into();
    fail_follow.idempotency_key = format!("fail-follow-{suffix}");
    let result_follow = append_followup(Some(pool.clone()), admin, id, fail_follow).await;
    sqlx::raw_sql(&format!("DROP TRIGGER {trigger}"))
        .execute(&pool)
        .await?;
    assert!(result_capture.is_err());
    assert!(result_follow.is_err());
    let mut conn = pool.acquire().await?;
    assert!(
        store::capture_by_key(&mut conn, admin, &store::digest(fail_key.as_bytes()))
            .await?
            .is_none()
    );
    drop(conn);
    assert_eq!(
        followups(Some(pool.clone()), id, FollowupQuery::default())
            .await?
            .latest_version,
        3
    );
    let saved_history = history(
        Some(pool.clone()),
        SnapshotQuery {
            asset_id: Some(asset),
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await?;
    assert_eq!(
        (
            saved_history.total,
            saved_history.snapshots.len(),
            saved_history.snapshots[0].id
        ),
        (2, 1, id)
    );
    assert_eq!(
        json!(&original),
        json!(detail(Some(pool.clone()), id).await?)
    );
    let balance: String = sqlx::query_scalar(
        "SELECT CAST(available AS CHAR) FROM wallet_accounts WHERE user_id=? AND asset_id=?",
    )
    .bind(user)
    .bind(asset)
    .fetch_one(&pool)
    .await?;
    assert_eq!(balance, "11.000000000000000000");
    let journals: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM platform_financial_journal WHERE asset_id=?")
            .bind(asset)
            .fetch_one(&pool)
            .await?;
    assert_eq!(journals, 105);
    let ledgers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wallet_ledger WHERE asset_id=?")
        .bind(asset)
        .fetch_one(&pool)
        .await?;
    assert_eq!(ledgers, 0);

    // Parent locks are independent. Empty child ranges must not acquire next-key locks.
    let left = capture(
        Some(pool.clone()),
        admin,
        CaptureRequest {
            asset_id: asset,
            reason: "parallel parent left".into(),
            idempotency_key: format!("left-{suffix}"),
        },
    )
    .await?;
    let right = capture(
        Some(pool.clone()),
        admin,
        CaptureRequest {
            asset_id: asset,
            reason: "parallel parent right".into(),
            idempotency_key: format!("right-{suffix}"),
        },
    )
    .await?;
    let mut left_conn = pool.acquire().await?;
    let mut right_conn = pool.acquire().await?;
    reader::prepare_snapshot(&mut left_conn).await?;
    reader::prepare_snapshot(&mut right_conn).await?;
    let mut left_tx = left_conn.begin().await?;
    let mut right_tx = right_conn.begin().await?;
    assert!(
        store::lock_latest_followup(&mut left_tx, left.summary.id)
            .await?
            .is_none()
    );
    assert!(
        store::lock_latest_followup(&mut right_tx, right.summary.id)
            .await?
            .is_none()
    );
    let mut left_request = follow();
    left_request.idempotency_key = format!("left-follow-{suffix}");
    let mut right_request = follow();
    right_request.idempotency_key = format!("right-follow-{suffix}");
    let (left_result, right_result) = tokio::join!(
        async {
            let saved = store::insert_followup(
                &mut left_tx,
                admin,
                left.summary.id,
                &left_request,
                &store::digest(b"parallel-left"),
                Utc::now(),
            )
            .await?;
            left_tx.commit().await?;
            Ok::<_, sqlx::Error>(saved)
        },
        async {
            let saved = store::insert_followup(
                &mut right_tx,
                admin,
                right.summary.id,
                &right_request,
                &store::digest(b"parallel-right"),
                Utc::now(),
            )
            .await?;
            right_tx.commit().await?;
            Ok::<_, sqlx::Error>(saved)
        }
    );
    assert_eq!(left_result?.version, 1);
    assert_eq!(right_result?.version, 1);
    drop(left_conn);
    drop(right_conn);
    let left = capture(
        Some(pool.clone()),
        admin,
        CaptureRequest {
            asset_id: asset,
            reason: "parallel application left".into(),
            idempotency_key: format!("app-left-{suffix}"),
        },
    )
    .await?;
    let right = capture(
        Some(pool.clone()),
        admin,
        CaptureRequest {
            asset_id: asset,
            reason: "parallel application right".into(),
            idempotency_key: format!("app-right-{suffix}"),
        },
    )
    .await?;
    let mut left_request = follow();
    left_request.due_at = Some(253_402_300_799_999);
    left_request.idempotency_key = format!("left-application-{suffix}");
    let mut right_request = follow();
    right_request.due_at = Some(-30_610_224_000_000);
    right_request.idempotency_key = format!("right-application-{suffix}");
    let (left_result, right_result) = tokio::join!(
        append_followup(Some(pool.clone()), admin, left.summary.id, left_request),
        append_followup(Some(pool.clone()), admin, right.summary.id, right_request),
    );
    assert_eq!(left_result?.version, 1);
    assert_eq!(right_result?.version, 1);
    assert_eq!(
        followups(
            Some(pool.clone()),
            left.summary.id,
            FollowupQuery::default()
        )
        .await?
        .records[0]
            .due_at,
        Some(253_402_300_799_999)
    );
    assert_eq!(
        followups(
            Some(pool.clone()),
            right.summary.id,
            FollowupQuery::default()
        )
        .await?
        .records[0]
            .due_at,
        Some(-30_610_224_000_000)
    );
    verify_snapshot_routes(&pool, &url, admin, role, id, asset, &suffix).await?;
    Ok(())
}

async fn verify_snapshot_routes(
    pool: &MySqlPool,
    url: &str,
    admin: u64,
    role: u64,
    id: u64,
    asset: u64,
    suffix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let settings: crate::config::Settings = serde_json::from_value(json!({
        "app_env":"test","database_url":url,"mongodb_uri":"mongodb://127.0.0.1:27017",
        "mongodb_database":"unused","redis_url":"redis://127.0.0.1:6379",
        "rabbitmq_url":"amqp://127.0.0.1","jwt_secret":"reconciliation-test-secret",
        "bitget_rest_base_url":"http://127.0.0.1","bitget_ws_url":"ws://127.0.0.1",
        "htx_rest_base_url":"http://127.0.0.1","htx_ws_url":"ws://127.0.0.1"
    }))?;
    let token = crate::modules::auth::issue_token(
        &settings,
        format!("admin:{admin}"),
        crate::modules::auth::TokenScope::Admin,
        900,
    )?;
    let app = crate::build_router(crate::state::AppState::new(settings).with_mysql(pool.clone()));
    let base = "/admin/api/v1/financial-reconciliation/snapshots";
    let make = |method: &str, path: &str, auth: bool, body: serde_json::Value| {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("Content-Type", "application/json");
        if auth {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req.body(Body::from(body.to_string())).unwrap()
    };
    for path in [
        format!("{base}?asset_id={asset}"),
        format!("{base}/{id}"),
        format!("{base}/{id}/follow-ups"),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(make("GET", &path, false, json!({})))
                .await?
                .status(),
            401,
            "unauthenticated GET {path}"
        );
        assert_eq!(
            app.clone()
                .oneshot(make("GET", &path, true, json!({})))
                .await?
                .status(),
            200,
            "{path}"
        );
    }
    let body = json!({"asset_id":asset,"reason":"route capture","idempotency_key":format!("route-{suffix}")});
    assert_eq!(
        app.clone()
            .oneshot(make("POST", base, true, body.clone()))
            .await?
            .status(),
        403
    );
    assert_eq!(
        app.clone()
            .oneshot(make(
                "POST",
                &format!("{base}/{id}/follow-ups"),
                true,
                json!({})
            ))
            .await?
            .status(),
        403
    );
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY('governance.financial.read','governance.financial.operate') WHERE id=?")
        .bind(role).execute(pool).await?;
    assert_eq!(
        app.clone()
            .oneshot(make("POST", base, true, body))
            .await?
            .status(),
        200
    );
    let follow = json!({"expected_version":3,"owner_admin_id":null,"due_at":null,
        "notes":"route check","reason":"route follow","idempotency_key":format!("route-follow-{suffix}")});
    assert_eq!(
        app.clone()
            .oneshot(make(
                "POST",
                &format!("{base}/{id}/follow-ups"),
                true,
                follow
            ))
            .await?
            .status(),
        200
    );
    for (method, path, status) in [
        ("DELETE", format!("{base}/{id}"), 403),
        ("PUT", base.into(), 403),
        ("POST", format!("{base}/{id}/follow-ups/extra"), 404),
        ("GET", format!("{base}/0"), 403),
        ("GET", format!("{base}/bad"), 403),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(make(method, &path, true, json!({})))
                .await?
                .status(),
            status,
            "unmapped {method} {path}"
        );
    }
    sqlx::query("UPDATE admin_roles SET permissions=JSON_ARRAY() WHERE id=?")
        .bind(role)
        .execute(pool)
        .await?;
    assert_eq!(
        app.oneshot(make("GET", &format!("{base}/{id}"), true, json!({})))
            .await?
            .status(),
        403
    );
    Ok(())
}
