//! events bounded context service compatibility façade.
//!
//! 真实实现按 WebSocket、事件路由、重试、outbox、inbox、生产分发、指标告警与 RabbitMQ 适配拆分。
//! 本文件只保留模块声明和稳定 re-export，既有 `events::service::*` 与 `events::*` 路径保持不变。
//!
//! 拆分的目的是让单个文件聚焦一件事：`outbox` 与 `inbox` 各自处理一侧的批次编排，
//! `retry` 集中退避与死信判定，`routing` 负责频道与路由键规则，`websocket` 承载广播 hub 与连接循环，
//! `rabbitmq` 是 broker 适配器，`production_dispatch` 把消息分派给具体业务 handler，
//! `metrics` 汇总投递与积压指标。
//! 各子模块统一以通配 re-export 暴露，因此新增公开项无需改动本文件，
//! 但也要求子模块之间的公开名称保持互不冲突。

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
