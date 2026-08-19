//! 在线客服限界上下文。
//!
//! 一个用户只有一条持久会话，消息只追加不覆盖，双侧已读进度独立。
//! 用户的客服所有者只取服务端 `user_referrals.root_agent_id` 对应的 active 直属代理，
//! 不继承报表系统的代理子树可见性；管理员保留全局兜底。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
