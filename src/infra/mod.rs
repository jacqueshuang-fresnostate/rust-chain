//! 跨限界上下文的基础设施适配层：集中收口对外部中间件和通用安全能力的接入，供各业务模块共享同一份实现。
//! 五个 `connect` 入口分别建立 MySQL、Mongo、Redis、RabbitMQ 连接和认证会话管理器，全部在启动装配阶段调用一次。
//! 这些入口只负责建连并返回可共享句柄，不做健康探测，也一律不提供内存或本地缓存形式的降级，失败必须让启动中止。
//! 邮件与密钥两个模块不涉及长连接：前者按调用方传入的配置临时建立 SMTP 传输，后者提供凭据加解密与掩码工具。
//! 业务专属的存储访问不放在这里，应留在各自上下文的基础设施层，本目录只承载真正被多个上下文复用的部分。

pub mod admin_request_context;
pub mod auth;
pub mod email;
pub mod mongo;
pub mod mysql;
pub mod rabbitmq;
pub mod redis;
pub mod secrets;
