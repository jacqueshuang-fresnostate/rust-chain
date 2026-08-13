//! events 限界上下文：全站的事件总线，负责把业务变更可靠地送达内部消费者与在线客户端。
//!
//! 两条投递链路并存且语义不同。持久化链路走 outbox/inbox 模式：业务在自身事务内把事件写进 outbox，
//! 发布 worker 扫描后投递到 RabbitMQ，消费侧先在 inbox 登记去重再执行业务处理，
//! 整体为 at-least-once，重复由 inbox 的消费者名加消息标识去重键吸收，顺序不做保证。
//! 实时链路走进程内广播 hub 直连 WebSocket，用于给前端推送行情与用户私有事件，
//! 它不持久化也不重发，属于尽力投递，客户端最终应以查询接口为准。
//!
//! 一条贯穿全局的约束：WebSocket 事件必须在数据库事务提交成功之后才发布。
//! 事务尚未提交就推送，会让客户端看到随后被回滚的数据；各业务上下文因此普遍采用
//! 「用例返回是否首次执行的标记、由包装层在提交后广播」的写法。
//!
//! 失败处理上，outbox 与 inbox 共用最多 5 次、固定 30 秒间隔的退避策略，即等距重试而非指数退避；
//! 预算耗尽后进入死信终态，死信不会自动重投也不转发到独立交换机，只能由管理员经带审计的重排恢复。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;
pub use self::domain::{
    INBOX_CONSUMED, INBOX_DEAD_LETTER, INBOX_PROCESSING, INBOX_PROCESSING_LEASE_SECONDS,
    INBOX_PROCESSING_TOKEN_FORMAT, INBOX_RETRY, OUTBOX_DEAD_LETTER, OUTBOX_PENDING,
    OUTBOX_PUBLISHED, OUTBOX_RETRY,
};
pub use infrastructure::{MySqlEventInboxRepository, MySqlEventOutboxRepository};
pub use repository::{EventInboxRepository, EventOutboxRepository};

pub use service::*;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_events_mod_tests.rs"]
mod tests;
