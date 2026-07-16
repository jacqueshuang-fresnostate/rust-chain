//! kyc bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::kyc::{
        domain::{
            KycConfigValidationInput, KycSubmissionConfigRules, KycSubmissionValidationInput,
            optional_string, required_string, validate_kyc_config, validate_kyc_status,
            validate_kyc_submission, validate_review_status,
        },
        infrastructure::{
            ensure_default_config_in_tx, insert_user_kyc_submission_in_tx,
            latest_kyc_submission as latest_kyc_submission_from_infra,
            list_kyc_submissions as list_kyc_submissions_from_infra,
            load_kyc_config as load_kyc_config_from_infra, load_kyc_config_in_tx,
            load_kyc_submission as load_kyc_submission_from_infra, load_kyc_submission_in_tx,
            lock_kyc_config_in_tx, lock_kyc_submission_in_tx, lock_pending_kyc_submission_id_in_tx,
            lock_user_kyc_state_in_tx, update_kyc_submission_review_in_tx,
            update_user_kyc_level_in_tx, upsert_kyc_config_in_tx,
        },
        presentation::{
            KycConfigChange, KycConfigResponse, KycReviewChange, KycSubmissionResponse,
            KycSubmissionSummary, ListKycSubmissionsFilter, ReviewKycSubmissionRequest,
            SaveKycConfigRequest, SubmitKycRequest,
        },
    },
};
use sqlx::{MySql, Pool, Transaction};

pub(crate) async fn load_kyc_config(pool: &Pool<MySql>) -> AppResult<KycConfigResponse> {
    load_kyc_config_from_infra(pool).await
}

pub(crate) async fn save_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    request: SaveKycConfigRequest,
) -> AppResult<KycConfigChange> {
    // 保证配置读取、校验和写入都在同一事务中，避免配置快照与审计信息不一致。
    ensure_default_config_in_tx(tx).await?;
    let before = lock_kyc_config_in_tx(tx).await?;

    let config = validate_kyc_config(KycConfigValidationInput {
        enabled: request.enabled,
        target_kyc_level: request.target_kyc_level,
        required_documents: request.required_documents,
        allowed_countries: request.allowed_countries,
        country_document_types: request.country_document_types,
        max_document_size_bytes: request.max_document_size_bytes,
    })?;

    upsert_kyc_config_in_tx(
        tx,
        admin_id,
        config.enabled,
        config.target_kyc_level,
        config.required_documents,
        config.allowed_countries,
        config.country_document_types,
        config.max_document_size_bytes,
    )
    .await?;

    let after = load_kyc_config_in_tx(tx).await?;
    Ok(KycConfigChange { before, after })
}

pub(crate) async fn latest_kyc_submission(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<KycSubmissionResponse>> {
    latest_kyc_submission_from_infra(pool, user_id).await
}

pub(crate) async fn create_user_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    request: SubmitKycRequest,
) -> AppResult<KycSubmissionResponse> {
    let config = load_kyc_config_in_tx(tx).await?;
    if !config.enabled {
        return Err(AppError::Validation("kyc is disabled".to_owned()));
    }

    let user = lock_user_kyc_state_in_tx(tx, user_id).await?;
    if user.status != "active" {
        return Err(AppError::Unauthorized);
    }
    if user.kyc_level >= config.target_kyc_level {
        return Err(AppError::Conflict(
            "user kyc level already satisfies current config".to_owned(),
        ));
    }

    let pending_submission_id = lock_pending_kyc_submission_id_in_tx(tx, user_id).await?;
    if pending_submission_id.is_some() {
        return Err(AppError::Conflict(
            "kyc submission is already pending".to_owned(),
        ));
    }

    let submission = validate_kyc_submission(
        KycSubmissionValidationInput {
            real_name: request.real_name,
            country: request.country,
            id_number: request.id_number,
            submission_type: request.submission_type,
            enterprise_name: request.enterprise_name,
            business_registration_number: request.business_registration_number,
            document_type: request.document_type,
            document_front_image: request.document_front_image,
            document_back_image: request.document_back_image,
            document_handheld_image: request.document_handheld_image,
        },
        &KycSubmissionConfigRules {
            required_documents: config.required_documents.clone(),
            allowed_countries: config.allowed_countries.clone(),
            country_document_types: config.country_document_types.clone(),
            max_document_size_bytes: config.max_document_size_bytes,
        },
    )?;

    let submission_id = insert_user_kyc_submission_in_tx(
        tx,
        user_id,
        &submission.real_name,
        &submission.country,
        &submission.id_number,
        &submission.submission_type,
        submission.enterprise_name.as_deref(),
        submission.business_registration_number.as_deref(),
        &submission.document_type,
        &submission.document_front_image,
        &submission.document_back_image,
        &submission.document_handheld_image,
        config.target_kyc_level,
    )
    .await?;

    load_kyc_submission_in_tx(tx, submission_id).await
}

pub(crate) async fn list_kyc_submissions(
    pool: &Pool<MySql>,
    filter: ListKycSubmissionsFilter,
) -> AppResult<Vec<KycSubmissionSummary>> {
    let status = filter
        .status
        .as_deref()
        .map(validate_kyc_status)
        .transpose()?;
    let email = optional_string(filter.email);
    list_kyc_submissions_from_infra(
        pool,
        ListKycSubmissionsFilter {
            user_id: filter.user_id,
            email,
            status,
            limit: filter.limit,
        },
    )
    .await
}

pub(crate) async fn load_kyc_submission(
    pool: &Pool<MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    load_kyc_submission_from_infra(pool, submission_id).await
}

pub(crate) async fn review_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
    admin_id: u64,
    request: ReviewKycSubmissionRequest,
) -> AppResult<KycReviewChange> {
    let before = lock_kyc_submission_in_tx(tx, submission_id).await?;
    if before.status != "pending" {
        return Err(AppError::Conflict(
            "kyc submission is already reviewed".to_owned(),
        ));
    }

    let status = validate_review_status(&request.status)?;
    let approved_level = if status == "approved" {
        let level = request.kyc_level.unwrap_or(before.target_kyc_level);
        if level <= 0 {
            return Err(AppError::Validation(
                "kyc_level must be positive when approving".to_owned(),
            ));
        }
        Some(level)
    } else {
        None
    };

    let reason = required_string(request.reason, "reason", 512)?;
    update_kyc_submission_review_in_tx(tx, submission_id, admin_id, &status, &reason).await?;

    if let Some(level) = approved_level {
        update_user_kyc_level_in_tx(tx, before.user_id, level).await?;
    }

    let after = load_kyc_submission_in_tx(tx, submission_id).await?;
    Ok(KycReviewChange { before, after })
}
