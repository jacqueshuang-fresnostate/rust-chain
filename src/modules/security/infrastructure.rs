//! security bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 本文件是安全上下文全部 MySQL 访问的落地处，覆盖四类数据：
//! 全局安全策略配置、用户与管理员各自的二次验证设置、登录二次验证挑战，以及用户资金密码哈希的读取。
//! 用户与管理员的二次验证刻意拆成两套表和两套函数：管理员没有登录开关（后台强制要求），
//! 管理员挑战带失败次数上限而用户挑战不带，两者的状态机不同，合并会让约束互相污染。
//! 写入普遍采用 `INSERT ... ON DUPLICATE KEY UPDATE`，因为设置行在用户注册时并不创建，
//! 首次绑定必须能连行一起建，同时也让重复调用天然幂等。
//! 并发口径需要特别注意：本层的写入都是单条自治语句，既不加行锁也不校验受影响行数，
//! 因此无法独立防止「读取后被并发替换」的覆盖问题，状态前置校验一律由 application 层承担。
//! 各函数注释会分别标明这一点，调用方不得假设本层提供了乐观锁或条件更新语义。
//! 密钥红线：所有 `totp_secret_encrypted` 字段进出本层时都是密文，加解密在 application 层完成；
//! 密文与资金密码哈希都属敏感凭据，只可在内存中流转，禁止进入响应体、审计记录或日志。

use crate::{
    architecture::InfrastructureLayer,
    error::AppResult,
    modules::security::domain::{
        AdminLoginTwoFactorChallenge, AdminTwoFactorSettings, CreatedLoginTwoFactorChallenge,
        LoginTwoFactorChallenge, LoginTwoFactorChallengeType, UserSecurityPolicy,
        UserTwoFactorSettings, decode_security_policy_value, login_challenge_expired,
    },
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{FromRow, MySql, Pool, mysql::MySqlRow, types::Json as SqlxJson};
use uuid::Uuid;

pub const USER_SECURITY_POLICY_KEY: &str = "user_security_policy";
pub const LOGIN_CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Debug)]
pub struct SecurityRepository;

impl InfrastructureLayer for SecurityRepository {}

#[derive(Debug, sqlx::FromRow)]
struct UserTwoFactorSettingsRow {
    user_id: u64,
    totp_secret_encrypted: Option<String>,
    totp_enabled: bool,
    login_2fa_enabled: bool,
    confirmed_at: Option<DateTime<Utc>>,
    last_verified_at: Option<DateTime<Utc>>,
}

impl From<UserTwoFactorSettingsRow> for UserTwoFactorSettings {
    /// 把用户二次验证设置的数据库行原样搬到领域快照，逐字段平移且不做任何判定或脱敏。
    /// 这层显式映射的意义在于隔离依赖：领域结构体因此不必派生 SQLx 特征，
    /// 领域层也就无需导入 SQLx，持久化形态的变化不会渗透进业务模型。
    /// 加密后的 TOTP 密钥随快照一并带出，调用方需按敏感凭据处理。
    fn from(row: UserTwoFactorSettingsRow) -> Self {
        Self {
            user_id: row.user_id,
            totp_secret_encrypted: row.totp_secret_encrypted,
            totp_enabled: row.totp_enabled,
            login_2fa_enabled: row.login_2fa_enabled,
            confirmed_at: row.confirmed_at,
            last_verified_at: row.last_verified_at,
        }
    }
}

// 管理后台的锁行查询仍以领域值作为返回类型；SQLx 适配实现留在基础设施层，
// 并统一经过持久化 row 到领域值的显式映射，避免领域模块导入 SQLx。
impl<'r> FromRow<'r, MySqlRow> for UserTwoFactorSettings {
    /// 让领域快照可直接作为 `query_as` 的目标类型，供后台那些以领域值为返回类型的锁行查询使用。
    /// 实现方式是先解析为持久化行结构体再经 `From` 转换，而不是在领域类型上重复列字段映射，
    /// 这样列名与字段的对应关系只在行结构体一处维护，两边不会漂移。
    fn from_row(row: &'r MySqlRow) -> Result<Self, sqlx::Error> {
        UserTwoFactorSettingsRow::from_row(row).map(Into::into)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AdminTwoFactorSettingsRow {
    admin_id: u64,
    totp_secret_encrypted: Option<String>,
    totp_enabled: bool,
    confirmed_at: Option<DateTime<Utc>>,
    last_verified_at: Option<DateTime<Utc>>,
}

impl From<AdminTwoFactorSettingsRow> for AdminTwoFactorSettings {
    /// 把管理员二次验证设置的数据库行搬到领域快照，与用户版本同构但少一个登录开关字段。
    /// 管理员后台登录是否要求二次验证由系统统一决定，表中不存在可供个人关闭的开关列，
    /// 因此两套映射无法合并复用。
    fn from(row: AdminTwoFactorSettingsRow) -> Self {
        Self {
            admin_id: row.admin_id,
            totp_secret_encrypted: row.totp_secret_encrypted,
            totp_enabled: row.totp_enabled,
            confirmed_at: row.confirmed_at,
            last_verified_at: row.last_verified_at,
        }
    }
}

/// 按固定键读取全局安全策略配置，是整个平台策略判定的唯一数据来源。
/// 策略是单例而非按用户存储，因此查询不带用户维度，所有账号共享同一份配置。
/// 取回的 JSON 交给领域解码函数处理，后者负责兼容历史上的字符串包装与 `0/1` 布尔写法，
/// 归一只发生在内存中，本函数不会把规范化后的形态回写数据库。
/// 记录缺失或值为 JSON null 时回落到领域默认策略，保证系统在未配置时仍有明确且偏保守的行为，
/// 而不是因为读不到配置就放行全部动作。
/// 只读查询，无锁无写入；解码失败会作为内部错误上抛，不会静默降级为默认值。
pub async fn load_security_policy(pool: &Pool<MySql>) -> AppResult<UserSecurityPolicy> {
    let policy = sqlx::query_scalar::<_, SqlxJson<Value>>(
        r#"SELECT policy_value
           FROM security_policy_configs
           WHERE policy_key = ?
           LIMIT 1"#,
    )
    .bind(USER_SECURITY_POLICY_KEY)
    .fetch_optional(pool)
    .await?;

    policy
        .map(|value| decode_security_policy_value(value.0))
        .transpose()
        .map(|policy| policy.unwrap_or_default())
}

/// 把一份完整的安全策略写回配置表，按固定键 upsert，首次保存建行、后续保存覆盖。
/// 写入的是结构体序列化后的规范 JSON，因此保存一次即可把历史遗留的字符串包装与 `0/1`
/// 写法彻底替换为标准布尔形态，读取端的兼容归一对这条记录自此不再需要生效。
/// 整份策略整体覆盖而非按字段合并，调用方必须传入完整策略；
/// 若只想改一项，须先读出当前策略、修改后再整体回写，否则未携带的字段会被覆盖成传入值。
/// `admin_id` 记录本次变更的操作者，只作追溯用途，本函数不校验其是否具备修改权限。
/// 单条自治语句，不参与事务，成功即已落库；序列化或执行失败直接上抛，不会返回虚假成功。
pub async fn save_security_policy(
    pool: &Pool<MySql>,
    policy: &UserSecurityPolicy,
    admin_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO security_policy_configs (policy_key, policy_value, updated_by)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE
               policy_value = VALUES(policy_value),
               updated_by = VALUES(updated_by)"#,
    )
    .bind(USER_SECURITY_POLICY_KEY)
    .bind(SqlxJson(policy.clone()))
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 读取用户的二次验证设置，含加密密钥、两个开关与确认、最近验证两个时间戳。
/// 设置行在用户注册时并不创建，只有进入绑定流程才会产生，因此查不到记录属于正常情况，
/// 此时返回全零值的未绑定快照，让调用方无需区分「没有记录」与「有记录但未启用」。
/// 返回的密钥是密文，解密需要凭证加密密钥且只能在 application 层进行；
/// 该字段禁止直接序列化进响应体或写入日志。
/// 只读且不加锁，因此返回的快照可能在调用方后续判定期间被并发修改，
/// 依赖状态判定的写入路径需自行承担这一竞态。
pub async fn load_user_two_factor(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<UserTwoFactorSettings> {
    let settings = sqlx::query_as::<_, UserTwoFactorSettingsRow>(
        r#"SELECT user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled,
                  confirmed_at, last_verified_at
           FROM user_two_factor_settings
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings
        .map(Into::into)
        .unwrap_or_else(|| UserTwoFactorSettings::empty(user_id)))
}

/// 写入一枚处于待确认状态的用户 TOTP 密钥，是绑定流程第一步的落库动作。
/// 写入时把该用户二次验证行的其余字段全部重置：启用标志与登录开关置假，两个时间戳清空，
/// 因此这一行在语义上回到「已生成密钥但尚未验证」的起点，此时二次验证并不生效。
/// 危险边界：SQL 不带任何状态条件，对已完成绑定的用户调用会直接关闭其现有绑定并替换密钥。
/// 调用方必须在此之前确认用户尚未绑定，本层不做这道检查。
/// 输入必须是已加密的密文，明文密钥不得传入。
/// 单条自治语句，无锁；两个并发的生成请求会各自覆盖，最终只有后写入的密钥能通过确认。
pub async fn save_pending_totp_secret(
    pool: &Pool<MySql>,
    user_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, FALSE, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = FALSE,
              login_2fa_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(user_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

/// 在动态码校验通过后正式启用用户 TOTP，是绑定流程第二步的落库动作。
/// 写入调用方传入的密文而非沿用库中现值，这样生效的密钥必定就是刚刚验证通过的那一枚，
/// 即便期间被并发替换过也不会启用一枚从未验证过的密钥。
/// 同时把启用标志置真并把确认时间与最近验证时间都设为当前时刻。
/// 登录二次验证开关只在插入分支初始化为假，冲突分支不触碰该列，
/// 因此重新确认不会意外关掉用户此前已开启的登录二次验证。
/// SQL 不比较库中待确认密钥也不检查受影响行数，无法防止读取后的并发覆盖，
/// 状态前置校验由 application 层负责。本函数只改设置行，不写审计也不签发任何令牌。
pub async fn confirm_user_totp(
    pool: &Pool<MySql>,
    user_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_two_factor_settings
              (user_id, totp_secret_encrypted, totp_enabled, login_2fa_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, TRUE, FALSE, CURRENT_TIMESTAMP(6), CURRENT_TIMESTAMP(6))
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = TRUE,
              confirmed_at = CURRENT_TIMESTAMP(6),
              last_verified_at = CURRENT_TIMESTAMP(6)"#,
    )
    .bind(user_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

/// 切换用户「登录时要求二次验证」的开关，WHERE 中的 `totp_enabled = TRUE` 是一道数据库级保险：
/// 未绑定 TOTP 的账号无论如何都不会被打开登录二次验证，否则用户将因缺少验证手段而被锁在门外。
/// 这里用 UPDATE 而非 upsert，因为开关只对已存在且已绑定的设置行有意义，不应为它建行。
/// 不检查受影响行数，所以用户不存在、无设置行或已被并发解绑时同样返回成功但实际未改动任何数据；
/// 调用方需先读状态判定是否允许切换，并理解此处存在读写之间的竞态窗口。
/// 只改这一个开关，不触碰密钥、启用标志和任何时间戳。
pub async fn set_user_login_two_factor(
    pool: &Pool<MySql>,
    user_id: u64,
    enabled: bool,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_two_factor_settings
           SET login_2fa_enabled = ?
           WHERE user_id = ? AND totp_enabled = TRUE"#,
    )
    .bind(enabled)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 把用户二次验证设置整体清回未绑定状态：密钥置空、启用标志与登录开关置假、两个时间戳清空。
/// 登录开关必须一并关闭，否则密钥已删而开关仍开，用户下次登录会被要求提交一个无从生成的动态码。
/// 用 upsert 而非 UPDATE，使得对从未建过设置行的用户调用也能落下一行明确的未绑定记录，
/// 这让重置动作在任何起始状态下都有一致结果。
/// 天然幂等，重复调用不报错也不产生额外影响。
/// 不做任何前置校验，因此这是一个无条件的强制解绑，调用方须自行确认操作者已通过邮箱验证码等身份证明。
pub async fn reset_user_two_factor(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
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
    .execute(pool)
    .await?;

    Ok(())
}

/// 在用户密码校验通过但尚未放行会话时创建一枚登录二次验证挑战，作为两步登录之间的凭据。
/// 挑战 ID 用 UUIDv7 生成，其中含时间戳分量因而单调递增，既保证全局唯一又便于按时间排查。
/// `challenge_type` 区分两条后续流程：已绑定用户走动态码验证，未绑定但策略强制时走首次绑定，
/// 类型随记录持久化，避免在第二步再去推断用户当时的状态。
/// 有效期固定为 `LOGIN_CHALLENGE_TTL_SECONDS` 秒，落库前转 naive UTC 与列的无时区语义对齐。
/// 每次调用都新增一条记录，不复用也不作废该用户此前未消费的挑战，
/// 因此同一用户可同时存在多枚有效挑战，防重放依靠逐枚消费而非唯一性。
/// 返回值含明文挑战 ID 供前端回传，本函数不签发任何访问令牌或会话。
pub async fn create_login_two_factor_challenge(
    pool: &Pool<MySql>,
    user_id: u64,
    challenge_type: LoginTwoFactorChallengeType,
) -> AppResult<CreatedLoginTwoFactorChallenge> {
    let challenge_id = Uuid::now_v7().to_string();
    let expires_at = Utc::now() + Duration::seconds(LOGIN_CHALLENGE_TTL_SECONDS);
    sqlx::query(
        r#"INSERT INTO login_two_factor_challenges
              (challenge_id, user_id, challenge_type, expires_at)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(&challenge_id)
    .bind(user_id)
    .bind(challenge_type.as_str())
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;

    Ok(CreatedLoginTwoFactorChallenge {
        challenge_id,
        expires_at,
        expires_in_seconds: LOGIN_CHALLENGE_TTL_SECONDS,
    })
}

/// 按挑战 ID 回读用户登录挑战的完整快照，供第二步判定其是否仍可用。
/// 关键设计：记录不存在时返回统一的「挑战已过期」错误，而不是未找到。
/// 伪造或猜测挑战 ID 因此得到与真实过期挑战完全一致的响应，无法据此判断某个 ID 是否存在。
/// 存储中的类型字符串经领域函数还原为枚举，未知值报校验错误而不降级，避免流程走错分支。
/// 本函数只负责取值，是否过期、是否已消费全部由调用方对照快照自行判定；
/// 不加锁也不修改任何状态，读取与后续消费之间存在竞态窗口。
pub async fn load_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<LoginTwoFactorChallenge> {
    let row = sqlx::query_as::<_, (String, u64, String, DateTime<Utc>, Option<DateTime<Utc>>)>(
        r#"SELECT challenge_id, user_id, challenge_type, expires_at, consumed_at
           FROM login_two_factor_challenges
           WHERE challenge_id = ?
           LIMIT 1"#,
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(login_challenge_expired)?;

    Ok(LoginTwoFactorChallenge {
        challenge_id: row.0,
        user_id: row.1,
        challenge_type: LoginTwoFactorChallengeType::from_storage(&row.2)?,
        expires_at: row.3,
        consumed_at: row.4,
    })
}

/// 把用户登录挑战标记为已消费，WHERE 中的 `consumed_at IS NULL` 确保只写第一次，
/// 后续重复调用不会刷新时间戳，因而首次消费时刻在记录中保持稳定可追溯。
/// 重要限制：本函数不检查受影响行数，无论挑战不存在、已消费还是刚被本次更新，一律返回成功。
/// 这意味着单靠调用它无法实现严格防重放——两个并发请求可以都收到成功。
/// 需要保证只有一个请求继续签发会话的调用方，必须自行判断受影响行数或引入其他互斥手段。
/// 消费时间由数据库当前时间填写，避免应用节点之间的时钟差异。
pub async fn consume_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE login_two_factor_challenges
           SET consumed_at = CURRENT_TIMESTAMP(6)
           WHERE challenge_id = ? AND consumed_at IS NULL"#,
    )
    .bind(challenge_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 读取管理员的二次验证设置，字段比用户版本少一个登录开关。
/// 后台登录是否强制二次验证由系统策略决定，管理员无法自行关闭，因此表中不存在该开关列。
/// 与用户侧同理，设置行只在进入绑定流程后才产生，查不到时返回未绑定的零值快照。
/// 返回的密钥为密文，仅供安全用例在内存中解密比对，不得透传进任何管理端响应结构。
/// 只读且不加锁。
pub async fn load_admin_two_factor(
    pool: &Pool<MySql>,
    admin_id: u64,
) -> AppResult<AdminTwoFactorSettings> {
    let settings = sqlx::query_as::<_, AdminTwoFactorSettingsRow>(
        r#"SELECT admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at
           FROM admin_two_factor_settings
           WHERE admin_id = ?
           LIMIT 1"#,
    )
    .bind(admin_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings
        .map(Into::into)
        .unwrap_or_else(|| AdminTwoFactorSettings::empty(admin_id)))
}

/// 写入管理员待确认的 TOTP 密钥密文，同时把启用标志置假、确认与最近验证时间清空。
/// 与用户版本的差别仅在于没有登录开关需要重置，其余语义完全一致。
/// 同样不带状态条件：对已绑定的管理员调用会直接关闭其现有绑定，
/// 由于后台登录强制二次验证，误调用可能导致该管理员暂时无法登录，前置检查务必在 application 层完成。
/// 输入必须已加密，密文不得写入日志或响应。单条自治语句，无锁，并发生成会互相覆盖。
pub async fn save_pending_admin_totp_secret(
    pool: &Pool<MySql>,
    admin_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
              (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(admin_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

/// 在动态码校验通过后启用管理员 TOTP，写入的是调用方传入并刚刚验证过的那份密文。
/// 启用标志置真，确认时间与最近验证时间同时设为数据库当前时刻——
/// 确认本身就是一次成功验证，所以两个时间戳在此刻相同是预期行为。
/// SQL 不比较库中现存的待确认密钥，也不检查受影响行数，无法独立防止读取后的并发覆盖。
/// 本函数只落设置行，既不签发管理端令牌也不消费任何登录挑战，后者由调用方另行处理。
pub async fn confirm_admin_totp(
    pool: &Pool<MySql>,
    admin_id: u64,
    encrypted_secret: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
              (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, ?, TRUE, CURRENT_TIMESTAMP(6), CURRENT_TIMESTAMP(6))
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = VALUES(totp_secret_encrypted),
              totp_enabled = TRUE,
              confirmed_at = CURRENT_TIMESTAMP(6),
              last_verified_at = CURRENT_TIMESTAMP(6)"#,
    )
    .bind(admin_id)
    .bind(encrypted_secret)
    .execute(pool)
    .await?;

    Ok(())
}

/// 把管理员二次验证设置清回未绑定状态：密钥置空、启用标志置假、两个时间戳清空。
/// 用 upsert 使从未建过设置行的管理员也能落下明确的未绑定记录，任何起始状态下结果一致。
/// 需要特别注意的运营影响：后台登录强制要求二次验证，因此重置后该管理员必须重新完成绑定，
/// 期间登录流程会把他导向首次绑定分支而非直接放行。
/// 天然幂等，重复调用不报错；不做任何权限校验，调用方须确认发起者具备重置他人二次验证的权限。
pub async fn reset_admin_two_factor(pool: &Pool<MySql>, admin_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_two_factor_settings
              (admin_id, totp_secret_encrypted, totp_enabled, confirmed_at, last_verified_at)
           VALUES (?, NULL, FALSE, NULL, NULL)
           ON DUPLICATE KEY UPDATE
              totp_secret_encrypted = NULL,
              totp_enabled = FALSE,
              confirmed_at = NULL,
              last_verified_at = NULL"#,
    )
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 记录管理员最后一次 TOTP 校验通过时间，不更改密钥或启用状态。
/// SQL 不检查受影响行数；记录缺失或被并发清除时也返回成功且不补建设置。
pub async fn record_admin_totp_verified(pool: &Pool<MySql>, admin_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_two_factor_settings
           SET last_verified_at = CURRENT_TIMESTAMP(6)
           WHERE admin_id = ?"#,
    )
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 在管理员密码校验通过后创建一枚后台登录二次验证挑战。
/// 与用户版本的两点结构差异：管理员挑战不带类型字段（后台流程唯一，无需区分登录与首次绑定），
/// 但带 `attempt_count` 失败计数列，由数据库默认值从零起算，配合上限常量阻断对挑战的暴力试码。
/// 挑战 ID 同样用 UUIDv7，有效期同样固定为 `LOGIN_CHALLENGE_TTL_SECONDS` 秒并转 naive UTC 落库。
/// 每次调用新增记录，不作废该管理员既有的未消费挑战。
/// 返回值只含挑战 ID 与过期信息，不含任何后台令牌。
pub async fn create_admin_login_two_factor_challenge(
    pool: &Pool<MySql>,
    admin_id: u64,
    auth_session_version: u64,
) -> AppResult<CreatedLoginTwoFactorChallenge> {
    let challenge_id = Uuid::now_v7().to_string();
    let expires_at = Utc::now() + Duration::seconds(LOGIN_CHALLENGE_TTL_SECONDS);
    sqlx::query(
        r#"INSERT INTO admin_login_two_factor_challenges
              (challenge_id, admin_id, auth_session_version, expires_at)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(&challenge_id)
    .bind(admin_id)
    .bind(auth_session_version)
    .bind(expires_at.naive_utc())
    .execute(pool)
    .await?;

    Ok(CreatedLoginTwoFactorChallenge {
        challenge_id,
        expires_at,
        expires_in_seconds: LOGIN_CHALLENGE_TTL_SECONDS,
    })
}

/// 按 ID 回读管理员登录挑战快照，比用户版本多带一个 `attempt_count` 字段。
/// 调用方需据此对照 `ADMIN_LOGIN_TWO_FACTOR_ATTEMPT_LIMIT` 判断该挑战是否已被试码耗尽，
/// 耗尽后即使未到过期时间也应视为不可用。
/// 记录不存在时同样返回统一的「挑战已过期」错误，使不存在、过期、已消费、次数用尽四种情形对外表现一致。
/// 只取值不判定、不加锁、不修改状态，过期与次数判定全部由调用方完成。
pub async fn load_admin_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<AdminLoginTwoFactorChallenge> {
    let row = sqlx::query_as::<_, (String, u64, u64, u32, DateTime<Utc>, Option<DateTime<Utc>>)>(
        r#"SELECT challenges.challenge_id, challenges.admin_id,
                  challenges.auth_session_version, challenges.attempt_count,
                  challenges.expires_at, challenges.consumed_at
           FROM admin_login_two_factor_challenges challenges
           INNER JOIN admin_users admins
                   ON admins.id = challenges.admin_id
                  AND admins.status = 'active'
                  AND admins.auth_session_version = challenges.auth_session_version
           WHERE challenges.challenge_id = ?
           LIMIT 1"#,
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(login_challenge_expired)?;

    Ok(AdminLoginTwoFactorChallenge {
        challenge_id: row.0,
        admin_id: row.1,
        auth_session_version: row.2,
        attempt_count: row.3,
        expires_at: row.4,
        consumed_at: row.5,
    })
}

/// 给管理员登录挑战的失败计数加一，在动态码校验不通过时调用，用于逼近尝试上限后作废该挑战。
/// 采用数据库端自增避免并发试码丢失计数。
/// WHERE 只按挑战 ID 匹配，不限定未消费或未过期，因此对已消费或已过期的挑战调用同样会累加，
/// 计数本身不构成状态判定，可用性判断仍以调用方读取快照后的比对为准。
/// 不检查受影响行数，挑战不存在时也返回成功，此时实际未修改任何数据。
/// 调用方必须在返回错误前确保这次自增已生效，否则失败次数无法累积，上限保护将形同虚设。
pub async fn increment_admin_login_two_factor_attempt(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_login_two_factor_challenges
           SET attempt_count = attempt_count + 1
           WHERE challenge_id = ?"#,
    )
    .bind(challenge_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 把管理员登录挑战标记为已消费，语义与用户版本一致但作用于后台挑战表。
/// `consumed_at IS NULL` 保证只有首次消费会写入时间戳，重复调用不会覆盖原始消费时刻。
/// 同样不检查受影响行数，因此不存在、已消费与本次刚消费三种情况都返回成功，
/// 单靠本函数无法保证并发请求中只有一个能继续签发后台令牌。
/// 后台会话权限高于普通用户，需要严格互斥的调用方应在此之外补充判定手段。
/// 消费时间取数据库当前时间，不受应用节点时钟影响。
pub async fn consume_admin_login_two_factor_challenge(
    pool: &Pool<MySql>,
    challenge_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE admin_login_two_factor_challenges
           SET consumed_at = CURRENT_TIMESTAMP(6)
           WHERE challenge_id = ? AND consumed_at IS NULL"#,
    )
    .bind(challenge_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 取出用户的资金密码哈希，供安全上下文在支付类动作前比对用户提交的口令。
/// 两种「没有」被压平成同一个 `None`：安全行不存在，以及安全行存在但哈希列为空，
/// 对调用方而言两者处置一致，都意味着该用户尚未设置资金密码。
/// 只读取不比对：明文与哈希的比对在 application 层完成，基础设施层不接触任何口令明文。
/// 不加锁，也不属于任何事务，仅用于校验路径；需要防并发覆盖的写入路径应改用 user 上下文的锁定版本。
/// 返回的哈希属敏感凭据，只可在内存中参与校验，禁止进入响应体、审计记录或日志。
pub async fn load_user_fund_password_hash(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<String>> {
    let hash: Option<String> = sqlx::query_scalar(
        r#"SELECT fund_password_hash
           FROM user_security
           WHERE user_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    Ok(hash)
}

/// 记录用户最后一次 TOTP 通过时间，不改变登录开关或密钥内容。
/// SQL 不检查受影响行数；设置行缺失时以零行更新的成功结果返回。
pub async fn record_user_totp_verified(pool: &Pool<MySql>, user_id: u64) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_two_factor_settings
           SET last_verified_at = CURRENT_TIMESTAMP(6)
           WHERE user_id = ?"#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
