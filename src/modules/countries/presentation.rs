//! countries bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::{architecture::PresentationLayer, modules::countries::domain::PublicCountry};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PublicCountryResponse {
    country_code: String,
    country_name: String,
    default_locale: String,
    supported_locales: Vec<String>,
}

impl PresentationLayer for PublicCountryResponse {}

#[derive(Debug, Serialize)]
pub struct PublicCountriesResponse {
    pub countries: Vec<PublicCountryResponse>,
}

impl From<PublicCountry> for PublicCountryResponse {
    fn from(country: PublicCountry) -> Self {
        Self {
            country_code: country.country_code,
            country_name: country.country_name,
            default_locale: country.default_locale,
            supported_locales: country.supported_locales,
        }
    }
}
