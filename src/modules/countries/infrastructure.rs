//! countries bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer, error::AppResult, modules::countries::domain::PublicCountry,
};
use sqlx::{MySql, Pool, types::Json as SqlxJson};

#[derive(Debug)]
pub struct CountryConfigRepository;

impl InfrastructureLayer for CountryConfigRepository {}

#[derive(Debug, sqlx::FromRow)]
struct PublicCountryRow {
    country_code: String,
    country_name: String,
    default_locale: String,
    supported_locales: SqlxJson<Vec<String>>,
}

impl From<PublicCountryRow> for PublicCountry {
    fn from(row: PublicCountryRow) -> Self {
        Self {
            country_code: row.country_code,
            country_name: row.country_name,
            default_locale: row.default_locale,
            supported_locales: row.supported_locales.0,
        }
    }
}

pub async fn fetch_public_countries(pool: &Pool<MySql>) -> AppResult<Vec<PublicCountry>> {
    let rows = sqlx::query_as::<_, PublicCountryRow>(
        r#"SELECT country_code, country_name, default_locale, supported_locales
           FROM country_configs
           WHERE registration_enabled = TRUE AND status = 'active'
           ORDER BY sort_order ASC, country_code ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}
