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
        agent_commission_settlement, earn_auto_redemption, event_inbox, event_outbox, loan_overdue,
        margin_interest, margin_liquidation, market_feed, seconds_contract_settlement,
        synthetic_market, unlock_scanner, wallet_chain,
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

    // KLINE_RECOVERY_ENABLED/BATCH_LIMIT 兼容为实时模拟行情开关与扫描上限；
    // INTERVAL_SECONDS 仍仅解析旧部署配置，历史缺口只能由后台预览确认后手动执行。
    if state.settings.kline_recovery_enabled
        && state.mysql.is_some()
        && state.mongo.is_some()
        && state.redis.is_some()
    {
        let synthetic_market_state = state.clone();
        let max_strategies_per_round = state.settings.kline_recovery_batch_limit;
        tokio::spawn(async move {
            tracing::info!(
                interval_seconds = 1_u64,
                max_strategies_per_round,
                legacy_interval_seconds = synthetic_market_state
                    .settings
                    .kline_recovery_interval_seconds,
                "模拟行情实时循环已启动；仅生成当前分钟，停机缺口不会自动补写"
            );
            if let Err(error) =
                synthetic_market::run_loop(synthetic_market_state, 1, max_strategies_per_round)
                    .await
            {
                tracing::error!(%error, "模拟行情实时循环已停止");
            }
        });
    } else if state.settings.kline_recovery_enabled {
        tracing::warn!(
            mysql = state.mysql.is_some(),
            mongo = state.mongo.is_some(),
            redis = state.redis.is_some(),
            "模拟行情实时循环未启动：缺少 MySQL、Mongo 或 Redis"
        );
    } else {
        tracing::info!("模拟行情实时循环已由 KLINE_RECOVERY_ENABLED 兼容开关关闭");
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

    if state.settings.agent_commission_auto_settle_enabled
        && let Some(pool) = state.mysql.clone()
    {
        let interval_seconds = state.settings.agent_commission_auto_settle_interval_seconds;
        let min_age_seconds = state.settings.agent_commission_auto_settle_min_age_seconds;
        let batch_limit = state.settings.agent_commission_auto_settle_batch_limit;
        tokio::spawn(async move {
            if let Err(error) = agent_commission_settlement::run_loop(
                pool,
                interval_seconds,
                min_age_seconds,
                batch_limit,
            )
            .await
            {
                tracing::error!(%error, "代理佣金自动结算循环已停止");
            }
        });
    }

    let loan_overdue_config = loan_overdue::LoanOverdueWorkerConfig::from_env();
    if loan_overdue_config.enabled
        && let Some(pool) = state.mysql.clone()
    {
        tokio::spawn(async move {
            if let Err(error) = loan_overdue::run_loop(
                pool,
                loan_overdue_config.interval_seconds,
                loan_overdue_config.batch_limit,
            )
            .await
            {
                tracing::error!(%error, "贷款逾期扫描循环已停止");
            }
        });
    }

    let wallet_chain_config = wallet_chain::WalletChainWorkerConfig::from_env();
    if wallet_chain_config.enabled && state.mysql.is_some() {
        let wallet_chain_state = state.clone();
        tokio::spawn(async move {
            if let Err(error) =
                wallet_chain::run_loop(wallet_chain_state, wallet_chain_config).await
            {
                tracing::error!(%error, "钱包链任务循环已停止");
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
