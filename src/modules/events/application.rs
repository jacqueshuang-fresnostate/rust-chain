//! events bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::{
    error::AppResult,
    modules::events::{presentation::PrivateWsQuery, service::PrivateWsAuth},
    state::AppState,
};

/// 由路由层构建完成的查询参数触发私有 WebSocket 鉴权，应用层统一对外部 token 进行消费。
pub(crate) async fn authorize_private_ws(
    state: &AppState,
    query: PrivateWsQuery,
) -> AppResult<PrivateWsAuth> {
    // 保持“路由只透传参数，身份解析集中到应用层”的边界。
    PrivateWsAuth::from_token_query(query.token.as_deref(), state).await
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_events_application_tests.rs"]
mod tests;
