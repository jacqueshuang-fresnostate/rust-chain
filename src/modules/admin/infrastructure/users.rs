use super::*;

const INTERNAL_USER_EMAIL_DOMAIN: &str = "@internal.local";

const INTERNAL_USER_EMAIL_PATTERN: &str = "%@internal.local";

const ADMIN_USER_INVITE_CODE_CREATE_ATTEMPTS: usize = 12;

#[derive(Debug)]
pub(crate) struct AdminUserListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug)]
pub(crate) struct AdminUserInsert {
    pub(crate) email: Option<String>,
    pub(crate) phone: Option<String>,
    pub(crate) password_hash: String,
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
}

pub(crate) async fn list_admin_users(
    pool: &Pool<MySql>,
    filter: AdminUserListFilter,
) -> AppResult<(Vec<AdminUserResponse>, i64)> {
    let mut rows = admin_user_query();
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM users");
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if !filter.include_internal {
            push_exclude_internal_user_email(builder, "users.email");
        }
        if let Some(user_id) = filter.user_id {
            builder.push(" AND users.id = ");
            builder.push_bind(user_id);
        }
        if let Some(email) = filter.email.clone() {
            builder.push(" AND users.email = ");
            builder.push_bind(email);
        }
        if let Some(status) = filter.status.clone() {
            builder.push(" AND users.status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY users.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

pub(crate) async fn load_admin_user(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    let mut builder = admin_user_query();
    builder.push(" WHERE users.id = ");
    builder.push_bind(user_id);
    builder
        .build_query_as::<AdminUserResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn insert_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: AdminUserInsert,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO users (email, phone, password_hash, status, kyc_level)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(input.email.as_deref())
    .bind(input.phone.as_deref())
    .bind(&input.password_hash)
    .bind(&input.status)
    .bind(input.kyc_level)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_user_error)?;
    Ok(result.last_insert_id())
}

pub(crate) async fn load_admin_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    let mut builder = admin_user_query();
    builder.push(" WHERE users.id = ");
    builder.push_bind(user_id);
    builder
        .build_query_as::<AdminUserResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn ensure_admin_user_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(())
}

pub(crate) async fn load_admin_user_two_factor_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    let settings = sqlx::query_as::<_, UserTwoFactorSettings>(
        r#"SELECT user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled,
                  confirmed_at, last_verified_at
           FROM user_two_factor_settings
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(settings.unwrap_or_else(|| UserTwoFactorSettings::empty(user_id)))
}

pub(crate) async fn reset_admin_user_two_factor_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, NULL, FALSE, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = NULL,
              totp_enabled = FALSE,
              login_2fa_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(UserTwoFactorSettings::empty(user_id))
}

pub(crate) async fn create_user_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    for _ in 0..ADMIN_USER_INVITE_CODE_CREATE_ATTEMPTS {
        let code = generate_user_invite_code()?;
        let result = sqlx::query(
            r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
               VALUES ('user', ?, ?, 'active')"#,
        )
        .bind(user_id)
        .bind(&code)
        .execute(&mut **tx)
        .await;

        match result {
            Ok(_) => return Ok(()),
            Err(error) if is_mysql_duplicate_key(&error) => continue,
            Err(error) => return Err(AppError::from(error)),
        }
    }

    Err(AppError::Internal(
        "failed to create unique user invite code".to_owned(),
    ))
}

fn admin_user_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT users.id, users.email, users.phone, invite_codes.code AS invite_code,
                  users.status, users.kyc_level, users.created_at, users.updated_at
           FROM users
           LEFT JOIN invite_codes
             ON invite_codes.owner_type = 'user'
            AND invite_codes.owner_id = users.id
            AND invite_codes.id = (
                SELECT MIN(user_invite_codes.id)
                FROM invite_codes user_invite_codes
                WHERE user_invite_codes.owner_type = 'user'
                  AND user_invite_codes.owner_id = users.id
            )"#,
    )
}

fn map_duplicate_user_error(error: sqlx::Error) -> AppError {
    if is_mysql_duplicate_key(&error) {
        AppError::Conflict("user already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

pub(super) fn push_user_id_filter(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id_column: &'static str,
    user_id: u64,
) {
    builder.push(" AND ");
    builder.push(user_id_column);
    builder.push(" = ");
    builder.push_bind(user_id);
}

pub(super) fn push_user_email_filter(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id_column: &'static str,
    email: Option<String>,
) {
    if let Some(email) = optional_string(email) {
        builder.push(" AND EXISTS (SELECT 1 FROM users WHERE users.id = ");
        builder.push(user_id_column);
        builder.push(" AND users.email = ");
        builder.push_bind(email);
        builder.push(")");
    }
}

pub(super) fn push_optional_user_and_status_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
) {
    if let Some(user_id) = user_id {
        push_user_id_filter(builder, "user_id", user_id);
    }
    push_user_email_filter(builder, "user_id", email);
    if let Some(status) = optional_string(status) {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
}

pub(super) fn push_exclude_internal_user_email(
    builder: &mut QueryBuilder<'_, MySql>,
    email_column: &'static str,
) {
    builder.push(" AND ");
    builder.push("(");
    builder.push(email_column);
    builder.push(" IS NULL OR ");
    builder.push(email_column);
    builder.push(" NOT LIKE ");
    builder.push_bind(INTERNAL_USER_EMAIL_PATTERN);
    builder.push(")");
}

pub(super) async fn resolve_user_id_filter(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
) -> AppResult<Option<u64>> {
    let Some(email) = optional_string(email) else {
        return Ok(user_id);
    };
    let resolved_user_id =
        sqlx::query_scalar::<_, u64>("SELECT id FROM users WHERE email = ? LIMIT 1")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    Ok(match (user_id, resolved_user_id) {
        (Some(requested_user_id), Some(email_user_id)) if requested_user_id == email_user_id => {
            Some(requested_user_id)
        }
        (Some(_), _) => None,
        (None, resolved_user_id) => resolved_user_id,
    })
}

pub(super) fn is_internal_user_email(email: &str) -> bool {
    email
        .trim()
        .to_ascii_lowercase()
        .ends_with(INTERNAL_USER_EMAIL_DOMAIN)
}
