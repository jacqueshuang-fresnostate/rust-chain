use crate::{
    error::{AppError, AppResult},
    modules::auth::{hash_password, normalize_username},
};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{Acquire, MySqlConnection, MySqlPool};
use std::{env, fmt};

pub const DEFAULT_BOOTSTRAP_ADMIN_ROLE_NAME: &str = "super_admin";
pub const DEFAULT_BOOTSTRAP_ADMIN_USERNAME: &str = "admin";

const BOOTSTRAP_ADMIN_LOCK_NAME: &str = "exchange.bootstrap.default_admin";
const BOOTSTRAP_ADMIN_LOCK_TIMEOUT_SECONDS: i32 = 30;
const DEFAULT_BOOTSTRAP_ADMIN_PASSWORD: &str = "Qaz123456@";
const BOOTSTRAP_ADMIN_PASSWORD_MIN_CHARS: usize = 8;
const BOOTSTRAP_ADMIN_PASSWORD_MAX_CHARS: usize = 128;
const BOOTSTRAP_ADMIN_ROLE_NAME_MAX_CHARS: usize = 64;

pub struct BootstrapAdminConfig {
    username: String,
    password: SecretString,
    role_name: String,
}

impl BootstrapAdminConfig {
    pub fn built_in_defaults() -> AppResult<Self> {
        Self::from_values(
            DEFAULT_BOOTSTRAP_ADMIN_USERNAME.to_owned(),
            DEFAULT_BOOTSTRAP_ADMIN_PASSWORD.to_owned(),
            Some(DEFAULT_BOOTSTRAP_ADMIN_ROLE_NAME.to_owned()),
        )
    }

    pub fn from_env() -> AppResult<Self> {
        Self::from_values(
            env_or_default("BOOTSTRAP_ADMIN_USERNAME", DEFAULT_BOOTSTRAP_ADMIN_USERNAME)?,
            env_or_default("BOOTSTRAP_ADMIN_PASSWORD", DEFAULT_BOOTSTRAP_ADMIN_PASSWORD)?,
            optional_env("BOOTSTRAP_ADMIN_ROLE_NAME")?,
        )
    }

    pub fn from_values(
        username: String,
        password: String,
        role_name: Option<String>,
    ) -> AppResult<Self> {
        let username = normalize_username(&username)?;
        let password_length = password.chars().count();
        if !(BOOTSTRAP_ADMIN_PASSWORD_MIN_CHARS..=BOOTSTRAP_ADMIN_PASSWORD_MAX_CHARS)
            .contains(&password_length)
        {
            return Err(AppError::Validation(format!(
                "BOOTSTRAP_ADMIN_PASSWORD must be {BOOTSTRAP_ADMIN_PASSWORD_MIN_CHARS}-{BOOTSTRAP_ADMIN_PASSWORD_MAX_CHARS} characters long"
            )));
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

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn role_name(&self) -> &str {
        &self.role_name
    }
}

impl fmt::Debug for BootstrapAdminConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BootstrapAdminConfig")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("role_name", &self.role_name)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapAdminOutcome {
    Created,
    SkippedExistingAdmin,
}

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
            sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_OBJECT())")
                .bind(&config.role_name)
                .execute(&mut *transaction)
                .await?
                .last_insert_id()
        }
    };

    // 任意管理员存在时必须整体跳过；角色与首个管理员也必须同事务提交，避免留下孤立角色。
    sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id, status) VALUES (?, ?, ?, 'active')",
    )
    .bind(&config.username)
    .bind(password_hash)
    .bind(role_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(BootstrapAdminOutcome::Created)
}

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

fn optional_env(name: &str) -> AppResult<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(AppError::Validation(format!(
            "{name} must contain valid UTF-8"
        ))),
    }
}

fn env_or_default(name: &str, default: &str) -> AppResult<String> {
    Ok(optional_trimmed(optional_env(name)?).unwrap_or_else(|| default.to_owned()))
}

fn optional_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

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
