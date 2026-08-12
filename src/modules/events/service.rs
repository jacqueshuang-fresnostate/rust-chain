//! events bounded context service compatibility façade.
//!
//! 真实实现按 WebSocket、事件路由、重试、outbox、inbox、生产分发、指标告警与 RabbitMQ 适配拆分。
//! 本文件只保留模块声明和稳定 re-export，既有 `events::service::*` 与 `events::*` 路径保持不变。

mod inbox;
mod metrics;
mod outbox;
mod production_dispatch;
mod rabbitmq;
mod retry;
mod routing;
mod websocket;

pub use inbox::*;
pub use metrics::*;
pub use outbox::*;
pub use production_dispatch::*;
pub use rabbitmq::*;
pub use retry::*;
pub use routing::*;
pub use websocket::*;
