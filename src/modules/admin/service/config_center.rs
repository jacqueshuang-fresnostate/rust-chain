//! 后台配置中心目录、状态判定、过滤与错误摘要规则。

use crate::{
    error::{AppError, AppResult},
    modules::admin::repository::AdminConfigCenterFactRecord,
};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BTreeSet};

const CONFIG_CENTER_QUERY_MAX_CHARS: usize = 100;
const CONFIG_CENTER_ERROR_MAX_CHARS: usize = 160;

/// 后台配置域的稳定目录项；代码、中文名称、分组和页面路径均由后端维护，前端不得复制一份状态目录自行推断。
#[derive(Debug, Clone, Copy)]
pub(crate) struct AdminConfigCenterDefinition {
    pub(crate) code: &'static str,
    pub(crate) name: &'static str,
    pub(crate) group: &'static str,
    pub(crate) group_name: &'static str,
    pub(crate) config_path: &'static str,
    pub(crate) operation_path: Option<&'static str>,
}

/// 配置中心必须覆盖的主要设置域，数组顺序同时是 API 的稳定展示顺序。
pub(crate) const ADMIN_CONFIG_CENTER_DEFINITIONS: &[AdminConfigCenterDefinition] = &[
    AdminConfigCenterDefinition {
        code: "prediction_settings",
        name: "预测配置",
        group: "market",
        group_name: "行情与交易",
        config_path: "/admin/prediction/settings",
        operation_path: Some("/admin/prediction/sync"),
    },
    AdminConfigCenterDefinition {
        code: "market_feed",
        name: "行情订阅",
        group: "market",
        group_name: "行情与交易",
        config_path: "/admin/market/feed-config",
        operation_path: None,
    },
    AdminConfigCenterDefinition {
        code: "market_strategy",
        name: "行情策略",
        group: "market",
        group_name: "行情与交易",
        config_path: "/admin/market/strategies",
        operation_path: None,
    },
    AdminConfigCenterDefinition {
        code: "kyc_rules",
        name: "KYC 规则",
        group: "compliance",
        group_name: "合规与安全",
        config_path: "/admin/users/kyc/settings",
        operation_path: Some("/admin/users/kyc/reviews"),
    },
    AdminConfigCenterDefinition {
        code: "security_policy",
        name: "安全策略",
        group: "compliance",
        group_name: "合规与安全",
        config_path: "/admin/system/security-policy",
        operation_path: None,
    },
    AdminConfigCenterDefinition {
        code: "country_configs",
        name: "国家配置",
        group: "compliance",
        group_name: "合规与安全",
        config_path: "/admin/system/countries",
        operation_path: None,
    },
    AdminConfigCenterDefinition {
        code: "loan_products",
        name: "贷款产品",
        group: "products",
        group_name: "产品配置",
        config_path: "/admin/loan/products",
        operation_path: Some("/admin/loan/orders"),
    },
    AdminConfigCenterDefinition {
        code: "margin_products",
        name: "杠杆产品",
        group: "products",
        group_name: "产品配置",
        config_path: "/admin/margin/products",
        operation_path: Some("/admin/margin/positions"),
    },
    AdminConfigCenterDefinition {
        code: "seconds_contract_products",
        name: "秒合约产品",
        group: "products",
        group_name: "产品配置",
        config_path: "/admin/seconds-contract/products",
        operation_path: Some("/admin/seconds-contract/orders"),
    },
    AdminConfigCenterDefinition {
        code: "earn_products",
        name: "理财产品",
        group: "products",
        group_name: "产品配置",
        config_path: "/admin/earn/products",
        operation_path: Some("/admin/earn/subscriptions"),
    },
    AdminConfigCenterDefinition {
        code: "smtp",
        name: "SMTP 邮件",
        group: "platform",
        group_name: "平台集成",
        config_path: "/admin/system/smtp",
        operation_path: None,
    },
    AdminConfigCenterDefinition {
        code: "upload_storage",
        name: "上传存储",
        group: "platform",
        group_name: "平台集成",
        config_path: "/admin/system/uploads",
        operation_path: None,
    },
    AdminConfigCenterDefinition {
        code: "platform_brand",
        name: "平台品牌",
        group: "platform",
        group_name: "平台集成",
        config_path: "/admin/system/brand",
        operation_path: None,
    },
];

/// 配置中心对外使用的四态结果。
/// 判定优先级固定为未配置、运行异常、待应用、正常，使运行失败不会被版本差异掩盖。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdminConfigCenterStatus {
    Unconfigured,
    PendingApply,
    RuntimeError,
    Normal,
}

impl AdminConfigCenterStatus {
    /// 返回 API 与查询过滤共用的稳定 snake_case 状态码。
    pub(crate) const fn as_code(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::PendingApply => "pending_apply",
            Self::RuntimeError => "runtime_error",
            Self::Normal => "normal",
        }
    }
}

/// 配置域当前运行状态；静态设置使用 `not_applicable`，不能被前端误解为未知故障。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdminConfigCenterRuntimeStatus {
    NotApplicable,
    Unknown,
    Healthy,
    Running,
    Stopped,
    Error,
}

impl AdminConfigCenterRuntimeStatus {
    /// 返回运行状态的稳定 snake_case 代码，不携带数据库或进程内部错误文本。
    pub(crate) const fn as_code(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

/// 一个配置域经纯规则归一后的后台读模型，不含任何凭据值或原始错误。
#[derive(Debug, Clone)]
pub(crate) struct AdminConfigCenterItem {
    pub(crate) code: &'static str,
    pub(crate) name: &'static str,
    pub(crate) group: &'static str,
    pub(crate) group_name: &'static str,
    pub(crate) config_path: &'static str,
    pub(crate) operation_path: Option<&'static str>,
    pub(crate) configured_count: u64,
    pub(crate) config_status: AdminConfigCenterStatus,
    pub(crate) published_version: Option<u64>,
    pub(crate) applied_version: Option<u64>,
    pub(crate) runtime_status: AdminConfigCenterRuntimeStatus,
    pub(crate) last_modified_at: Option<DateTime<Utc>>,
    pub(crate) last_applied_at: Option<DateTime<Utc>>,
    pub(crate) last_tested_at: Option<DateTime<Utc>>,
    pub(crate) last_error_summary: Option<String>,
}

/// 搜索和分组后的状态分面计数；状态筛选不会反过来清空其他状态计数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AdminConfigCenterSummary {
    pub(crate) total: u64,
    pub(crate) unconfigured: u64,
    pub(crate) pending_apply: u64,
    pub(crate) runtime_error: u64,
    pub(crate) normal: u64,
}

/// 配置中心纯查询结果，`total` 是最终返回条数，`summary.total` 是应用状态筛选前的分面总数。
#[derive(Debug)]
pub(crate) struct AdminConfigCenterView {
    pub(crate) items: Vec<AdminConfigCenterItem>,
    pub(crate) total: u64,
    pub(crate) summary: AdminConfigCenterSummary,
}

/// 已规范化的配置中心过滤条件；空白参数折叠为空，非法分组或状态会在查询数据库前被拒绝。
#[derive(Debug, Default)]
pub(crate) struct AdminConfigCenterFilter {
    query: Option<String>,
    group: Option<String>,
    status: Option<AdminConfigCenterStatus>,
}

impl AdminConfigCenterFilter {
    /// 规范化 query/group/status；搜索最多一百个字符，分组和状态必须来自后端稳定目录。
    pub(crate) fn new(
        query: Option<String>,
        group: Option<String>,
        status: Option<String>,
    ) -> AppResult<Self> {
        let query = normalized_optional(query);
        if query
            .as_ref()
            .is_some_and(|value| value.chars().count() > CONFIG_CENTER_QUERY_MAX_CHARS)
        {
            return Err(AppError::Validation(
                "config center query is too long".to_owned(),
            ));
        }

        let group = normalized_optional(group).map(|value| value.to_ascii_lowercase());
        if group.as_ref().is_some_and(|group| {
            !ADMIN_CONFIG_CENTER_DEFINITIONS
                .iter()
                .any(|definition| definition.group == group)
        }) {
            return Err(AppError::Validation(
                "config center group is invalid".to_owned(),
            ));
        }

        let status = normalized_optional(status)
            .map(|value| parse_config_status(&value.to_ascii_lowercase()))
            .transpose()?;

        Ok(Self {
            query: query.map(|value| value.to_lowercase()),
            group,
            status,
        })
    }
}

/// 依据后端目录和权威事实构造配置中心结果，并依次应用搜索、分组、状态过滤。
/// 事实必须与十三个稳定代码一一对应；缺失、重复或额外代码表示 SQL 与目录漂移，直接失败而不伪装成未配置。
pub(crate) fn build_admin_config_center_view(
    facts: Vec<AdminConfigCenterFactRecord>,
    filter: AdminConfigCenterFilter,
) -> AppResult<AdminConfigCenterView> {
    let mut facts_by_code = exact_facts_by_code(facts)?;
    let mut items = Vec::with_capacity(ADMIN_CONFIG_CENTER_DEFINITIONS.len());
    for definition in ADMIN_CONFIG_CENTER_DEFINITIONS {
        let fact = facts_by_code.remove(definition.code).ok_or_else(|| {
            AppError::Internal("config center facts do not match the catalog".to_owned())
        })?;
        items.push(config_center_item(*definition, fact)?);
    }

    let mut scoped = items
        .into_iter()
        .filter(|item| matches_search(item, filter.query.as_deref()))
        .filter(|item| {
            filter
                .group
                .as_deref()
                .is_none_or(|group| item.group == group)
        })
        .collect::<Vec<_>>();
    let summary = summarize_config_center(&scoped);

    if let Some(status) = filter.status {
        scoped.retain(|item| item.config_status == status);
    }
    let total = scoped.len() as u64;

    Ok(AdminConfigCenterView {
        items: scoped,
        total,
        summary,
    })
}

/// 将内部错误折叠空白、识别常见凭据标记并限制为 160 个字符。
/// 一旦检测到令牌、密码、密钥、Authorization 或带用户信息的 URL，整段替换为固定提示，避免部分遮罩留下明文。
pub(crate) fn safe_admin_config_error_summary(error: Option<&str>) -> Option<String> {
    let normalized = error?
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return None;
    }

    let lowercase = normalized.to_lowercase();
    let contains_sensitive_marker = [
        "password",
        "passwd",
        "pwd=",
        "token",
        "secret",
        "api_key",
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "access_key",
        "private_key",
        "密码",
        "密钥",
        "令牌",
        "凭据",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker));
    let contains_url_user_info = lowercase.contains("://") && lowercase.contains('@');
    if contains_sensitive_marker || contains_url_user_info {
        return Some("运行错误包含敏感信息，详细内容已隐藏".to_owned());
    }

    let mut chars = normalized.chars();
    let summary = chars
        .by_ref()
        .take(CONFIG_CENTER_ERROR_MAX_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        Some(format!("{summary}…"))
    } else {
        Some(summary)
    }
}

fn config_center_item(
    definition: AdminConfigCenterDefinition,
    fact: AdminConfigCenterFactRecord,
) -> AppResult<AdminConfigCenterItem> {
    let runtime_status = parse_runtime_status(&fact.runtime_status)?;
    let config_status = derive_config_status(&fact, runtime_status);
    let last_error_summary = if runtime_status == AdminConfigCenterRuntimeStatus::Error {
        safe_admin_config_error_summary(fact.recent_error.as_deref())
            .or_else(|| Some("运行状态异常，详细信息请查看服务端日志".to_owned()))
    } else {
        None
    };

    Ok(AdminConfigCenterItem {
        code: definition.code,
        name: definition.name,
        group: definition.group,
        group_name: definition.group_name,
        config_path: definition.config_path,
        operation_path: definition.operation_path,
        configured_count: fact.configured_count,
        config_status,
        published_version: fact.published_version,
        applied_version: fact.applied_version,
        runtime_status,
        last_modified_at: fact.last_modified_at,
        last_applied_at: fact.last_applied_at,
        last_tested_at: fact.last_tested_at,
        last_error_summary,
    })
}

fn derive_config_status(
    fact: &AdminConfigCenterFactRecord,
    runtime_status: AdminConfigCenterRuntimeStatus,
) -> AdminConfigCenterStatus {
    if fact.configured_count == 0 {
        return AdminConfigCenterStatus::Unconfigured;
    }
    if runtime_status == AdminConfigCenterRuntimeStatus::Error {
        return AdminConfigCenterStatus::RuntimeError;
    }
    let version_pending =
        fact.published_version.is_some() && fact.published_version != fact.applied_version;
    if fact.pending_apply_count > 0 || version_pending {
        return AdminConfigCenterStatus::PendingApply;
    }
    AdminConfigCenterStatus::Normal
}

fn exact_facts_by_code(
    facts: Vec<AdminConfigCenterFactRecord>,
) -> AppResult<BTreeMap<String, AdminConfigCenterFactRecord>> {
    let expected = ADMIN_CONFIG_CENTER_DEFINITIONS
        .iter()
        .map(|definition| definition.code)
        .collect::<BTreeSet<_>>();
    let mut facts_by_code = BTreeMap::new();
    for fact in facts {
        if !expected.contains(fact.code.as_str())
            || facts_by_code.insert(fact.code.clone(), fact).is_some()
        {
            return Err(AppError::Internal(
                "config center facts do not match the catalog".to_owned(),
            ));
        }
    }
    if facts_by_code.len() != expected.len() {
        return Err(AppError::Internal(
            "config center facts do not match the catalog".to_owned(),
        ));
    }
    Ok(facts_by_code)
}

fn matches_search(item: &AdminConfigCenterItem, query: Option<&str>) -> bool {
    query.is_none_or(|query| {
        [
            item.code,
            item.name,
            item.group,
            item.group_name,
            item.config_path,
            item.operation_path.unwrap_or_default(),
        ]
        .iter()
        .any(|value| value.to_lowercase().contains(query))
    })
}

fn summarize_config_center(items: &[AdminConfigCenterItem]) -> AdminConfigCenterSummary {
    let mut summary = AdminConfigCenterSummary {
        total: items.len() as u64,
        ..AdminConfigCenterSummary::default()
    };
    for item in items {
        match item.config_status {
            AdminConfigCenterStatus::Unconfigured => summary.unconfigured += 1,
            AdminConfigCenterStatus::PendingApply => summary.pending_apply += 1,
            AdminConfigCenterStatus::RuntimeError => summary.runtime_error += 1,
            AdminConfigCenterStatus::Normal => summary.normal += 1,
        }
    }
    summary
}

fn parse_config_status(value: &str) -> AppResult<AdminConfigCenterStatus> {
    match value {
        "unconfigured" => Ok(AdminConfigCenterStatus::Unconfigured),
        "pending_apply" => Ok(AdminConfigCenterStatus::PendingApply),
        "runtime_error" => Ok(AdminConfigCenterStatus::RuntimeError),
        "normal" => Ok(AdminConfigCenterStatus::Normal),
        _ => Err(AppError::Validation(
            "config center status is invalid".to_owned(),
        )),
    }
}

fn parse_runtime_status(value: &str) -> AppResult<AdminConfigCenterRuntimeStatus> {
    match value {
        "not_applicable" => Ok(AdminConfigCenterRuntimeStatus::NotApplicable),
        "unknown" => Ok(AdminConfigCenterRuntimeStatus::Unknown),
        "healthy" => Ok(AdminConfigCenterRuntimeStatus::Healthy),
        "running" => Ok(AdminConfigCenterRuntimeStatus::Running),
        "stopped" => Ok(AdminConfigCenterRuntimeStatus::Stopped),
        "error" => Ok(AdminConfigCenterRuntimeStatus::Error),
        _ => Err(AppError::Internal(
            "config center runtime status is invalid".to_owned(),
        )),
    }
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_admin_service_config_center_tests.rs"]
mod tests;
