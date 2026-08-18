use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use chrono::{TimeDelta, TimeZone, Utc};
use exchange_api::{
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{
            EventOutboxRepository, EventOutboxService, EventOutboxWriter, InboxRetryDecision,
            InboxRetryPolicy, MySqlEventOutboxRepository, NewOutboxEvent, OutboxInsertResult,
            OutboxMessage, OutboxPublisher, RabbitMqOutboxPublisher, RabbitMqPublishEnvelope,
            routes,
        },
    },
    state::AppState,
};
use lapin::{Connection, ConnectionProperties, options::ExchangeDeleteOptions};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::mysql::MySqlPoolOptions;
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

fn test_state() -> AppState {
    AppState::new(Settings {
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
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
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
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    })
}

#[derive(Clone, Default)]
struct RecordingOutboxRepository {
    events: Arc<Mutex<Vec<NewOutboxEvent>>>,
}

#[axum::async_trait]
impl EventOutboxRepository for RecordingOutboxRepository {
    async fn insert_event(
        &self,
        event: NewOutboxEvent,
    ) -> exchange_api::error::AppResult<OutboxInsertResult> {
        let mut events = self.events.lock().await;
        if let Some((index, _)) = events
            .iter()
            .enumerate()
            .find(|(_, stored)| stored.idempotency_key == event.idempotency_key)
        {
            return Ok(OutboxInsertResult::Duplicate {
                id: (index + 1) as u64,
            });
        }
        events.push(event);
        Ok(OutboxInsertResult::Inserted {
            id: events.len() as u64,
        })
    }

    async fn fetch_publishable_batch(
        &self,
        _limit: u32,
        _now: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<Vec<OutboxMessage>> {
        Ok(Vec::new())
    }

    async fn mark_published(
        &self,
        _id: u64,
        _published_at: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<()> {
        Ok(())
    }

    async fn mark_retry(
        &self,
        _id: u64,
        _retry_count: u32,
        _next_retry_at: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<()> {
        Ok(())
    }

    async fn mark_dead_letter(
        &self,
        _id: u64,
        _retry_count: u32,
        _failed_at: chrono::DateTime<Utc>,
    ) -> exchange_api::error::AppResult<()> {
        Ok(())
    }
}

async fn mysql_pool() -> Option<sqlx::MySqlPool> {
    let database_url = match env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping MySQL event record test because DATABASE_URL is not set");
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

async fn post_publish_once(app: axum::Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri("/events/outbox/publish-once")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .oneshot(request.body(Body::from("{}")).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let payload = serde_json::from_slice(&body).unwrap();

    (status, payload)
}

#[test]
fn new_outbox_event_derives_stable_idempotency_key() {
    let created_at = Utc.with_ymd_and_hms(2026, 5, 26, 11, 0, 0).unwrap();

    let event = NewOutboxEvent::new(
        "convert_order",
        "42",
        "confirmed",
        "convert.order.confirmed",
        json!({ "quote_id": "quote-42" }),
        created_at,
    );

    assert_eq!(event.aggregate_type, "convert_order");
    assert_eq!(event.aggregate_id, "42");
    assert_eq!(event.event_type, "confirmed");
    assert_eq!(event.routing_key, "convert.order.confirmed");
    assert_eq!(event.idempotency_key, "convert_order:42:confirmed");
    assert_eq!(event.created_at, created_at);
}

#[test]
fn inbox_retry_policy_retries_before_dead_letter_threshold() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 11, 0, 0).unwrap();
    let policy = InboxRetryPolicy::new(3, TimeDelta::seconds(15)).unwrap();

    assert_eq!(
        policy.record_failure(0, now).unwrap(),
        InboxRetryDecision::Retry {
            attempt_count: 1,
            next_retry_at: now + TimeDelta::seconds(15),
        }
    );
    assert_eq!(
        policy.record_failure(2, now).unwrap(),
        InboxRetryDecision::DeadLetter { attempt_count: 3 }
    );
}

#[tokio::test]
async fn event_outbox_writer_persists_market_feed_events_once() {
    let repository = RecordingOutboxRepository::default();
    let writer = EventOutboxWriter::new(repository.clone());
    let event = exchange_api::modules::market::adapters::MarketFeedEvent::from_frame(
        &exchange_api::modules::market::adapters::MarketFeedFrame::bitget_ticker(
            r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        ),
    )
    .unwrap();

    let first = writer
        .write_market_feed_event(event.clone(), Utc::now())
        .await
        .unwrap();
    let second = writer
        .write_market_feed_event(event, Utc::now())
        .await
        .unwrap();
    let events = repository.events.lock().await;

    assert!(matches!(first, OutboxInsertResult::Inserted { id: 1 }));
    assert!(matches!(second, OutboxInsertResult::Duplicate { id: 1 }));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].aggregate_type, "market_ticker");
    assert_eq!(events[0].aggregate_id, "BTCUSDT");
    assert_eq!(events[0].event_type, "ticker_updated");
    assert_eq!(events[0].routing_key, "market.BTCUSDT.ticker");
    assert_eq!(
        events[0].idempotency_key,
        "market_feed:bitget:BTCUSDT:ticker:1710000000000"
    );
    assert_eq!(events[0].payload["last_price"], "70000.12");
}

#[tokio::test]
async fn event_outbox_writer_keeps_distinct_market_feed_updates() {
    let repository = RecordingOutboxRepository::default();
    let writer = EventOutboxWriter::new(repository.clone());
    let first_event = exchange_api::modules::market::adapters::MarketFeedEvent::from_frame(
        &exchange_api::modules::market::adapters::MarketFeedFrame::bitget_ticker(
            r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]}"#,
        ),
    )
    .unwrap();
    let second_event = exchange_api::modules::market::adapters::MarketFeedEvent::from_frame(
        &exchange_api::modules::market::adapters::MarketFeedFrame::bitget_ticker(
            r#"{"arg":{"instId":"BTCUSDT"},"data":[{"lastPr":"70001.12","baseVolume":"126.50","ts":"1710000001000"}]}"#,
        ),
    )
    .unwrap();

    let first = writer
        .write_market_feed_event(first_event, Utc::now())
        .await
        .unwrap();
    let second = writer
        .write_market_feed_event(second_event, Utc::now())
        .await
        .unwrap();
    let events = repository.events.lock().await;

    assert!(matches!(first, OutboxInsertResult::Inserted { id: 1 }));
    assert!(matches!(second, OutboxInsertResult::Inserted { id: 2 }));
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].idempotency_key, events[1].idempotency_key);
}

#[tokio::test]
async fn event_outbox_writer_keeps_distinct_kline_updates_for_same_open_time() {
    let repository = RecordingOutboxRepository::default();
    let writer = EventOutboxWriter::new(repository.clone());
    let first_event = exchange_api::modules::market::adapters::MarketFeedEvent::from_frame(
        &exchange_api::modules::market::adapters::MarketFeedFrame::bitget_kline(
            r#"{"arg":{"channel":"candle1m","instId":"BTCUSDT"},"ts":"1710000005000","data":[["1710000000000","70000.00","70010.00","69990.00","70005.00","12.30"]]}"#,
        ),
    )
    .unwrap();
    let second_event = exchange_api::modules::market::adapters::MarketFeedEvent::from_frame(
        &exchange_api::modules::market::adapters::MarketFeedFrame::bitget_kline(
            r#"{"arg":{"channel":"candle1m","instId":"BTCUSDT"},"ts":"1710000006000","data":[["1710000000000","70000.00","70010.00","69990.00","70005.00","12.30"]]}"#,
        ),
    )
    .unwrap();

    let first = writer
        .write_market_feed_event(first_event, Utc::now())
        .await
        .unwrap();
    let second = writer
        .write_market_feed_event(second_event, Utc::now())
        .await
        .unwrap();
    let events = repository.events.lock().await;

    assert!(matches!(first, OutboxInsertResult::Inserted { id: 1 }));
    assert!(matches!(second, OutboxInsertResult::Inserted { id: 2 }));
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].idempotency_key, events[1].idempotency_key);
}

#[test]
fn rabbitmq_publish_envelope_uses_outbox_routing_and_idempotency_key() {
    let outbox = OutboxMessage {
        id: 7,
        aggregate_type: "convert_order".to_owned(),
        aggregate_id: "42".to_owned(),
        event_type: "confirmed".to_owned(),
        routing_key: "convert.order.confirmed".to_owned(),
        idempotency_key: "convert_order:42:confirmed".to_owned(),
        payload: json!({ "quote_id": "quote-42" }),
        retry_count: 0,
    };

    let envelope = RabbitMqPublishEnvelope::from_outbox("exchange.events", &outbox).unwrap();

    assert_eq!(envelope.exchange, "exchange.events");
    assert_eq!(envelope.routing_key, "convert.order.confirmed");
    assert_eq!(envelope.message_id, "convert_order:42:confirmed");
    assert_eq!(envelope.content_type, "application/json");
    assert_eq!(
        serde_json::from_slice::<Value>(&envelope.payload).unwrap(),
        json!({
            "aggregate_type": "convert_order",
            "aggregate_id": "42",
            "event_type": "confirmed",
            "routing_key": "convert.order.confirmed",
            "idempotency_key": "convert_order:42:confirmed",
            "payload": { "quote_id": "quote-42" }
        })
    );
}

#[tokio::test]
async fn event_outbox_publish_handler_requires_admin_auth() {
    let state = test_state();
    let user_token = issue_token(
        &state.settings,
        "user:1",
        TokenScope::User,
        state.settings.jwt_access_ttl_seconds,
    )
    .unwrap();
    let app = routes::routes().with_state(state);

    let (missing_status, missing_payload) = post_publish_once(app.clone(), None).await;
    let (user_status, user_payload) = post_publish_once(app, Some(&user_token)).await;

    assert_eq!(missing_status, StatusCode::UNAUTHORIZED);
    assert_eq!(missing_payload["code"], "UNAUTHORIZED");
    assert_eq!(user_status, StatusCode::FORBIDDEN);
    assert_eq!(user_payload["code"], "FORBIDDEN");
}

#[tokio::test]
async fn event_outbox_publish_handler_returns_clear_error_without_mysql() {
    let state = test_state();
    let admin_token = admin_token(&state);
    let app = routes::routes().with_state(state);

    let (status, payload) = post_publish_once(app, Some(&admin_token)).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("mysql pool is not configured for event outbox persistence")
    );
}

#[tokio::test]
async fn event_outbox_service_returns_clear_error_without_rabbitmq() {
    let state = test_state();
    let mysql = MySqlPoolOptions::new()
        .connect_lazy("mysql://test:test@localhost/test")
        .unwrap();
    let state = state.with_mysql(mysql);

    // 路由在 P0 RBAC 后会先回查管理员权限；缺少 RabbitMQ 的装配合同应在应用层直接验证，
    // 避免用不可连接的假 MySQL 把授权失败误判为 publisher 配置失败。
    let error =
        match EventOutboxService::<MySqlEventOutboxRepository, RabbitMqOutboxPublisher>::from_state(
            &state,
        ) {
            Ok(_) => panic!("outbox service must reject a missing RabbitMQ connection"),
            Err(error) => error,
        };
    assert!(
        error
            .to_string()
            .contains("rabbitmq connection is not configured for event outbox publisher")
    );
}

#[tokio::test]
async fn rabbitmq_outbox_publisher_declares_exchange_before_publish()
-> Result<(), Box<dyn std::error::Error>> {
    let rabbitmq_url = match env::var("RABBITMQ_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(()),
    };
    let connection =
        Arc::new(Connection::connect(&rabbitmq_url, ConnectionProperties::default()).await?);
    let exchange = format!("exchange.events.test.{}", Uuid::now_v7());
    let cleanup_channel = connection.create_channel().await?;
    let publisher = RabbitMqOutboxPublisher::new(connection.clone(), exchange.clone());
    let message = OutboxMessage {
        id: 1,
        aggregate_type: "convert_order".to_owned(),
        aggregate_id: "42".to_owned(),
        event_type: "confirmed".to_owned(),
        routing_key: "convert.order.confirmed".to_owned(),
        idempotency_key: "convert_order:42:confirmed".to_owned(),
        payload: json!({ "quote_id": "quote-42" }),
        retry_count: 0,
    };

    publisher.publish(&message).await?;

    cleanup_channel
        .exchange_delete(&exchange, ExchangeDeleteOptions::default())
        .await?;
    connection.close(0, "test complete").await?;
    Ok(())
}

fn admin_token(state: &AppState) -> String {
    issue_token(
        &state.settings,
        "admin:1",
        TokenScope::Admin,
        state.settings.jwt_access_ttl_seconds,
    )
    .unwrap()
}

#[tokio::test]
async fn event_record_routes_require_admin_auth() {
    let state = test_state();
    let user_token = issue_token(
        &state.settings,
        "user:1",
        TokenScope::User,
        state.settings.jwt_access_ttl_seconds,
    )
    .unwrap();
    let app = routes::routes().with_state(state);

    for uri in ["/events/outbox", "/events/inbox"] {
        let missing = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let forbidden = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(AUTHORIZATION, format!("Bearer {user_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn event_outbox_requeue_restores_dead_letter_to_pending_and_audits() {
    let Some(pool) = mysql_pool().await else {
        return;
    };
    let state = test_state();
    let suffix = Uuid::now_v7().simple().to_string();
    let role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind(format!("events-role-{}", &suffix[16..32]))
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin_id =
        sqlx::query("INSERT INTO admin_users (username, password_hash, role_id) VALUES (?, ?, ?)")
            .bind(format!("events-admin-{}", &suffix[16..32]))
            .bind("not-a-real-hash")
            .bind(role_id)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let idempotency_key = format!("events-dead-letter-{suffix}");
    let outbox_id = sqlx::query(
        r#"INSERT INTO event_outbox
           (aggregate_type, aggregate_id, event_type, routing_key, idempotency_key, payload_json,
            status, retry_count)
           VALUES ('test_aggregate', '1', 'created', 'test.created', ?, JSON_OBJECT(), 'dead_letter', 5)"#,
    )
    .bind(&idempotency_key)
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_id();

    let token = issue_token(
        &state.settings,
        format!("admin:{admin_id}"),
        TokenScope::Admin,
        state.settings.jwt_access_ttl_seconds,
    )
    .unwrap();
    let app = routes::routes().with_state(state.with_mysql(pool.clone()));

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/events/outbox?status=dead_letter&limit=100")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_payload: Value =
        serde_json::from_slice(&to_bytes(listed.into_body(), 1_048_576).await.unwrap()).unwrap();
    assert!(
        listed_payload["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["id"] == outbox_id)
    );
    assert!(listed_payload["total"].as_i64().unwrap() >= 1);

    let requeue = |body: &'static str| {
        let app = app.clone();
        let token = token.clone();
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/events/outbox/{outbox_id}/requeue"))
                    .header(AUTHORIZATION, format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    let missing_reason = requeue("{}").await;
    assert_eq!(missing_reason.status(), StatusCode::BAD_REQUEST);

    let requeued = requeue(r#"{"reason":"重放死信"}"#).await;
    assert_eq!(requeued.status(), StatusCode::OK);
    let (status, retry_count): (String, i32) =
        sqlx::query_as("SELECT status, retry_count FROM event_outbox WHERE id = ?")
            .bind(outbox_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(retry_count, 0);

    // 已回到 pending 的事件不能重复重放。
    let repeated = requeue(r#"{"reason":"重复重放"}"#).await;
    assert_eq!(repeated.status(), StatusCode::CONFLICT);

    let (audit_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE admin_id = ? AND action = 'event_outbox.requeue' AND target_id = ?",
    )
    .bind(admin_id)
    .bind(outbox_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);

    sqlx::query("DELETE FROM admin_audit_logs WHERE admin_id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM event_outbox WHERE id = ?")
        .bind(outbox_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM admin_users WHERE id = ?")
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM admin_roles WHERE id = ?")
        .bind(role_id)
        .execute(&pool)
        .await
        .unwrap();
}
