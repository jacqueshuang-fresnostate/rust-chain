//! platform 路由层。
//!
//! 负责平台品牌接口的 HTTP 路由聚合，仅编排请求参数与应用服务调用。
//! 这里只暴露面向终端的品牌读取端点，配置修改由后台管理侧的路由负责，两者共用同一份应用层用例。

use crate::{error::AppResult, state::AppState};
use axum::{Json, Router, extract::State, routing::get};

/// 从 HTTP 状态中取得平台品牌查询使用的数据库连接池。
///
/// 本函数只承担传输层依赖装配；连接池缺失时返回明确错误，避免服务层依赖全局状态。
fn mysql_pool(state: &AppState) -> AppResult<crate::state::MySqlPool> {
    state.mysql.clone().ok_or_else(|| {
        crate::error::AppError::Internal(
            "mysql pool is not configured for platform route".to_owned(),
        )
    })
}

/// 装配平台品牌的公开读取端点，只注册一个免登录 GET 路径。
/// 品牌配置的写入不在这里，由后台管理路由在带审计的事务中完成，因此本路由表不含任何写方法。
pub fn routes() -> Router<AppState> {
    Router::new().route("/platform/brand", get(get_platform_brand_route))
}

/// 返回当前生效的站点名称、Logo 地址与图表提供方，供前端在启动时初始化品牌外观。
/// 首次访问会先幂等补齐默认配置行，因此全新部署也能拿到可用取值而不是未找到。
/// 无入参、无鉴权、不缓存，后台改完配置后下一次请求即刻生效。
async fn get_platform_brand_route(
    State(state): State<AppState>,
) -> AppResult<Json<super::presentation::PlatformBrandResponse>> {
    Ok(Json(
        super::application::load_platform_brand(&mysql_pool(&state)?).await?,
    ))
}
