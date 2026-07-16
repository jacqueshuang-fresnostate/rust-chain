use exchange_api::{
    build_router,
    config::Settings,
    infra::{self, email::SmtpEmailSender},
    modules::admin::{
        application::load_enabled_admin_market_feed_config,
        service::market_feed_runtime_config_from_response,
    },
    modules::{events::EventBroadcastHub, prediction},
    state::AppState,
    workers::{
        earn_auto_redemption, event_inbox, event_outbox, kline_recovery, margin_interest,
        margin_liquidation, market_feed, seconds_contract_settlement, unlock_scanner,
    },
};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = Settings::from_env()?;
    let addr = settings.socket_addr();

    let mysql = infra::mysql::connect(&settings).await?;
    let mongo = infra::mongo::connect(&settings).await?;
    let redis = infra::redis::connect(&settings).await?;
    let auth_manager = infra::auth::connect(&settings).await?;
    let rabbitmq = infra::rabbitmq::connect(&settings).await?;

    let market_feed_supervisor = market_feed::MarketFeedSupervisorHandle::new();
    let state = AppState::new(settings)
        .with_mysql(mysql)
        .with_mongo(mongo)
        .with_redis(redis)
        .with_auth_manager(auth_manager)
        .with_rabbitmq(rabbitmq)
        .with_event_broadcast_hub(EventBroadcastHub::new(1024))
        .with_market_feed_supervisor(market_feed_supervisor.clone())
        .with_email_sender(Arc::new(SmtpEmailSender));

    if let Some(pool) = state.mysql.clone() {
        let market_feed_state = state.clone();
        tokio::spawn(async move {
            let db_config = match load_enabled_admin_market_feed_config(&pool).await {
                Ok(config) => config,
                Err(error) => {
                    tracing::error!(%error, "加载行情订阅数据库配置失败");
                    None
                }
            };
            let runtime_config = match db_config.as_ref() {
                Some(config) => {
                    market_feed_runtime_config_from_response(&market_feed_state.settings, config)
                }
                None => market_feed::MarketFeedRuntimeConfig::new(
                    &market_feed_state.settings,
                    market_feed_state.settings.market_feed_symbols.clone(),
                    market_feed_state.settings.market_feed_intervals.clone(),
                    market_feed_state.settings.market_feed_providers.clone(),
                    market_feed_state.settings.market_feed_reconnect_seconds,
                ),
            };
            match runtime_config {
                Ok(config) if config.enabled() => {
                    let version = db_config.as_ref().map(|config| config.version).unwrap_or(0);
                    if let Err(error) = market_feed_supervisor
                        .reload(market_feed_state, config, version)
                        .await
                    {
                        tracing::error!(%error, "行情订阅循环已停止");
                    }
                }
                Ok(_) => tracing::info!("行情 WebSocket 循环已禁用：未配置交易对"),
                Err(error) => tracing::error!(%error, "行情订阅运行配置失败"),
            }
        });
    }

    if state.settings.event_outbox_publisher_enabled
        && state.mysql.is_some()
        && state.rabbitmq.is_some()
    {
        let event_outbox_state = state.clone();
        let interval_seconds = state.settings.event_outbox_publisher_interval_seconds;
        tokio::spawn(async move {
            if let Err(error) = event_outbox::run_loop(event_outbox_state, interval_seconds).await {
                tracing::error!(%error, "事件 outbox 循环已停止");
            }
        });
    }

    if state.settings.unlock_scanner_enabled && state.mysql.is_some() {
        let unlock_scanner_state = state.clone();
        let interval_seconds = state.settings.unlock_scanner_interval_seconds;
        let batch_limit = state.settings.unlock_scanner_batch_limit;
        tokio::spawn(async move {
            if let Err(error) =
                unlock_scanner::run_loop(unlock_scanner_state, interval_seconds, batch_limit).await
            {
                tracing::error!(%error, "解禁扫描循环已停止");
            }
        });
    }

    if state.settings.kline_recovery_enabled && state.mysql.is_some() && state.mongo.is_some() {
        let kline_recovery_state = state.clone();
        let interval_seconds = state.settings.kline_recovery_interval_seconds;
        let batch_limit = state.settings.kline_recovery_batch_limit;
        tokio::spawn(async move {
            if let Err(error) =
                kline_recovery::run_loop(kline_recovery_state, interval_seconds, batch_limit).await
            {
                tracing::error!(%error, "K 线恢复循环已停止");
            }
        });
    }

    if state.settings.seconds_contract_settlement_enabled
        && state.mysql.is_some()
        && state.redis.is_some()
    {
        let seconds_contract_settlement_state = state.clone();
        let interval_seconds = state.settings.seconds_contract_settlement_interval_seconds;
        let batch_limit = state.settings.seconds_contract_settlement_batch_limit;
        tokio::spawn(async move {
            if let Err(error) = seconds_contract_settlement::run_loop(
                seconds_contract_settlement_state,
                interval_seconds,
                batch_limit,
            )
            .await
            {
                tracing::error!(%error, "秒合约结算循环已停止");
            }
        });
    }

    if state.settings.earn_auto_redemption_enabled && state.mysql.is_some() {
        let earn_auto_redemption_state = state.clone();
        let interval_seconds = state.settings.earn_auto_redemption_interval_seconds;
        let batch_limit = state.settings.earn_auto_redemption_batch_limit;
        tokio::spawn(async move {
            if let Err(error) = earn_auto_redemption::run_loop(
                earn_auto_redemption_state,
                interval_seconds,
                batch_limit,
            )
            .await
            {
                tracing::error!(%error, "理财自动赎回循环已停止");
            }
        });
    }

    if state.settings.margin_liquidation_enabled && state.mysql.is_some() && state.redis.is_some() {
        let margin_liquidation_state = state.clone();
        let interval_seconds = state.settings.margin_liquidation_interval_seconds;
        let batch_limit = state.settings.margin_liquidation_batch_limit;
        tokio::spawn(async move {
            if let Err(error) = margin_liquidation::run_loop(
                margin_liquidation_state,
                interval_seconds,
                batch_limit,
            )
            .await
            {
                tracing::error!(%error, "杠杆强平循环已停止");
            }
        });
    }

    if state.settings.margin_interest_enabled
        && let Some(pool) = state.mysql.clone()
    {
        let interval_seconds = state.settings.margin_interest_interval_seconds;
        let batch_limit = state.settings.margin_interest_batch_limit;
        tokio::spawn(async move {
            if let Err(error) = margin_interest::run_loop(pool, interval_seconds, batch_limit).await
            {
                tracing::error!(%error, "杠杆利息循环已停止");
            }
        });
    }

    if state.mysql.is_some() {
        let prediction_sync_state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = prediction::run_sync_loop(prediction_sync_state).await {
                tracing::error!(%error, "竞猜市场同步循环已停止");
            }
        });
    }

    let event_inbox_config = event_inbox::EventInboxWorkerConfig::from_env()?;
    if let Some(startup) = event_inbox_config.startup() {
        let event_inbox_state = state.clone();
        let queue_name = startup.queue_name().to_owned();
        let consumer_tag = startup.consumer_tag().to_owned();
        let retry_scanner_state = state.clone();
        let retry_consumer_name = queue_name.clone();
        let retry_scan_seconds =
            startup.retry_scan_seconds(state.settings.event_inbox_retry_scan_seconds);
        tokio::spawn(async move {
            if let Err(error) = event_inbox::run_retry_scanner_loop(
                retry_scanner_state,
                retry_consumer_name,
                retry_scan_seconds,
            )
            .await
            {
                tracing::error!(%error, "事件 inbox 重试扫描已停止");
            }
        });
        tokio::spawn(async move {
            if let Err(error) =
                event_inbox::run_loop(event_inbox_state, queue_name, consumer_tag).await
            {
                tracing::error!(%error, "事件 inbox 消费循环已停止");
            }
        });
    }

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "交易所 API 已开始监听");

    axum::serve(listener, app).await?;
    Ok(())
}
