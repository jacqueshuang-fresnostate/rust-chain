use anyhow::{Context, Result, bail, ensure};
use exchange_api::{
    error::AppError,
    modules::auth::{
        ActorType, AuthRepository, MySqlAuthRepository, StoredActorCredential, hash_password,
        verify_password,
    },
};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use std::{collections::BTreeMap, fmt::Debug};
use url::Url;
use uuid::Uuid;

const AUTH_CREDENTIAL_MIGRATION: &str =
    include_str!("../migrations/0098_auth_credential_text_metadata.sql");
const USER_PASSWORD: &str = "User-Credential-0098!";
const ADMIN_PASSWORD: &str = "Admin-Credential-0098!";
const AGENT_PASSWORD: &str = "Agent-Credential-0098!";

#[derive(Debug, sqlx::FromRow)]
struct ColumnMetadata {
    table_name: String,
    column_name: String,
    data_type: String,
    character_set_name: Option<String>,
    collation_name: Option<String>,
    is_nullable: String,
    column_default: Option<String>,
    character_maximum_length: Option<i64>,
}

struct CredentialFixture {
    user_id: u64,
    user_email: String,
    user_phone: String,
    user_username: String,
    user_password_hash: String,
    user_status: String,
    admin_id: u64,
    admin_username: String,
    admin_password_hash: String,
    admin_status: String,
    agent_admin_id: u64,
    agent_username: String,
    agent_password_hash: String,
    agent_status: String,
}

#[tokio::test]
async fn auth_credential_binary_metadata_drift_is_repaired_for_repository_queries() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("skipping auth credential migration test because DATABASE_URL is not set");
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated auth credential migration test");
    let database_name = format!("auth_credential_meta_{}", Uuid::now_v7().simple());
    let create_database = format!(
        "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    );
    if let Err(error) = sqlx::query(&create_database).execute(&server_pool).await {
        eprintln!(
            "skipping auth credential migration test because an isolated database cannot be created: {error}"
        );
        server_pool.close().await;
        return;
    }

    let mut test_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    test_url.set_path(&format!("/{database_name}"));
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(test_url.as_str())
        .await
        .expect("connect to isolated auth credential migration database");

    let exercise_result = exercise_migration_contract(&pool).await;
    pool.close().await;
    let drop_database = format!("DROP DATABASE `{database_name}`");
    let cleanup_result = sqlx::query(&drop_database)
        .execute(&server_pool)
        .await
        .context("drop isolated auth credential migration database");
    server_pool.close().await;

    exercise_result.expect("auth credential text metadata migration contract");
    cleanup_result.expect("auth credential migration test database cleanup");
}

async fn exercise_migration_contract(pool: &MySqlPool) -> Result<()> {
    create_auth_schema(pool).await?;
    let fixture = seed_credentials(pool, "drifted", false).await?;
    let repository = MySqlAuthRepository::new(pool.clone());

    reproduce_varbinary_drift(pool).await?;
    assert_credential_metadata(pool, "varbinary", None, None).await?;
    assert_binary_values(pool, &fixture).await?;
    assert_repository_decode_failures(&repository, &fixture, "VARBINARY").await?;

    execute_exact_migration(pool, "repair VARBINARY metadata").await?;
    assert_credential_metadata(pool, "varchar", Some("utf8mb4"), Some("utf8mb4_unicode_ci"))
        .await?;
    assert_repository_credentials(&repository, &fixture).await?;

    let default_fixture = seed_credentials(pool, "defaults", true).await?;
    assert_repository_credentials(&repository, &default_fixture).await?;

    reproduce_binary_collated_varchar_drift(pool).await?;
    assert_credential_metadata(pool, "varchar", Some("utf8mb4"), Some("utf8mb4_bin")).await?;
    assert_binary_values(pool, &fixture).await?;
    assert_binary_values(pool, &default_fixture).await?;
    assert_repository_decode_failures(&repository, &fixture, "binary-collated VARCHAR").await?;

    execute_exact_migration(pool, "repair binary-collated VARCHAR metadata").await?;
    assert_credential_metadata(pool, "varchar", Some("utf8mb4"), Some("utf8mb4_unicode_ci"))
        .await?;
    assert_repository_credentials(&repository, &fixture).await?;
    assert_repository_credentials(&repository, &default_fixture).await?;

    execute_exact_migration(pool, "run migration against already-correct metadata").await?;
    assert_credential_metadata(pool, "varchar", Some("utf8mb4"), Some("utf8mb4_unicode_ci"))
        .await?;
    assert_repository_credentials(&repository, &fixture).await?;
    assert_repository_credentials(&repository, &default_fixture).await?;

    Ok(())
}

async fn create_auth_schema(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"CREATE TABLE users (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               email VARCHAR(255) NULL UNIQUE,
               phone VARCHAR(32) NULL UNIQUE,
               username VARCHAR(64) NULL UNIQUE,
               password_hash VARCHAR(255)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
               status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
                   NOT NULL DEFAULT 'active'
           ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

           CREATE TABLE admin_users (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               username VARCHAR(64) NOT NULL UNIQUE,
               password_hash VARCHAR(255)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
               status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
                   NOT NULL DEFAULT 'active'
           ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

           CREATE TABLE agents (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               status VARCHAR(32) NOT NULL DEFAULT 'active',
               path VARCHAR(2048) NOT NULL
           ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

           CREATE TABLE agent_admin_users (
               id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
               agent_id BIGINT UNSIGNED NOT NULL,
               username VARCHAR(64) NOT NULL UNIQUE,
               password_hash VARCHAR(255)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
               status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
                   NOT NULL DEFAULT 'active'
           ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"#,
    )
    .execute(pool)
    .await
    .context("create focused auth repository schema")?;
    Ok(())
}

async fn seed_credentials(
    pool: &MySqlPool,
    label: &str,
    use_status_defaults: bool,
) -> Result<CredentialFixture> {
    let user_email = format!("{label}-user@example.test");
    let user_phone = format!("+1555{label:0>8}");
    let user_username = format!("{label}_user");
    let admin_username = format!("{label}_admin");
    let agent_username = format!("{label}_agent");
    let user_password_hash = hash_password(USER_PASSWORD)?;
    let admin_password_hash = hash_password(ADMIN_PASSWORD)?;
    let agent_password_hash = hash_password(AGENT_PASSWORD)?;
    let user_status = if use_status_defaults {
        "active"
    } else {
        "suspended"
    };
    let admin_status = if use_status_defaults {
        "active"
    } else {
        "disabled"
    };
    let agent_status = if use_status_defaults {
        "active"
    } else {
        "locked"
    };

    let user_id = if use_status_defaults {
        sqlx::query("INSERT INTO users (email, phone, username, password_hash) VALUES (?, ?, ?, ?)")
            .bind(&user_email)
            .bind(&user_phone)
            .bind(&user_username)
            .bind(&user_password_hash)
            .execute(pool)
            .await
            .context("insert user credential using status default")?
            .last_insert_id()
    } else {
        sqlx::query(
            r#"INSERT INTO users (email, phone, username, password_hash, status)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(&user_email)
        .bind(&user_phone)
        .bind(&user_username)
        .bind(&user_password_hash)
        .bind(user_status)
        .execute(pool)
        .await
        .context("insert user credential before metadata drift")?
        .last_insert_id()
    };

    let admin_id = if use_status_defaults {
        sqlx::query("INSERT INTO admin_users (username, password_hash) VALUES (?, ?)")
            .bind(&admin_username)
            .bind(&admin_password_hash)
            .execute(pool)
            .await
            .context("insert admin credential using status default")?
            .last_insert_id()
    } else {
        sqlx::query("INSERT INTO admin_users (username, password_hash, status) VALUES (?, ?, ?)")
            .bind(&admin_username)
            .bind(&admin_password_hash)
            .bind(admin_status)
            .execute(pool)
            .await
            .context("insert admin credential before metadata drift")?
            .last_insert_id()
    };

    let agent_id = sqlx::query("INSERT INTO agents (status, path) VALUES ('active', ?)")
        .bind(format!("/{label}"))
        .execute(pool)
        .await
        .context("insert active agent for credential lookup")?
        .last_insert_id();

    let agent_admin_id = if use_status_defaults {
        sqlx::query(
            r#"INSERT INTO agent_admin_users (agent_id, username, password_hash)
               VALUES (?, ?, ?)"#,
        )
        .bind(agent_id)
        .bind(&agent_username)
        .bind(&agent_password_hash)
        .execute(pool)
        .await
        .context("insert agent credential using status default")?
        .last_insert_id()
    } else {
        sqlx::query(
            r#"INSERT INTO agent_admin_users (agent_id, username, password_hash, status)
               VALUES (?, ?, ?, ?)"#,
        )
        .bind(agent_id)
        .bind(&agent_username)
        .bind(&agent_password_hash)
        .bind(agent_status)
        .execute(pool)
        .await
        .context("insert agent credential before metadata drift")?
        .last_insert_id()
    };

    Ok(CredentialFixture {
        user_id,
        user_email,
        user_phone,
        user_username,
        user_password_hash,
        user_status: user_status.to_owned(),
        admin_id,
        admin_username,
        admin_password_hash,
        admin_status: admin_status.to_owned(),
        agent_admin_id,
        agent_username,
        agent_password_hash,
        agent_status: agent_status.to_owned(),
    })
}

async fn reproduce_varbinary_drift(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"ALTER TABLE users
               MODIFY COLUMN password_hash VARBINARY(255) NOT NULL,
               MODIFY COLUMN status VARBINARY(32) NOT NULL DEFAULT 'active';

           ALTER TABLE admin_users
               MODIFY COLUMN password_hash VARBINARY(255) NOT NULL,
               MODIFY COLUMN status VARBINARY(32) NOT NULL DEFAULT 'active';

           ALTER TABLE agent_admin_users
               MODIFY COLUMN password_hash VARBINARY(255) NOT NULL,
               MODIFY COLUMN status VARBINARY(32) NOT NULL DEFAULT 'active'"#,
    )
    .execute(pool)
    .await
    .context("reproduce auth credential VARBINARY drift")?;
    Ok(())
}

async fn reproduce_binary_collated_varchar_drift(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"ALTER TABLE users
               MODIFY COLUMN password_hash VARCHAR(255)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
               MODIFY COLUMN status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NOT NULL DEFAULT 'active';

           ALTER TABLE admin_users
               MODIFY COLUMN password_hash VARCHAR(255)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
               MODIFY COLUMN status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NOT NULL DEFAULT 'active';

           ALTER TABLE agent_admin_users
               MODIFY COLUMN password_hash VARCHAR(255)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
               MODIFY COLUMN status VARCHAR(32)
                   CHARACTER SET utf8mb4 COLLATE utf8mb4_bin
                   NOT NULL DEFAULT 'active'"#,
    )
    .execute(pool)
    .await
    .context("reproduce auth credential binary-collated VARCHAR drift")?;
    Ok(())
}

async fn execute_exact_migration(pool: &MySqlPool, context: &str) -> Result<()> {
    sqlx::raw_sql(AUTH_CREDENTIAL_MIGRATION)
        .execute(pool)
        .await
        .with_context(|| context.to_owned())?;
    Ok(())
}

async fn assert_repository_decode_failures(
    repository: &MySqlAuthRepository,
    fixture: &CredentialFixture,
    drift: &str,
) -> Result<()> {
    assert_column_decode(
        repository.find_user_by_email(&fixture.user_email).await,
        "find_user_by_email",
        drift,
    )?;
    assert_column_decode(
        repository.find_user_by_phone(&fixture.user_phone).await,
        "find_user_by_phone",
        drift,
    )?;
    assert_column_decode(
        repository
            .find_user_by_username(&fixture.user_username)
            .await,
        "find_user_by_username",
        drift,
    )?;
    assert_column_decode(
        repository
            .find_admin_by_username(&fixture.admin_username)
            .await,
        "find_admin_by_username",
        drift,
    )?;
    assert_column_decode(
        repository
            .find_agent_by_username(&fixture.agent_username)
            .await,
        "find_agent_by_username",
        drift,
    )?;
    Ok(())
}

fn assert_column_decode<T: Debug>(
    result: std::result::Result<T, AppError>,
    lookup: &str,
    drift: &str,
) -> Result<()> {
    let error = result.expect_err("binary metadata must reproduce repository decode failure");
    let AppError::Database(sqlx::Error::ColumnDecode { index, .. }) = &error else {
        bail!("{lookup} must return SQLx ColumnDecode for {drift}, got {error:?}");
    };
    ensure!(
        index == "1",
        "{lookup} must fail at column 1 (password_hash) for {drift}, got column {index}"
    );
    Ok(())
}

async fn assert_repository_credentials(
    repository: &MySqlAuthRepository,
    fixture: &CredentialFixture,
) -> Result<()> {
    let user_by_email = repository
        .find_user_by_email(&fixture.user_email)
        .await
        .context("lookup migrated user credential by email")?
        .context("migrated user credential by email must exist")?;
    assert_credential(
        &user_by_email,
        ActorType::User,
        fixture.user_id,
        Some(fixture.user_id),
        &fixture.user_password_hash,
        &fixture.user_status,
        USER_PASSWORD,
    )?;

    let user_by_phone = repository
        .find_user_by_phone(&fixture.user_phone)
        .await
        .context("lookup migrated user credential by phone")?
        .context("migrated user credential by phone must exist")?;
    assert_credential(
        &user_by_phone,
        ActorType::User,
        fixture.user_id,
        Some(fixture.user_id),
        &fixture.user_password_hash,
        &fixture.user_status,
        USER_PASSWORD,
    )?;

    let user_by_username = repository
        .find_user_by_username(&fixture.user_username)
        .await
        .context("lookup migrated user credential by username")?
        .context("migrated user credential by username must exist")?;
    assert_credential(
        &user_by_username,
        ActorType::User,
        fixture.user_id,
        Some(fixture.user_id),
        &fixture.user_password_hash,
        &fixture.user_status,
        USER_PASSWORD,
    )?;

    let admin = repository
        .find_admin_by_username(&fixture.admin_username)
        .await
        .context("lookup migrated admin credential")?
        .context("migrated admin credential must exist")?;
    assert_credential(
        &admin,
        ActorType::Admin,
        fixture.admin_id,
        None,
        &fixture.admin_password_hash,
        &fixture.admin_status,
        ADMIN_PASSWORD,
    )?;

    let agent = repository
        .find_agent_by_username(&fixture.agent_username)
        .await
        .context("lookup migrated agent credential")?
        .context("migrated agent credential must exist")?;
    assert_credential(
        &agent,
        ActorType::Agent,
        fixture.agent_admin_id,
        None,
        &fixture.agent_password_hash,
        &fixture.agent_status,
        AGENT_PASSWORD,
    )?;

    Ok(())
}

fn assert_credential(
    credential: &StoredActorCredential,
    actor_type: ActorType,
    actor_id: u64,
    user_id: Option<u64>,
    password_hash: &str,
    status: &str,
    password: &str,
) -> Result<()> {
    ensure!(credential.actor.actor_type == actor_type);
    ensure!(credential.actor.actor_id == actor_id);
    ensure!(credential.actor.user_id == user_id);
    ensure!(credential.password_hash == password_hash);
    ensure!(credential.status == status);
    ensure!(verify_password(&credential.password_hash, password)?);
    Ok(())
}

async fn assert_binary_values(pool: &MySqlPool, fixture: &CredentialFixture) -> Result<()> {
    let user: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT password_hash, status FROM users WHERE id = ?")
            .bind(fixture.user_id)
            .fetch_one(pool)
            .await
            .context("read drifted user credential as binary values")?;
    ensure!(user.0 == fixture.user_password_hash.as_bytes());
    ensure!(user.1 == fixture.user_status.as_bytes());

    let admin: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT password_hash, status FROM admin_users WHERE id = ?")
            .bind(fixture.admin_id)
            .fetch_one(pool)
            .await
            .context("read drifted admin credential as binary values")?;
    ensure!(admin.0 == fixture.admin_password_hash.as_bytes());
    ensure!(admin.1 == fixture.admin_status.as_bytes());

    let agent: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT password_hash, status FROM agent_admin_users WHERE id = ?")
            .bind(fixture.agent_admin_id)
            .fetch_one(pool)
            .await
            .context("read drifted agent credential as binary values")?;
    ensure!(agent.0 == fixture.agent_password_hash.as_bytes());
    ensure!(agent.1 == fixture.agent_status.as_bytes());
    Ok(())
}

async fn assert_credential_metadata(
    pool: &MySqlPool,
    expected_data_type: &str,
    expected_character_set: Option<&str>,
    expected_collation: Option<&str>,
) -> Result<()> {
    let rows = sqlx::query_as::<_, ColumnMetadata>(
        r#"SELECT CAST(TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS table_name,
                  CAST(COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS column_name,
                  CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4) AS data_type,
                  CAST(CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS character_set_name,
                  CAST(COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS collation_name,
                  CAST(IS_NULLABLE AS CHAR(3) CHARACTER SET utf8mb4) AS is_nullable,
                  CAST(COLUMN_DEFAULT AS CHAR(255) CHARACTER SET utf8mb4) AS column_default,
                  CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME IN ('users', 'admin_users', 'agent_admin_users')
             AND COLUMN_NAME IN ('password_hash', 'status')"#,
    )
    .fetch_all(pool)
    .await
    .context("read auth credential column metadata")?
    .into_iter()
    .map(|row| ((row.table_name.clone(), row.column_name.clone()), row))
    .collect::<BTreeMap<_, _>>();

    let expected_status_default = if expected_data_type == "varbinary" {
        "0x616374697665"
    } else {
        "active"
    };
    let expected = [
        ("users", "password_hash", 255, None),
        ("users", "status", 32, Some(expected_status_default)),
        ("admin_users", "password_hash", 255, None),
        ("admin_users", "status", 32, Some(expected_status_default)),
        ("agent_admin_users", "password_hash", 255, None),
        (
            "agent_admin_users",
            "status",
            32,
            Some(expected_status_default),
        ),
    ];
    ensure!(rows.len() == expected.len());

    for (table_name, column_name, length, default) in expected {
        let row = rows
            .get(&(table_name.to_owned(), column_name.to_owned()))
            .with_context(|| format!("missing metadata for {table_name}.{column_name}"))?;
        ensure!(row.data_type == expected_data_type);
        ensure!(row.character_set_name.as_deref() == expected_character_set);
        ensure!(row.collation_name.as_deref() == expected_collation);
        ensure!(row.is_nullable == "NO");
        ensure!(row.column_default.as_deref() == default);
        ensure!(row.character_maximum_length == Some(length));
    }

    Ok(())
}
