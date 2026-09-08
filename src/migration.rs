//! 迁移进程的连接重试与失败诊断辅助。
//!
//! 迁移器运行在独立容器中，外部 MySQL 可能比容器晚几秒进入可接受连接的状态。
//! 本模块只对明确属于启动竞态的连接错误做有限重试；认证、URL 格式、权限和
//! migration SQL 错误不会被吞掉，也不会自动修改 `_sqlx_migrations`。

use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{
    error::Error as StdError,
    io::ErrorKind,
    time::{Duration, Instant as StdInstant},
};
use tokio::time::{sleep, timeout};

pub const MIGRATION_CONNECT_MAX_ATTEMPTS_ENV: &str = "MIGRATION_CONNECT_MAX_ATTEMPTS";
pub const MIGRATION_CONNECT_RETRY_DELAY_SECONDS_ENV: &str = "MIGRATION_CONNECT_RETRY_DELAY_SECONDS";

pub const DEFAULT_MIGRATION_CONNECT_MAX_ATTEMPTS: u32 = 30;
pub const DEFAULT_MIGRATION_CONNECT_RETRY_DELAY_SECONDS: u64 = 2;
pub const MAX_MIGRATION_CONNECT_ATTEMPTS: u32 = 30;
pub const MAX_MIGRATION_CONNECT_RETRY_DELAY_SECONDS: u64 = 30;
/// 建连重试（包含每次建连超时和退避）的硬总时限，不能通过环境变量延长。
pub const MAX_MIGRATION_CONNECT_TOTAL_WAIT_SECONDS: u64 = 300;

// SQLx 自身的连接池也会在 acquire timeout 内处理一次连接建立。把每次尝试限制在
// 五秒，避免错误的外部地址阻塞单次尝试；此外由硬总时限兜住所有环境变量组合。
const CONNECT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);
const DIAGNOSTIC_QUERY_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_DIRTY_VERSIONS_IN_LOG: usize = 8;

/// 迁移初始连接的有界重试参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationConnectConfig {
    pub max_attempts: u32,
    pub retry_delay_seconds: u64,
}

impl Default for MigrationConnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MIGRATION_CONNECT_MAX_ATTEMPTS,
            retry_delay_seconds: DEFAULT_MIGRATION_CONNECT_RETRY_DELAY_SECONDS,
        }
    }
}

impl MigrationConnectConfig {
    /// 从迁移专用环境变量解析参数；未设置时使用默认值，空值和超出范围的值直接失败。
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_optional_values(
            read_optional_env(MIGRATION_CONNECT_MAX_ATTEMPTS_ENV)?,
            read_optional_env(MIGRATION_CONNECT_RETRY_DELAY_SECONDS_ENV)?,
        )
    }

    /// 用显式字符串构造配置，供单元测试和嵌入式调用复用同一边界规则。
    pub fn from_optional_values(
        max_attempts: Option<String>,
        retry_delay_seconds: Option<String>,
    ) -> anyhow::Result<Self> {
        let max_attempts = parse_bounded_u32(
            MIGRATION_CONNECT_MAX_ATTEMPTS_ENV,
            max_attempts,
            DEFAULT_MIGRATION_CONNECT_MAX_ATTEMPTS,
            1,
            MAX_MIGRATION_CONNECT_ATTEMPTS,
        )?;
        let retry_delay_seconds = parse_bounded_u64(
            MIGRATION_CONNECT_RETRY_DELAY_SECONDS_ENV,
            retry_delay_seconds,
            DEFAULT_MIGRATION_CONNECT_RETRY_DELAY_SECONDS,
            0,
            MAX_MIGRATION_CONNECT_RETRY_DELAY_SECONDS,
        )?;
        let config = Self {
            max_attempts,
            retry_delay_seconds,
        };
        config.validate()?;
        Ok(config)
    }

    pub const fn retry_delay(&self) -> Duration {
        Duration::from_secs(self.retry_delay_seconds)
    }

    /// 校验公开字段，防止调用方绕过 `from_optional_values` 手工构造出无界配置。
    /// 迁移入口在发起任何网络连接前调用此方法，确保重试次数、退避间隔和总等待窗口均受限。
    pub fn validate(&self) -> anyhow::Result<()> {
        if !(1..=MAX_MIGRATION_CONNECT_ATTEMPTS).contains(&self.max_attempts) {
            anyhow::bail!(
                "{MIGRATION_CONNECT_MAX_ATTEMPTS_ENV} must be an integer between 1 and {MAX_MIGRATION_CONNECT_ATTEMPTS}"
            );
        }
        if self.retry_delay_seconds > MAX_MIGRATION_CONNECT_RETRY_DELAY_SECONDS {
            anyhow::bail!(
                "{MIGRATION_CONNECT_RETRY_DELAY_SECONDS_ENV} must be an integer between 0 and {MAX_MIGRATION_CONNECT_RETRY_DELAY_SECONDS}"
            );
        }
        Ok(())
    }
}

fn read_optional_env(name: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("{name} must contain valid UTF-8")
        }
    }
}

fn parse_bounded_u32(
    name: &str,
    value: Option<String>,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> anyhow::Result<u32> {
    let Some(value) = value else {
        return Ok(default);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{name} must be an integer between {minimum} and {maximum}");
    }
    let parsed = trimmed.parse::<u32>().map_err(|_| {
        anyhow::anyhow!("{name} must be an integer between {minimum} and {maximum}")
    })?;
    if !(minimum..=maximum).contains(&parsed) {
        anyhow::bail!("{name} must be an integer between {minimum} and {maximum}");
    }
    Ok(parsed)
}

fn parse_bounded_u64(
    name: &str,
    value: Option<String>,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> anyhow::Result<u64> {
    let Some(value) = value else {
        return Ok(default);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{name} must be an integer between {minimum} and {maximum}");
    }
    let parsed = trimmed.parse::<u64>().map_err(|_| {
        anyhow::anyhow!("{name} must be an integer between {minimum} and {maximum}")
    })?;
    if !(minimum..=maximum).contains(&parsed) {
        anyhow::bail!("{name} must be an integer between {minimum} and {maximum}");
    }
    Ok(parsed)
}

/// 以有限次数建立单连接迁移池。只有网络启动竞态或 SQLx 明确标识的暂时性连接错误会重试。
pub async fn connect_with_retry(
    database_url: &str,
    config: MigrationConnectConfig,
) -> anyhow::Result<MySqlPool> {
    config.validate()?;
    let retry_delay = config.retry_delay();
    let mut last_error = None;
    let started_at = StdInstant::now();
    let deadline = started_at + Duration::from_secs(MAX_MIGRATION_CONNECT_TOTAL_WAIT_SECONDS);
    let mut attempts_completed = 0_u32;
    let mut total_timeout_reached = false;

    for attempt in 1..=config.max_attempts {
        let remaining = deadline.saturating_duration_since(StdInstant::now());
        if remaining.is_zero() {
            total_timeout_reached = true;
            break;
        }
        let attempt_timeout = CONNECT_ATTEMPT_TIMEOUT.min(remaining);
        tracing::info!(
            attempt,
            max_attempts = config.max_attempts,
            retry_in_seconds = config.retry_delay_seconds,
            attempt_timeout_seconds = attempt_timeout.as_secs_f64(),
            total_timeout_seconds = MAX_MIGRATION_CONNECT_TOTAL_WAIT_SECONDS,
            "迁移数据库连接尝试"
        );

        let connect_future = MySqlPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(attempt_timeout)
            .connect(database_url);
        let result = match timeout(attempt_timeout, connect_future).await {
            Ok(result) => result,
            Err(_) => Err(sqlx::Error::PoolTimedOut),
        };
        attempts_completed = attempt;

        match result {
            Ok(pool) => {
                tracing::info!(attempt, "迁移数据库连接已建立");
                return Ok(pool);
            }
            Err(error) if attempt < config.max_attempts && is_transient_connect_error(&error) => {
                let summary = safe_sqlx_error(&error);
                tracing::warn!(
                    attempt,
                    max_attempts = config.max_attempts,
                    error = %summary,
                    retry_in_seconds = config.retry_delay_seconds,
                    "迁移数据库连接暂时不可用，将按有界策略重试"
                );
                last_error = Some(summary);
                let remaining = deadline.saturating_duration_since(StdInstant::now());
                if remaining.is_zero() {
                    total_timeout_reached = true;
                    break;
                }
                let delay = retry_delay.min(remaining);
                if !delay.is_zero() {
                    sleep(delay).await;
                }
                if delay < retry_delay
                    && deadline
                        .saturating_duration_since(StdInstant::now())
                        .is_zero()
                {
                    total_timeout_reached = true;
                    break;
                }
            }
            Err(error) => {
                let summary = safe_sqlx_error(&error);
                let kind = if is_transient_connect_error(&error) {
                    "暂时性连接错误已达到重试上限"
                } else {
                    "非暂时性连接错误"
                };
                return Err(anyhow::anyhow!(
                    "连接 MySQL 失败（第 {attempt}/{} 次，{kind}）：{summary}",
                    config.max_attempts
                ));
            }
        }
    }

    if total_timeout_reached {
        let summary = last_error.unwrap_or_else(|| "未知连接错误".to_owned());
        tracing::error!(
            attempts = attempts_completed,
            total_timeout_seconds = MAX_MIGRATION_CONNECT_TOTAL_WAIT_SECONDS,
            error = %summary,
            "迁移数据库连接重试达到总等待上限"
        );
        Err(anyhow::anyhow!(
            "连接 MySQL 失败（已尝试 {attempts_completed} 次，达到总等待上限 {MAX_MIGRATION_CONNECT_TOTAL_WAIT_SECONDS} 秒）：{summary}"
        ))
    } else {
        let summary = last_error.unwrap_or_else(|| "未知连接错误".to_owned());
        Err(anyhow::anyhow!(
            "连接 MySQL 失败（已尝试 {attempts_completed} 次）：{summary}"
        ))
    }
}

/// 判断错误是否属于初始连接阶段可以短暂恢复的类别。
///
/// 认证失败、URL 解析失败、TLS 证书错误和 SQL 权限错误均返回 `false`，避免部署在错误配置上
/// 无意义地等待。MySQL 服务器返回的 08 类 SQLSTATE 以及少数明确的启动/容量错误才会重试。
pub fn is_transient_connect_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Io(error) => is_transient_io_error(error),
        sqlx::Error::Database(error) => {
            let sql_state_is_connection = error
                .code()
                .is_some_and(|code| code.as_ref().starts_with("08"));
            if sql_state_is_connection {
                return true;
            }

            error
                .as_error()
                .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                .is_some_and(|mysql_error| is_transient_mysql_error_code(mysql_error.number()))
        }
        sqlx::Error::PoolTimedOut | sqlx::Error::WorkerCrashed => true,
        _ => false,
    }
}

fn is_transient_io_error(error: &std::io::Error) -> bool {
    match error.kind() {
        ErrorKind::ConnectionRefused
        | ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::NotConnected
        | ErrorKind::TimedOut
        | ErrorKind::WouldBlock
        | ErrorKind::UnexpectedEof
        | ErrorKind::AddrNotAvailable
        | ErrorKind::BrokenPipe
        | ErrorKind::Interrupted => true,
        // Tokio surfaces temporary getaddrinfo failures as `Other`. Match only the
        // platform messages that explicitly indicate a retryable DNS condition;
        // permanent "name or service not known" errors remain fail-fast.
        ErrorKind::Other => {
            let message = error.to_string().to_ascii_lowercase();
            [
                "temporary failure in name resolution",
                "temporary failure resolving",
                "eai_again",
                "dns server returned failure",
            ]
            .iter()
            .any(|marker| message.contains(marker))
        }
        _ => false,
    }
}

fn is_transient_mysql_error_code(number: u16) -> bool {
    // ER_CON_COUNT_ERROR, ER_HANDSHAKE_ERROR, ER_SERVER_SHUTDOWN,
    // ER_IPSOCK_ERROR, ER_TOO_MANY_USER_CONNECTIONS, CR_CONNECTION_ERROR,
    // CR_CONN_HOST_ERROR, CR_SERVER_GONE_ERROR, CR_SERVER_LOST and
    // CR_SERVER_LOST_EXTENDED. The latter client codes can be surfaced as
    // database errors during the handshake by MySQL-compatible servers.
    matches!(
        number,
        1040 | 1043 | 1053 | 1080 | 1203 | 2002 | 2003 | 2006 | 2013 | 2055
    )
}

/// 迁移失败时可安全写入日志的错误摘要；不回显连接串、口令或配置错误的原始文本。
pub fn safe_sqlx_error(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Configuration(_) => "连接配置无效（请检查 DATABASE_URL 格式）".to_owned(),
        sqlx::Error::Io(error) => format!("网络 I/O 错误（类别：{:?}）", error.kind()),
        sqlx::Error::Database(error) => {
            let code = error.code().map(|value| value.into_owned());
            let message = sanitize_log_text(error.message());
            match code {
                Some(code) => format!("MySQL 错误 code={code}：{message}"),
                None => format!("MySQL 错误：{message}"),
            }
        }
        sqlx::Error::Tls(_) => "TLS 握手失败（请检查证书与加密配置）".to_owned(),
        sqlx::Error::PoolTimedOut => "连接尝试超时".to_owned(),
        sqlx::Error::PoolClosed => "连接池已关闭".to_owned(),
        sqlx::Error::WorkerCrashed => "连接池后台 worker 已退出".to_owned(),
        sqlx::Error::Protocol(message) => format!("MySQL 协议错误：{}", sanitize_log_text(message)),
        other => sanitize_log_text(&other.to_string()),
    }
}

fn sanitize_log_text(value: &str) -> String {
    // SQLx 的 MySQL 连接配置错误可能包含完整 URI；对日志中的常见 URI 片段做保守脱敏，
    // 同时折叠换行，避免错误文本伪造额外日志行。配置错误本身在上面已直接折叠。
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    for scheme in ["mysql://", "mysql+mysql://", "mariadb://"] {
        let mut search_from = 0;
        while let Some(relative_start) = sanitized[search_from..].find(scheme) {
            let start = search_from + relative_start;
            let authority_start = start + scheme.len();
            let end = sanitized[authority_start..]
                .find(|character: char| {
                    character.is_whitespace()
                        || matches!(character, ')' | ']' | '}' | ',' | ';' | '"' | '\'')
                })
                .map(|offset| authority_start + offset)
                .unwrap_or(sanitized.len());
            let authority = &sanitized[authority_start..end];
            let Some(at_offset) = authority.rfind('@') else {
                search_from = end;
                continue;
            };
            let user_info = &authority[..at_offset];
            let Some(colon_offset) = user_info.find(':') else {
                search_from = end;
                continue;
            };
            let password_start = authority_start + colon_offset + 1;
            let password_end = authority_start + at_offset;
            sanitized.replace_range(password_start..password_end, "[REDACTED]");
            search_from = password_start + "[REDACTED]".len();
        }
    }
    sanitized
}

/// SQLx migration 表的非敏感状态摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDiagnostics {
    pub server_version: Option<String>,
    pub migrations_table_exists: Option<bool>,
    pub migration_count: Option<u64>,
    pub latest_version: Option<i64>,
    pub dirty_versions: Vec<i64>,
    pub query_errors: Vec<String>,
}

impl MigrationDiagnostics {
    /// 判断诊断快照是否发现 `success=FALSE` 的 migration；只读结果供日志和运维排障使用，
    /// 不触发任何自动修复或状态写入。
    pub fn has_dirty_migrations(&self) -> bool {
        !self.dirty_versions.is_empty()
    }
}

/// 尽力收集服务器版本和 SQLx migration 状态。每个查询都有短超时，诊断失败只记录摘要，
/// 绝不替换调用方持有的原始 migration 错误。
pub async fn collect_migration_diagnostics(pool: &MySqlPool) -> MigrationDiagnostics {
    let mut diagnostics = MigrationDiagnostics {
        server_version: None,
        migrations_table_exists: None,
        migration_count: None,
        latest_version: None,
        dirty_versions: Vec::new(),
        query_errors: Vec::new(),
    };

    diagnostics.server_version = match timeout(
        DIAGNOSTIC_QUERY_TIMEOUT,
        sqlx::query_scalar::<_, String>("SELECT VERSION()").fetch_one(pool),
    )
    .await
    {
        Ok(Ok(version)) => Some(sanitize_log_text(&version)),
        Ok(Err(error)) => {
            diagnostics
                .query_errors
                .push(format!("查询 MySQL 版本失败：{}", safe_sqlx_error(&error)));
            None
        }
        Err(_) => {
            diagnostics
                .query_errors
                .push("查询 MySQL 版本超时".to_owned());
            None
        }
    };

    let table_exists = match timeout(
        DIAGNOSTIC_QUERY_TIMEOUT,
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = '_sqlx_migrations')",
        )
        .fetch_one(pool),
    )
    .await
    {
        Ok(Ok(value)) => Some(value != 0),
        Ok(Err(error)) => {
            diagnostics.query_errors.push(format!(
                "检查 _sqlx_migrations 是否存在失败：{}",
                safe_sqlx_error(&error)
            ));
            None
        }
        Err(_) => {
            diagnostics
                .query_errors
                .push("检查 _sqlx_migrations 是否存在超时".to_owned());
            None
        }
    };
    diagnostics.migrations_table_exists = table_exists;

    if table_exists == Some(true) {
        diagnostics.migration_count = match timeout(
            DIAGNOSTIC_QUERY_TIMEOUT,
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations").fetch_one(pool),
        )
        .await
        {
            Ok(Ok(count)) => Some(count.max(0) as u64),
            Ok(Err(error)) => {
                diagnostics.query_errors.push(format!(
                    "查询 migration 数量失败：{}",
                    safe_sqlx_error(&error)
                ));
                None
            }
            Err(_) => {
                diagnostics
                    .query_errors
                    .push("查询 migration 数量超时".to_owned());
                None
            }
        };

        diagnostics.latest_version = match timeout(
            DIAGNOSTIC_QUERY_TIMEOUT,
            sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(version) FROM _sqlx_migrations")
                .fetch_one(pool),
        )
        .await
        {
            Ok(Ok(version)) => version,
            Ok(Err(error)) => {
                diagnostics.query_errors.push(format!(
                    "查询最新 migration 版本失败：{}",
                    safe_sqlx_error(&error)
                ));
                None
            }
            Err(_) => {
                diagnostics
                    .query_errors
                    .push("查询最新 migration 版本超时".to_owned());
                None
            }
        };

        diagnostics.dirty_versions = match timeout(
            DIAGNOSTIC_QUERY_TIMEOUT,
            sqlx::query_scalar::<_, i64>(
                "SELECT version FROM _sqlx_migrations \
                 WHERE success = FALSE ORDER BY version DESC LIMIT 8",
            )
            .fetch_all(pool),
        )
        .await
        {
            Ok(Ok(versions)) => versions
                .into_iter()
                .take(MAX_DIRTY_VERSIONS_IN_LOG)
                .collect(),
            Ok(Err(error)) => {
                diagnostics.query_errors.push(format!(
                    "查询 dirty migration 失败：{}",
                    safe_sqlx_error(&error)
                ));
                Vec::new()
            }
            Err(_) => {
                diagnostics
                    .query_errors
                    .push("查询 dirty migration 超时".to_owned());
                Vec::new()
            }
        };
    }

    diagnostics
}

/// 把错误及其 `source()` 链拼成单行，供迁移器在保留原始失败原因的同时统一日志格式。
pub fn format_error_chain(error: &(dyn StdError + 'static)) -> String {
    let mut messages = Vec::new();
    let mut current = Some(error);
    while let Some(error) = current {
        let message = error
            .downcast_ref::<sqlx::Error>()
            .map(safe_sqlx_error)
            .unwrap_or_else(|| sanitize_log_text(&error.to_string()));
        messages.push(message);
        current = error.source();
    }
    messages.join(" -> ")
}

#[cfg(test)]
#[path = "../tests/unit_src/src_migration_tests.rs"]
mod tests;
