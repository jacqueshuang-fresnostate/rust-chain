//! 交易所后端服务的库根：对外导出各限界上下文模块，并集中装配整个 HTTP 路由树。
//! 路由按调用方分成三段前缀，用户端挂在 `/api/v1`，后台挂在 `/admin/api/v1`，代理门户挂在 `/agent/api/v1`。
//! 健康检查、接口文档与事件推送不加前缀，直接挂在根路径，方便探针与网关无差别访问。
//! 二进制入口只负责建立外部依赖并拉起后台任务，请求处理链路的组合全部收敛在本文件的路由装配里。

pub mod architecture;
pub mod bootstrap;
pub mod config;
pub mod error;
pub mod infra;
pub mod modules;
pub mod openapi;
pub mod state;
pub mod time;
pub mod workers;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use state::AppState;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::ToSchema;

/// 装配全站路由树：先按调用方把子路由合并成用户端、后台与代理门户三组，再统一挂到各自的路径前缀下。
/// 用户端聚合了认证、钱包、行情、现货、秒合约、杠杆、理财、借贷、竞猜、新闻等全部面向终端用户的模块。
/// 后台与代理门户只合并各自需要的子集，因此同一业务模块常常提供多套路由函数，按端暴露不同的操作能力。
/// 事件路由被有意挂载两次，既保留根路径上的推送入口，也提供带用户端前缀的版本，两者共享同一批处理器。
/// 最后统一注入共享状态并叠加 CORS 与链路追踪层；此处 CORS 采用宽松策略，跨域收敛需要在网关侧完成。
pub fn build_router(state: AppState) -> Router {
    let user_api = Router::new()
        .merge(modules::auth::routes::user_routes())
        .merge(modules::countries::routes())
        .merge(modules::platform::routes())
        .merge(modules::user::routes::routes())
        .merge(modules::wallet::routes::routes())
        .merge(modules::quick_recharge::user_routes())
        .merge(modules::quick_recharge::public_routes())
        .merge(modules::market::routes::routes())
        .merge(modules::spot::routes::routes())
        .merge(modules::new_coin::routes::user_routes())
        .merge(modules::convert::routes::user_routes())
        .merge(modules::seconds_contract::routes::user_routes())
        .merge(modules::margin::routes::user_routes())
        .merge(modules::earn::routes::user_routes())
        .merge(modules::loan::user_routes())
        .merge(modules::prediction::user_routes())
        .merge(modules::news::routes::routes())
        .merge(modules::events::routes::routes());

    let admin_api = Router::new()
        .merge(modules::auth::routes::admin_routes())
        .merge(modules::wallet::routes::admin_routes())
        .merge(modules::spot::routes::admin_routes())
        .merge(modules::admin::routes::routes())
        .merge(modules::quick_recharge::admin_routes())
        .merge(modules::seconds_contract::routes::admin_routes())
        .merge(modules::margin::routes::admin_routes())
        .merge(modules::earn::routes::admin_routes())
        .merge(modules::loan::admin_routes())
        .merge(modules::prediction::admin_routes());

    let agent_api = Router::new()
        .merge(modules::auth::routes::agent_routes())
        .merge(modules::agent::routes::routes());

    Router::new()
        .route("/health", get(health))
        .merge(openapi::routes())
        .merge(modules::events::routes::routes())
        .nest("/api/v1", user_api)
        .nest("/admin/api/v1", admin_api)
        .nest("/agent/api/v1", agent_api)
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    status: &'static str,
}

/// 处理存活探针请求，恒定返回状态为 ok 的固定响应体，用于容器编排判断进程是否还在接受连接。
/// 这里刻意忽略共享状态，不去探测 MySQL、Redis 等外部依赖，因此依赖故障时该接口依然返回成功。
/// 需要区分「进程活着」与「依赖可用」时，应另行提供就绪检查，不要把判断逻辑加进这个探针。
pub async fn health(State(_state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[cfg(test)]
#[path = "../tests/unit_src/src_lib_tests.rs"]
mod tests;
