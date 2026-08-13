//! news bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 定义公开新闻的查询串结构与对外响应结构，时间列一律序列化为 Unix 毫秒时间戳。
//! 响应中的内容字段直接透出库中的多语言 JSON 文档，其结构为版本号、默认语言和内容项数组，
//! 每个内容项自带语言与国家代码；服务端不裁剪也不重排这些项，客户端据默认语言做回退选择。

use crate::{
    architecture::PresentationLayer,
    modules::news::domain::{PublicNewsFilter, optional_string, route_limit, route_offset},
    time::{option_unix_millis, unix_millis},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

/// 公开新闻列表的查询串，全部字段可选，缺省即该维度不参与过滤。
#[derive(Debug, Deserialize)]
pub struct PublicNewsQuery {
    pub category: Option<String>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    /// 关键词参数，同时匹配标题与多语言内容原文。
    pub q: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl From<PublicNewsQuery> for PublicNewsFilter {
    /// 把原始查询串折算成领域过滤条件：四个文本维度统一去空白并把空串折叠为不过滤，
    /// 分页则在此处夹到允许区间，因此进入应用层的条件已经是受控取值。
    /// 转换只做归一不做合法性判断，分类白名单与国家、语言的格式校验留到拼装 SQL 时才执行。
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

/// 单条公开新闻的对外结构，字段与数据库列一一对应，公共接口下状态恒为已发布。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PublicNewsItemResponse {
    pub id: u64,
    /// 列表用主标题，与内容项内的各语言标题相互独立。
    pub title: String,
    /// 大图横幅地址，未配图时为空。
    pub banner_url: Option<String>,
    /// 列表小图标地址，未配图时为空。
    pub small_logo_url: Option<String>,
    pub category: String,
    pub status: String,
    /// 目标国家代码，为空或 GLOBAL 表示面向所有地区展示。
    pub country_code: Option<String>,
    /// 默认语言，客户端在内容项中找不到用户语言时回退到该项。
    pub default_locale: String,
    /// 多语言内容文档，含版本号、默认语言与内容项数组，服务端原样透出不做裁剪。
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
