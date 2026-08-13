//! countries bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 国家配置上下文对外只有一个免登录只读用例，即取出开放注册的国家清单并转成响应结构。
//! 不开事务、不写库、不缓存，过滤与排序全部由基础设施层的 SQL 决定。

use crate::{
    architecture::ApplicationLayer,
    error::AppResult,
    modules::countries::{
        infrastructure::fetch_public_countries,
        presentation::{PublicCountriesResponse, PublicCountryResponse},
    },
};
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct ListPublicCountries;

impl ApplicationLayer for ListPublicCountries {}

/// 查询公开注册国家并转换为传输响应，只暴露启用且允许注册的配置。
/// 数据库错误原样向上传递；应用层不缓存结果，也不改变国家排序。
pub async fn list_public_countries(pool: &Pool<MySql>) -> AppResult<PublicCountriesResponse> {
    // 公开国家列表只暴露允许注册且启用的国家，避免前端展示不可用地区。
    let countries = fetch_public_countries(pool)
        .await?
        .into_iter()
        .map(PublicCountryResponse::from)
        .collect();
    Ok(PublicCountriesResponse { countries })
}
