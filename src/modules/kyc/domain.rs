//! kyc bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

use crate::architecture::DomainLayer;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

const MAX_DOCUMENT_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const DOCUMENT_PAYLOAD_PADDING_BYTES: u64 = 2048;
const IDENTITY_FRONT_DOCUMENT: &str = "identity_front";
const IDENTITY_BACK_DOCUMENT: &str = "identity_back";
const DEFAULT_DOCUMENT_TYPE: &str = "identity_card";
const DEFAULT_SUBMISSION_TYPE: &str = "personal";
const ENTERPRISE_SUBMISSION_TYPE: &str = "enterprise";
const SUPPORTED_DOCUMENT_TYPES: &[&str] = &[
    "identity_card",
    "passport",
    "driver_license",
    "residence_permit",
];

#[derive(Debug)]
pub struct DomainLayerMarker;

impl DomainLayer for DomainLayerMarker {}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct KycCountryDocumentTypeRule {
    pub country: String,
    pub document_types: Vec<String>,
    #[serde(default)]
    pub handheld_document_types: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct KycConfigValidationInput {
    pub(crate) enabled: bool,
    pub(crate) target_kyc_level: i32,
    pub(crate) required_documents: Vec<String>,
    pub(crate) allowed_countries: Vec<String>,
    pub(crate) country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub(crate) max_document_size_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct ValidatedKycConfig {
    pub(crate) enabled: bool,
    pub(crate) target_kyc_level: i32,
    pub(crate) required_documents: Vec<String>,
    pub(crate) allowed_countries: Vec<String>,
    pub(crate) country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub(crate) max_document_size_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct KycSubmissionConfigRules {
    pub(crate) required_documents: Vec<String>,
    pub(crate) allowed_countries: Vec<String>,
    pub(crate) country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub(crate) max_document_size_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct KycSubmissionValidationInput {
    pub(crate) real_name: String,
    pub(crate) country: String,
    pub(crate) id_number: String,
    pub(crate) submission_type: Option<String>,
    pub(crate) enterprise_name: Option<String>,
    pub(crate) business_registration_number: Option<String>,
    pub(crate) document_type: Option<String>,
    pub(crate) document_front_image: String,
    pub(crate) document_back_image: String,
    pub(crate) document_handheld_image: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ValidatedKycSubmission {
    pub(crate) real_name: String,
    pub(crate) country: String,
    pub(crate) id_number: String,
    pub(crate) submission_type: String,
    pub(crate) enterprise_name: Option<String>,
    pub(crate) business_registration_number: Option<String>,
    pub(crate) document_type: String,
    pub(crate) document_front_image: String,
    pub(crate) document_back_image: String,
    pub(crate) document_handheld_image: Option<String>,
}

pub(crate) fn validate_kyc_config(
    input: KycConfigValidationInput,
) -> AppResult<ValidatedKycConfig> {
    if input.target_kyc_level <= 0 {
        return Err(AppError::Validation(
            "target_kyc_level must be positive".to_owned(),
        ));
    }
    if input.max_document_size_bytes < 1024
        || input.max_document_size_bytes > MAX_DOCUMENT_SIZE_BYTES
    {
        return Err(AppError::Validation(format!(
            "max_document_size_bytes must be between 1024 and {MAX_DOCUMENT_SIZE_BYTES}"
        )));
    }
    let required_documents =
        normalize_unique_values(&input.required_documents, "required_documents", 64)?;
    if required_documents.is_empty() {
        return Err(AppError::Validation(
            "required_documents is required".to_owned(),
        ));
    }
    for document in &required_documents {
        if document != IDENTITY_FRONT_DOCUMENT && document != IDENTITY_BACK_DOCUMENT {
            return Err(AppError::Validation(
                "required_documents only supports identity_front and identity_back".to_owned(),
            ));
        }
    }

    Ok(ValidatedKycConfig {
        enabled: input.enabled,
        target_kyc_level: input.target_kyc_level,
        required_documents,
        allowed_countries: normalize_unique_values(
            &input.allowed_countries,
            "allowed_countries",
            128,
        )?,
        country_document_types: validate_country_document_types(&input.country_document_types)?,
        max_document_size_bytes: input.max_document_size_bytes,
    })
}

pub(crate) fn validate_kyc_submission(
    input: KycSubmissionValidationInput,
    config: &KycSubmissionConfigRules,
) -> AppResult<ValidatedKycSubmission> {
    let real_name = required_string(Some(input.real_name), "real_name", 128)?;
    let country = required_string(Some(input.country), "country", 128)?;
    let id_number = required_string(Some(input.id_number), "id_number", 128)?;
    let submission_type = validate_submission_type(
        optional_string(input.submission_type)
            .unwrap_or_else(|| DEFAULT_SUBMISSION_TYPE.to_owned()),
    )?;
    let enterprise_name = optional_string(input.enterprise_name);
    let business_registration_number = optional_string(input.business_registration_number);
    if submission_type == ENTERPRISE_SUBMISSION_TYPE {
        // 企业认证时，企业名称和统一社会信用代码均为必填项，方便后台识别主体。
        let _ = required_string(enterprise_name.clone(), "enterprise_name", 128)?;
        let _ = required_string(
            business_registration_number.clone(),
            "business_registration_number",
            128,
        )?;
    }

    let document_type = validate_document_type(
        optional_string(input.document_type).unwrap_or_else(|| DEFAULT_DOCUMENT_TYPE.to_owned()),
        "document_type",
    )?;

    if !config.allowed_countries.is_empty()
        && !config
            .allowed_countries
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(country.as_str()))
    {
        return Err(AppError::Validation("country is not allowed".to_owned()));
    }
    validate_document_type_allowed_for_country(&country, &document_type, config)?;

    let document_front_image = required_string(
        Some(input.document_front_image),
        "document_front_image",
        encoded_payload_limit(config.max_document_size_bytes) as usize,
    )?;
    let document_back_image = required_string(
        Some(input.document_back_image),
        "document_back_image",
        encoded_payload_limit(config.max_document_size_bytes) as usize,
    )?;
    let document_handheld_image = optional_string(input.document_handheld_image)
        .map(|value| {
            required_string(
                Some(value),
                "document_handheld_image",
                encoded_payload_limit(config.max_document_size_bytes) as usize,
            )
        })
        .transpose()?;

    if config
        .required_documents
        .iter()
        .any(|document| document == IDENTITY_FRONT_DOCUMENT)
        && document_front_image.is_empty()
    {
        return Err(AppError::Validation(
            "document_front_image is required".to_owned(),
        ));
    }
    if config
        .required_documents
        .iter()
        .any(|document| document == IDENTITY_BACK_DOCUMENT)
        && document_back_image.is_empty()
    {
        return Err(AppError::Validation(
            "document_back_image is required".to_owned(),
        ));
    }
    if requires_handheld_document_image(&country, &document_type, config)
        && document_handheld_image
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        return Err(AppError::Validation(
            "document_handheld_image is required".to_owned(),
        ));
    }

    Ok(ValidatedKycSubmission {
        real_name,
        country,
        id_number,
        submission_type,
        enterprise_name,
        business_registration_number,
        document_type,
        document_front_image,
        document_back_image,
        document_handheld_image,
    })
}

fn validate_country_document_types(
    rules: &[KycCountryDocumentTypeRule],
) -> AppResult<Vec<KycCountryDocumentTypeRule>> {
    let mut result = Vec::new();
    for rule in rules {
        let country = required_string(
            Some(rule.country.clone()),
            "country_document_types.country",
            128,
        )?;
        if result.iter().any(|current: &KycCountryDocumentTypeRule| {
            current.country.eq_ignore_ascii_case(&country)
        }) {
            return Err(AppError::Validation(
                "country_document_types has duplicated country".to_owned(),
            ));
        }

        let document_types = normalize_document_types(
            &rule.document_types,
            "country_document_types.document_types",
        )?;
        if document_types.is_empty() {
            return Err(AppError::Validation(
                "country_document_types.document_types is required".to_owned(),
            ));
        }
        let handheld_document_types = normalize_document_types(
            &rule.handheld_document_types,
            "country_document_types.handheld_document_types",
        )?;
        if handheld_document_types
            .iter()
            .any(|document_type| !document_types.contains(document_type))
        {
            return Err(AppError::Validation(
                "country_document_types.handheld_document_types must be included in document_types"
                    .to_owned(),
            ));
        }
        result.push(KycCountryDocumentTypeRule {
            country,
            document_types,
            handheld_document_types,
        });
    }
    Ok(result)
}

fn normalize_document_types(values: &[String], field: &str) -> AppResult<Vec<String>> {
    let mut result = Vec::new();
    for value in values {
        let document_type = validate_document_type(value.clone(), field)?;
        if !result.contains(&document_type) {
            result.push(document_type);
        }
    }
    Ok(result)
}

fn validate_document_type(value: String, field: &str) -> AppResult<String> {
    let document_type = required_string(Some(value), field, 64)?.to_ascii_lowercase();
    if !document_type
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return Err(AppError::Validation(format!("{field} format is invalid")));
    }
    if !SUPPORTED_DOCUMENT_TYPES.contains(&document_type.as_str()) {
        return Err(AppError::Validation(format!("{field} is not supported")));
    }
    Ok(document_type)
}

fn validate_submission_type(value: String) -> AppResult<String> {
    let submission_type = required_string(Some(value), "submission_type", 16)?.to_ascii_lowercase();
    if !matches!(
        submission_type.as_str(),
        DEFAULT_SUBMISSION_TYPE | ENTERPRISE_SUBMISSION_TYPE
    ) {
        return Err(AppError::Validation(
            "submission_type must be one of personal or enterprise".to_owned(),
        ));
    }
    Ok(submission_type)
}

fn validate_document_type_allowed_for_country(
    country: &str,
    document_type: &str,
    config: &KycSubmissionConfigRules,
) -> AppResult<()> {
    if config.country_document_types.is_empty() {
        return Ok(());
    }
    let Some(rule) = config
        .country_document_types
        .iter()
        .find(|rule| rule.country.eq_ignore_ascii_case(country))
    else {
        return Err(AppError::Validation(
            "country document types are not configured".to_owned(),
        ));
    };
    if rule
        .document_types
        .iter()
        .any(|allowed| allowed == document_type)
    {
        Ok(())
    } else {
        Err(AppError::Validation(
            "document_type is not allowed for country".to_owned(),
        ))
    }
}

fn requires_handheld_document_image(
    country: &str,
    document_type: &str,
    config: &KycSubmissionConfigRules,
) -> bool {
    config
        .country_document_types
        .iter()
        .find(|rule| rule.country.eq_ignore_ascii_case(country))
        .is_some_and(|rule| {
            rule.handheld_document_types
                .iter()
                .any(|required| required == document_type)
        })
}

fn normalize_unique_values(
    values: &[String],
    field: &str,
    max_chars: usize,
) -> AppResult<Vec<String>> {
    let mut result = Vec::new();
    for value in values {
        let item = required_string(Some(value.clone()), field, max_chars)?;
        if !result.iter().any(|current: &String| current == &item) {
            result.push(item);
        }
    }
    Ok(result)
}

pub(crate) fn required_string(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> AppResult<String> {
    let Some(value) = optional_string(value) else {
        return Err(AppError::Validation(format!("{field} is required")));
    };
    if value.chars().count() > max_chars {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(value)
}

pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn encoded_payload_limit(size_bytes: u64) -> u64 {
    size_bytes
        .saturating_mul(4)
        .saturating_div(3)
        .saturating_add(DOCUMENT_PAYLOAD_PADDING_BYTES)
}

pub(crate) fn validate_kyc_status(value: &str) -> AppResult<String> {
    let status = value.trim().to_ascii_lowercase();
    if matches!(status.as_str(), "pending" | "approved" | "rejected") {
        Ok(status)
    } else {
        Err(AppError::Validation("status is invalid".to_owned()))
    }
}

pub(crate) fn validate_review_status(value: &str) -> AppResult<String> {
    let status = validate_kyc_status(value)?;
    if status == "pending" {
        return Err(AppError::Validation(
            "review status cannot be pending".to_owned(),
        ));
    }
    Ok(status)
}
