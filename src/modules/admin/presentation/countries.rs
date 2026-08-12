//! 后台国家与注册区域配置的请求、响应 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminCountriesQuery {
    pub(crate) country_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) registration_enabled: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminCountriesQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminCountryRequest {
    pub(crate) country_code: String,
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) status: Option<String>,
    pub(crate) sort_order: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAdminCountryRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminCountryRequest {
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: Vec<String>,
    pub(crate) registration_enabled: bool,
    pub(crate) sort_order: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminCountryRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminCountryStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminCountryStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminCountryResponse {
    pub(crate) id: u64,
    pub(crate) country_code: String,
    pub(crate) country_name: String,
    pub(crate) remark: String,
    pub(crate) default_locale: String,
    pub(crate) supported_locales: SqlxJson<Vec<String>>,
    pub(crate) registration_enabled: bool,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminCountryResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminCountriesResponse {
    pub(crate) countries: Vec<AdminCountryResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminCountriesResponse {}
