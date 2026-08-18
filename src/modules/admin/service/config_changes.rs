//! 高风险配置变更的输入规范化、脱敏与双人复核状态机。

use crate::{
    error::{AppError, AppResult},
    modules::admin::repository::AdminConfigChangeRecord,
};
use serde_json::Value;

const METADATA_MAX_LEN: usize = 64;

/// 复核结论只允许通过或拒绝，避免传输层自由字符串扩散到持久化状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdminConfigReviewDecision {
    Approve,
    Reject,
}

impl AdminConfigReviewDecision {
    /// 把接口传入的复核动作归一为受约束枚举，兼容动词和完成态两种英文代码。
    /// 输入会先去除首尾空白；未知值返回可展示的校验错误，不会进入数据库状态字段或审计快照。
    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value.trim() {
            "approve" | "approved" => Ok(Self::Approve),
            "reject" | "rejected" => Ok(Self::Reject),
            _ => Err(AppError::Validation(
                "decision must be approve or reject".to_owned(),
            )),
        }
    }

    /// 返回持久化状态机使用的稳定完成态代码，确保复核校验、条件更新和审计筛选采用同一字面量。
    /// 该映射为纯函数且不会读取记录当前状态；是否允许转换仍由 `validate_config_review_transition` 判断。
    pub(crate) const fn status(self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
        }
    }
}

/// 状态机返回是否需要实际 UPDATE；`Replay` 表示完全相同的请求已经成功执行过。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdminConfigTransition {
    Apply,
    Replay,
}

/// 校验并归一配置域、目标、动作和风险等级。
/// 这些值会进入权限、查询和审计筛选，不允许空白、超长或包含任意标点。
pub(crate) fn normalize_config_change_metadata(
    config_domain: String,
    target_type: String,
    target_id: String,
    action: String,
    risk_level: Option<String>,
) -> AppResult<(String, String, String, String, String)> {
    let config_domain = normalized_code(config_domain, "config_domain")?;
    let target_type = normalized_code(target_type, "target_type")?;
    let target_id = normalized_target_id(target_id)?;
    let action = normalized_code(action, "action")?;
    let risk_level = risk_level
        .unwrap_or_else(|| "high".to_owned())
        .trim()
        .to_ascii_lowercase();
    if !matches!(risk_level.as_str(), "medium" | "high" | "critical") {
        return Err(AppError::Validation(
            "risk_level must be medium, high or critical".to_owned(),
        ));
    }
    Ok((config_domain, target_type, target_id, action, risk_level))
}

/// 对提交和审计 JSON 做递归脱敏。
/// 命中密码、令牌、私钥、API 凭据或加密密钥语义的字段只保留固定占位符，数组和嵌套对象同样处理。
pub(crate) fn sanitize_admin_config_snapshot(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    if is_sensitive_key(&key) {
                        (key, Value::String("***REDACTED***".to_owned()))
                    } else {
                        (key, sanitize_admin_config_snapshot(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(sanitize_admin_config_snapshot)
                .collect(),
        ),
        value => value,
    }
}

/// 校验复核转换并实现幂等重放。
/// 制作人永远不得复核自己的申请；只有 pending 可首次复核，完全相同的已完成复核返回 Replay。
pub(crate) fn validate_config_review_transition(
    record: &AdminConfigChangeRecord,
    reviewer_id: u64,
    decision: AdminConfigReviewDecision,
    review_reason: &str,
) -> AppResult<AdminConfigTransition> {
    if record.created_by == reviewer_id {
        return Err(AppError::Forbidden);
    }
    if record.status == "pending" {
        return Ok(AdminConfigTransition::Apply);
    }
    if record.status == decision.status()
        && record.reviewed_by == Some(reviewer_id)
        && record.review_reason.as_deref() == Some(review_reason)
    {
        return Ok(AdminConfigTransition::Replay);
    }
    Err(AppError::Conflict(format!(
        "config change request is already {}",
        record.status
    )))
}

/// 校验执行转换并实现幂等重放。
/// 仅 approved 可首次标记应用；applied 再次调用直接返回 Replay，其他终态拒绝推进。
pub(crate) fn validate_config_apply_transition(
    record: &AdminConfigChangeRecord,
) -> AppResult<AdminConfigTransition> {
    match record.status.as_str() {
        "approved" => Ok(AdminConfigTransition::Apply),
        "applied" => Ok(AdminConfigTransition::Replay),
        status => Err(AppError::Conflict(format!(
            "config change request cannot be applied from {status}"
        ))),
    }
}

fn normalized_code(value: String, field: &str) -> AppResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.chars().count() > METADATA_MAX_LEN
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(value)
}

fn normalized_target_id(value: String) -> AppResult<String> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.chars().count() > METADATA_MAX_LEN
        || value.chars().any(char::is_control)
    {
        return Err(AppError::Validation("target_id is invalid".to_owned()));
    }
    Ok(value)
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase().replace('-', "_");
    normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("credential")
        || normalized == "private_key"
        || normalized.ends_with("_private_key")
        || normalized == "api_key"
        || normalized.ends_with("_api_key")
        || normalized == "access_key"
        || normalized.ends_with("_access_key")
        || normalized == "encryption_key"
        || normalized.ends_with("_encryption_key")
}
