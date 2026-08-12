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

/// 分页查询后台用户，返回符合调用方筛选条件的记录及相同谓词下的总数。
/// 后台用户列表与计数通过连接池分别执行且均不加锁；并发写入可能造成页数据与总数快照不同，SQL 或字段映射失败直接返回错误。
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

/// 按传入主键或筛选条件从连接池读取后台用户并映射为应用层所需的完整记录。
/// 后台用户不追加行锁，查询不创建事务；记录缺失时返回未找到，SQL 或字段解码失败直接返回错误，不产生审计副作用。
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

/// 在调用方事务中插入邮箱/手机、密码散列、初始状态和 KYC 等级，并返回用户 ID。
/// 邮箱或手机等唯一键冲突映射为“用户已存在”；函数不保存明文密码，调用方负责同事务创建邀请码和后台审计。
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

/// 在调用方事务快照中按用户 ID 回读联系方式、首枚 active 邀请码、状态和 KYC 等级。
/// 查询不追加锁；用户缺失返回未找到，读取失败由创建/更新用例回滚，函数不暴露密码散列。
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

/// 在调用方事务中按用户 ID 锁定用户主记录，确认后续状态或安全设置写入具有有效目标。
/// `FOR UPDATE` 锁持有至调用方事务结束；用户缺失返回未找到，函数不检查用户状态，也不自行提交或写审计。
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

/// 在调用方事务中按用户 ID 仅覆盖账户状态。
/// 更新不检查受影响行数或撤销会话；调用方须先锁定用户、校验目标状态，并与后台审计统一提交。
pub(crate) async fn update_admin_user_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET status = ? WHERE id = ?")
        .bind(status)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 在调用方事务中锁定用户的双因素设置并返回当前值；尚未建档时返回该用户的空设置。
/// `FOR UPDATE` 只在记录存在时取得行锁，调用方应先锁用户主记录；函数不解密 TOTP secret 或提交事务。
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

/// 在调用方事务中新增或覆盖用户双因素设置，清空 TOTP 密文和验证时间并关闭两个 2FA 开关。
/// user_id 唯一键使重复重置保持相同空状态；函数不撤销会话，调用方负责先锁用户/设置并与审计原子提交。
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

/// 在调用方事务中为指定用户生成并插入一枚 active 邀请码。
/// 随机码唯一键冲突时最多重试十二次，其他 SQL 错误立即返回，耗尽后返回内部错误；函数不锁用户、不提交事务，调用方负责与用户创建和审计原子提交。
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

/// 向现有 QueryBuilder 追加用户编号筛选谓词及绑定参数，确保列表和计数查询复用相同过滤语义。
/// user_id 为必填值，函数始终追加 `列 = ?` 并绑定参数；列名由内部调用方提供，不执行查询或校验用户存在性。
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

/// 向现有 QueryBuilder 追加用户邮箱筛选谓词及绑定参数，确保列表和计数查询复用相同过滤语义。
/// 邮箱去空后才追加关联 users 表的 EXISTS 精确匹配；空值不改变 builder，函数不执行 SQL 或规范化邮箱大小写。
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

/// 向列表和计数 QueryBuilder 同步追加可选用户编号与状态谓词，保持分页总数口径一致。
/// 用户 ID、邮箱和去空后的状态分别按精确值追加，缺失条件保持原查询；函数只修改 builder，不检查状态枚举或访问数据库。
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

/// 向现有 QueryBuilder 追加内部用户邮箱谓词及绑定参数，确保列表和计数查询复用相同过滤语义。
/// 追加“邮箱为空或不匹配 `%@internal.local`”谓词，用于默认排除内部账号；函数不执行查询，LIKE 匹配语义由数据库排序规则决定。
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

/// 根据显式用户编号或邮箱查询并解析唯一用户编号，供后台用户相关列表统一筛选。
/// 该步骤只读且不加锁；邮箱无匹配时按现有查询语义返回空筛选结果，数据库失败向上返回。
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

/// 判断邮箱是否属于平台内部保留账号，用于后台列表排除系统用户而不泄露实现细节。
/// 该检查是无 I/O 的确定性字符串规则，不修改输入，也不产生错误或副作用。
pub(super) fn is_internal_user_email(email: &str) -> bool {
    email
        .trim()
        .to_ascii_lowercase()
        .ends_with(INTERNAL_USER_EMAIL_DOMAIN)
}
