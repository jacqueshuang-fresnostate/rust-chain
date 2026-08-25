use anyhow::{Context, Result, ensure};
use exchange_api::{
    bootstrap::{BootstrapAdminConfig, BootstrapAdminOutcome, bootstrap_default_admin},
    modules::auth::verify_password,
};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{
    fs,
    process::{Command, Output},
};
use url::Url;
use uuid::Uuid;

const BOOTSTRAP_ADMIN_LOCK_NAME: &str = "exchange.bootstrap.default_admin";

#[test]
fn bootstrap_config_validates_normalizes_and_redacts_values() {
    let config = BootstrapAdminConfig::from_values(
        "  Bootstrap_Admin  ".to_owned(),
        "safe-bootstrap-password".to_owned(),
        Some("  Super-Admin  ".to_owned()),
    )
    .unwrap();

    assert_eq!(config.username(), "bootstrap_admin");
    assert_eq!(config.role_name(), "super-admin");
    let debug = format!("{config:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("safe-bootstrap-password"));

    let default_role = BootstrapAdminConfig::from_values(
        "default_role_admin".to_owned(),
        "valid-password".to_owned(),
        None,
    )
    .unwrap();
    assert_eq!(default_role.role_name(), "super_admin");

    for invalid in [
        BootstrapAdminConfig::from_values(
            "bad username".to_owned(),
            "valid-password".to_owned(),
            None,
        ),
        BootstrapAdminConfig::from_values("valid_admin".to_owned(), "short".to_owned(), None),
        BootstrapAdminConfig::from_values("valid_admin".to_owned(), "Qaz123456@".to_owned(), None),
        BootstrapAdminConfig::from_values(
            "valid_admin".to_owned(),
            "valid-password".to_owned(),
            Some("bad role".to_owned()),
        ),
    ] {
        assert!(invalid.is_err());
    }
}

#[test]
fn deployment_examples_have_no_fixed_bootstrap_password_fallback() {
    for path in [
        "docker-compose.example.yml",
        "docker-compose.1panel.example.yml",
        "docker-compose.env.example",
        "docker-compose.1panel.env.example",
    ] {
        let source = fs::read_to_string(path).expect("read deployment example");
        let bootstrap_source = source
            .lines()
            .filter(|line| line.contains("BOOTSTRAP_"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !bootstrap_source.contains("Qaz123456@"),
            "fixed bootstrap password leaked in {path}"
        );
        assert!(
            source.contains("BOOTSTRAP_MODE"),
            "explicit bootstrap mode missing in {path}"
        );
    }
}

#[tokio::test]
async fn bootstrap_creates_once_skips_existing_admins_and_reuses_roles() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping bootstrap MySQL integration test because DATABASE_URL is not set");
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated bootstrap test");
    let database_name = format!("exchange_bootstrap_test_{}", Uuid::now_v7().simple());
    let create_database = format!(
        "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    );
    if let Err(error) = sqlx::query(&create_database).execute(&server_pool).await {
        eprintln!(
            "skipping bootstrap MySQL integration test because an isolated database cannot be created: {error}"
        );
        server_pool.close().await;
        return;
    }

    let mut test_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    test_url.set_path(&format!("/{database_name}"));
    let default_migrate_output = run_migrate(test_url.as_str(), &[])
        .expect("run the real migration binary without bootstrap environment variables");
    let default_migrate_logs = combined_output(&default_migrate_output);
    assert!(
        default_migrate_output.status.success(),
        "default migration failed:\n{default_migrate_logs}"
    );
    assert!(!default_migrate_logs.contains("Qaz123456@"));

    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(test_url.as_str())
        .await
        .expect("connect to isolated bootstrap test database");

    let exercise_result = exercise_bootstrap_contract(&pool, test_url.as_str()).await;
    pool.close().await;
    let drop_database = format!("DROP DATABASE `{database_name}`");
    let cleanup_result = sqlx::query(&drop_database)
        .execute(&server_pool)
        .await
        .context("drop isolated bootstrap test database");
    server_pool.close().await;

    exercise_result.expect("bootstrap contract");
    cleanup_result.expect("bootstrap test database cleanup");
}

async fn exercise_bootstrap_contract(pool: &MySqlPool, database_url: &str) -> Result<()> {
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_users")
            .fetch_one(pool)
            .await?
            == 0,
        "migrator must not create an administrator unless bootstrap mode is explicit"
    );

    for environment in [
        vec![("BOOTSTRAP_MODE", "create_admin")],
        vec![
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_PASSWORD", "   "),
        ],
        vec![
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_PASSWORD", "Qaz123456@"),
        ],
    ] {
        let output = run_migrate(database_url, &environment)?;
        ensure!(
            !output.status.success(),
            "missing, blank, and known default bootstrap passwords must fail"
        );
        ensure!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_users")
                .fetch_one(pool)
                .await?
                == 0
        );
    }

    let initial_password = "one-time-bootstrap-7!safe";
    let created = run_migrate(
        database_url,
        &[
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_PASSWORD", initial_password),
            ("BOOTSTRAP_ADMIN_PASSWORD_FILE", ""),
        ],
    )?;
    ensure!(
        created.status.success(),
        "valid explicit bootstrap must succeed"
    );
    let (username, password_hash, status, role_name, must_change_password): (
        String,
        String,
        String,
        String,
        bool,
    ) = sqlx::query_as(
        r#"SELECT admin_users.username, admin_users.password_hash, admin_users.status,
                      admin_roles.name, admin_users.must_change_password
               FROM admin_users
               INNER JOIN admin_roles ON admin_roles.id = admin_users.role_id
               LIMIT 1"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(username == "admin");
    ensure!(status == "active");
    ensure!(role_name == "super_admin");
    ensure!(must_change_password);
    ensure!(password_hash != initial_password);
    ensure!(password_hash.starts_with("$argon2"));
    ensure!(verify_password(&password_hash, initial_password)?);
    ensure_bootstrap_lock_is_free(pool).await?;

    let ignored_password = "different-bootstrap-password";
    let skipped_migrate_output = run_migrate(
        database_url,
        &[
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_USERNAME", "other_admin"),
            ("BOOTSTRAP_ADMIN_PASSWORD", ignored_password),
            ("BOOTSTRAP_ADMIN_ROLE_NAME", "unused_role"),
        ],
    )?;
    let skipped_migrate_logs = combined_output(&skipped_migrate_output);
    ensure!(
        skipped_migrate_output.status.success(),
        "idempotent migration failed:\n{skipped_migrate_logs}"
    );
    ensure!(!skipped_migrate_logs.contains(ignored_password));
    let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_users")
        .fetch_one(pool)
        .await?;
    let unused_role_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM admin_roles WHERE name = 'unused_role'")
            .fetch_one(pool)
            .await?;
    let unchanged_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM admin_users WHERE username = ?")
            .bind(&username)
            .fetch_one(pool)
            .await?;
    ensure!(admin_count == 1);
    ensure!(unused_role_count == 0);
    ensure!(unchanged_hash == password_hash);
    ensure_bootstrap_lock_is_free(pool).await?;

    sqlx::query("DELETE FROM admin_users").execute(pool).await?;
    sqlx::query("DELETE FROM admin_roles").execute(pool).await?;

    let invalid_password = "x7!Z2";
    let invalid_migrate_output = run_migrate(
        database_url,
        &[
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_USERNAME", "invalid_admin"),
            ("BOOTSTRAP_ADMIN_PASSWORD", invalid_password),
            ("BOOTSTRAP_ADMIN_ROLE_NAME", "invalid_role"),
        ],
    )?;
    let invalid_migrate_logs = combined_output(&invalid_migrate_output);
    ensure!(
        !invalid_migrate_output.status.success(),
        "invalid bootstrap password must fail the migration process"
    );
    ensure!(!invalid_migrate_logs.contains(invalid_password));
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_users")
            .fetch_one(pool)
            .await?
            == 0
    );
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_roles")
            .fetch_one(pool)
            .await?
            == 0
    );
    ensure_bootstrap_lock_is_free(pool).await?;

    let reused_role_id =
        sqlx::query("INSERT INTO admin_roles (name, permissions) VALUES (?, JSON_ARRAY('*'))")
            .bind("reused_role")
            .execute(pool)
            .await?
            .last_insert_id();
    let reused_password = "reuse-bootstrap-password";
    let override_migrate_output = run_migrate(
        database_url,
        &[
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_USERNAME", "  Reuse_Admin  "),
            ("BOOTSTRAP_ADMIN_PASSWORD", reused_password),
            ("BOOTSTRAP_ADMIN_ROLE_NAME", "  Reused_Role  "),
        ],
    )?;
    let override_migrate_logs = combined_output(&override_migrate_output);
    ensure!(
        override_migrate_output.status.success(),
        "override migration failed:\n{override_migrate_logs}"
    );
    ensure!(!override_migrate_logs.contains(reused_password));
    let (actual_role_id, role_count, reused_password_hash): (u64, i64, String) = sqlx::query_as(
        r#"SELECT admin_users.role_id,
                  (SELECT COUNT(*) FROM admin_roles WHERE name = 'reused_role'),
                  admin_users.password_hash
           FROM admin_users
           WHERE admin_users.username = 'reuse_admin'"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(actual_role_id == reused_role_id);
    ensure!(role_count == 1);
    ensure!(verify_password(&reused_password_hash, reused_password)?);
    ensure_bootstrap_lock_is_free(pool).await?;

    sqlx::query("DELETE FROM admin_users").execute(pool).await?;
    sqlx::query("DELETE FROM admin_roles").execute(pool).await?;
    let first = BootstrapAdminConfig::from_values(
        "concurrent_admin_a".to_owned(),
        "concurrent-bootstrap-password-a".to_owned(),
        Some("concurrent_role_a".to_owned()),
    )?;
    let second = BootstrapAdminConfig::from_values(
        "concurrent_admin_b".to_owned(),
        "concurrent-bootstrap-password-b".to_owned(),
        Some("concurrent_role_b".to_owned()),
    )?;
    let (first_outcome, second_outcome) = tokio::join!(
        bootstrap_default_admin(pool, &first),
        bootstrap_default_admin(pool, &second)
    );
    let outcomes = [first_outcome?, second_outcome?];
    ensure!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == BootstrapAdminOutcome::Created)
            .count()
            == 1
    );
    ensure!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == BootstrapAdminOutcome::SkippedExistingAdmin)
            .count()
            == 1
    );
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_users")
            .fetch_one(pool)
            .await?
            == 1
    );
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_roles")
            .fetch_one(pool)
            .await?
            == 1
    );
    ensure_bootstrap_lock_is_free(pool).await?;

    sqlx::query("DELETE FROM admin_users").execute(pool).await?;
    sqlx::query("DELETE FROM admin_roles").execute(pool).await?;
    sqlx::raw_sql(
        r#"CREATE TRIGGER bootstrap_admin_force_failure
           BEFORE INSERT ON admin_users
           FOR EACH ROW
           SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'forced bootstrap administrator failure'"#,
    )
    .execute(pool)
    .await?;
    let rollback_password = "rollback-bootstrap-password";
    let rollback = BootstrapAdminConfig::from_values(
        "rollback_admin".to_owned(),
        rollback_password.to_owned(),
        Some("rollback_role".to_owned()),
    )?;
    let rollback_error = bootstrap_default_admin(pool, &rollback)
        .await
        .expect_err("forced admin insert failure must fail bootstrap");
    ensure!(!rollback_error.to_string().contains(rollback_password));
    ensure!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_users")
            .fetch_one(pool)
            .await?
            == 0
    );
    ensure!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM admin_roles WHERE name = 'rollback_role'"
        )
        .fetch_one(pool)
        .await?
            == 0,
        "role creation must roll back with the failed administrator insert"
    );
    ensure_bootstrap_lock_is_free(pool).await?;
    sqlx::raw_sql("DROP TRIGGER bootstrap_admin_force_failure")
        .execute(pool)
        .await?;

    let recovered_password = "recovered-one-time-password!7";
    let recovered_secret_path = std::env::temp_dir().join(format!(
        "exchange-bootstrap-secret-{}",
        Uuid::now_v7().simple()
    ));
    fs::write(&recovered_secret_path, format!("{recovered_password}\n"))?;
    let recovered_secret_path = recovered_secret_path.to_string_lossy().into_owned();
    let recovered = run_migrate(
        database_url,
        &[
            ("BOOTSTRAP_MODE", "create_admin"),
            ("BOOTSTRAP_ADMIN_PASSWORD", ""),
            ("BOOTSTRAP_ADMIN_PASSWORD_FILE", &recovered_secret_path),
        ],
    );
    fs::remove_file(&recovered_secret_path)?;
    let recovered = recovered?;
    ensure!(
        recovered.status.success(),
        "bootstrap must recover from a Docker Secret file after the forced transaction failure: {}",
        combined_output(&recovered)
    );
    let recovered_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM admin_users WHERE username = 'admin'")
            .fetch_one(pool)
            .await?;
    ensure!(verify_password(&recovered_hash, recovered_password)?);
    ensure_bootstrap_lock_is_free(pool).await?;

    Ok(())
}

async fn ensure_bootstrap_lock_is_free(pool: &MySqlPool) -> Result<()> {
    let lock_owner = sqlx::query_scalar::<_, Option<u64>>("SELECT IS_USED_LOCK(?)")
        .bind(BOOTSTRAP_ADMIN_LOCK_NAME)
        .fetch_one(pool)
        .await?;
    ensure!(
        lock_owner.is_none(),
        "bootstrap named lock is still held by MySQL connection {lock_owner:?}"
    );
    Ok(())
}

fn run_migrate(database_url: &str, bootstrap_environment: &[(&str, &str)]) -> Result<Output> {
    let working_directory = std::env::temp_dir().join(format!(
        "exchange-bootstrap-migrate-{}",
        Uuid::now_v7().simple()
    ));
    fs::create_dir(&working_directory).context("create isolated migration working directory")?;

    let mut command = Command::new(env!("CARGO_BIN_EXE_exchange-migrate"));
    command
        .current_dir(&working_directory)
        .env("DATABASE_URL", database_url)
        .env("RUST_LOG", "info")
        .env_remove("BOOTSTRAP_MODE")
        .env_remove("BOOTSTRAP_ADMIN_USERNAME")
        .env_remove("BOOTSTRAP_ADMIN_PASSWORD")
        .env_remove("BOOTSTRAP_ADMIN_PASSWORD_FILE")
        .env_remove("BOOTSTRAP_ADMIN_ROLE_NAME");
    for (name, value) in bootstrap_environment {
        command.env(name, value);
    }

    let output = command.output().context("run exchange-migrate");
    let cleanup_result =
        fs::remove_dir(&working_directory).context("remove isolated migration working directory");
    let output = output?;
    cleanup_result?;
    Ok(output)
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
