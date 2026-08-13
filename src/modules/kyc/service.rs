//! kyc bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 本文件只承担一件事：把 KYC 配置与申请的响应对象转换为写入审计表的 JSON 快照。
//! 之所以独立成层，是因为审计快照的字段选择本身就是一项安全决策，
//! 需要一个不依赖数据库、可被单独测试和审阅的位置来固定这份口径。
//! 脱敏边界：证件号在此被掩码，证件图片只记录「是否已上传」而不记录内容；
//! 但姓名、邮箱、电话、企业注册号与审核理由仍以原文进入快照，
//! 因此输出只适合写入受控的审计存储，绝不可复制到普通日志或对外响应。
//! 时间统一序列化为 Unix 毫秒整数，避免审计记录受时区或格式化配置影响而产生歧义。
//! 本层不执行任何写入，落库与事务边界由调用方掌握。

use crate::modules::kyc::presentation::{KycConfigResponse, KycSubmissionResponse};
use serde_json::{Value, json};

/// 把一份 KYC 配置快照转成审计 JSON，用于记录运营对认证规则的每次调整。
/// 字段按白名单逐个列出而非整体序列化响应对象，这样响应结构新增字段时不会被动混入审计，
/// 审计内容的变化始终是一次显式改动。
/// 两个时间戳统一输出为 Unix 毫秒整数，与申请审计保持同一时间口径。
/// 配置本身不含个人数据，因此无需脱敏，与申请审计的处理方式完全不同。
/// 只构造快照不写库，调用方须把它与配置更新放进同一事务，才能保证改动与审计同生共死。
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

/// 对证件号做保留首尾的掩码，使审计既能核对同一证件是否重复提交，又不留存完整号码。
/// 长度超过八个字符时保留前四位与后四位，中间固定替换为四个星号，
/// 注意星号数量是固定的而非按被遮盖长度补齐，因此无法从掩码结果反推原始长度。
/// 长度不超过八个字符时整串替换为等长星号：此时保留首尾四位会暴露几乎全部内容，只能全遮。
/// 按 Unicode 字符而非字节切分，含中文的证件号不会被截断成无效编码。
fn mask_identity_number(value: &str) -> String {
    let length = value.chars().count();
    if length <= 8 {
        return "*".repeat(length);
    }
    let prefix: String = value.chars().take(4).collect();
    let suffix: String = value.chars().skip(length - 4).collect();
    format!("{prefix}****{suffix}")
}
