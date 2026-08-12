//! kyc bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。

use crate::modules::kyc::presentation::{KycConfigResponse, KycSubmissionResponse};
use serde_json::{Value, json};

/// 将 KYC 配置快照转为稳定审计 JSON，时间统一使用毫秒且不暴露额外存储字段。
/// 函数仅构造快照而不写审计表，持久化失败和事务回滚语义由调用方负责。
pub fn kyc_config_audit_json(config: &KycConfigResponse) -> Value {
    json!({
        "id": config.id,
        "name": config.name,
        "enabled": config.enabled,
        "target_kyc_level": config.target_kyc_level,
        "required_documents": config.required_documents,
        "allowed_countries": config.allowed_countries,
        "country_document_types": config.country_document_types,
        "max_document_size_bytes": config.max_document_size_bytes,
        "updated_by": config.updated_by,
        "created_at": config.created_at.timestamp_millis(),
        "updated_at": config.updated_at.timestamp_millis(),
    })
}

/// 将 KYC 申请转为审计 JSON，身份号仅保留首尾四位，材料仅记录是否已设置。
/// 输出仍含姓名、邮箱、电话、企业登记号和审核理由等个人数据，仅适合受控审计存储；
/// 函数本身不写库，调用方须随业务事务持久化，并避免把该 JSON 复制到普通日志或公开响应。
pub fn kyc_submission_audit_json(submission: &KycSubmissionResponse) -> Value {
    json!({
        "id": submission.id,
        "user_id": submission.user_id,
        "email": submission.email,
        "phone": submission.phone,
        "real_name": submission.real_name,
        "country": submission.country,
        "submission_type": submission.submission_type,
        "enterprise_name": submission.enterprise_name,
        "business_registration_number": submission.business_registration_number,
        "id_number_mask": mask_identity_number(&submission.id_number),
        "document_type": submission.document_type,
        "document_front_image_set": !submission.document_front_image.is_empty(),
        "document_back_image_set": !submission.document_back_image.is_empty(),
        "document_handheld_image_set": submission.document_handheld_image.as_deref().is_some_and(|value| !value.is_empty()),
        "status": submission.status,
        "target_kyc_level": submission.target_kyc_level,
        "reviewed_by": submission.reviewed_by,
        "review_reason": submission.review_reason,
        "submitted_at": submission.submitted_at.timestamp_millis(),
        "reviewed_at": submission.reviewed_at.map(|value| value.timestamp_millis()),
        "created_at": submission.created_at.timestamp_millis(),
        "updated_at": submission.updated_at.timestamp_millis(),
    })
}

fn mask_identity_number(value: &str) -> String {
    let length = value.chars().count();
    if length <= 8 {
        return "*".repeat(length);
    }
    let prefix: String = value.chars().take(4).collect();
    let suffix: String = value.chars().skip(length - 4).collect();
    format!("{prefix}****{suffix}")
}
