//! kyc bounded context 聚合模块。
//!
//! 统一管理 KYC 提交与审核流程的 DDD 分层暴露，保持路由、应用、领域、仓储的边界清晰。
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub(crate) use application::{
    create_user_kyc_submission_in_tx, latest_kyc_submission, list_kyc_submissions, load_kyc_config,
    load_kyc_submission, review_kyc_submission_in_tx, save_kyc_config_in_tx,
};
pub use presentation::{
    KycConfigChange, KycConfigResponse, KycCountryDocumentTypeRule, KycReviewChange,
    KycStatusResponse, KycSubmissionResponse, KycSubmissionSummary, KycSubmissionsResponse,
    ListKycSubmissionsFilter, ReviewKycSubmissionRequest, SaveKycConfigRequest, SubmitKycRequest,
};
pub use service::{kyc_config_audit_json, kyc_submission_audit_json};
