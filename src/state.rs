use crate::{
    config::Settings, infra::email::EmailSender, modules::events::EventBroadcastHub,
    workers::market_feed::MarketFeedSupervisorHandle,
};
use mongodb::Database;
use redis::aio::ConnectionManager;
use sa_token_core::SaTokenManager;
use sqlx::{MySql, Pool};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub mysql: Option<Pool<MySql>>,
    pub mongo: Option<Database>,
    pub redis: Option<ConnectionManager>,
    pub auth_manager: Option<Arc<SaTokenManager>>,
    pub rabbitmq: Option<Arc<lapin::Connection>>,
    pub event_broadcast_hub: Option<EventBroadcastHub>,
    pub market_feed_supervisor: Option<MarketFeedSupervisorHandle>,
    pub email_sender: Option<Arc<dyn EmailSender>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Arc::new(settings),
            mysql: None,
            mongo: None,
            redis: None,
            auth_manager: None,
            rabbitmq: None,
            event_broadcast_hub: None,
            market_feed_supervisor: None,
            email_sender: None,
        }
    }

    pub fn with_mysql(mut self, mysql: Pool<MySql>) -> Self {
        self.mysql = Some(mysql);
        self
    }

    pub fn with_mongo(mut self, mongo: Database) -> Self {
        self.mongo = Some(mongo);
        self
    }

    pub fn with_redis(mut self, redis: ConnectionManager) -> Self {
        self.redis = Some(redis);
        self
    }

    pub fn with_auth_manager(mut self, auth_manager: Arc<SaTokenManager>) -> Self {
        self.auth_manager = Some(auth_manager);
        self
    }

    pub fn with_rabbitmq(mut self, rabbitmq: lapin::Connection) -> Self {
        self.rabbitmq = Some(Arc::new(rabbitmq));
        self
    }

    pub fn with_event_broadcast_hub(mut self, hub: EventBroadcastHub) -> Self {
        self.event_broadcast_hub = Some(hub);
        self
    }

    pub fn with_market_feed_supervisor(mut self, supervisor: MarketFeedSupervisorHandle) -> Self {
        self.market_feed_supervisor = Some(supervisor);
        self
    }

    pub fn with_email_sender(mut self, sender: Arc<dyn EmailSender>) -> Self {
        self.email_sender = Some(sender);
        self
    }
}
