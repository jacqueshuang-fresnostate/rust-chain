use super::*;
use std::{error::Error, fmt, io};

#[test]
fn migration_connect_config_uses_bounded_defaults() {
    let config = MigrationConnectConfig::from_optional_values(None, None).unwrap();
    assert_eq!(config, MigrationConnectConfig::default());
    assert_eq!(config.max_attempts, 30);
    assert_eq!(config.retry_delay_seconds, 2);
    assert_eq!(config.retry_delay(), Duration::from_secs(2));
}

#[test]
fn migration_connect_config_accepts_edges_and_rejects_invalid_values() {
    let config =
        MigrationConnectConfig::from_optional_values(Some(" 1 ".to_owned()), Some("30".to_owned()))
            .unwrap();
    assert_eq!(config.max_attempts, 1);
    assert_eq!(config.retry_delay_seconds, 30);

    for (attempts, delay) in [
        (Some(""), Some("0")),
        (Some("0"), Some("0")),
        (Some("31"), Some("0")),
        (Some("abc"), Some("0")),
        (Some("1"), Some("-1")),
        (Some("1"), Some("31")),
        (Some("1"), Some("abc")),
    ] {
        let result = MigrationConnectConfig::from_optional_values(
            attempts.map(str::to_owned),
            delay.map(str::to_owned),
        );
        assert!(
            result.is_err(),
            "unexpectedly accepted {attempts:?}/{delay:?}"
        );
    }
}

#[test]
fn manually_constructed_migration_connect_config_is_validated_before_use() {
    assert!(
        (MigrationConnectConfig {
            max_attempts: 0,
            retry_delay_seconds: 0,
        })
        .validate()
        .is_err()
    );
    assert!(
        (MigrationConnectConfig {
            max_attempts: MAX_MIGRATION_CONNECT_ATTEMPTS + 1,
            retry_delay_seconds: 0,
        })
        .validate()
        .is_err()
    );
    assert!(
        (MigrationConnectConfig {
            max_attempts: 1,
            retry_delay_seconds: MAX_MIGRATION_CONNECT_RETRY_DELAY_SECONDS + 1,
        })
        .validate()
        .is_err()
    );
    assert!(
        (MigrationConnectConfig {
            max_attempts: MAX_MIGRATION_CONNECT_ATTEMPTS,
            retry_delay_seconds: MAX_MIGRATION_CONNECT_RETRY_DELAY_SECONDS,
        })
        .validate()
        .is_ok()
    );
}

#[test]
fn transient_connection_classification_only_covers_startup_network_errors() {
    for kind in [
        io::ErrorKind::ConnectionRefused,
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::ConnectionAborted,
        io::ErrorKind::NotConnected,
        io::ErrorKind::TimedOut,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::AddrNotAvailable,
        io::ErrorKind::BrokenPipe,
    ] {
        let error = sqlx::Error::Io(io::Error::new(kind, "transient"));
        assert!(is_transient_connect_error(&error), "{kind:?} should retry");
    }

    assert!(is_transient_connect_error(&sqlx::Error::Io(
        io::Error::other(
            "failed to lookup address information: temporary failure in name resolution"
        )
    )));
    assert!(!is_transient_connect_error(&sqlx::Error::Io(
        io::Error::other(
            "failed to lookup address information: nodename nor servname provided, or not known",
        )
    )));

    for kind in [
        io::ErrorKind::InvalidInput,
        io::ErrorKind::InvalidData,
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::NotFound,
    ] {
        let error = sqlx::Error::Io(io::Error::new(kind, "configuration"));
        assert!(
            !is_transient_connect_error(&error),
            "{kind:?} must fail fast"
        );
    }

    assert!(!is_transient_connect_error(&sqlx::Error::Configuration(
        Box::new(TestError("invalid URL")),
    )));
    assert!(!is_transient_connect_error(&sqlx::Error::Tls(Box::new(
        TestError("invalid certificate")
    ),)));
}

#[test]
fn mysql_startup_error_code_allowlist_includes_lost_connections_only() {
    for code in [1040, 1043, 1053, 1080, 1203, 2002, 2003, 2006, 2013, 2055] {
        assert!(
            is_transient_mysql_error_code(code),
            "MySQL startup code {code} should retry"
        );
    }
    for code in [1045, 1049, 1142, 1130] {
        assert!(
            !is_transient_mysql_error_code(code),
            "MySQL configuration/permission code {code} must fail fast"
        );
    }
}

#[test]
fn connection_error_summary_redacts_configuration_details() {
    let error = sqlx::Error::Configuration(Box::new(TestError(
        "mysql://user:super-secret@db.example/exchange",
    )));
    assert_eq!(
        safe_sqlx_error(&error),
        "连接配置无效（请检查 DATABASE_URL 格式）"
    );
}

#[test]
fn error_chain_is_single_line_and_preserves_sources_without_secret_text() {
    let error = OuterError {
        source: TestError("inner\nline"),
    };
    let chain = format_error_chain(&error);
    assert_eq!(chain, "outer -> inner line");
    assert!(!chain.contains('\n'));
}

#[test]
fn error_chain_uses_the_safe_sqlx_summary_for_nested_database_errors() {
    let error = OuterSqlxError {
        source: sqlx::Error::Configuration(Box::new(TestError(
            "mysql://user:super-secret@db.example/exchange",
        ))),
    };
    let chain = format_error_chain(&error);
    assert!(chain.contains("连接配置无效"));
    assert!(!chain.contains("super-secret"));
}

#[derive(Debug)]
struct TestError(&'static str);

impl fmt::Display for TestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for TestError {}

#[derive(Debug)]
struct OuterError {
    source: TestError,
}

impl fmt::Display for OuterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("outer")
    }
}

impl Error for OuterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug)]
struct OuterSqlxError {
    source: sqlx::Error,
}

impl fmt::Display for OuterSqlxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("outer")
    }
}

impl Error for OuterSqlxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}
