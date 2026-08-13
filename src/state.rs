//! 应用共享状态：把启动阶段建立好的外部依赖收拢成一个可克隆句柄，作为 Axum 的 `State` 注入所有路由与后台协程。
//! 每个依赖都是 `Option`，缺失代表该资源在本次启动中未接入，处理器必须自行判空并按业务语义降级或报错。
//! 装配采用链式 `with_*` 方法逐项填充，未调用的字段保持为空，因此测试可以只挂载真正需要的依赖。
//! 克隆得到的副本共享同一批底层连接与句柄，不会重新建连，也不会复制连接池或广播通道本身。

use crate::{
    config::Settings, infra::email::EmailSender, modules::events::EventBroadcastHub,
    workers::market_feed::MarketFeedSupervisorHandle,
};
use mongodb::Database;
use redis::aio::ConnectionManager;
use sa_token_core::SaTokenManager;
use sqlx::{MySql, Pool};
use std::sync::Arc;

/// 应用运行时共享的 MySQL 连接池类型别名，供传输层装配依赖而不直接引用 SQLx SDK。
pub type MySqlPool = Pool<MySql>;

/// 跨请求共享的运行时依赖集合，克隆成本只是若干指针复制，可以放心按值传给每个处理器和协程。
/// 除配置外全部为可选依赖，判空结果直接决定某项功能在当前部署下是否可用。
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub mysql: Option<MySqlPool>,
    pub mongo: Option<Database>,
    pub redis: Option<ConnectionManager>,
    pub auth_manager: Option<Arc<SaTokenManager>>,
    pub rabbitmq: Option<Arc<lapin::Connection>>,
    pub event_broadcast_hub: Option<EventBroadcastHub>,
    pub market_feed_supervisor: Option<MarketFeedSupervisorHandle>,
    pub email_sender: Option<Arc<dyn EmailSender>>,
}

impl AppState {
    /// 以一份配置为起点建立共享状态，所有外部依赖初始为空，需要由调用方按需逐个挂载。
    /// 配置被包进 `Arc` 之后不可再改，运行期任何地方读到的都是启动时那份快照。
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

    /// 挂载 MySQL 连接池，它是订单、钱包、账本等全部强一致业务数据的唯一权威存储。
    /// 缺少这项时依赖数据库的接口只能报错，多数后台任务也会因判空而不被拉起。
    pub fn with_mysql(mut self, mysql: MySqlPool) -> Self {
        self.mysql = Some(mysql);
        self
    }

    /// 挂载 MongoDB 数据库句柄，用于承载 K 线这类体量大、按交易对分集合的行情历史数据。
    /// 句柄在构造时并未真正发起网络请求，因此这里挂载成功不代表 Mongo 当前可达。
    pub fn with_mongo(mut self, mongo: Database) -> Self {
        self.mongo = Some(mongo);
        self
    }

    /// 挂载 Redis 连接管理器，行情缓存、限频计数与各 worker 的协调键都走这一个共享实例。
    /// 该管理器内部自带断线重连，克隆副本仍复用同一条底层连接，不会成倍放大连接数。
    pub fn with_redis(mut self, redis: ConnectionManager) -> Self {
        self.redis = Some(redis);
        self
    }

    /// 挂载认证会话管理器，用户、管理员与代理三类登录态的签发、校验和注销都由它统一处理。
    /// 缺少这项时鉴权中间件无法验证令牌，需要登录的接口会整体不可用。
    pub fn with_auth_manager(mut self, auth_manager: Arc<SaTokenManager>) -> Self {
        self.auth_manager = Some(auth_manager);
        self
    }

    /// 挂载 RabbitMQ 连接，供事件 outbox 发布与 inbox 消费共用；连接在此被包进 `Arc` 以便跨协程共享。
    /// 只有连接本身被共享，channel 由各使用方自行开启，因此挂载不会声明任何队列或交换机。
    pub fn with_rabbitmq(mut self, rabbitmq: lapin::Connection) -> Self {
        self.rabbitmq = Some(Arc::new(rabbitmq));
        self
    }

    /// 挂载进程内事件广播 hub，作为 WebSocket 推送的扇出中枢，把业务事件分发给所有在线订阅者。
    /// 广播只在当前进程内有效且不持久化，多实例部署时各实例只能推送自己产生的消息。
    pub fn with_event_broadcast_hub(mut self, hub: EventBroadcastHub) -> Self {
        self.event_broadcast_hub = Some(hub);
        self
    }

    /// 挂载行情订阅监督器句柄，后台修改行情配置后通过它热重载订阅，无需重启进程。
    /// 句柄内部持有共享状态，克隆出来的副本操作的是同一个监督器，不会各自拉起一份订阅任务。
    pub fn with_market_feed_supervisor(mut self, supervisor: MarketFeedSupervisorHandle) -> Self {
        self.market_feed_supervisor = Some(supervisor);
        self
    }

    /// 挂载邮件发送实现，注册验证码、绑定邮箱与安全提醒等外发邮件都经由这个 trait 对象完成。
    /// 这里只提供发送能力，SMTP 主机与账号来自数据库中的后台配置，因此挂载时不需要任何凭据。
    pub fn with_email_sender(mut self, sender: Arc<dyn EmailSender>) -> Self {
        self.email_sender = Some(sender);
        self
    }
}
