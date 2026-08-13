//! countries bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 国家配置为纯只读接口，没有请求体结构，这里只定义单个国家与国家列表两个响应形态。

use crate::{architecture::PresentationLayer, modules::countries::domain::PublicCountry};
use serde::Serialize;

/// 单个国家的对外结构，字段私有，只能由领域对象转换得到。
#[derive(Debug, Serialize)]
pub struct PublicCountryResponse {
    country_code: String,
    country_name: String,
    default_locale: String,
    supported_locales: Vec<String>,
}

impl PresentationLayer for PublicCountryResponse {}

/// 国家列表响应，顺序沿用后台配置的排序，不做二次排序也不分页。
#[derive(Debug, Serialize)]
pub struct PublicCountriesResponse {
    pub countries: Vec<PublicCountryResponse>,
}

impl From<PublicCountry> for PublicCountryResponse {
    /// 把国家配置的领域对象逐字段搬进对外响应，字段名与语言顺序保持不变。
    /// 响应结构的字段均为私有，只能经由本转换构造，避免其他层绕过领域对象直接拼装出对外内容。
    fn from(country: PublicCountry) -> Self {
        Self {
            country_code: country.country_code,
            country_name: country.country_name,
            default_locale: country.default_locale,
            supported_locales: country.supported_locales,
        }
    }
}
