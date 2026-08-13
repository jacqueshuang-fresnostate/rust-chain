//! platform bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 平台品牌配置在库中只有固定行名的一行，本文件围绕它提供幂等补齐、只读读取、加锁读取与写入四类操作。
//! 后台保存路径的固定顺序是先补齐、再加锁读旧值、然后写入、最后回读新值，
//! 全程在调用方事务内完成，使并发保存被串行化且审计能拿到完整的前后快照。
//! 带 in_tx 后缀的函数一律不提交也不回滚。

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

/// 从连接池读取默认品牌配置行并转成领域快照，不加行锁因而不阻塞并发的后台保存。
/// 记录不存在时返回未找到而不是就地伪造一份默认值，让缺失的初始化问题在接口层显性暴露，
/// 而不是让前端拿到一份实际并未落库的配置；调用方通常会先执行幂等补齐再调用本函数。
pub async fn load_platform_brand_row(pool: &Pool<MySql>) -> AppResult<PlatformBrand> {
    let row = sqlx::query_as::<_, PlatformBrandRow>(&select_platform_brand_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(platform_brand(row))
}

/// 在调用方事务中读取默认品牌快照，用于写入后回读变更结果。
/// 这里刻意不加行锁：同一事务在保存前已通过锁定读取持有该行的排他锁，回读时再次加锁没有意义。
/// 因为处于同一事务内，读到的是本事务尚未提交的最新值，即写后读一致。
/// 记录缺失同样返回未找到，由调用方连同整个事务一起回滚。
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
/// 语句按固定行名做插入或更新，冲突时覆盖站点名、Logo 地址、图表提供方并把操作人改写为当前管理员。
/// 三个业务字段都以传入值整体覆盖而非按需合并，因此调用方必须先补齐要保留的旧值再调用。
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

/// 返回补齐默认品牌配置行的 SQL，供连接池版与事务版两个入口共用同一份语句。
/// 冲突分支写成把行名赋给自己这种空更新，是为了在记录已存在时既不报错也不触碰管理员保存过的任何字段，
/// 从而让该语句可以在每次读取前无条件执行；站点名与图表提供方的初值来自这里和数据库列默认值。
fn default_platform_brand_insert_sql() -> &'static str {
    r#"INSERT INTO platform_brand_configs
       (name, platform_name, logo_url)
       VALUES ('default', 'Hippo Exchange', NULL)
       ON DUPLICATE KEY UPDATE name = name"#
}

/// 生成按行名查询品牌配置的语句，参数决定是否追加排他锁子句。
/// 三个读取入口共用同一份列清单，保证只读查询、事务内回读与加锁读取映射到完全一致的字段集合。
/// 拼接部分只有固定的锁子句字面量，行名始终以绑定参数传入，不存在拼接外部输入的路径。
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

/// 把数据库行按字段逐一搬进领域快照，是本文件三个读取入口的共同出口。
/// 纯结构转换，不补默认值也不做校验，因此领域层看到的取值与库中完全一致。
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
