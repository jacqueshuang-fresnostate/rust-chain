//! modules bounded contexts 聚合入口。
//!
//! 按 DDD 上下文注册各业务域模块，避免横向引用绕过边界层次。
pub mod admin;
pub mod agent;
pub mod auth;
pub mod convert;
pub mod countries;
pub mod earn;
pub mod events;
pub mod kyc;
pub mod loan;
pub mod margin;
pub mod market;
pub mod new_coin;
pub mod news;
pub mod platform;
pub mod prediction;
pub mod quick_recharge;
pub mod risk;
pub mod seconds_contract;
pub mod security;
pub mod spot;
pub mod user;
pub mod wallet;
