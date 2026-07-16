//! kyc bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    error::{AppError, AppResult},
    modules::kyc::domain::KycCountryDocumentTypeRule,
    modules::kyc::presentation::{
        KycConfigResponse, KycSubmissionResponse, KycSubmissionSummary, ListKycSubmissionsFilter,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

const DEFAULT_CONFIG_NAME: &str = "default";

#[derive(Debug, sqlx::FromRow)]
struct KycConfigRow {
    id: u64,
    name: String,
    enabled: bool,
    target_kyc_level: i32,
    required_documents_json: SqlxJson<Vec<String>>,
    allowed_countries_json: SqlxJson<Vec<String>>,
    country_document_types_json: SqlxJson<Vec<KycCountryDocumentTypeRule>>,
    max_document_size_bytes: u64,
    updated_by: Option<u64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct KycSubmissionRow {
    id: u64,
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    real_name: String,
    country: String,
    id_number: String,
    submission_type: String,
    enterprise_name: Option<String>,
    business_registration_number: Option<String>,
    document_type: String,
    document_front_image: String,
    document_back_image: String,
    document_handheld_image: Option<String>,
    status: String,
    target_kyc_level: i32,
    reviewed_by: Option<u64>,
    review_reason: Option<String>,
    submitted_at: DateTime<Utc>,
    reviewed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct KycSubmissionSummaryRow {
    id: u64,
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    real_name: String,
    country: String,
    id_number: String,
    submission_type: String,
    enterprise_name: Option<String>,
    business_registration_number: Option<String>,
    document_type: String,
    status: String,
    target_kyc_level: i32,
    reviewed_by: Option<u64>,
    review_reason: Option<String>,
    submitted_at: DateTime<Utc>,
    reviewed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) struct UserKycStateRecord {
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
}

pub(crate) async fn load_kyc_config(pool: &Pool<MySql>) -> AppResult<KycConfigResponse> {
    ensure_default_config(pool).await?;
    let row = sqlx::query_as::<_, KycConfigRow>(&select_kyc_config_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(config_response(row))
}

pub(crate) async fn load_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<KycConfigResponse> {
    ensure_default_config_in_tx(tx).await?;
    let row = sqlx::query_as::<_, KycConfigRow>(&select_kyc_config_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(config_response(row))
}

pub(crate) async fn lock_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<KycConfigResponse> {
    let row = sqlx::query_as::<_, KycConfigRow>(&select_kyc_config_sql(true))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(config_response(row))
}

pub(crate) async fn upsert_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    enabled: bool,
    target_kyc_level: i32,
    required_documents: Vec<String>,
    allowed_countries: Vec<String>,
    country_document_types: Vec<KycCountryDocumentTypeRule>,
    max_document_size_bytes: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO kyc_configs
           (name, enabled, target_kyc_level, required_documents_json, allowed_countries_json, country_document_types_json, max_document_size_bytes, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE enabled = VALUES(enabled),
                                   target_kyc_level = VALUES(target_kyc_level),
                                   required_documents_json = VALUES(required_documents_json),
                                   allowed_countries_json = VALUES(allowed_countries_json),
                                   country_document_types_json = VALUES(country_document_types_json),
                                   max_document_size_bytes = VALUES(max_document_size_bytes),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .bind(enabled)
    .bind(target_kyc_level)
    .bind(SqlxJson(required_documents))
    .bind(SqlxJson(allowed_countries))
    .bind(SqlxJson(country_document_types))
    .bind(max_document_size_bytes)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn latest_kyc_submission(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<KycSubmissionResponse>> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.user_id = ? ORDER BY submissions.submitted_at DESC, submissions.id DESC LIMIT 1",
        select_kyc_submission_sql()
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(submission_response))
}

pub(crate) async fn list_kyc_submissions(
    pool: &Pool<MySql>,
    filter: ListKycSubmissionsFilter,
) -> AppResult<Vec<KycSubmissionSummary>> {
    let mut builder = QueryBuilder::<MySql>::new(select_kyc_submission_summary_sql());
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = filter.user_id {
        builder.push(" AND submissions.user_id = ");
        builder.push_bind(user_id);
    }
    if let Some(email) = filter.email {
        builder.push(" AND users.email = ");
        builder.push_bind(email);
    }
    if let Some(status) = filter.status {
        builder.push(" AND submissions.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY submissions.submitted_at DESC, submissions.id DESC LIMIT ");
    builder.push_bind(i64::from(filter.limit.clamp(1, 100)));

    let rows = builder
        .build_query_as::<KycSubmissionSummaryRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(submission_summary).collect())
}

pub(crate) async fn load_kyc_submission(
    pool: &Pool<MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.id = ? LIMIT 1",
        select_kyc_submission_sql()
    ))
    .bind(submission_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(submission_response(row))
}

pub(crate) async fn load_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.id = ? LIMIT 1",
        select_kyc_submission_sql()
    ))
    .bind(submission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(submission_response(row))
}

pub(crate) async fn lock_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.id = ? LIMIT 1 FOR UPDATE",
        select_kyc_submission_sql()
    ))
    .bind(submission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(submission_response(row))
}

pub(crate) async fn lock_user_kyc_state_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserKycStateRecord> {
    sqlx::query_as::<_, (String, i32)>(
        r#"SELECT status, kyc_level
           FROM users
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|(status, kyc_level)| UserKycStateRecord { status, kyc_level })
    .ok_or(AppError::Unauthorized)
}

pub(crate) async fn lock_pending_kyc_submission_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<u64>> {
    sqlx::query_scalar(
        r#"SELECT id
           FROM user_kyc_submissions
           WHERE user_id = ? AND status = 'pending'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn insert_user_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    real_name: &str,
    country: &str,
    id_number: &str,
    submission_type: &str,
    enterprise_name: Option<&str>,
    business_registration_number: Option<&str>,
    document_type: &str,
    document_front_image: &str,
    document_back_image: &str,
    document_handheld_image: &Option<String>,
    target_kyc_level: i32,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO user_kyc_submissions
           (user_id, real_name, country, id_number, submission_type, enterprise_name, business_registration_number, document_type, document_front_image, document_back_image, document_handheld_image, status, target_kyc_level)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(user_id)
    .bind(real_name)
    .bind(country)
    .bind(id_number)
    .bind(submission_type)
    .bind(enterprise_name)
    .bind(business_registration_number)
    .bind(document_type)
    .bind(document_front_image)
    .bind(document_back_image)
    .bind(document_handheld_image)
    .bind(target_kyc_level)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_kyc_submission_review_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
    admin_id: u64,
    status: &str,
    reason: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_kyc_submissions
           SET status = ?, reviewed_by = ?, review_reason = ?, reviewed_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(status)
    .bind(admin_id)
    .bind(reason)
    .bind(submission_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_user_kyc_level_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    level: i32,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE users
           SET kyc_level = GREATEST(kyc_level, ?)
           WHERE id = ?"#,
    )
    .bind(level)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn ensure_default_config(pool: &Pool<MySql>) -> AppResult<()> {
    sqlx::query(default_config_insert_sql())
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn ensure_default_config_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<()> {
    sqlx::query(default_config_insert_sql())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn default_config_insert_sql() -> &'static str {
    r#"INSERT INTO kyc_configs
       (name, enabled, target_kyc_level, required_documents_json, allowed_countries_json, country_document_types_json, max_document_size_bytes)
       VALUES ('default', TRUE, 1, JSON_ARRAY('identity_front', 'identity_back'), JSON_ARRAY(), JSON_ARRAY(), 5242880)
       ON DUPLICATE KEY UPDATE name = name"#
}

fn select_kyc_config_sql(for_update: bool) -> String {
    let mut sql = String::from(
        r#"SELECT id, name, enabled, target_kyc_level, required_documents_json,
                  allowed_countries_json, country_document_types_json, max_document_size_bytes, updated_by, created_at, updated_at
           FROM kyc_configs
           WHERE name = ?"#,
    );
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn select_kyc_submission_summary_sql() -> &'static str {
    r#"SELECT submissions.id, submissions.user_id, users.email, users.phone,
              submissions.real_name, submissions.country, submissions.id_number, submissions.submission_type,
              submissions.enterprise_name, submissions.business_registration_number,
              submissions.document_type, submissions.status, submissions.target_kyc_level,
              submissions.reviewed_by, submissions.review_reason, submissions.submitted_at,
              submissions.reviewed_at, submissions.updated_at
       FROM user_kyc_submissions submissions
       LEFT JOIN users ON users.id = submissions.user_id"#
}

fn select_kyc_submission_sql() -> &'static str {
    r#"SELECT submissions.id, submissions.user_id, users.email, users.phone,
              submissions.real_name, submissions.country, submissions.id_number, submissions.submission_type,
              submissions.enterprise_name, submissions.business_registration_number,
              submissions.document_type, submissions.document_front_image, submissions.document_back_image,
              submissions.document_handheld_image,
              submissions.status, submissions.target_kyc_level, submissions.reviewed_by,
              submissions.review_reason, submissions.submitted_at, submissions.reviewed_at,
              submissions.created_at, submissions.updated_at
       FROM user_kyc_submissions submissions
       LEFT JOIN users ON users.id = submissions.user_id"#
}

fn config_response(row: KycConfigRow) -> KycConfigResponse {
    KycConfigResponse {
        id: row.id,
        name: row.name,
        enabled: row.enabled,
        target_kyc_level: row.target_kyc_level,
        required_documents: row.required_documents_json.0,
        allowed_countries: row.allowed_countries_json.0,
        country_document_types: row.country_document_types_json.0,
        max_document_size_bytes: row.max_document_size_bytes,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn submission_response(row: KycSubmissionRow) -> KycSubmissionResponse {
    KycSubmissionResponse {
        id: row.id,
        user_id: row.user_id,
        email: row.email,
        phone: row.phone,
        real_name: row.real_name,
        country: row.country,
        id_number: row.id_number,
        submission_type: row.submission_type,
        enterprise_name: row.enterprise_name,
        business_registration_number: row.business_registration_number,
        document_type: row.document_type,
        document_front_image: row.document_front_image,
        document_back_image: row.document_back_image,
        document_handheld_image: row.document_handheld_image,
        status: row.status,
        target_kyc_level: row.target_kyc_level,
        reviewed_by: row.reviewed_by,
        review_reason: row.review_reason,
        submitted_at: row.submitted_at,
        reviewed_at: row.reviewed_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn submission_summary(row: KycSubmissionSummaryRow) -> KycSubmissionSummary {
    KycSubmissionSummary {
        id: row.id,
        user_id: row.user_id,
        email: row.email,
        phone: row.phone,
        real_name: row.real_name,
        country: row.country,
        id_number: row.id_number,
        submission_type: row.submission_type,
        enterprise_name: row.enterprise_name,
        business_registration_number: row.business_registration_number,
        document_type: row.document_type,
        status: row.status,
        target_kyc_level: row.target_kyc_level,
        reviewed_by: row.reviewed_by,
        review_reason: row.review_reason,
        submitted_at: row.submitted_at,
        reviewed_at: row.reviewed_at,
        updated_at: row.updated_at,
    }
}
