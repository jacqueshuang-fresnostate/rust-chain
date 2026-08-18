//! 后台配置中心只读聚合用例。

use crate::{
    error::AppResult,
    modules::admin::{
        infrastructure::load_admin_config_center_facts,
        presentation::{AdminConfigCenterQuery, AdminConfigCenterResponse},
        service::{AdminConfigCenterFilter, build_admin_config_center_view},
    },
};
use sqlx::{MySql, Pool};

use super::admin_mysql_pool;

/// 读取十三个配置域的同一语句快照，并在内存中执行稳定目录映射、纯状态判定和过滤。
/// 该用例不解密凭据、不写数据库也不触发发布或测试；SQL、目录或状态代码漂移会使整个请求失败，避免返回部分权威结果。
pub(crate) async fn list_admin_config_center(
    pool: Option<Pool<MySql>>,
    query: AdminConfigCenterQuery,
) -> AppResult<AdminConfigCenterResponse> {
    let filter = AdminConfigCenterFilter::new(query.query, query.group, query.status)?;
    let pool = admin_mysql_pool(pool)?;
    let facts = load_admin_config_center_facts(&pool).await?;
    Ok(build_admin_config_center_view(facts, filter)?.into())
}
