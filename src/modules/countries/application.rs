//! countries bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

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

pub async fn list_public_countries(pool: &Pool<MySql>) -> AppResult<PublicCountriesResponse> {
    // 公开国家列表只暴露允许注册且启用的国家，避免前端展示不可用地区。
    let countries = fetch_public_countries(pool)
        .await?
        .into_iter()
        .map(PublicCountryResponse::from)
        .collect();
    Ok(PublicCountriesResponse { countries })
}
