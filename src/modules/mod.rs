//! modules bounded contexts 聚合入口。
//!
//! 按 DDD 上下文注册各业务域模块，避免横向引用绕过边界层次。
//! 每个子模块是一个限界上下文，内部统一按 domain、service、application、infrastructure、
//! presentation、routes 分层，跨上下文协作只允许经由对方的应用层入口，不得直接触达其仓储或 SQL。
//! 按职责大致可分为四类：交易撮合与持仓类的 spot、margin、seconds_contract、prediction、convert、new_coin；
//! 资金与账务类的 wallet、earn、loan、quick_recharge；用户与准入类的 auth、user、kyc、security、risk、agent；
//! 以及运营配置与后台类的 admin、platform、countries、news、market、events。
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
pub mod support;
pub mod user;
pub mod wallet;
