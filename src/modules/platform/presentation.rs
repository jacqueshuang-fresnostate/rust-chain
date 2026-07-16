//! platform bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::{architecture::PresentationLayer, time::unix_millis};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct SavePlatformBrandRequest {
    pub platform_name: String,
    pub logo_url: Option<String>,
    #[serde(default)]
    pub chart_provider: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct PlatformBrandResponse {
    pub id: u64,
    pub name: String,
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: String,
    pub updated_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
}

impl PresentationLayer for PlatformBrandResponse {}

/// 生成平台品牌变更审计日志所需的快照 JSON，避免在应用层手工拼接字段。
pub fn platform_brand_audit_json(brand: &PlatformBrandResponse) -> Value {
    json!({
        "id": brand.id,
        "name": brand.name,
        "platform_name": brand.platform_name,
        "logo_url": brand.logo_url,
        "chart_provider": brand.chart_provider,
        "updated_by": brand.updated_by,
        "created_at": brand.created_at.timestamp_millis(),
        "updated_at": brand.updated_at.timestamp_millis(),
    })
}
