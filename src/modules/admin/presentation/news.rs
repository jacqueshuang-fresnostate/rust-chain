//! 后台新闻内容、发布状态与分页查询 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminNewsQuery {
    pub(crate) status: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) q: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminNewsQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminNewsItemRequest {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) status: Option<String>,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAdminNewsItemRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminNewsItemRequest {
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: Value,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminNewsItemRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAdminNewsStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAdminNewsStatusRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminNewsItemResponse {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: String,
    pub(crate) status: String,
    pub(crate) country_code: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) content_json: SqlxJson<Value>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) created_by_admin_id: Option<u64>,
    pub(crate) updated_by_admin_id: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminNewsItemResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminNewsItemsResponse {
    pub(crate) news: Vec<AdminNewsItemResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminNewsItemsResponse {}
