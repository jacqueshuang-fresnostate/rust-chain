//! 管理员一次性引导：只有部署方显式开启引导模式时，才在后台账号表为空的环境创建首个管理员。
//! 引导口令只能来自环境变量或 Secret 文件；缺失、空值和已知公开默认值均在连接数据库前失败。
//! 整个过程先用 MySQL 命名锁串行化，再在事务里检查是否已存在管理员，确保多实例并发启动最多只产生一个初始账号。
//! 本模块只负责创建首个管理员及其所属角色，不写入任何具体权限，权限内容需要登录后台之后再行配置。

use crate::{
    error::{AppError, AppResult},
    modules::auth::{hash_password, normalize_username},
};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use sqlx::{Acquire, MySqlConnection, MySqlPool};
use std::{env, fmt, fs};

pub const DEFAULT_BOOTSTRAP_ADMIN_ROLE_NAME: &str = "super_admin";
pub const DEFAULT_BOOTSTRAP_ADMIN_USERNAME: &str = "admin";

const BOOTSTRAP_ADMIN_LOCK_NAME: &str = "exchange.bootstrap.default_admin";
const BOOTSTRAP_ADMIN_LOCK_TIMEOUT_SECONDS: i32 = 30;
const BOOTSTRAP_ADMIN_PASSWORD_MIN_CHARS: usize = 8;
const BOOTSTRAP_ADMIN_PASSWORD_MAX_CHARS: usize = 128;
const BOOTSTRAP_ADMIN_ROLE_NAME_MAX_CHARS: usize = 64;
// 已公开历史口令仅保存摘要，源码和镜像示例不得再携带可直接登录的明文。
const KNOWN_DEFAULT_BOOTSTRAP_PASSWORD_SHA256: &str =
    "c76e3324b6aa293f00cf010dabaf82a58c9d2ecb34df4f3c433de0822edb9421";

/// 管理员引导开关；缺省明确表示关闭，只有 `create_admin` 会触发账号创建。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapAdminMode {
    Disabled,
    CreateAdmin,
}

impl BootstrapAdminMode {
    /// 解析 `BOOTSTRAP_MODE`，缺失时安全关闭；显式空值或未知值直接报错，避免拼写错误意外改变部署行为。
    pub fn from_env() -> AppResult<Self> {
        Self::from_optional_value(optional_env("BOOTSTRAP_MODE")?)
    }

    fn from_optional_value(value: Option<String>) -> AppResult<Self> {
        match value {
            None => Ok(Self::Disabled),
            Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                "disabled" => Ok(Self::Disabled),
                "create_admin" => Ok(Self::CreateAdmin),
                _ => Err(AppError::Validation(
                    "BOOTSTRAP_MODE must be disabled or create_admin".to_owned(),
                )),
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BootstrapPasswordSource {
    Direct(String),
    File(String),
}

/// 已校验的引导管理员三要素，只能通过本文件的构造函数得到，保证字段都经过规范化。
/// 口令用 `SecretString` 承载并配有手写的 `Debug`，避免引导配置出现在日志时泄露明文。
pub struct BootstrapAdminConfig {
    username: String,
    password: SecretString,
    role_name: String,
}

impl BootstrapAdminConfig {
    /// 从部署环境读取一次性引导凭据；口令可来自 `BOOTSTRAP_ADMIN_PASSWORD` 或其 `_FILE` 变体，但二者不可同时提供非空值。
    /// Compose 会将未选中的另一来源展开为空串，该空占位不算第二个来源；若两者都缺失或为空仍直接失败。
    /// 用户名和角色名仍允许使用非敏感默认值；口令读取失败或命中历史公开值也会直接失败。
    pub fn from_env() -> AppResult<Self> {
        Self::from_values(
            env_or_default("BOOTSTRAP_ADMIN_USERNAME", DEFAULT_BOOTSTRAP_ADMIN_USERNAME)?,
            required_password_secret()?,
            optional_env("BOOTSTRAP_ADMIN_ROLE_NAME")?,
        )
    }

    /// 校验并规范化引导管理员三要素，是所有构造入口的唯一收口，保证环境变量与内置默认值遵守同一套规则。
    /// 用户名复用正式注册的规范化逻辑；口令按字符数而非字节数限制在八到一百二十八之间，越界返回校验错误。
    /// 角色名去空白后为空时回落到默认超级管理员角色，随后统一转小写并限制为字母、数字、下划线与连字符。
    /// 通过校验的口令只是被包进 `SecretString`，此处既不做哈希也不触碰数据库，构造成功不代表账号已经存在。
    pub fn from_values(
        username: String,
        password: String,
        role_name: Option<String>,
    ) -> AppResult<Self> {
        let username = normalize_username(&username)?;
        let password_length = password.chars().count();
        if password.trim().is_empty() {
            return Err(AppError::Validation(
                "BOOTSTRAP_ADMIN_PASSWORD must not be blank".to_owned(),
            ));
        }
        if !(BOOTSTRAP_ADMIN_PASSWORD_MIN_CHARS..=BOOTSTRAP_ADMIN_PASSWORD_MAX_CHARS)
            .contains(&password_length)
        {
            return Err(AppError::Validation(format!(
                "BOOTSTRAP_ADMIN_PASSWORD must be {BOOTSTRAP_ADMIN_PASSWORD_MIN_CHARS}-{BOOTSTRAP_ADMIN_PASSWORD_MAX_CHARS} characters long"
            )));
        }
        if hex::encode(Sha256::digest(password.as_bytes()))
            == KNOWN_DEFAULT_BOOTSTRAP_PASSWORD_SHA256
        {
            return Err(AppError::Validation(
                "BOOTSTRAP_ADMIN_PASSWORD is a known default and must be replaced".to_owned(),
            ));
        }

        let role_name = normalize_role_name(
            optional_trimmed(role_name)
                .as_deref()
                .unwrap_or(DEFAULT_BOOTSTRAP_ADMIN_ROLE_NAME),
        )?;

        Ok(Self {
            username,
            password: SecretString::new(password),
            role_name,
        })
    }

    /// 读取已规范化的引导管理员用户名，让插入语句与启动日志复用同一份取值，避免两处各自再清洗一遍。
    /// 返回值必定通过了与正式注册相同的校验，调用方不需要再裁剪空白或处理大小写。
    pub fn username(&self) -> &str {
        &self.username
    }

    /// 读取已规范化的引导角色名，事务里先按它查角色、查不到再新建，是角色查找与创建共用的键。
    /// 返回值已转成小写并限定了字符集，与数据库中的角色名比较时无需再做大小写折叠。
    pub fn role_name(&self) -> &str {
        &self.role_name
    }
}

impl fmt::Debug for BootstrapAdminConfig {
    /// 手写调试输出，只保留用户名与角色名，口令字段固定打印为占位串。
    /// 引导配置经常出现在启动日志和错误上下文里，因此必须屏蔽口令原文，不能改回自动派生的实现。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BootstrapAdminConfig")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("role_name", &self.role_name)
            .finish()
    }
}

/// 引导执行结果，用于区分本次真正创建了首个管理员，还是因为库里已有管理员而整体跳过。
/// 调用方据此决定启动日志措辞，两种取值都表示引导流程成功，不代表出现异常。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapAdminOutcome {
    Created,
    SkippedExistingAdmin,
}

/// 引导创建首个后台管理员，是本模块唯一对外的执行入口，通常由迁移程序在所有 migration 应用完成后调用。
/// 先从连接池取出一条连接申请 MySQL 命名锁，最多等待三十秒，超时按内部错误返回，避免多实例同时创建管理员。
/// 拿到锁后把实际写入交给加锁版本执行，无论成功还是失败都会尝试显式释放锁。
/// 释放失败时不会把这条可能仍持锁的连接放回连接池，而是直接关闭；若业务本身也出错，则优先上报业务错误。
pub async fn bootstrap_default_admin(
    pool: &MySqlPool,
    config: &BootstrapAdminConfig,
) -> AppResult<BootstrapAdminOutcome> {
    let mut connection = pool.acquire().await?;
    let lock_acquired = sqlx::query_scalar::<_, Option<i64>>("SELECT GET_LOCK(?, ?)")
        .bind(BOOTSTRAP_ADMIN_LOCK_NAME)
        .bind(BOOTSTRAP_ADMIN_LOCK_TIMEOUT_SECONDS)
        .fetch_one(&mut *connection)
        .await?;
    if lock_acquired != Some(1) {
        return Err(AppError::Internal(
            "timed out while acquiring the default administrator bootstrap lock".to_owned(),
        ));
    }

    let result = bootstrap_default_admin_while_locked(&mut connection, config).await;
    match release_bootstrap_lock(&mut connection).await {
        Ok(()) => result,
        Err(release_error) => {
            // 命名锁属于 MySQL 会话；显式释放失败时不得把可能仍持锁的连接放回连接池。
            connection.close().await?;
            match result {
                Ok(_) => Err(release_error),
                Err(bootstrap_error) => Err(bootstrap_error),
            }
        }
    }
}

/// 在已持有命名锁的连接上开启事务，完成一次幂等的首个管理员创建，返回值区分本次新建还是跳过。
/// 事务内先用 `FOR UPDATE` 读管理员表首行，只要存在任意一个管理员就提交空事务并跳过，不补建也不覆盖既有账号。
/// 角色按名称加锁查找，缺失时插入显式 `*` 权限，使首个管理员能完成后续角色与业务配置。
/// 口令在事务内哈希后写入，新管理员状态直接置为启用并标记首次强制改密；角色与管理员同事务提交，避免中途失败留下孤立角色。
async fn bootstrap_default_admin_while_locked(
    connection: &mut MySqlConnection,
    config: &BootstrapAdminConfig,
) -> AppResult<BootstrapAdminOutcome> {
    let mut transaction = connection.begin().await?;

    let existing_admin_id =
        sqlx::query_scalar::<_, u64>("SELECT id FROM admin_users ORDER BY id LIMIT 1 FOR UPDATE")
            .fetch_optional(&mut *transaction)
            .await?;
    if existing_admin_id.is_some() {
        transaction.commit().await?;
        return Ok(BootstrapAdminOutcome::SkippedExistingAdmin);
    }

    let password_hash = hash_password(config.password.expose_secret())?;
    let role_id = match sqlx::query_scalar::<_, u64>(
        "SELECT id FROM admin_roles WHERE name = ? LIMIT 1 FOR UPDATE",
    )
    .bind(&config.role_name)
    .fetch_optional(&mut *transaction)
    .await?
    {
        Some(role_id) => role_id,
        None => {
            sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
                .bind(&config.role_name)
                .execute(&mut *transaction)
                .await?
                .last_insert_id()
        }
    };

    // 任意管理员存在时必须整体跳过；角色与首个管理员也必须同事务提交，避免留下孤立角色。
    sqlx::query(
        "INSERT INTO admin_users (username, password_hash, must_change_password, role_id, status) VALUES (?, ?, TRUE, ?, 'active')",
    )
    .bind(&config.username)
    .bind(password_hash)
    .bind(role_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(BootstrapAdminOutcome::Created)
}

/// 在同一条连接上释放引导命名锁，只有数据库返回 1 才算释放成功。
/// 返回其他取值说明锁不存在或不属于当前会话，此时必须报错让调用方丢弃连接，不能当作释放成功继续复用。
async fn release_bootstrap_lock(connection: &mut MySqlConnection) -> AppResult<()> {
    let lock_released = sqlx::query_scalar::<_, Option<i64>>("SELECT RELEASE_LOCK(?)")
        .bind(BOOTSTRAP_ADMIN_LOCK_NAME)
        .fetch_one(connection)
        .await?;
    if lock_released == Some(1) {
        Ok(())
    } else {
        Err(AppError::Internal(
            "failed to release the default administrator bootstrap lock".to_owned(),
        ))
    }
}

/// 读取可选环境变量，并把「未设置」和「编码非法」两种情况区分开：前者返回 `None`，后者返回校验错误。
/// 这样处理是为了避免把编码错误静默当成未配置，导致部署方明明设置了口令却仍然按内置默认值引导。
fn optional_env(name: &str) -> AppResult<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(AppError::Validation(format!(
            "{name} must contain valid UTF-8"
        ))),
    }
}

/// 读取环境变量并在缺失、或去掉首尾空白后变成空串时回落到给定默认值，用于用户名和口令这类必须有值的项。
/// 只有编码非法才会向上报错；纯空白视同未配置，因此在配置文件里写空值不会产生空用户名或空口令。
fn env_or_default(name: &str, default: &str) -> AppResult<String> {
    Ok(optional_trimmed(optional_env(name)?).unwrap_or_else(|| default.to_owned()))
}

/// 读取引导口令的环境变量或 Docker Secret 文件；Compose 为未选中来源注入的空占位被视为未配置。
/// 两个来源均缺失/空白、同时非空、文件不可读或 Secret 内容为空都返回配置错误，绝不退回固定口令。
fn required_password_secret() -> AppResult<String> {
    let direct_raw = optional_env("BOOTSTRAP_ADMIN_PASSWORD")?;
    let file_raw = optional_env("BOOTSTRAP_ADMIN_PASSWORD_FILE")?;
    match select_password_source(direct_raw, file_raw)? {
        BootstrapPasswordSource::Direct(value) => Ok(value),
        BootstrapPasswordSource::File(path) => {
            let value = fs::read_to_string(path.trim()).map_err(|error| {
                AppError::Validation(format!(
                    "BOOTSTRAP_ADMIN_PASSWORD_FILE could not be read: {error}"
                ))
            })?;
            let value = value.trim_end_matches(['\r', '\n']).to_owned();
            if value.trim().is_empty() {
                Err(AppError::Validation(
                    "BOOTSTRAP_ADMIN_PASSWORD_FILE must not be empty".to_owned(),
                ))
            } else {
                Ok(value)
            }
        }
    }
}

fn select_password_source(
    direct_raw: Option<String>,
    file_raw: Option<String>,
) -> AppResult<BootstrapPasswordSource> {
    let direct = direct_raw.as_ref().filter(|value| !value.trim().is_empty());
    let file = file_raw.as_ref().filter(|value| !value.trim().is_empty());
    if direct.is_some() && file.is_some() {
        return Err(AppError::Validation(
            "set only one of BOOTSTRAP_ADMIN_PASSWORD or BOOTSTRAP_ADMIN_PASSWORD_FILE".to_owned(),
        ));
    }

    match (direct, file) {
        (Some(value), None) => Ok(BootstrapPasswordSource::Direct(value.to_owned())),
        (None, Some(path)) => Ok(BootstrapPasswordSource::File(path.trim().to_owned())),
        _ if direct_raw.is_some() && file_raw.is_none() => Err(AppError::Validation(
            "BOOTSTRAP_ADMIN_PASSWORD must not be blank".to_owned(),
        )),
        _ if file_raw.is_some() && direct_raw.is_none() => Err(AppError::Validation(
            "BOOTSTRAP_ADMIN_PASSWORD_FILE must not be blank".to_owned(),
        )),
        _ => Err(AppError::Validation(
            "BOOTSTRAP_ADMIN_PASSWORD or BOOTSTRAP_ADMIN_PASSWORD_FILE is required and must not be blank"
                .to_owned(),
        )),
    }
}

/// 裁掉可选文本的首尾空白，并把裁剪后变成空串的输入折叠为 `None`，统一「留空即未配置」的语义。
/// 该转换只处理空白，不涉及大小写、字符集或长度约束，这些规则由各自的规范化函数负责。
fn optional_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 规范化引导角色名：先去掉首尾空白并转成小写，再要求长度落在一到六十四个字符之间。
/// 字符集只允许字母、数字、下划线和连字符，任何越界长度或非法字符都返回校验错误而不是自动剔除。
/// 统一转小写是为了让按名称查找角色时不会因大小写差异重复建出同名角色。
fn normalize_role_name(value: &str) -> AppResult<String> {
    let role_name = value.trim().to_ascii_lowercase();
    let length = role_name.chars().count();
    if !(1..=BOOTSTRAP_ADMIN_ROLE_NAME_MAX_CHARS).contains(&length)
        || !role_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err(AppError::Validation(format!(
            "BOOTSTRAP_ADMIN_ROLE_NAME must be 1-{BOOTSTRAP_ADMIN_ROLE_NAME_MAX_CHARS} characters and contain only letters, numbers, underscore, or hyphen"
        )));
    }
    Ok(role_name)
}

#[cfg(test)]
#[path = "../tests/unit_src/src_bootstrap_tests.rs"]
mod tests;
