//! prediction 路由层。
//!
//! 统一承接预测模块的用户与管理员 HTTP 入口，保持业务逻辑留在 application 层。
//! 本文件只声明路径、方法与处理器的绑定关系，不含任何校验、SQL 或资金逻辑。
//! 用户端与管理端分成两棵子路由，由上层挂载到各自的前缀与中间件之下，
//! 因此这里出现的路径都是相对路径，同名的 `/prediction/markets` 在两端指向不同处理器。
//! 鉴权不在本层声明，而是由各处理器自身的 `UserAuth` 与 `AdminAuth` 提取器完成，
//! 阅读时需回到 application 层才能确认某个端点是否需要登录。
//! 同一路径的多个方法通过链式调用挂载，读写共用路径时请一并检查两个处理器的语义差异。

use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch, post},
};

use super::application as app;

/// 装配面向普通用户的预测端点：公共配置、市场列表与详情、创建报价、查单与下单。
/// 前三个是无需登录的公开读接口，配置接口返回可下注资产与费率，市场接口只暴露可见标的。
/// `/prediction/orders` 一个路径承担两种语义，GET 查本人订单、POST 以报价加幂等键下单，
/// 两者都在处理器内经 `UserAuth` 鉴权，用户身份只取自会话不接受参数传入。
/// 下单是本模块唯一从用户侧发起的资金操作，其冻结与扣费的原子性由基础设施事务保证。
/// 报价与下单分离为两个端点，用户须先取报价再凭报价编号下单，路由层不校验两者的先后。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/prediction/config", get(app::get_user_config))
        .route("/prediction/markets", get(app::list_user_markets))
        .route("/prediction/markets/:id", get(app::get_user_market))
        .route("/prediction/quotes", post(app::create_quote))
        .route(
            "/prediction/orders",
            get(app::list_user_orders).post(app::create_order),
        )
}

/// 装配面向管理员的预测端点，覆盖全局设置、资产配置、市场管理、订单查看、结算与同步。
/// 全部处理器都要求 `AdminAuth`，且管理端的市场与订单查询不施加可见性或用户维度过滤，
/// 因此已隐藏市场与他人订单在此均可访问。
/// 资产配置提供两条等价写入路径：向集合 POST 时资产编号取自请求体，
/// 向单项 PATCH 时取自路径段，底层同为 upsert，配置不存在会新建而非报错。
/// 真正会移动资金的只有市场结算这一个端点，它按 yes、no 或 invalid 分别派奖或退款；
/// 手动同步虽以拉取上游为主，但在自动结算模式下也可能连带触发结算并动账。
/// 其余端点均为配置或只读，不影响在途订单已固化的费率与赔付上限。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/prediction/settings",
            get(app::get_admin_settings).patch(app::save_admin_settings),
        )
        .route(
            "/prediction/asset-configs",
            get(app::list_admin_asset_configs).post(app::upsert_admin_asset_config),
        )
        .route(
            "/prediction/asset-configs/:asset_id",
            patch(app::update_admin_asset_config),
        )
        .route("/prediction/markets", get(app::list_admin_markets))
        .route(
            "/prediction/markets/:id",
            get(app::get_admin_market).patch(app::update_admin_market),
        )
        .route(
            "/prediction/markets/:id/settle",
            post(app::settle_admin_market),
        )
        .route("/prediction/orders", get(app::list_admin_orders))
        .route("/prediction/orders/:id", get(app::get_admin_order))
        .route("/prediction/sync", post(app::trigger_admin_sync))
        .route("/prediction/sync/logs", get(app::list_admin_sync_logs))
}
