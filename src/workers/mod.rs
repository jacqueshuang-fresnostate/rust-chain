//! 后台常驻任务聚合入口。
//!
//! 这里注册全部按固定间隔轮询运行的 worker，它们与 HTTP 请求链路互不阻塞，
//! 负责推进那些无法在单次请求内完成的状态迁移：代理佣金结算、新币锁仓解禁、杠杆计息与强平、
//! 理财到期赎回、借贷逾期处置、秒合约到期结算、行情采集与 K 线补齐、链上钱包对账，
//! 以及事件收发箱的投递与消费。
//! 各任务遵循同一套约定：单轮处理有配额上限，单项失败只记录并继续而不中断整轮，
//! 幂等一律由数据库状态或唯一键承担，因此进程重启后未完成的记录会被重新捞起处理。

pub mod agent_commission_settlement;
pub mod earn_auto_redemption;
pub mod event_inbox;
pub mod event_outbox;
pub mod kline_recovery;
pub mod loan_overdue;
pub mod margin_interest;
pub mod margin_liquidation;
pub mod market_feed;
pub mod seconds_contract_settlement;
pub mod synthetic_market;
pub mod unlock_scanner;
pub mod wallet_chain;
