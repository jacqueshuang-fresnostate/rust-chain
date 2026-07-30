use anyhow::{Context, Result, ensure};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use exchange_api::{
    build_router,
    config::Settings,
    error::AppError,
    modules::auth::{
        ActorType, AuthRepository, MySqlAuthRepository, StoredActorCredential, TokenScope,
        hash_password, issue_token, verify_password,
    },
    state::AppState,
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::{MySqlPool, migrate::Migrator, mysql::MySqlPoolOptions};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

const SCHEMA_TEXT_METADATA_MIGRATION: &str =
    include_str!("../migrations/0099_schema_wide_text_metadata.sql");
const SCHEMA_TEXT_METADATA_VERSION: i64 = 99;
const EXPECTED_BUSINESS_TABLES: usize = 96;
const EXPECTED_TEXT_COLUMNS: usize = 377;
const TEXT_DATA_TYPES: &str = "'char','varchar','tinytext','text','mediumtext','longtext'";
const USER_PASSWORD: &str = "Schema-User-0099!";
const ADMIN_PASSWORD: &str = "Schema-Admin-0099!";
const AGENT_PASSWORD: &str = "Schema-Agent-0099!";

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
struct ColumnMetadata {
    table_name: String,
    column_name: String,
    ordinal_position: i64,
    data_type: String,
    column_type: String,
    character_set_name: Option<String>,
    collation_name: Option<String>,
    is_nullable: String,
    column_default: Option<String>,
    column_comment: String,
    extra: String,
    generation_expression: String,
    character_maximum_length: Option<i64>,
    character_octet_length: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
struct IndexMetadata {
    table_name: String,
    index_name: String,
    non_unique: i64,
    seq_in_index: i64,
    column_name: Option<String>,
    collation: Option<String>,
    sub_part: Option<i64>,
    packed: Option<String>,
    nullable: String,
    index_type: String,
    comment: String,
    index_comment: String,
    is_visible: String,
    expression: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
struct ForeignKeyMetadata {
    table_name: String,
    constraint_name: String,
    column_name: String,
    ordinal_position: i64,
    position_in_unique_constraint: Option<i64>,
    referenced_table_name: String,
    referenced_column_name: String,
    unique_constraint_name: String,
    match_option: String,
    update_rule: String,
    delete_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
struct TableMetadata {
    table_name: String,
    table_collation: String,
}

struct Fixture {
    route_admin_password_hash: String,
    user_email: String,
    user_phone: String,
    user_username: String,
    user_password_hash: String,
    user_status: String,
    admin_username: String,
    admin_password_hash: String,
    admin_status: String,
    agent_username: String,
    agent_password_hash: String,
    agent_status: String,
}

struct SeededFixture {
    user_id: u64,
    route_admin_id: u64,
    admin_id: u64,
    agent_admin_id: u64,
}

impl Fixture {
    fn new() -> Result<Self> {
        Ok(Self {
            route_admin_password_hash: hash_password("Schema-Route-Admin-0099!")?,
            user_email: "schema-user-0099@example.test".to_owned(),
            user_phone: "+15550000999".to_owned(),
            user_username: "schema_user_0099".to_owned(),
            user_password_hash: hash_password(USER_PASSWORD)?,
            user_status: "locked".to_owned(),
            admin_username: "schema_admin_0099".to_owned(),
            admin_password_hash: hash_password(ADMIN_PASSWORD)?,
            admin_status: "disabled".to_owned(),
            agent_username: "schema_agent_0099".to_owned(),
            agent_password_hash: hash_password(AGENT_PASSWORD)?,
            agent_status: "suspended".to_owned(),
        })
    }
}

#[tokio::test]
async fn schema_wide_text_metadata_drift_is_repaired_from_canonical_mysql_schema() {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!(
                "skipping schema text metadata migration test because DATABASE_URL is not set"
            );
            return;
        }
    };

    let mut server_url = Url::parse(&database_url).expect("DATABASE_URL must be a valid URL");
    server_url.set_path("/");
    let server_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(server_url.as_str())
        .await
        .expect("connect to MySQL server for isolated schema metadata migration test");
    let suffix = Uuid::now_v7().simple().to_string();
    let canonical_database = format!("text_meta_canonical_{suffix}");
    let repair_database = format!("text_meta_repair_{suffix}");

    for database_name in [&canonical_database, &repair_database] {
        let create_database = format!(
            "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        );
        if let Err(error) = sqlx::query(&create_database).execute(&server_pool).await {
            eprintln!(
                "skipping schema text metadata migration test because database {database_name} cannot be created: {error}"
            );
            let _ = cleanup_databases(&server_pool, [&canonical_database, &repair_database]).await;
            server_pool.close().await;
            return;
        }
    }

    let canonical_pool = connect_database(&database_url, &canonical_database)
        .await
        .expect("connect to canonical schema database");
    let repair_pool = connect_database(&database_url, &repair_database)
        .await
        .expect("connect to repair schema database");
    let exercise_result = exercise_schema_repair(&canonical_pool, &repair_pool).await;

    canonical_pool.close().await;
    repair_pool.close().await;
    let cleanup_result =
        cleanup_databases(&server_pool, [&canonical_database, &repair_database]).await;
    server_pool.close().await;

    exercise_result.expect("schema-wide text metadata migration contract");
    cleanup_result.expect("schema text metadata migration database cleanup");
}

async fn exercise_schema_repair(canonical_pool: &MySqlPool, repair_pool: &MySqlPool) -> Result<()> {
    let canonical_migrator = migrator_before_schema_text_repair();
    canonical_migrator
        .run(canonical_pool)
        .await
        .context("run migrations 0001-0098 on fresh canonical database")?;
    canonical_migrator
        .run(canonical_pool)
        .await
        .context("rerun migrations 0001-0098 without pending work")?;
    assert_migration_0099_not_applied(canonical_pool).await?;

    let full_migrator = sqlx::migrate!("./migrations");
    full_migrator
        .run(repair_pool)
        .await
        .context("run full migrations on fresh repair database")?;
    full_migrator
        .run(repair_pool)
        .await
        .context("rerun full migrations without pending work")?;
    assert_migration_0099_applied(repair_pool).await?;

    add_blob_probe(canonical_pool).await?;
    add_blob_probe(repair_pool).await?;
    let fixture = Fixture::new()?;
    let canonical_seeded = seed_fixture(canonical_pool, &fixture).await?;
    let repair_seeded = seed_fixture(repair_pool, &fixture).await?;
    ensure!(canonical_seeded.user_id == repair_seeded.user_id);
    ensure!(canonical_seeded.route_admin_id == repair_seeded.route_admin_id);
    ensure!(canonical_seeded.admin_id == repair_seeded.admin_id);
    ensure!(canonical_seeded.agent_admin_id == repair_seeded.agent_admin_id);

    let canonical_columns = collect_column_metadata(canonical_pool).await?;
    let canonical_text_columns = canonical_columns
        .iter()
        .filter(|column| is_text_data_type(&column.data_type))
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        canonical_text_columns.len() == EXPECTED_TEXT_COLUMNS,
        "fresh canonical schema must contain {EXPECTED_TEXT_COLUMNS} text columns, found {}",
        canonical_text_columns.len()
    );
    ensure!(
        canonical_text_columns.iter().all(|column| {
            column.generation_expression.is_empty()
                && !column.extra.contains("DEFAULT_GENERATED")
                && !column.extra.contains("VIRTUAL GENERATED")
                && !column.extra.contains("STORED GENERATED")
        }),
        "0099 generator requires explicit handling before canonical text columns may use generated/default expressions"
    );
    let canonical_tables = collect_table_metadata(canonical_pool).await?;
    ensure!(
        canonical_tables.len() == EXPECTED_BUSINESS_TABLES,
        "fresh canonical schema must contain {EXPECTED_BUSINESS_TABLES} business tables, found {}",
        canonical_tables.len()
    );
    assert_database_and_table_defaults(canonical_pool, &canonical_tables).await?;
    assert_zero_unsafe_text_or_binary_drift(canonical_pool).await?;
    let canonical_indexes = collect_index_metadata(canonical_pool).await?;
    let canonical_foreign_keys = collect_foreign_key_metadata(canonical_pool).await?;
    assert_no_text_foreign_keys(&canonical_text_columns, &canonical_foreign_keys)?;
    let canonical_values = collect_text_values(canonical_pool, &canonical_text_columns).await?;
    let canonical_blob = load_blob_probe(canonical_pool).await?;

    ensure!(
        collect_column_metadata(repair_pool).await? == canonical_columns,
        "fresh full migration result must exactly match the independent 0001-0098 canonical column metadata"
    );
    ensure!(
        collect_index_metadata(repair_pool).await? == canonical_indexes,
        "fresh full migration result must exactly match the independent 0001-0098 canonical indexes"
    );
    ensure!(
        collect_foreign_key_metadata(repair_pool).await? == canonical_foreign_keys,
        "fresh full migration result must exactly match the independent 0001-0098 canonical foreign keys"
    );
    ensure!(
        collect_text_values(repair_pool, &canonical_text_columns).await? == canonical_values,
        "fresh full migration result must preserve canonical text values"
    );

    sqlx::raw_sql(SCHEMA_TEXT_METADATA_MIGRATION)
        .execute(canonical_pool)
        .await
        .context("execute exact 0099 migration against already-correct schema")?;
    ensure!(
        collect_column_metadata(canonical_pool).await? == canonical_columns,
        "0099 must not change canonical column definitions"
    );
    ensure!(
        collect_index_metadata(canonical_pool).await? == canonical_indexes,
        "0099 must not change canonical indexes"
    );
    ensure!(
        collect_foreign_key_metadata(canonical_pool).await? == canonical_foreign_keys,
        "0099 must not change canonical foreign keys"
    );
    ensure!(
        collect_text_values(canonical_pool, &canonical_text_columns).await? == canonical_values,
        "0099 must not change canonical text values"
    );
    ensure!(load_blob_probe(canonical_pool).await? == canonical_blob);

    drift_kyc_name_to_varbinary(repair_pool).await?;
    assert_kyc_name_metadata(repair_pool, "varbinary", None, None).await?;
    assert_kyc_route_decode_failure(repair_pool, repair_seeded.route_admin_id, "VARBINARY").await?;

    drift_entire_business_schema(repair_pool, &canonical_tables, &canonical_text_columns).await?;
    let unsafe_columns = unsafe_text_or_binary_drift_count(repair_pool).await?;
    ensure!(
        unsafe_columns == EXPECTED_TEXT_COLUMNS as i64,
        "all {EXPECTED_TEXT_COLUMNS} canonical text columns must be drifted, found {unsafe_columns}"
    );
    ensure!(
        collect_text_values(repair_pool, &canonical_text_columns).await? == canonical_values,
        "drift fixture must preserve every business text value before repair"
    );
    ensure!(load_blob_probe(repair_pool).await? == canonical_blob);

    let repository = MySqlAuthRepository::new(repair_pool.clone());
    assert_auth_decode_failures(&repository, &fixture).await?;
    assert_prediction_decode_failure(repair_pool).await?;

    sqlx::raw_sql(SCHEMA_TEXT_METADATA_MIGRATION)
        .execute(repair_pool)
        .await
        .context("execute exact 0099 migration against schema-wide drift")?;

    assert_zero_unsafe_text_or_binary_drift(repair_pool).await?;
    assert_database_and_table_defaults(repair_pool, &canonical_tables).await?;
    ensure!(
        collect_column_metadata(repair_pool).await? == canonical_columns,
        "repaired column metadata must exactly match the fresh canonical schema"
    );
    ensure!(
        collect_index_metadata(repair_pool).await? == canonical_indexes,
        "repaired indexes must exactly match the fresh canonical schema"
    );
    ensure!(
        collect_foreign_key_metadata(repair_pool).await? == canonical_foreign_keys,
        "repaired foreign keys must exactly match the fresh canonical schema"
    );
    ensure!(
        collect_text_values(repair_pool, &canonical_text_columns).await? == canonical_values,
        "repair must preserve every business text value"
    );
    ensure!(
        load_blob_probe(repair_pool).await? == canonical_blob,
        "repair must leave BLOB metadata and bytes untouched"
    );
    assert_kyc_route_success(repair_pool, repair_seeded.route_admin_id).await?;
    assert_auth_credentials(&repository, &fixture, &repair_seeded).await?;
    assert_prediction_values(repair_pool).await?;

    sqlx::raw_sql(SCHEMA_TEXT_METADATA_MIGRATION)
        .execute(repair_pool)
        .await
        .context("rerun exact 0099 migration after repair")?;
    ensure!(collect_column_metadata(repair_pool).await? == canonical_columns);
    ensure!(collect_index_metadata(repair_pool).await? == canonical_indexes);
    ensure!(collect_foreign_key_metadata(repair_pool).await? == canonical_foreign_keys);
    ensure!(collect_text_values(repair_pool, &canonical_text_columns).await? == canonical_values);
    ensure!(load_blob_probe(repair_pool).await? == canonical_blob);
    assert_invalid_utf8_fails_without_replacement(repair_pool).await?;
    Ok(())
}

fn migrator_before_schema_text_repair() -> Migrator {
    let mut migrator = sqlx::migrate!("./migrations");
    let migrations = migrator
        .iter()
        .filter(|migration| migration.version < SCHEMA_TEXT_METADATA_VERSION)
        .cloned()
        .collect();
    migrator.migrations = Cow::Owned(migrations);
    migrator
}

async fn connect_database(database_url: &str, database_name: &str) -> Result<MySqlPool> {
    let mut url = Url::parse(database_url).context("parse DATABASE_URL")?;
    url.set_path(&format!("/{database_name}"));
    MySqlPoolOptions::new()
        .max_connections(4)
        .connect(url.as_str())
        .await
        .with_context(|| format!("connect to isolated database {database_name}"))
}

async fn cleanup_databases<'a>(
    server_pool: &MySqlPool,
    database_names: impl IntoIterator<Item = &'a String>,
) -> Result<()> {
    for database_name in database_names {
        let drop_database = format!("DROP DATABASE IF EXISTS `{database_name}`");
        sqlx::query(&drop_database)
            .execute(server_pool)
            .await
            .with_context(|| format!("drop isolated database {database_name}"))?;
    }
    Ok(())
}

async fn assert_migration_0099_applied(pool: &MySqlPool) -> Result<()> {
    let success: i64 = sqlx::query_scalar("SELECT success FROM _sqlx_migrations WHERE version = ?")
        .bind(SCHEMA_TEXT_METADATA_VERSION)
        .fetch_one(pool)
        .await
        .context("read SQLx migration 0099 status")?;
    ensure!(success == 1, "migration 0099 must be marked successful");
    Ok(())
}

async fn assert_migration_0099_not_applied(pool: &MySqlPool) -> Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = ?")
        .bind(SCHEMA_TEXT_METADATA_VERSION)
        .fetch_one(pool)
        .await
        .context("confirm canonical database stops before migration 0099")?;
    ensure!(
        count == 0,
        "independent canonical database must stop after migration 0098"
    );
    Ok(())
}

async fn add_blob_probe(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"ALTER TABLE kyc_configs
               ADD COLUMN text_metadata_repair_blob_probe BLOB NULL
                   COMMENT 'binary payload probe'"#,
    )
    .execute(pool)
    .await
    .context("add BLOB payload probe outside the repair target")?;
    Ok(())
}

async fn seed_fixture(pool: &MySqlPool, fixture: &Fixture) -> Result<SeededFixture> {
    let role_id = sqlx::query(
        "INSERT INTO admin_roles (name, permissions) VALUES ('schema_text_metadata_role', JSON_OBJECT())",
    )
    .execute(pool)
    .await?
    .last_insert_id();
    let route_admin_id = sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id, status) VALUES (?, ?, ?, 'active')",
    )
    .bind("schema_route_admin_0099")
    .bind(&fixture.route_admin_password_hash)
    .bind(role_id)
    .execute(pool)
    .await?
    .last_insert_id();
    let admin_id = sqlx::query(
        "INSERT INTO admin_users (username, password_hash, role_id, status) VALUES (?, ?, ?, ?)",
    )
    .bind(&fixture.admin_username)
    .bind(&fixture.admin_password_hash)
    .bind(role_id)
    .bind(&fixture.admin_status)
    .execute(pool)
    .await?
    .last_insert_id();
    let user_id = sqlx::query(
        r#"INSERT INTO users
               (username, email, phone, country_code, preferred_locale, password_hash, status)
           VALUES (?, ?, ?, 'HK', 'zh-HK', ?, ?)"#,
    )
    .bind(&fixture.user_username)
    .bind(&fixture.user_email)
    .bind(&fixture.user_phone)
    .bind(&fixture.user_password_hash)
    .bind(&fixture.user_status)
    .execute(pool)
    .await?
    .last_insert_id();
    let agent_id = sqlx::query(
        r#"INSERT INTO agents (user_id, agent_code, level, path, status)
           VALUES (?, 'SCHEMA_AGENT_0099', 1, 'schema-agent-0099', 'active')"#,
    )
    .bind(user_id)
    .execute(pool)
    .await?
    .last_insert_id();
    let agent_admin_id = sqlx::query(
        "INSERT INTO agent_admin_users (agent_id, username, password_hash, status) VALUES (?, ?, ?, ?)",
    )
    .bind(agent_id)
    .bind(&fixture.agent_username)
    .bind(&fixture.agent_password_hash)
    .bind(&fixture.agent_status)
    .execute(pool)
    .await?
    .last_insert_id();

    sqlx::query(
        r#"UPDATE kyc_configs
           SET name = 'default',
               enabled = TRUE,
               target_kyc_level = 2,
               required_documents_json = JSON_ARRAY('identity_front', 'identity_back'),
               allowed_countries_json = JSON_ARRAY('中国', 'Hong Kong'),
               country_document_types_json = JSON_ARRAY(
                   JSON_OBJECT(
                       'country', '中国',
                       'document_types', JSON_ARRAY('identity_card', 'passport'),
                       'handheld_document_types', JSON_ARRAY('passport')
                   )
               ),
               max_document_size_bytes = 6291456,
               updated_by = ?,
               text_metadata_repair_blob_probe = X'00FF10207F80'
           WHERE id = 1"#,
    )
    .bind(route_admin_id)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"UPDATE prediction_settings
           SET default_settlement_mode = 'auto_settle',
               default_invalid_refund_policy = 'refund_stake_only',
               last_sync_status = 'failed',
               last_sync_error = '同步失败：上游返回无效结果'
           WHERE id = 1"#,
    )
    .execute(pool)
    .await?;

    Ok(SeededFixture {
        user_id,
        route_admin_id,
        admin_id,
        agent_admin_id,
    })
}

async fn collect_column_metadata(pool: &MySqlPool) -> Result<Vec<ColumnMetadata>> {
    sqlx::query_as::<_, ColumnMetadata>(
        r#"SELECT CAST(TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS table_name,
                  CAST(COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS column_name,
                  CAST(ORDINAL_POSITION AS SIGNED) AS ordinal_position,
                  CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4) AS data_type,
                  CAST(COLUMN_TYPE AS CHAR(512) CHARACTER SET utf8mb4) AS column_type,
                  CAST(CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS character_set_name,
                  CAST(COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS collation_name,
                  CAST(IS_NULLABLE AS CHAR(3) CHARACTER SET utf8mb4) AS is_nullable,
                  CAST(COLUMN_DEFAULT AS CHAR(2048) CHARACTER SET utf8mb4)
                      AS column_default,
                  CAST(COLUMN_COMMENT AS CHAR(2048) CHARACTER SET utf8mb4)
                      AS column_comment,
                  CAST(EXTRA AS CHAR(255) CHARACTER SET utf8mb4) AS extra,
                  CAST(GENERATION_EXPRESSION AS CHAR(2048) CHARACTER SET utf8mb4)
                      AS generation_expression,
                  CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length,
                  CAST(CHARACTER_OCTET_LENGTH AS SIGNED) AS character_octet_length
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME <> '_sqlx_migrations'
           ORDER BY TABLE_NAME, ORDINAL_POSITION"#,
    )
    .fetch_all(pool)
    .await
    .context("collect business column metadata")
}

async fn collect_index_metadata(pool: &MySqlPool) -> Result<Vec<IndexMetadata>> {
    sqlx::query_as::<_, IndexMetadata>(
        r#"SELECT CAST(TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS table_name,
                  CAST(INDEX_NAME AS CHAR(128) CHARACTER SET utf8mb4) AS index_name,
                  CAST(NON_UNIQUE AS SIGNED) AS non_unique,
                  CAST(SEQ_IN_INDEX AS SIGNED) AS seq_in_index,
                  CAST(COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS column_name,
                  CAST(COLLATION AS CHAR(1) CHARACTER SET utf8mb4) AS collation,
                  CAST(SUB_PART AS SIGNED) AS sub_part,
                  CAST(PACKED AS CHAR(64) CHARACTER SET utf8mb4) AS packed,
                  CAST(NULLABLE AS CHAR(3) CHARACTER SET utf8mb4) AS nullable,
                  CAST(INDEX_TYPE AS CHAR(32) CHARACTER SET utf8mb4) AS index_type,
                  CAST(COMMENT AS CHAR(255) CHARACTER SET utf8mb4) AS comment,
                  CAST(INDEX_COMMENT AS CHAR(1024) CHARACTER SET utf8mb4) AS index_comment,
                  CAST(IS_VISIBLE AS CHAR(3) CHARACTER SET utf8mb4) AS is_visible,
                  CAST(EXPRESSION AS CHAR(2048) CHARACTER SET utf8mb4) AS expression
           FROM information_schema.STATISTICS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME <> '_sqlx_migrations'
           ORDER BY TABLE_NAME, INDEX_NAME, SEQ_IN_INDEX"#,
    )
    .fetch_all(pool)
    .await
    .context("collect business index metadata")
}

async fn collect_foreign_key_metadata(pool: &MySqlPool) -> Result<Vec<ForeignKeyMetadata>> {
    sqlx::query_as::<_, ForeignKeyMetadata>(
        r#"SELECT CAST(k.TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS table_name,
                  CAST(k.CONSTRAINT_NAME AS CHAR(128) CHARACTER SET utf8mb4)
                      AS constraint_name,
                  CAST(k.COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS column_name,
                  CAST(k.ORDINAL_POSITION AS SIGNED) AS ordinal_position,
                  CAST(k.POSITION_IN_UNIQUE_CONSTRAINT AS SIGNED)
                      AS position_in_unique_constraint,
                  CAST(k.REFERENCED_TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS referenced_table_name,
                  CAST(k.REFERENCED_COLUMN_NAME AS CHAR(64) CHARACTER SET utf8mb4)
                      AS referenced_column_name,
                  CAST(r.UNIQUE_CONSTRAINT_NAME AS CHAR(128) CHARACTER SET utf8mb4)
                      AS unique_constraint_name,
                  CAST(r.MATCH_OPTION AS CHAR(16) CHARACTER SET utf8mb4) AS match_option,
                  CAST(r.UPDATE_RULE AS CHAR(16) CHARACTER SET utf8mb4) AS update_rule,
                  CAST(r.DELETE_RULE AS CHAR(16) CHARACTER SET utf8mb4) AS delete_rule
           FROM information_schema.KEY_COLUMN_USAGE k
           INNER JOIN information_schema.REFERENTIAL_CONSTRAINTS r
                   ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA
                  AND r.TABLE_NAME = k.TABLE_NAME
                  AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME
           WHERE k.CONSTRAINT_SCHEMA = DATABASE()
             AND k.REFERENCED_TABLE_NAME IS NOT NULL
           ORDER BY k.TABLE_NAME, k.CONSTRAINT_NAME, k.ORDINAL_POSITION"#,
    )
    .fetch_all(pool)
    .await
    .context("collect business foreign key metadata")
}

fn assert_no_text_foreign_keys(
    text_columns: &[ColumnMetadata],
    foreign_keys: &[ForeignKeyMetadata],
) -> Result<()> {
    let text_positions = text_columns
        .iter()
        .map(|column| (column.table_name.as_str(), column.column_name.as_str()))
        .collect::<BTreeSet<_>>();
    ensure!(
        foreign_keys.iter().all(|foreign_key| {
            !text_positions.contains(&(
                foreign_key.table_name.as_str(),
                foreign_key.column_name.as_str(),
            ))
        }),
        "canonical schema unexpectedly contains a text foreign key; review 0099 generation and drift coverage"
    );
    Ok(())
}

async fn collect_table_metadata(pool: &MySqlPool) -> Result<Vec<TableMetadata>> {
    sqlx::query_as::<_, TableMetadata>(
        r#"SELECT CAST(TABLE_NAME AS CHAR(64) CHARACTER SET utf8mb4) AS table_name,
                  CAST(TABLE_COLLATION AS CHAR(64) CHARACTER SET utf8mb4) AS table_collation
           FROM information_schema.TABLES
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME <> '_sqlx_migrations'
             AND TABLE_TYPE = 'BASE TABLE'
           ORDER BY TABLE_NAME"#,
    )
    .fetch_all(pool)
    .await
    .context("collect business table metadata")
}

async fn collect_text_values(
    pool: &MySqlPool,
    columns: &[ColumnMetadata],
) -> Result<BTreeMap<(String, String), Vec<Option<String>>>> {
    let mut values = BTreeMap::new();
    for column in columns {
        let sql = format!(
            "SELECT HEX({}) FROM {}",
            quote_identifier(&column.column_name),
            quote_identifier(&column.table_name)
        );
        let mut column_values = sqlx::query_scalar::<_, Option<String>>(&sql)
            .fetch_all(pool)
            .await
            .with_context(|| {
                format!(
                    "collect text bytes for {}.{}",
                    column.table_name, column.column_name
                )
            })?;
        column_values.sort();
        values.insert(
            (column.table_name.clone(), column.column_name.clone()),
            column_values,
        );
    }
    Ok(values)
}

async fn load_blob_probe(
    pool: &MySqlPool,
) -> Result<(String, Option<String>, Option<String>, Vec<u8>)> {
    sqlx::query_as(
        r#"SELECT CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4),
                  (SELECT text_metadata_repair_blob_probe FROM kyc_configs WHERE id = 1)
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'kyc_configs'
             AND COLUMN_NAME = 'text_metadata_repair_blob_probe'"#,
    )
    .fetch_one(pool)
    .await
    .context("read BLOB probe metadata and bytes")
}

async fn drift_kyc_name_to_varbinary(pool: &MySqlPool) -> Result<()> {
    sqlx::raw_sql(
        r#"ALTER TABLE kyc_configs
               MODIFY COLUMN name VARBINARY(64) NOT NULL
                   COMMENT 'KYC 配置：显示名称'"#,
    )
    .execute(pool)
    .await
    .context("reproduce production kyc_configs.name VARBINARY drift")?;
    Ok(())
}

async fn assert_invalid_utf8_fails_without_replacement(pool: &MySqlPool) -> Result<()> {
    drift_kyc_name_to_varbinary(pool).await?;
    sqlx::query("UPDATE kyc_configs SET name = X'FF' WHERE id = 1")
        .execute(pool)
        .await
        .context("inject one invalid UTF-8 byte into drifted KYC text")?;
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ? AND success = TRUE")
        .bind(SCHEMA_TEXT_METADATA_VERSION)
        .execute(pool)
        .await
        .context("make 0099 pending for the invalid UTF-8 migration-runner test")?;

    let full_migrator = sqlx::migrate!("./migrations");
    let error = full_migrator
        .run(pool)
        .await
        .expect_err("0099 must reject invalid UTF-8 instead of replacing stored bytes");
    ensure!(
        error.to_string().contains("Incorrect string value"),
        "invalid UTF-8 must fail with a MySQL conversion error, got: {error}"
    );

    let actual: (String, String, i64) = sqlx::query_as(
        r#"SELECT CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4),
                  HEX((SELECT name FROM kyc_configs WHERE id = 1)),
                  (SELECT CAST(success AS SIGNED)
                   FROM _sqlx_migrations
                   WHERE version = ?)
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'kyc_configs'
             AND COLUMN_NAME = 'name'"#,
    )
    .bind(SCHEMA_TEXT_METADATA_VERSION)
    .fetch_one(pool)
    .await
    .context("verify invalid UTF-8 failure preserves the original binary byte")?;
    ensure!(actual.0 == "varbinary");
    ensure!(actual.1 == "FF");
    ensure!(
        actual.2 == 0,
        "failed MySQL DDL migration must retain SQLx success=FALSE"
    );
    Ok(())
}

async fn drift_entire_business_schema(
    pool: &MySqlPool,
    tables: &[TableMetadata],
    columns: &[ColumnMetadata],
) -> Result<()> {
    sqlx::raw_sql("ALTER DATABASE CHARACTER SET latin1 COLLATE latin1_swedish_ci")
        .execute(pool)
        .await
        .context("drift database default character set and collation")?;

    let mut columns_by_table = BTreeMap::<&str, Vec<&ColumnMetadata>>::new();
    for column in columns {
        columns_by_table
            .entry(&column.table_name)
            .or_default()
            .push(column);
    }

    for table in tables {
        let mut ddl = format!(
            "ALTER TABLE {} DEFAULT CHARACTER SET latin1 COLLATE latin1_swedish_ci",
            quote_identifier(&table.table_name)
        );
        if let Some(table_columns) = columns_by_table.get(table.table_name.as_str()) {
            for column in table_columns {
                ddl.push_str(", MODIFY COLUMN ");
                ddl.push_str(&quote_identifier(&column.column_name));
                ddl.push(' ');
                ddl.push_str(&drift_column_type(column));
                ddl.push(' ');
                ddl.push_str(if column.is_nullable == "YES" {
                    "NULL"
                } else {
                    "NOT NULL"
                });
                if let Some(default) = column.column_default.as_deref() {
                    ddl.push_str(" DEFAULT ");
                    ddl.push_str(&quote_string(default));
                } else if column.is_nullable == "YES" {
                    ddl.push_str(" DEFAULT NULL");
                }
                ddl.push_str(" COMMENT ");
                ddl.push_str(&quote_string(&column.column_comment));
            }
        }
        sqlx::raw_sql(&ddl)
            .execute(pool)
            .await
            .with_context(|| format!("drift text metadata for table {}", table.table_name))?;
    }
    Ok(())
}

fn drift_column_type(column: &ColumnMetadata) -> String {
    let actual_binary = matches!(
        (column.table_name.as_str(), column.column_name.as_str()),
        ("kyc_configs", "name")
            | ("users", "password_hash")
            | ("users", "status")
            | ("admin_users", "password_hash")
            | ("admin_users", "status")
            | ("agent_admin_users", "password_hash")
            | ("agent_admin_users", "status")
            | ("admin_login_two_factor_challenges", "challenge_id")
    );
    if actual_binary {
        let length = column
            .character_maximum_length
            .expect("CHAR/VARCHAR canonical column must have a length");
        if column.data_type == "char" {
            format!("BINARY({length})")
        } else {
            format!("VARBINARY({length})")
        }
    } else {
        format!(
            "{} CHARACTER SET utf8mb4 COLLATE utf8mb4_bin",
            column.column_type
        )
    }
}

async fn assert_kyc_name_metadata(
    pool: &MySqlPool,
    data_type: &str,
    character_set: Option<&str>,
    collation: Option<&str>,
) -> Result<()> {
    let actual: (String, Option<String>, Option<String>, String, i64) = sqlx::query_as(
        r#"SELECT CAST(DATA_TYPE AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(IS_NULLABLE AS CHAR(3) CHARACTER SET utf8mb4),
                  CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED)
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME = 'kyc_configs'
             AND COLUMN_NAME = 'name'"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(actual.0 == data_type);
    ensure!(actual.1.as_deref() == character_set);
    ensure!(actual.2.as_deref() == collation);
    ensure!(actual.3 == "NO");
    ensure!(actual.4 == 64);
    Ok(())
}

async fn assert_kyc_route_decode_failure(
    pool: &MySqlPool,
    route_admin_id: u64,
    expected_sql_type: &str,
) -> Result<()> {
    let (status, payload) = request_kyc_config(pool, route_admin_id).await?;
    ensure!(status == StatusCode::INTERNAL_SERVER_ERROR);
    ensure!(payload["code"] == "DATABASE_ERROR");
    let message = payload["message"].as_str().unwrap_or_default();
    ensure!(
        message.contains("column \"name\""),
        "KYC production query must fail while decoding kyc_configs.name, got: {message}"
    );
    ensure!(
        message.contains(expected_sql_type),
        "KYC production query must report {expected_sql_type}, got: {message}"
    );
    Ok(())
}

async fn assert_kyc_route_success(pool: &MySqlPool, route_admin_id: u64) -> Result<()> {
    let (status, payload) = request_kyc_config(pool, route_admin_id).await?;
    ensure!(
        status == StatusCode::OK,
        "KYC production query must recover after 0099, got {status}: {payload}"
    );
    ensure!(payload["name"] == "default");
    ensure!(payload["target_kyc_level"] == 2);
    ensure!(payload["allowed_countries"] == json!(["中国", "Hong Kong"]));
    Ok(())
}

async fn request_kyc_config(pool: &MySqlPool, route_admin_id: u64) -> Result<(StatusCode, Value)> {
    let settings = test_settings();
    let token = issue_token(
        &settings,
        format!("admin:{route_admin_id}"),
        TokenScope::Admin,
        900,
    )?;
    let app = build_router(AppState::new(settings).with_mysql(pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/v1/kyc/config")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let payload = serde_json::from_slice(&body).context("decode KYC route JSON response")?;
    Ok((status, payload))
}

async fn assert_auth_decode_failures(
    repository: &MySqlAuthRepository,
    fixture: &Fixture,
) -> Result<()> {
    let lookups = [
        (
            "user email",
            repository.find_user_by_email(&fixture.user_email).await,
        ),
        (
            "user phone",
            repository.find_user_by_phone(&fixture.user_phone).await,
        ),
        (
            "user username",
            repository
                .find_user_by_username(&fixture.user_username)
                .await,
        ),
        (
            "admin username",
            repository
                .find_admin_by_username(&fixture.admin_username)
                .await,
        ),
        (
            "agent username",
            repository
                .find_agent_by_username(&fixture.agent_username)
                .await,
        ),
    ];
    for (lookup, result) in lookups {
        let error = result.expect_err("VARBINARY credentials must fail String decoding");
        let AppError::Database(sqlx::Error::ColumnDecode { index, .. }) = error else {
            anyhow::bail!("expected ColumnDecode for {lookup}, got {error:?}");
        };
        ensure!(
            index == "1",
            "{lookup} must fail at column 1 (password_hash), got {index}"
        );
    }
    Ok(())
}

async fn assert_auth_credentials(
    repository: &MySqlAuthRepository,
    fixture: &Fixture,
    seeded: &SeededFixture,
) -> Result<()> {
    let user_email = repository
        .find_user_by_email(&fixture.user_email)
        .await?
        .context("repaired user email credential must exist")?;
    let user_phone = repository
        .find_user_by_phone(&fixture.user_phone)
        .await?
        .context("repaired user phone credential must exist")?;
    let user_username = repository
        .find_user_by_username(&fixture.user_username)
        .await?
        .context("repaired user username credential must exist")?;
    for credential in [&user_email, &user_phone, &user_username] {
        assert_credential(
            credential,
            ActorType::User,
            seeded.user_id,
            Some(seeded.user_id),
            &fixture.user_password_hash,
            &fixture.user_status,
            USER_PASSWORD,
        )?;
    }

    let admin = repository
        .find_admin_by_username(&fixture.admin_username)
        .await?
        .context("repaired admin credential must exist")?;
    assert_credential(
        &admin,
        ActorType::Admin,
        seeded.admin_id,
        None,
        &fixture.admin_password_hash,
        &fixture.admin_status,
        ADMIN_PASSWORD,
    )?;
    let agent = repository
        .find_agent_by_username(&fixture.agent_username)
        .await?
        .context("repaired agent credential must exist")?;
    assert_credential(
        &agent,
        ActorType::Agent,
        seeded.agent_admin_id,
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

async fn assert_prediction_decode_failure(pool: &MySqlPool) -> Result<()> {
    let error = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        r#"SELECT default_settlement_mode,
                  default_invalid_refund_policy,
                  last_sync_status,
                  last_sync_error
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await
    .expect_err("binary-collated prediction settings must fail String decoding");
    ensure!(
        matches!(error, sqlx::Error::ColumnDecode { .. }),
        "expected prediction settings ColumnDecode, got {error:?}"
    );
    Ok(())
}

async fn assert_prediction_values(pool: &MySqlPool) -> Result<()> {
    let values: (String, String, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT default_settlement_mode,
                  default_invalid_refund_policy,
                  last_sync_status,
                  last_sync_error
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(values.0 == "auto_settle");
    ensure!(values.1 == "refund_stake_only");
    ensure!(values.2.as_deref() == Some("failed"));
    ensure!(values.3.as_deref() == Some("同步失败：上游返回无效结果"));
    Ok(())
}

async fn assert_database_and_table_defaults(
    pool: &MySqlPool,
    canonical_tables: &[TableMetadata],
) -> Result<()> {
    let schema: (String, String) = sqlx::query_as(
        r#"SELECT CAST(DEFAULT_CHARACTER_SET_NAME AS CHAR(64) CHARACTER SET utf8mb4),
                  CAST(DEFAULT_COLLATION_NAME AS CHAR(64) CHARACTER SET utf8mb4)
           FROM information_schema.SCHEMATA
           WHERE SCHEMA_NAME = DATABASE()"#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(schema.0 == "utf8mb4");
    ensure!(schema.1 == "utf8mb4_unicode_ci");

    let actual_tables = collect_table_metadata(pool).await?;
    ensure!(
        actual_tables
            .iter()
            .all(|table| table.table_collation == "utf8mb4_unicode_ci")
    );
    let expected_names = canonical_tables
        .iter()
        .map(|table| &table.table_name)
        .collect::<BTreeSet<_>>();
    let actual_names = actual_tables
        .iter()
        .map(|table| &table.table_name)
        .collect::<BTreeSet<_>>();
    ensure!(actual_names == expected_names);
    Ok(())
}

async fn assert_zero_unsafe_text_or_binary_drift(pool: &MySqlPool) -> Result<()> {
    let count = unsafe_text_or_binary_drift_count(pool).await?;
    ensure!(
        count == 0,
        "business schema still contains {count} unsafe binary text columns"
    );
    Ok(())
}

async fn unsafe_text_or_binary_drift_count(pool: &MySqlPool) -> Result<i64> {
    let sql = format!(
        r#"SELECT COUNT(*)
           FROM information_schema.COLUMNS
           WHERE TABLE_SCHEMA = DATABASE()
             AND TABLE_NAME <> '_sqlx_migrations'
             AND (
                 DATA_TYPE IN ('binary', 'varbinary')
                 OR (
                     DATA_TYPE IN ({TEXT_DATA_TYPES})
                     AND (
                         CHARACTER_SET_NAME = 'binary'
                         OR COLLATION_NAME = 'binary'
                         OR RIGHT(COLLATION_NAME, 4) = '_bin'
                     )
                 )
             )"#
    );
    sqlx::query_scalar(&sql)
        .fetch_one(pool)
        .await
        .context("audit unsafe business text and binary metadata")
}

fn is_text_data_type(data_type: &str) -> bool {
    matches!(
        data_type,
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext"
    )
}

fn quote_identifier(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn quote_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\0', "\\0")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\'', "\\'");
    format!("'{escaped}'")
}

fn test_settings() -> Settings {
    Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("schema-text-metadata-test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
        agent_commission_auto_settle_enabled: false,
        agent_commission_auto_settle_interval_seconds: 60,
        agent_commission_auto_settle_min_age_seconds: 3600,
        agent_commission_auto_settle_batch_limit: 100,
    }
}
