//! 高风险配置变更申请的用例编排。

use super::*;
use crate::modules::admin::{
    infrastructure::{
        AdminAuditLogEntry, AdminConfigChangeListFilter, insert_admin_audit_log_entry_in_tx,
        insert_admin_config_change_in_tx, list_admin_config_changes,
        load_admin_config_change_in_tx, mark_admin_config_change_applied_in_tx,
        update_admin_config_review_in_tx,
    },
    presentation::{
        AdminConfigChangeQuery, AdminConfigChangeResponse, AdminConfigChangesResponse,
        ApplyAdminConfigChangeRequest, CreateAdminConfigChangeRequest,
        ReviewAdminConfigChangeRequest,
    },
    repository::AdminConfigChangeWrite,
    service::{
        AdminConfigReviewDecision, AdminConfigTransition, normalize_config_change_metadata,
        required_admin_audit_reason, sanitize_admin_config_snapshot,
        validate_config_apply_transition, validate_config_review_transition,
    },
};
use serde_json::Value;

/// 创建待复核配置变更申请；申请与 `config_change.requested` 审计在同一事务提交。
/// before/proposed 在落库前递归脱敏，因此后续列表、审批和审计都不会回显凭据明文。
pub(crate) async fn create_admin_config_change(
    pool: &Pool<MySql>,
    admin_id: u64,
    request: CreateAdminConfigChangeRequest,
) -> AppResult<AdminConfigChangeResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let (config_domain, target_type, target_id, action, risk_level) =
        normalize_config_change_metadata(
            request.config_domain,
            request.target_type,
            request.target_id,
            request.action,
            request.risk_level,
        )?;
    if !request.proposed_json.is_object() {
        return Err(AppError::Validation(
            "proposed_json must be a JSON object".to_owned(),
        ));
    }

    let mut tx = pool.begin().await?;
    let record = insert_admin_config_change_in_tx(
        &mut tx,
        AdminConfigChangeWrite {
            request_no: format!("ACR-{}", Uuid::now_v7()),
            config_domain,
            target_type,
            target_id,
            action,
            base_revision: request.base_revision,
            before_json: request.before_json.map(sanitize_admin_config_snapshot),
            proposed_json: sanitize_admin_config_snapshot(request.proposed_json),
            reason: reason.clone(),
            risk_level,
            created_by: admin_id,
        },
    )
    .await?;
    let response = AdminConfigChangeResponse::from(record);
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "config_change.requested",
            target_type: "admin_config_change_request",
            target_id: response.id,
            before_json: None,
            after_json: Some(config_change_audit_json(&response)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}

/// 复核配置变更申请，强制制作人与复核人不同。
/// 完全相同的复核重试返回现有结果而不重复写审计；冲突结论或终态转换返回 409。
pub(crate) async fn review_admin_config_change(
    pool: &Pool<MySql>,
    admin_id: u64,
    id: u64,
    request: ReviewAdminConfigChangeRequest,
) -> AppResult<AdminConfigChangeResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let decision = AdminConfigReviewDecision::parse(&request.decision)?;
    let mut tx = pool.begin().await?;
    let before = load_admin_config_change_in_tx(&mut tx, id, true).await?;
    let transition = validate_config_review_transition(&before, admin_id, decision, &reason)?;
    if transition == AdminConfigTransition::Replay {
        tx.commit().await?;
        return Ok(before.into());
    }

    update_admin_config_review_in_tx(&mut tx, id, decision.status(), admin_id, &reason).await?;
    let after = load_admin_config_change_in_tx(&mut tx, id, false).await?;
    let response = AdminConfigChangeResponse::from(after);
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: match decision {
                AdminConfigReviewDecision::Approve => "config_change.approved",
                AdminConfigReviewDecision::Reject => "config_change.rejected",
            },
            target_type: "admin_config_change_request",
            target_id: id,
            before_json: Some(config_change_audit_json(&before.into())),
            after_json: Some(config_change_audit_json(&response)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}

/// 把已通过申请标记为已应用。
/// 该通用底座只负责审批状态与审计原子性；具体业务配置应先在同一业务用例中应用，再调用对应事务函数推进状态。
/// 重放 applied 请求直接返回既有快照，不会重复产生应用审计。
pub(crate) async fn apply_admin_config_change(
    pool: &Pool<MySql>,
    admin_id: u64,
    id: u64,
    request: ApplyAdminConfigChangeRequest,
) -> AppResult<AdminConfigChangeResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let mut tx = pool.begin().await?;
    let before = load_admin_config_change_in_tx(&mut tx, id, true).await?;
    if validate_config_apply_transition(&before)? == AdminConfigTransition::Replay {
        tx.commit().await?;
        return Ok(before.into());
    }

    mark_admin_config_change_applied_in_tx(&mut tx, id, admin_id).await?;
    let after = load_admin_config_change_in_tx(&mut tx, id, false).await?;
    let response = AdminConfigChangeResponse::from(after);
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "config_change.applied",
            target_type: "admin_config_change_request",
            target_id: id,
            before_json: Some(config_change_audit_json(&before.into())),
            after_json: Some(config_change_audit_json(&response)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}

/// 查询高风险变更申请，筛选值做去空白归一，分页口径与其他后台列表一致。
pub(crate) async fn list_admin_config_change_requests(
    pool: &Pool<MySql>,
    query: AdminConfigChangeQuery,
) -> AppResult<AdminConfigChangesResponse> {
    let (requests, total) = list_admin_config_changes(
        pool,
        AdminConfigChangeListFilter {
            config_domain: query.config_domain.and_then(optional_string),
            target_type: query.target_type.and_then(optional_string),
            status: query.status.and_then(optional_string),
            created_by: query.created_by,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminConfigChangesResponse {
        requests: requests.into_iter().map(Into::into).collect(),
        total,
    })
}

fn config_change_audit_json(response: &AdminConfigChangeResponse) -> Value {
    json!({
        "request_no": response.request_no,
        "config_domain": response.config_domain,
        "target_type": response.target_type,
        "target_id": response.target_id,
        "action": response.action,
        "base_revision": response.base_revision,
        "proposed_json": response.proposed_json,
        "risk_level": response.risk_level,
        "status": response.status,
        "created_by": response.created_by,
        "reviewed_by": response.reviewed_by,
        "applied_by": response.applied_by,
    })
}
