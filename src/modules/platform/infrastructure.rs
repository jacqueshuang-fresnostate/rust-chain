//! platform bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::platform::domain::{DEFAULT_CONFIG_NAME, PlatformBrand},
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug)]
pub struct PlatformBrandRepository;

impl InfrastructureLayer for PlatformBrandRepository {}

#[derive(Debug, sqlx::FromRow)]
struct PlatformBrandRow {
    id: u64,
    name: String,
    platform_name: String,
    logo_url: Option<String>,
    chart_provider: String,
    updated_by: Option<u64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// 幂等补齐默认平台品牌记录，已存在配置时不覆盖管理员保存的字段。
/// 该入口使用连接池独立执行，供公开读取前保证基础记录存在。
pub async fn ensure_default_platform_brand(pool: &Pool<MySql>) -> AppResult<()> {
    sqlx::query(default_platform_brand_insert_sql())
        .execute(pool)
        .await?;
    Ok(())
}

/// 在调用方事务中幂等补齐默认品牌记录，不自行提交或回滚。
/// 供配置变更用例与后续行锁共享同一事务，避免初始化和更新之间出现竞态。
pub async fn ensure_default_platform_brand_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<()> {
    sqlx::query(default_platform_brand_insert_sql())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 从连接池读取默认品牌配置；记录不存在时返回 `NotFound` 而非伪造数据。
pub async fn load_platform_brand_row(pool: &Pool<MySql>) -> AppResult<PlatformBrand> {
    let row = sqlx::query_as::<_, PlatformBrandRow>(&select_platform_brand_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(platform_brand(row))
}

/// 在调用方事务中读取默认品牌快照但不加行锁，用于保存后的结果回读。
pub async fn load_platform_brand_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<PlatformBrand> {
    let row = sqlx::query_as::<_, PlatformBrandRow>(&select_platform_brand_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(platform_brand(row))
}

/// 以 `FOR UPDATE` 锁定默认品牌记录并返回变更前快照。
/// 锁持续到调用方提交或回滚，用于串行化并发管理端配置修改。
pub async fn lock_platform_brand_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<PlatformBrand> {
    let row = sqlx::query_as::<_, PlatformBrandRow>(&select_platform_brand_sql(true))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(platform_brand(row))
}

/// 在调用方事务中新增或更新默认品牌配置，并记录最后操作管理员。
/// 本函数不提交事务；任一 SQL 错误由上层回滚，避免品牌配置与审计记录分离。
pub async fn upsert_platform_brand_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    platform_name: &str,
    logo_url: &Option<String>,
    chart_provider: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO platform_brand_configs
           (name, platform_name, logo_url, chart_provider, updated_by)
           VALUES (?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE platform_name = VALUES(platform_name),
                                   logo_url = VALUES(logo_url),
                                   chart_provider = VALUES(chart_provider),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .bind(platform_name)
    .bind(logo_url)
    .bind(chart_provider)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn default_platform_brand_insert_sql() -> &'static str {
    r#"INSERT INTO platform_brand_configs
       (name, platform_name, logo_url)
       VALUES ('default', 'Hippo Exchange', NULL)
       ON DUPLICATE KEY UPDATE name = name"#
}

fn select_platform_brand_sql(for_update: bool) -> String {
    let mut sql = String::from(
        r#"SELECT id, name, platform_name, logo_url, chart_provider, updated_by, created_at, updated_at
           FROM platform_brand_configs
           WHERE name = ?"#,
    );
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

fn platform_brand(row: PlatformBrandRow) -> PlatformBrand {
    PlatformBrand {
        id: row.id,
        name: row.name,
        platform_name: row.platform_name,
        logo_url: row.logo_url,
        chart_provider: row.chart_provider,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
