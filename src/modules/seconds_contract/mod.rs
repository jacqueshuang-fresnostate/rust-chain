//! 秒合约限界上下文：短周期二元期权式交易，用户选定方向后到期比对开仓价与结算价判定胜负。
//!
//! 业务链路为产品配置、开仓下单、到期结算、订单查询四段。产品由后台按交易对配置若干周期档位，
//! 每档独立设定赔率与投注额区间；用户按档位开仓，本金即时从共享现货钱包的可用余额扣除；
//! 到期后由结算 worker 或后台人工结算判定胜负，赢单按订单固化的赔率把本金加净收益一次性入账，
//! 输单不再退还本金。
//!
//! 模块按 DDD 分层：`routes` 负责鉴权与参数搬运，`application` 持有事务边界与用例编排，
//! `service` 提供无 I/O 的校验、赔付计算与事件拼装，`infrastructure` 独占 SQL 与 Redis 访问，
//! `repository` 定义读写数据契约，`presentation` 定义对外 DTO。
//!
//! 本上下文涉及真实资金，两条不变式贯穿全部代码：开仓价与结算价只能取自服务端行情，
//! 绝不采纳客户端上送；开仓以幂等键、结算以订单终态各自保证重放不会造成重复扣款或重复派奖。
pub mod application;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_seconds_contract_tests.rs"]
mod tests;
