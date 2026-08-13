//! user bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 本文件是用户上下文全部 MySQL 访问的落地处，覆盖 `users`、`user_security`、
//! `user_third_party_bindings`、`user_email_verifications`、`user_referrals`、`invite_codes`、
//! `refresh_tokens`、`agents` 与 `audit_events` 等表的读写。
//! 命名约定：带 `_in_tx` 后缀的函数接收调用方事务且绝不自行 commit 或 rollback，
//! 事务边界一律由 application 层掌握；不带该后缀的函数使用连接池自治执行单条语句。
//! 加锁约定：需要防并发的读取统一用 `SELECT ... FOR UPDATE`，
//! 邀请绑定链路的锁顺序固定为先用户自身、后邀请码、再代理层级，避免与其他用例交叉形成死锁。
//! 隐私边界：本层可以读出邮箱、手机号和各类密码哈希，但只把它们回传给调用方，
//! 任何哈希都不会进入响应结构体，`UserProfileResponse` 只暴露 `fund_password_set` 这类布尔标志。
//! 错误映射：MySQL 1062 唯一键冲突被翻译成语义明确的 `AppError::Conflict`，
//! 身份类记录缺失一律映射为 `AppError::Unauthorized` 而非 `NotFound`，避免用错误类型探测账号是否存在。

use crate::{
    error::{AppError, AppResult},
    modules::user::{
        presentation::{
            MyInviteUserResponse, ReferralBindingResponse, ReferralCodeResponse,
            ThirdPartyBindingResponse, UserProfileResponse,
        },
        repository::{
            EmailVerificationRecord, InviteCodeRecord, ReferralLinkRecord, UserPasswordRecord,
        },
    },
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
struct UserProfileRow {
    id: u64,
    username: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    avatar_url: Option<String>,
    country_code: Option<String>,
    preferred_locale: Option<String>,
    default_locale: Option<String>,
    supported_locales: Option<SqlxJson<Vec<String>>>,
    status: String,
    kyc_level: i32,
    email_verified_at: Option<DateTime<Utc>>,
    fund_password_set: bool,
    created_at: DateTime<Utc>,
}

impl From<UserProfileRow> for UserProfileResponse {
    /// 把资料查询的数据库行转换为对外响应结构，逐字段平移，不做业务判断。
    /// 唯一的形态调整是把 `supported_locales` 从 SQLx 的 JSON 包装解开为裸字符串数组。
    /// `fund_password_set` 在 SQL 中已由哈希是否为空推导为布尔值，因此这里不会接触到任何哈希原文。
    fn from(row: UserProfileRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            email: row.email,
            phone: row.phone,
            avatar_url: row.avatar_url,
            country_code: row.country_code,
            preferred_locale: row.preferred_locale,
            default_locale: row.default_locale,
            supported_locales: row.supported_locales.map(|value| value.0),
            status: row.status,
            kyc_level: row.kyc_level,
            email_verified_at: row.email_verified_at,
            fund_password_set: row.fund_password_set,
            created_at: row.created_at,
        }
    }
}

/// 一条 SQL 拼出用户资料全貌：主表基本信息、按国家代码左连的本地化配置，以及由安全表推导的资金密码标志。
/// 两处都用 LEFT JOIN，因此未设置国家或尚无安全行的用户同样能查到资料，缺失部分留空而不是整行消失。
/// 资金密码只以 `fund_password_set` 布尔值形式暴露，哈希在 SQL 层就被 CASE 表达式折叠掉，不会离开数据库。
/// 用户不存在时返回 `AppError::Unauthorized` 而非 `NotFound`：该接口只服务本人查询，
/// 查不到即意味着令牌所指账号已失效，用未授权语义可避免据此判断某个 ID 是否注册过。
/// 纯只读查询，不加锁也不写入；返回内容含邮箱与手机号等本人隐私字段，调用方不得跨用户缓存或落日志。
pub(crate) async fn load_user_profile(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserProfileResponse> {
    let profile = sqlx::query_as::<_, UserProfileRow>(
        r#"SELECT users.id, users.username, users.email, users.phone, users.avatar_url,
                  users.country_code, users.preferred_locale,
                  countries.default_locale, countries.supported_locales,
                  users.status, users.kyc_level, users.email_verified_at,
                  CASE WHEN security.fund_password_hash IS NULL THEN FALSE ELSE TRUE END AS fund_password_set,
                  users.created_at
           FROM users
           LEFT JOIN user_security security ON security.user_id = users.id
           LEFT JOIN country_configs countries ON countries.country_code = users.country_code
           WHERE users.id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    Ok(profile.into())
}

/// 以最轻量的只读查询确认用户主键存在，作为只读用例的前置守卫，缺失时返回 `AppError::Unauthorized`。
/// 只看主键是否存在，不检查 `status` 字段，因此被停用的账号在此仍会通过；
/// 需要状态判定的写入路径必须改用 `ensure_active_user_in_tx`。
/// 不加锁也不开事务，所以返回成功仅代表查询瞬间存在，调用方不能据此假设后续操作期间用户不会被删改。
pub(crate) async fn ensure_user_exists(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(())
}

/// 在调用方事务内用 `FOR UPDATE` 锁定用户行，确保随后的写入期间该账号不被并发修改或删除。
/// 与连接池版本的差别不只是加锁：这里的行锁一直持有到调用方提交或回滚，
/// 是邀请绑定等用例把「用户仍然有效」与后续写入绑成一个原子单元的基础。
/// 同样只判断主键存在而不筛选 `status`，缺失时返回 `AppError::Unauthorized`；本函数不提交事务。
pub(crate) async fn ensure_user_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(())
}

/// 把头像文件上传完成后得到的下载地址写回用户资料，覆盖旧值且不保留历史。
/// 传入的应当是上传服务返回的地址而非用户自填 URL，本函数不校验其格式、协议或可达性。
/// 以受影响行数判断用户是否存在：为零时返回 `AppError::Unauthorized`。
/// 注意 MySQL 在新旧值完全相同时也可能报告零行，因此重复提交同一头像地址会被判为未授权，
/// 调用链上游的上传步骤保证每次生成的地址不同，这一分支在正常流程中不会命中。
/// 自治执行单条语句，不参与调用方事务，失败时上传产生的对象不会被回收。
pub(crate) async fn update_user_avatar_url(
    pool: &Pool<MySql>,
    user_id: u64,
    avatar_url: &str,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE users SET avatar_url = ? WHERE id = ?")
        .bind(avatar_url)
        .bind(user_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

/// 锁定活跃用户行并取出改名前的用户名，一次查询同时满足加锁与审计取值两个需要。
/// 条件中带 `status = 'active'`，因此被停用的账号无法改名，与仅判存在的守卫函数形成区别。
/// 返回 `Option<String>` 的外层是查询结果、内层是列本身可空：用户名尚未设置时返回 `Ok(None)`，
/// 表示这是一次从空到有的首次设置，审计中的改前值应记为空。
/// 用户不存在或非活跃时返回 `AppError::Unauthorized`；行锁持有至调用方事务结束。
pub(crate) async fn lock_active_user_username_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT username FROM users WHERE id = ? AND status = 'active' LIMIT 1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)
}

/// 锁定用户行并取出登录密码哈希与账号状态，供改密与设置资金密码时校验身份。
/// 这里刻意不在 SQL 中过滤 `status`，而是把状态原样交给调用方：
/// 调用方需要把「状态非 active」和「密码不匹配」合并成同一个 `AppError::Unauthorized` 返回，
/// 若在 SQL 层就筛掉非活跃账号，两种失败会产生不同的错误路径而被用来探测账号状态。
/// 行锁保证从读出哈希到写入新哈希期间没有并发改密，避免两个请求互相覆盖。
/// 返回值携带密码哈希，调用方只可用于校验和覆盖，不得写入审计、日志或响应。
pub(crate) async fn lock_user_password_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserPasswordRecord> {
    let row = sqlx::query_as::<_, (u64, String, String)>(
        r#"SELECT id, password_hash, status
           FROM users
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(UserPasswordRecord {
        id: row.0,
        password_hash: row.1,
        status: row.2,
    })
}

/// 在事务内锁定用户行并同时要求状态为 `active`，是所有安全敏感写入的统一准入闸门。
/// 邮箱绑定、发送验证码、第三方绑定、修改资金密码等用例都先过这一关，
/// 确保被风控停用的账号无法继续变更凭证或联系方式。
/// 与 `ensure_user_exists_in_tx` 的唯一区别就是这道状态条件，两者不可互换。
/// 缺失与停用合并返回 `AppError::Unauthorized`，不区分二者；行锁持有到调用方提交或回滚。
pub(crate) async fn ensure_active_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query_as::<_, (u64,)>(
        r#"SELECT id
           FROM users
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(())
}

/// 覆盖用户的登录密码哈希，前置条件是调用方已在同一事务内锁定该用户行并校验过旧密码。
/// 只接受已经算好的哈希字符串，本函数不做散列也不做强度校验，明文绝不应传到这一层。
/// 不检查受影响行数：调用方持有行锁且已确认用户存在，零行只可能出现在契约被违反时。
/// 不提交事务，也不撤销任何会话，会话失效由改密用例在同事务内另行调用撤销函数完成。
pub(crate) async fn update_user_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 把该用户名下所有尚未撤销的刷新令牌批量打上撤销时间戳，使旧凭证无法再换取新的访问令牌。
/// 条件限定 `actor_type = 'user'`，因此同 ID 的管理员或代理令牌不受影响。
/// `revoked_at IS NULL` 既避免重复覆盖已撤销记录的原始时间，也让重复执行天然幂等。
/// 只处理 MySQL 中持久化的刷新令牌，Redis 侧的会话与访问令牌不在此列，
/// 需由改密用例在事务提交后另行调用会话撤销逻辑清理。
/// 与改密写入同事务提交，保证不会出现密码已改但旧刷新令牌仍然有效的窗口。
pub(crate) async fn revoke_user_refresh_tokens_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE refresh_tokens
           SET revoked_at = CURRENT_TIMESTAMP(6)
           WHERE actor_type = 'user' AND actor_id = ? AND revoked_at IS NULL"#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 列出该用户全部第三方账号绑定记录，按更新时间倒序、主键倒序排列，最近变动的排在最前。
/// 以主键作为次级排序键是为了让同一时刻批量写入的记录也有稳定顺序，避免分页或对比时结果抖动。
/// 只读本地绑定表，不调用 Coinbase、Telegram 等外部接口，因此返回的是绑定时留存的快照，
/// 不能反映第三方账号此后是否被改名或注销。
/// 不做条数限制，也不过滤 `status`：已解绑记录若保留在表中同样会被返回，由上层按状态展示。
pub(crate) async fn list_user_third_party_bindings(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<ThirdPartyBindingResponse>> {
    sqlx::query_as::<_, ThirdPartyBindingResponse>(
        r#"SELECT provider, account_identifier, display_name, status, created_at, updated_at
           FROM user_third_party_bindings
           WHERE user_id = ?
           ORDER BY updated_at DESC, id DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 读取该用户名下自有邀请码，按主键升序只取第一条，即最早创建的那一枚。
/// 固定取最早一条保证同一用户在任何时刻拿到的推广码稳定不变，即使历史上因并发写入产生过多条记录。
/// 左连 `user_referrals` 附带该用户自身的根代理归属，供上层展示这枚码将把新人挂到哪家代理名下；
/// 用户自己尚未绑定推荐关系时该字段为空，不影响邀请码本身可用。
/// 从未生成过邀请码时返回 `Ok(None)`，由应用层决定是否触发生成，本函数不写入任何数据。
pub(crate) async fn load_user_invite_code(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<ReferralCodeResponse>> {
    sqlx::query_as::<_, ReferralCodeResponse>(
        r#"SELECT codes.id, codes.owner_type, codes.owner_id, codes.code,
                  codes.usage_limit, codes.used_count, codes.status,
                  referrals.root_agent_id, codes.created_at
           FROM invite_codes codes
           LEFT JOIN user_referrals referrals ON referrals.user_id = codes.owner_id
           WHERE codes.owner_type = 'user' AND codes.owner_id = ?
           ORDER BY codes.id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 写入一枚用户邀请码，按 `existing_code_id` 是否给出在改写与新建两条分支间切换。
/// 改写分支的 WHERE 同时限定主键、`owner_type = 'user'` 与 `owner_id`，
/// 三者必须同时匹配才会生效，避免传错主键时改掉他人或代理的邀请码。
/// 新建分支插入一条 `status = 'active'` 的记录，使用次数由数据库默认值起算。
/// 返回值语义有三层：写入成功返回 `Ok(true)`；
/// 撞上 code 唯一键（MySQL 1062）返回 `Ok(false)`，这是预期内的随机碰撞，调用方应换一个码重试；
/// 语句执行成功却影响零行说明改写分支的所有权校验没通过，返回 `AppError::Internal`。
/// 不参与调用方事务，每次调用是一条自治语句；不改动使用次数，也不触碰任何邀请绑定关系。
pub(crate) async fn write_user_invite_code(
    pool: &Pool<MySql>,
    user_id: u64,
    existing_code_id: Option<u64>,
    code: &str,
) -> AppResult<bool> {
    let result = if let Some(existing_code_id) = existing_code_id {
        sqlx::query(
            r#"UPDATE invite_codes
               SET code = ?
               WHERE id = ? AND owner_type = 'user' AND owner_id = ?"#,
        )
        .bind(code)
        .bind(existing_code_id)
        .bind(user_id)
        .execute(pool)
        .await
    } else {
        sqlx::query(
            r#"INSERT INTO invite_codes (owner_type, owner_id, code, status)
               VALUES ('user', ?, ?, 'active')"#,
        )
        .bind(user_id)
        .bind(code)
        .execute(pool)
        .await
    };

    match result {
        Ok(result) if result.rows_affected() > 0 => Ok(true),
        Ok(_) => Err(AppError::Internal(
            "failed to update user invite code".to_owned(),
        )),
        Err(error) if is_duplicate_key(&error) => Ok(false),
        Err(error) => Err(AppError::from(error)),
    }
}

/// 以 `FOR UPDATE` 锁定该用户的推荐绑定行，用来判断是否已经绑定过邀请人。
/// 加锁是防重复绑定的关键：即使记录不存在，行锁也会阻塞同一用户的并发绑定请求，
/// 使两个请求无法同时通过「尚未绑定」判断而各自插入一条。
/// 返回 `Ok(None)` 表示尚未绑定，调用方可继续走绑定流程；
/// 返回 `Some` 表示已绑定，调用方应直接把既有绑定原样回传，不再消耗邀请码次数。
/// 查询里的 `true AS bound` 是为了补齐响应结构的字段，凡查得到的记录都视为已绑定。
pub(crate) async fn lock_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<ReferralBindingResponse>> {
    sqlx::query_as::<_, ReferralBindingResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at,
                  true AS bound
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在当前事务快照内回读该用户的推荐绑定，用于绑定写入之后组装返回给前端的权威结果。
/// 与锁定版本的三点差异：不加 `FOR UPDATE`、缺失时报错而非返回空、语义上是「读已写入的结果」而非「判断是否已绑定」。
/// 因为在同一事务内执行，能读到本事务刚插入但尚未提交的那一行，不必等到提交后再查一次。
/// 记录缺失返回 `AppError::NotFound`，这在正常流程中意味着插入步骤未按预期生效。
pub(crate) async fn load_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralBindingResponse> {
    sqlx::query_as::<_, ReferralBindingResponse>(
        r#"SELECT user_id, direct_inviter_id, direct_inviter_type,
                  root_agent_id, depth, path, created_at,
                  true AS bound
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 校验一家代理及其整条上级链路是否都处于启用状态，只有全链有效才允许新用户挂靠到该代理名下。
/// 分两步：先锁定并读出目标代理自身的物化路径，它必须是 `active` 否则直接返回校验错误；
/// 再用 `path = ? OR ? LIKE CONCAT(path, '/%')` 匹配出自身与全部祖先节点，逐一检查状态。
/// 之所以不能只看直属代理，是因为上级公司一旦被停用，整棵下级树都应停止发展新用户，
/// 否则返佣会流向已被冻结的主体。
/// 祖先查询按 `level ASC, id ASC` 排序并加锁，这个自上而下的固定顺序是防死锁的关键：
/// 所有涉及代理层级的事务都按同一方向获取锁，不会出现两个事务反向持锁互等。
/// 匹配结果为空（路径数据缺失）或任一节点非 `active` 都返回 `AppError::Validation`，
/// 由调用方事务整体回滚，不留下部分写入。
pub(crate) async fn ensure_active_agent_in_tx(
    tx: &mut Transaction<'_, MySql>,
    agent_id: u64,
) -> AppResult<()> {
    let (path,) = sqlx::query_as::<_, (String,)>(
        r#"SELECT path
           FROM agents
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("agent is inactive or not found".to_owned()))?;

    // 直属代理仍为 active 也不够，任一上级停用后整家下级公司都不能继续发展用户。
    let ancestor_statuses = sqlx::query_scalar::<_, String>(
        r#"SELECT status
           FROM agents
           WHERE path = ? OR ? LIKE CONCAT(path, '/%')
           ORDER BY level ASC, id ASC
           FOR UPDATE"#,
    )
    .bind(&path)
    .bind(&path)
    .fetch_all(&mut **tx)
    .await?;
    if ancestor_statuses.is_empty() || ancestor_statuses.iter().any(|status| status != "active") {
        return Err(AppError::Validation(
            "agent hierarchy is inactive or invalid".to_owned(),
        ));
    }
    Ok(())
}

/// 锁定并读出邀请人自身的推荐链信息，用于为被邀请人派生下一层关系。
/// 取回的三个字段各有用途：根代理决定新人归属哪家公司，深度加一后成为新人的层级，
/// 路径追加新人节点后成为新人的物化路径，从而保持整条链可用前缀匹配查询。
/// 加锁避免邀请人的推荐关系在派生过程中被并发改动而导致父子层级不一致。
/// 邀请人自己还没有绑定推荐关系时返回 `AppError::Validation`，
/// 因此用户邀请用户的链条必须自上而下建立，无法从中间凭空接入。
pub(crate) async fn load_referral_link_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<ReferralLinkRecord> {
    let row = sqlx::query_as::<_, (Option<u64>, i32, String)>(
        r#"SELECT root_agent_id, depth, path
           FROM user_referrals
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("inviter has not bound an agent".to_owned()))?;
    Ok(ReferralLinkRecord {
        root_agent_id: row.0,
        depth: row.1,
        path: row.2,
    })
}

/// 按码值锁定一枚处于启用状态的邀请码，取回所有者与使用计数供绑定用例判定。
/// 加锁的核心目的是让「读已用次数」和「递增已用次数」串行化：
/// 若不加锁，多个请求可能同时读到同一个未达上限的计数，各自绑定成功从而突破用量限制。
/// 所有者以 `owner_type` 与 `owner_id` 两字段表达，可能是代理也可能是用户，
/// 具体走哪条归属分支由调用方按类型判断，本函数不做解释。
/// `usage_limit` 为空表示不限次数，上限判定同样由调用方完成。
/// 码不存在或状态非 `active` 时返回 `AppError::Validation`，两种情况合并同一消息，不透露码是否存在过。
pub(crate) async fn lock_active_invite_code_in_tx(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<InviteCodeRecord> {
    let row = sqlx::query_as::<_, (u64, String, u64, Option<i32>, i32)>(
        r#"SELECT id, owner_type, owner_id, usage_limit, used_count
           FROM invite_codes
           WHERE code = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("invite code is inactive or not found".to_owned()))?;
    Ok(InviteCodeRecord {
        id: row.0,
        owner_type: row.1,
        owner_id: row.2,
        usage_limit: row.3,
        used_count: row.4,
    })
}

/// 插入一条用户推荐绑定记录，一次写全直属邀请人、根代理归属、层级深度与物化路径。
/// 直属邀请人与根代理是两套并行关系：前者记录具体是谁介绍的，后者决定返佣归属哪家公司，
/// 二者必须在同一条记录中同时落库，否则会出现有介绍人却无归属或反之的残缺状态。
/// `depth` 与 `path` 由调用方基于邀请人链路派生，本函数不校验其自洽性，
/// 也不检查路径是否与深度匹配，正确性由绑定用例保证。
/// 表以 `user_id` 为主键，重复绑定会触发主键冲突而失败，这是防重绑的最后一道保险。
/// 不递增邀请码使用次数，也不提交事务，两者都由调用方在同一事务内另行完成。
pub(crate) async fn insert_user_referral_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    direct_inviter_id: u64,
    direct_inviter_type: &str,
    root_agent_id: Option<u64>,
    depth: i32,
    path: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_referrals
              (user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(direct_inviter_id)
    .bind(direct_inviter_type)
    .bind(root_agent_id)
    .bind(depth)
    .bind(path)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 把指定邀请码的已用次数加一，必须与推荐关系插入处于同一事务。
/// 采用 `used_count = used_count + 1` 的数据库端自增而非读改写，配合调用方先前持有的行锁，
/// 保证并发绑定不会丢失计数。
/// 本语句不检查是否超出 `usage_limit`：上限判定在锁定邀请码后由调用方完成，此处只负责累加。
/// 与绑定写入同事务提交，杜绝出现关系已建立却没扣次数，或扣了次数却没建立关系的情况。
pub(crate) async fn increment_invite_code_used_count_in_tx(
    tx: &mut Transaction<'_, MySql>,
    invite_code_id: u64,
) -> AppResult<()> {
    sqlx::query("UPDATE invite_codes SET used_count = used_count + 1 WHERE id = ?")
        .bind(invite_code_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 查询把该用户登记为直接邀请人的下级列表，条件同时限定 `direct_inviter_type = 'user'`，
/// 因此代理直邀的用户不会混入结果，同名 ID 的代理与用户也不会互相串号。
/// 内连 `users` 表补齐下级的联系方式与账号状态，故已被删除的用户不会出现在列表中。
/// 按创建时间升序、用户 ID 升序排列并硬性限制一百条，这是一个不分页的概览接口，
/// 邀请人数超过一百时只能看到最早的一批，完整数据需走后台统计。
/// 结果含下级的邮箱与手机号，属于他人隐私字段，只可返回给邀请人本人，不得对外扩散或落日志。
pub(crate) async fn list_direct_invited_users(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MyInviteUserResponse>> {
    sqlx::query_as::<_, MyInviteUserResponse>(
        r#"SELECT referrals.user_id, users.email, users.phone, users.status,
                  referrals.direct_inviter_type, referrals.direct_inviter_id,
                  referrals.root_agent_id, referrals.depth, referrals.path,
                  referrals.created_at
           FROM user_referrals referrals
           INNER JOIN users ON users.id = referrals.user_id
           WHERE referrals.direct_inviter_type = 'user'
             AND referrals.direct_inviter_id = ?
           ORDER BY referrals.created_at ASC, referrals.user_id ASC
           LIMIT 100"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 以 `INSERT ... ON DUPLICATE KEY UPDATE` 写入第三方绑定，依赖用户与提供方的联合唯一键实现幂等。
/// 同一用户同一提供方永远只有一行：首次调用插入，再次调用覆盖账号标识与展示名，不会堆叠多条历史。
/// 冲突分支会把 `status` 无条件重置为 `bound`，因此对先前已解绑的记录再次绑定等于重新激活该行。
/// 展示名接受 `None` 并原样写入空值，覆盖时同样生效，即用户清空展示名会真的把旧值抹掉。
/// 本函数只落本地标识，不联系任何第三方接口，也不验证账号所有权。
/// 不提交事务，绑定与审计由应用层放在同一事务内一起提交。
pub(crate) async fn upsert_user_third_party_binding_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    provider: &str,
    account_identifier: &str,
    display_name: &Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_third_party_bindings
              (user_id, provider, account_identifier, display_name, status)
           VALUES (?, ?, ?, ?, 'bound')
           ON DUPLICATE KEY UPDATE
              account_identifier = VALUES(account_identifier),
              display_name = VALUES(display_name),
              status = 'bound'"#,
    )
    .bind(user_id)
    .bind(provider)
    .bind(account_identifier)
    .bind(display_name)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 覆盖用户的登录用户名，前置条件是调用方已在同一事务内锁定该活跃用户行。
/// 传入值应当已经过认证域的规范化（含转小写），本函数不再做格式或字符集校验。
/// 重名由数据库唯一索引拦截，MySQL 1062 错误经 `map_duplicate_username` 翻译为
/// `AppError::Conflict` 并附带明确文案，而不是暴露原始数据库错误。
/// 不检查受影响行数，用户存在性由前置的锁定步骤保证；不提交事务。
pub(crate) async fn update_user_username_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    username: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET username = ? WHERE id = ?")
        .bind(username)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(map_duplicate_username)?;
    Ok(())
}

/// 锁定用户安全行并取出资金密码哈希，行锁保证从校验旧值到写入新值期间不被并发覆盖。
/// 两种「没有」被有意压平成同一个 `None`：安全行整体不存在，以及安全行存在但哈希列为空。
/// 对调用方而言这两种情况处置完全一致（要么视为未设置、要么走 upsert 创建），
/// 因此代码里用 `flatten` 消除了嵌套 `Option` 的区分。
/// 注意安全行不存在时并无行可锁，此时并发保护由 upsert 语句自身的唯一键冲突兜底。
/// 返回的哈希只用于比对与覆盖判断，不得进入响应、审计或日志。
pub(crate) async fn lock_fund_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    sqlx::query_scalar(
        r#"SELECT fund_password_hash
           FROM user_security
           WHERE user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map(|value: Option<Option<String>>| value.flatten())
    .map_err(AppError::from)
}

/// 断言用户已经设置过资金密码，用作重置流程的前置条件，避免向没有该凭证的账号发送重置码。
/// 复用锁定函数取值后立即丢弃哈希内容，只保留「是否存在」这一位信息，
/// 从而在获得同样行锁保护的同时不让哈希扩散到调用方。
/// 未设置时返回 `AppError::NotFound`，提示上层应改走创建流程而非重置流程。
pub(crate) async fn ensure_fund_password_exists_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    lock_fund_password_hash_in_tx(tx, user_id)
        .await?
        .map(|_| ())
        .ok_or(AppError::NotFound)
}

/// 写入资金密码哈希，用 upsert 同时覆盖「安全行尚不存在」与「安全行已存在但无资金密码」两种起点。
/// 之所以需要 upsert 而非纯 UPDATE，是因为用户注册时不一定生成安全行，首次设置资金密码可能要连行一起建。
/// 冲突分支只更新 `fund_password_hash` 一列，安全行上的其他字段保持原样不被这次写入波及。
/// 只接受算好的哈希，明文不到这一层；不校验是否已存在旧值，覆盖保护由调用方在事务内先行判断。
/// 不提交事务，审计由调用方在同事务内追加。
pub(crate) async fn upsert_fund_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    fund_password_hash: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_security (user_id, fund_password_hash)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE fund_password_hash = VALUES(fund_password_hash)"#,
    )
    .bind(user_id)
    .bind(fund_password_hash)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 只更新已存在安全行上的资金密码哈希，用于修改与重置这两种「必然已有旧值」的场景。
/// 与 upsert 版本的区别是这里不会建行：安全行缺失时语句影响零行且不报错，
/// 因此调用方必须先用锁定或断言函数确认记录存在，否则会静默地什么都没改。
/// 这一分工是有意的，它让「首次创建」与「修改既有」两条路径在数据库层就无法互相顶替。
/// 不提交事务；哈希由调用方算好传入，明文与旧哈希都不会出现在此。
pub(crate) async fn update_fund_password_hash_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    fund_password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE user_security SET fund_password_hash = ? WHERE user_id = ?")
        .bind(fund_password_hash)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 确认待绑定邮箱尚未被别的账号占用，是邮箱唯一性的业务层前置检查。
/// 条件里的 `id <> ?` 把当前用户自己排除在外，因此重新绑定已属于自己的同一地址不算冲突，
/// 用户可以借此重新验证邮箱而不被自己的旧记录挡住。
/// 检测到占用返回 `AppError::Conflict`，文案统一为 `email already exists`。
/// 这里只做普通读取而未加 `FOR UPDATE`，所以严格并发下仍可能有两个请求同时通过检查，
/// 最终由 `users.email` 唯一索引在写入阶段兜底，冲突同样被翻译为 `Conflict`。
pub(crate) async fn ensure_email_available_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
) -> AppResult<()> {
    let existing_user_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT id
           FROM users
           WHERE email = ? AND id <> ?
           LIMIT 1"#,
    )
    .bind(email)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    if existing_user_id.is_some() {
        return Err(AppError::Conflict("email already exists".to_owned()));
    }
    Ok(())
}

/// 实施验证码发送频率限制：取用户、邮箱、用途三元组下最新一条待验证记录的发送时间，
/// 若距 `now` 不足 `cooldown_seconds` 秒则拒绝本次发送。
/// 冷却窗口按三元组独立计算，所以绑定邮箱、重置二次验证、重置资金密码三类码互不挤占彼此的频率配额。
/// 只看 `status = 'pending'` 的记录：已被消费或已被取代的旧码不参与冷却判定，
/// 因此用户成功验证一次后可以立即发起下一轮新的验证流程。
/// 从未发送过时 `sent_at` 为空，直接放行。
/// 冷却期内返回 `AppError::Validation`，调用方据此中止且不会插入新码，也不会发信。
/// 未加行锁，极端并发下两个请求可能同时通过检查各发一封；这属于可接受的边界，
/// 因为后续的作废步骤仍会保证只有最新一枚码有效。
pub(crate) async fn ensure_email_verification_not_cooling_down_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
    now: DateTime<Utc>,
    cooldown_seconds: i64,
) -> AppResult<()> {
    let sent_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"SELECT sent_at
           FROM user_email_verifications
           WHERE user_id = ? AND email = ? AND purpose = ? AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(purpose)
    .fetch_optional(&mut **tx)
    .await?;
    if sent_at.is_some_and(|sent_at| sent_at + Duration::seconds(cooldown_seconds) > now) {
        return Err(AppError::Validation(
            "email verification code was sent recently".to_owned(),
        ));
    }
    Ok(())
}

/// 在插入新验证码之前，把该用户同一用途下所有仍处于 `pending` 的旧码批量置为 `superseded`。
/// 这是防重放的核心步骤：确保任一时刻每个用途最多只有一枚可用验证码，
/// 用户连续点击发送后，先前收到的邮件立即作废，无法用旧码完成验证。
/// 作废范围按用户加用途匹配而不限定邮箱地址，因此更换绑定目标邮箱时，
/// 指向旧地址的待验证码也会一并失效，避免用户拿着发往旧邮箱的码去验证新邮箱。
/// 状态条件让重复执行天然幂等，不会覆盖已消费记录的终态。不提交事务。
pub(crate) async fn supersede_pending_email_verifications_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    purpose: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET status = 'superseded'
           WHERE user_id = ? AND purpose = ? AND status = 'pending'"#,
    )
    .bind(user_id)
    .bind(purpose)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 插入一条待验证邮件码记录，状态固定为 `pending`，尝试次数由数据库默认值从零起算。
/// 入库的是验证码的密码哈希而非明文：即便数据库被读取，也无法反推出可用的验证码。
/// `purpose` 决定这枚码属于哪条业务线，后续的冷却、作废与消费都按同一用途匹配，跨用途不可通用。
/// `expires_at` 与 `sent_at` 都先转成 naive UTC 再绑定，与数据库列的无时区语义对齐，
/// 避免驱动按本地时区二次换算导致有效期偏移。
/// 前置条件是调用方已完成冷却检查并作废旧码，本函数不重复校验这些约束，也不发信、不提交事务。
pub(crate) async fn insert_pending_email_verification_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
    code_hash: &str,
    expires_at: DateTime<Utc>,
    sent_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_email_verifications
           (user_id, email, purpose, code_hash, status, expires_at, sent_at)
           VALUES (?, ?, ?, ?, 'pending', ?, ?)"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(purpose)
    .bind(code_hash)
    .bind(expires_at.naive_utc())
    .bind(sent_at.naive_utc())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 锁定用户、邮箱、用途三元组下最新一条 `pending` 验证码，取回比对与判定所需的全部字段。
/// 按主键倒序取一条，与作废机制配合后正常情况下本就只有一条可用记录，倒序只是额外保险。
/// `FOR UPDATE` 让「读出尝试次数、判断是否超限、递增次数」成为一个不可分割的序列，
/// 否则并发试错会因为读到相同的旧计数而突破次数上限。
/// 返回 `Ok(None)` 表示无可用记录，可能是从未发送、已被消费或已被新码取代，调用方应统一按「验证码无效」处理，
/// 不要对外区分这几种情况。
/// 取回的哈希仅供比对，过期与超限的判定规则由领域层函数统一执行，本函数不做任何判断。
pub(crate) async fn lock_latest_pending_email_verification_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    purpose: &str,
) -> AppResult<Option<EmailVerificationRecord>> {
    let row = sqlx::query_as::<_, (u64, String, i32, DateTime<Utc>)>(
        r#"SELECT id, code_hash, attempt_count, expires_at
           FROM user_email_verifications
           WHERE user_id = ? AND email = ? AND purpose = ? AND status = 'pending'
           ORDER BY id DESC
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(email)
    .bind(purpose)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(
        |(id, code_hash, attempt_count, expires_at)| EmailVerificationRecord {
            id,
            code_hash,
            attempt_count,
            expires_at,
        },
    ))
}

/// 锁定并读出用户当前已完成验证的邮箱地址，是所有重置类流程确定收件人的唯一途径。
/// 地址来自数据库而非请求参数，这是一条关键的安全边界：攻击者即使拿到会话也无法把重置码引导到自己的邮箱。
/// 三个条件缺一不可：账号处于 `active`、邮箱列非空、`email_verified_at` 已填写，
/// 因此仅填写过邮箱但未完成验证的用户无法走重置流程。
/// 行锁使读出地址与后续写入验证码记录之间不会插入邮箱变更，避免码发往旧地址而记录挂在新地址上。
/// 任一条件不满足统一返回 `AppError::Validation`，不区分具体原因；本函数不发信也不提交事务。
pub(crate) async fn lock_verified_user_email_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<String> {
    let email: Option<String> = sqlx::query_scalar(
        r#"SELECT email
           FROM users
           WHERE id = ? AND status = 'active' AND email_verified_at IS NOT NULL
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    email.ok_or_else(|| AppError::Validation("verified email is required".to_owned()))
}

/// 把指定验证码记录的失败尝试次数加一，在哈希比对不通过的分支被调用。
/// 使用数据库端自增，配合调用方持有的行锁保证并发试错不会丢失计数。
/// 关键契约：调用方在返回校验错误之前必须提交这次计数，绝不能回滚。
/// 若随错误一起回滚，累计次数将永远停在零，领域层的最大尝试次数判定就完全失效，
/// 六位数字验证码会退化为可被无限枚举的弱凭证。
/// 本函数只累加，不判断是否已达上限，也不改变记录状态。
pub(crate) async fn increment_email_verification_attempt_count_in_tx(
    tx: &mut Transaction<'_, MySql>,
    verification_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET attempt_count = attempt_count + 1
           WHERE id = ?"#,
    )
    .bind(verification_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在验证码校验通过后把邮箱正式写入用户主表，同时落下验证完成时间。
/// 地址与验证时间必须一起更新：只改地址会让新邮箱继承旧地址的已验证状态，
/// 使未经验证的邮箱被当作可信联系方式用于后续重置流程。
/// 时间戳转 naive UTC 后绑定，与列的无时区语义保持一致。
/// 唯一索引冲突经 `map_duplicate_email` 翻译为 `AppError::Conflict`；
/// 这是对先前非加锁可用性检查的兜底，覆盖两个请求同时绑定同一地址的竞态。
/// 不提交事务，验证码消费与审计由调用方在同事务内一并完成。
pub(crate) async fn update_user_bound_email_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    email: &str,
    verified_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE users
           SET email = ?, email_verified_at = ?
           WHERE id = ?"#,
    )
    .bind(email)
    .bind(verified_at.naive_utc())
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(map_duplicate_email)?;
    Ok(())
}

/// 把验证码记录从 `pending` 推进到 `verified` 终态并记录消费时刻，完成一次性凭证的核销。
/// 状态机在此闭合：`verified` 不再被后续的 pending 查询命中，因此同一枚码无法被二次使用。
/// WHERE 只按主键匹配而不带状态条件，依赖调用方先前的行锁保证不会核销到已被并发处理的记录。
/// 核销必须与真正的凭证变更（改邮箱、改资金密码等）同事务提交，
/// 否则会出现码已作废但业务未生效，或业务已生效而码仍可复用的错配。
pub(crate) async fn mark_email_verification_verified_in_tx(
    tx: &mut Transaction<'_, MySql>,
    verification_id: u64,
    verified_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_email_verifications
           SET status = 'verified', verified_at = ?
           WHERE id = ?"#,
    )
    .bind(verified_at.naive_utc())
    .bind(verification_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 向审计表追加一条以用户为操作主体的事件，`actor_type` 硬编码为 `user`，
/// 因此本函数只能记录用户自服务动作，管理员操作需走 admin 上下文各自的审计入口。
/// `before_json` 与 `after_json` 都可缺省：改名类事件两者齐备便于对比，
/// 开关或绑定类事件通常只写变更后的快照。
/// 隐私红线由调用方保证：写入前必须剔除密码明文与哈希、TOTP 密钥、邮件验证码、证件号等敏感原文，
/// 审计只应记录布尔标志、枚举状态或已脱敏摘要。
/// 与业务写入共用同一事务，审计失败会连同业务变更一起回滚，确保不存在无审计痕迹的敏感变更。
pub(crate) async fn insert_user_audit_event_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    action: &'static str,
    target_type: &'static str,
    target_id: String,
    before_json: Option<Value>,
    after_json: Option<Value>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO audit_events
           (actor_type, actor_id, action, target_type, target_id, before_json, after_json)
           VALUES ('user', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 取一个可读标识用作 TOTP 导入 URI 中的账号名，让用户在验证器应用里能认出这是哪个账号。
/// 按用户名、邮箱、手机号的顺序取第一个非空值，优先选择用户主动设置且最易辨认的字段。
/// 该标签会出现在验证器应用的条目名称中，因此邮箱或手机号可能被展示给持有该设备的人；
/// 这是用户本人扫码绑定的场景，不构成对外泄露，但调用方不应把它用于其他用途。
/// 三个字段全空或用户不存在时返回 `Ok(None)`，由调用方回落到 `user:<id>` 形式的占位标签。
pub(crate) async fn load_user_account_label(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    let label = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        "SELECT username, email, phone FROM users WHERE id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .and_then(|(username, email, phone)| username.or(email).or(phone));
    Ok(label)
}

/// 把用户名写入时的数据库错误翻译为业务错误：唯一键冲突转成带「用户名已存在」文案的
/// `AppError::Conflict`，其余错误按原样包进 `AppError::Database` 不做掩盖。
/// 之所以要区分，是因为重名属于用户可自行纠正的正常输入问题，应返回 409 而非 500。
fn map_duplicate_username(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("username already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

/// 与用户名版本同构，但用于邮箱绑定路径，冲突文案为「邮箱已存在」。
/// 两者分开而非合并成带参数的通用函数，是为了让错误文案在编译期就与具体字段绑定，
/// 不会因为传错参数而把邮箱冲突报成用户名冲突。
fn map_duplicate_email(error: sqlx::Error) -> AppError {
    if is_duplicate_key(&error) {
        AppError::Conflict("email already exists".to_owned())
    } else {
        AppError::Database(error)
    }
}

/// 判定一个 SQLx 错误是否为 MySQL 的唯一键冲突，依据是驱动返回的错误码字符串 `1062`。
/// 只认这一个错误码，不解析错误文本，因此不受数据库语言环境或版本文案变化影响；
/// 代价是无法区分究竟撞的是哪一个唯一索引，需要区分时由各调用点自行按上下文判断。
fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.code().as_deref() == Some("1062"))
}
