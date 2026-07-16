//! kyc bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。

use crate::time::{option_unix_millis, unix_millis};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::modules::kyc::domain::KycCountryDocumentTypeRule;

#[derive(Debug, Deserialize)]
pub struct SaveKycConfigRequest {
    pub enabled: bool,
    pub target_kyc_level: i32,
    pub required_documents: Vec<String>,
    pub allowed_countries: Vec<String>,
    #[serde(default)]
    pub country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub max_document_size_bytes: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct KycConfigResponse {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub target_kyc_level: i32,
    pub required_documents: Vec<String>,
    pub allowed_countries: Vec<String>,
    pub country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub max_document_size_bytes: u64,
    pub updated_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct KycStatusResponse {
    pub config: KycConfigResponse,
    pub latest_submission: Option<KycSubmissionResponse>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitKycRequest {
    pub real_name: String,
    pub country: String,
    pub id_number: String,
    #[serde(default)]
    pub submission_type: Option<String>,
    #[serde(default)]
    pub enterprise_name: Option<String>,
    #[serde(default)]
    pub business_registration_number: Option<String>,
    pub document_type: Option<String>,
    pub document_front_image: String,
    pub document_back_image: String,
    pub document_handheld_image: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct KycSubmissionResponse {
    pub id: u64,
    pub user_id: u64,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub real_name: String,
    pub country: String,
    pub id_number: String,
    pub document_type: String,
    pub document_front_image: String,
    pub document_back_image: String,
    pub submission_type: String,
    pub enterprise_name: Option<String>,
    pub business_registration_number: Option<String>,
    pub document_handheld_image: Option<String>,
    pub status: String,
    pub target_kyc_level: i32,
    pub reviewed_by: Option<u64>,
    pub review_reason: Option<String>,
    #[serde(with = "unix_millis")]
    pub submitted_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct KycSubmissionSummary {
    pub id: u64,
    pub user_id: u64,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub real_name: String,
    pub country: String,
    pub id_number: String,
    pub submission_type: String,
    pub enterprise_name: Option<String>,
    pub business_registration_number: Option<String>,
    pub document_type: String,
    pub status: String,
    pub target_kyc_level: i32,
    pub reviewed_by: Option<u64>,
    pub review_reason: Option<String>,
    #[serde(with = "unix_millis")]
    pub submitted_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct KycSubmissionsResponse {
    pub submissions: Vec<KycSubmissionSummary>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewKycSubmissionRequest {
    pub status: String,
    pub kyc_level: Option<i32>,
    pub reason: Option<String>,
}

#[derive(Debug)]
pub struct ListKycSubmissionsFilter {
    pub user_id: Option<u64>,
    pub email: Option<String>,
    pub status: Option<String>,
    pub limit: u32,
}

#[derive(Debug)]
pub struct KycConfigChange {
    pub before: KycConfigResponse,
    pub after: KycConfigResponse,
}

#[derive(Debug)]
pub struct KycReviewChange {
    pub before: KycSubmissionResponse,
    pub after: KycSubmissionResponse,
}
