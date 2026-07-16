//! news bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::{
    architecture::PresentationLayer,
    modules::news::domain::{PublicNewsFilter, optional_string, route_limit, route_offset},
    time::{option_unix_millis, unix_millis},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

#[derive(Debug, Deserialize)]
pub struct PublicNewsQuery {
    pub category: Option<String>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub q: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl From<PublicNewsQuery> for PublicNewsFilter {
    fn from(query: PublicNewsQuery) -> Self {
        Self {
            category: optional_string(query.category),
            country_code: optional_string(query.country_code),
            locale: optional_string(query.locale),
            keyword: optional_string(query.q),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        }
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PublicNewsItemResponse {
    pub id: u64,
    pub title: String,
    pub banner_url: Option<String>,
    pub small_logo_url: Option<String>,
    pub category: String,
    pub status: String,
    pub country_code: Option<String>,
    pub default_locale: String,
    pub content_json: SqlxJson<Value>,
    #[serde(default, with = "option_unix_millis")]
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(with = "unix_millis")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "unix_millis")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl PresentationLayer for PublicNewsItemResponse {}

#[derive(Debug, Serialize)]
pub struct PublicNewsItemsResponse {
    pub news: Vec<PublicNewsItemResponse>,
}
