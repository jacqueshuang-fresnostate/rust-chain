use axum::{body::Body, http::StatusCode};
use exchange_api::{
    build_router,
    config::Settings,
    modules::{
        auth::{TokenScope, issue_token},
        events::{
            EventBroadcastHub, EventBroadcastMessage, PrivateWsAuth, WebSocketChannel, routes,
        },
    },
    state::AppState,
};
use futures_util::{SinkExt, StreamExt};
use secrecy::SecretString;
use tokio::{net::TcpListener, sync::oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

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
        htx_rest_base_url: "wss://htx.test".to_owned(),
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

#[test]
fn websocket_channel_rejects_invalid_stream_names() {
    assert!(WebSocketChannel::public("ticker", "BTC-USDT").is_ok());
    assert!(WebSocketChannel::public("../ticker", "BTC-USDT").is_err());
    assert!(WebSocketChannel::public("ticker", "BTC/USDT").is_err());
}

#[test]
fn private_ws_auth_accepts_user_token_from_query_only() {
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();

    let auth = PrivateWsAuth::from_query(Some(&format!("token={token}")), &settings).unwrap();

    assert_eq!(auth.user_id, 42);
}

#[test]
fn private_ws_auth_rejects_non_user_scope() {
    let settings = test_settings();
    let token = issue_token(&settings, "admin:1", TokenScope::Admin, 900).unwrap();

    assert!(PrivateWsAuth::from_query(Some(&format!("token={token}")), &settings).is_err());
}

#[tokio::test]
async fn event_broadcast_hub_fans_out_matching_public_channel_messages() {
    let hub = EventBroadcastHub::new(16);
    let channel = WebSocketChannel::public("ticker", "BTC-USDT").unwrap();
    let mut receiver = hub.subscribe(&channel);
    let ignored = WebSocketChannel::public("ticker", "ETH-USDT").unwrap();

    hub.publish(EventBroadcastMessage::public(
        ignored,
        r#"{"symbol":"ETHUSDT"}"#,
    ));
    hub.publish(EventBroadcastMessage::public(
        channel,
        r#"{"symbol":"BTCUSDT","last_price":"70000.12"}"#,
    ));

    let message = receiver.recv().await.unwrap();
    assert_eq!(
        message.payload(),
        r#"{"symbol":"BTCUSDT","last_price":"70000.12"}"#
    );
}

#[tokio::test]
async fn event_broadcast_subscription_skips_lagged_unrelated_messages() {
    let hub = EventBroadcastHub::new(1);
    let channel = WebSocketChannel::public("ticker", "BTC-USDT").unwrap();
    let mut receiver = hub.subscribe(&channel);
    let ignored = WebSocketChannel::public("ticker", "ETH-USDT").unwrap();

    hub.publish(EventBroadcastMessage::public(
        ignored.clone(),
        r#"{"symbol":"ETHUSDT","seq":1}"#,
    ));
    hub.publish(EventBroadcastMessage::public(
        ignored,
        r#"{"symbol":"ETHUSDT","seq":2}"#,
    ));
    hub.publish(EventBroadcastMessage::public(
        channel,
        r#"{"symbol":"BTCUSDT","last_price":"70000.12"}"#,
    ));

    let message = receiver.recv().await.unwrap();
    assert_eq!(
        message.payload(),
        r#"{"symbol":"BTCUSDT","last_price":"70000.12"}"#
    );
}

#[tokio::test]
async fn events_routes_expose_public_and_private_ws_paths() {
    let app = routes::routes().with_state(
        AppState::new(test_settings()).with_event_broadcast_hub(EventBroadcastHub::new(16)),
    );

    let public_response = app
        .clone()
        .oneshot(ws_request("/ws/public/ticker/BTC-USDT"))
        .await
        .unwrap();
    assert_ne!(public_response.status(), StatusCode::NOT_FOUND);

    let private_response = app
        .oneshot(ws_request("/ws/private?token=invalid"))
        .await
        .unwrap();
    assert_ne!(private_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn build_router_exposes_root_websocket_paths() {
    let app = build_router(
        AppState::new(test_settings()).with_event_broadcast_hub(EventBroadcastHub::new(16)),
    );

    let public_response = app
        .clone()
        .oneshot(ws_request("/ws/public/ticker/BTC-USDT"))
        .await
        .unwrap();
    assert_ne!(public_response.status(), StatusCode::NOT_FOUND);

    let private_response = app
        .oneshot(ws_request("/ws/private?token=invalid"))
        .await
        .unwrap();
    assert_ne!(private_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn private_ws_receives_only_authenticated_user_broadcasts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let settings = test_settings();
    let token = issue_token(&settings, "user:42", TokenScope::User, 900).unwrap();
    let hub = EventBroadcastHub::new(16);
    let app =
        routes::routes().with_state(AppState::new(settings).with_event_broadcast_hub(hub.clone()));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    let (mut socket, _response) = connect_async(format!("ws://{address}/ws/private?token={token}"))
        .await
        .unwrap();
    let confirmation = socket.next().await.unwrap().unwrap();
    assert_eq!(
        confirmation.into_text().unwrap(),
        r#"{"type":"subscribed","channel":"private:user:42"}"#
    );
    hub.publish(EventBroadcastMessage::private_user(
        7,
        r#"{"type":"wallet.updated","user_id":7}"#,
    ));
    hub.publish(EventBroadcastMessage::private_user(
        42,
        r#"{"type":"wallet.updated","user_id":42}"#,
    ));

    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Text(r#"{"type":"wallet.updated","user_id":42}"#.to_owned())
    );
    socket.send(Message::Text("ping".to_owned())).await.unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Text("pong".to_owned())
    );
    shutdown_tx.send(()).unwrap();
}

#[tokio::test]
async fn public_ws_receives_broadcast_messages_after_subscription_confirmation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hub = EventBroadcastHub::new(16);
    let app = routes::routes()
        .with_state(AppState::new(test_settings()).with_event_broadcast_hub(hub.clone()));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    let (mut socket, _response) =
        connect_async(format!("ws://{address}/ws/public/ticker/BTC-USDT"))
            .await
            .unwrap();
    let confirmation = socket.next().await.unwrap().unwrap();
    assert_eq!(
        confirmation.into_text().unwrap(),
        r#"{"type":"subscribed","channel":"public:ticker:BTCUSDT"}"#
    );
    let channel = WebSocketChannel::public("ticker", "BTCUSDT").unwrap();
    hub.publish(EventBroadcastMessage::public(
        channel,
        r#"{"symbol":"BTCUSDT","last_price":"70000.12"}"#,
    ));

    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Text(r#"{"symbol":"BTCUSDT","last_price":"70000.12"}"#.to_owned())
    );
    socket.send(Message::Text("ping".to_owned())).await.unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Text("pong".to_owned())
    );
    shutdown_tx.send(()).unwrap();
}

fn ws_request(uri: &str) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .uri(uri)
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap()
}
