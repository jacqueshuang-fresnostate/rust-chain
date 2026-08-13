//! platform bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 定义品牌配置的保存请求、对外响应以及审计快照的 JSON 形态，时间列统一序列化为 Unix 毫秒时间戳。
//! 响应结构同时用于公开读取与后台回显，因此不含任何按角色裁剪的分支。

use crate::{architecture::PresentationLayer, time::unix_millis};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// 后台保存品牌配置的请求体，可选字段缺省表示保留数据库中的现有取值。
#[derive(Debug, Deserialize)]
pub struct SavePlatformBrandRequest {
    /// 站点名称，必填且去空白后不得为空。
    pub platform_name: String,
    pub logo_url: Option<String>,
    /// 图表提供方，未升级的旧版管理端不会提交该字段，此时沿用已发布配置。
    #[serde(default)]
    pub chart_provider: Option<String>,
    /// 变更原因，仅写入审计日志，不影响配置内容本身。
    pub reason: Option<String>,
}

/// 品牌配置的对外响应，公开读取与后台回显共用同一结构。
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct PlatformBrandResponse {
    pub id: u64,
    /// 配置行名，固定为 default。
    pub name: String,
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: String,
    /// 最后一次保存该配置的管理员，从未被修改过时为空。
    pub updated_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
}

impl PresentationLayer for PlatformBrandResponse {}

/// 把品牌配置快照序列化成审计日志用的 JSON 对象，保存流程会对变更前后各调用一次。
/// 字段与对外响应保持一致，时间统一折算成毫秒时间戳，使前后两份快照可以直接逐字段比对出差异。
/// 集中在此拼装是为了避免应用层各处手写字段列表而导致审计内容随时间漂移；本函数不落库也不脱敏。
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
