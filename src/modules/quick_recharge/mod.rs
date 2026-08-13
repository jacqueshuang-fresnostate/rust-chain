//! quick_recharge bounded context 聚合模块。
//!
//! 将快速充值功能的领域、服务与路由汇聚到统一入口，遵循 DDD 边界约束。
//!
//! 业务链路为：后台配置 GMPay 渠道参数与商户密钥，用户按法币金额下单，服务端生成本地订单号并向支付方
//! 建单换回收款地址，用户完成付款后支付方异步回调，服务端验签、逐项比对订单信息后为用户钱包入账。
//! 全流程只有验签通过的回调会改动余额，下单与查询链路都不碰钱包。
//!
//! 对外只导出请求响应 DTO、三组路由构造函数，以及供其他上下文复用的 `gmpay_signature`；
//! 配置行、订单行与运行时配置等含敏感信息的类型不越出本模块。
pub mod application;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

pub use presentation::{
    AdminQuickRechargeOrdersResponse, CreateQuickRechargeOrderRequest,
    DeleteQuickRechargeOrderRequest, QuickRechargeConfigResponse, QuickRechargeOrderResponse,
    QuickRechargeOrdersQuery, QuickRechargeOrdersResponse, QuickRechargeReturnTarget,
    SaveQuickRechargeConfigRequest, TestQuickRechargeConfigRequest,
    TestQuickRechargeConfigResponse, UserQuickRechargeConfigResponse,
};
pub use service::gmpay_signature;

pub use routes::{admin_routes, public_routes, user_routes};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_quick_recharge_tests.rs"]
mod tests;
