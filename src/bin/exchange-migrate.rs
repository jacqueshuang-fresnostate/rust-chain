//! 数据库迁移与首个管理员引导的独立可执行程序，部署流水线应在启动 API 服务之前先运行它。
//! 它把 `migrations` 目录在编译期嵌入二进制，因此运行时不依赖源码目录，可以直接在镜像里执行。
//! 迁移与引导都具备幂等性：已应用的 migration 会被跳过，库中已存在管理员时引导也只跳过而不覆盖。
//! 与 API 服务不同，这里只连接 MySQL，不接触 Mongo、Redis 或消息队列。

use anyhow::Context;
use exchange_api::bootstrap::{
    BootstrapAdminMode, BootstrapAdminOutcome, bootstrap_default_admin_from_env,
};
use exchange_api::migration::{
    MigrationConnectConfig, collect_migration_diagnostics, connect_with_retry, format_error_chain,
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// 依次执行数据库结构迁移与默认管理员引导，任一步失败都带中文上下文向上返回并让进程以非零码退出。
/// 连接串只从 `DATABASE_URL` 读取，先尝试加载 `.env` 但忽略其缺失；连接池限制为单连接，避免迁移期间并发改表。
/// 初始连接只按迁移专用的有界策略重试，认证、URL 格式和权限错误不会被延迟吞掉。
/// `BOOTSTRAP_MODE` 缺省为关闭；只有显式 `create_admin` 才读取一次性 Secret 并执行首管理员引导。
/// 无论引导成功与否都会先关闭连接池再判断结果，确保命名锁所在会话及时释放而不是等到进程退出。
/// 最终按新建还是跳过打印不同日志，两种情况都算执行成功，只有真正的错误才会中断流水线。
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL 未配置")?;
    let connect_config = MigrationConnectConfig::from_env().context("解析迁移连接重试配置失败")?;
    let pool = connect_with_retry(&database_url, connect_config).await?;

    if let Err(error) = MIGRATOR.run(&pool).await {
        let error_chain = format_error_chain(&error);
        tracing::error!(
            error_chain = %error_chain,
            "执行 SQLx migrations 失败（原始错误链）"
        );
        let diagnostics = collect_migration_diagnostics(&pool).await;
        tracing::error!(
            server_version = diagnostics.server_version.as_deref().unwrap_or("unknown"),
            migrations_table_exists = ?diagnostics.migrations_table_exists,
            migration_count = ?diagnostics.migration_count,
            latest_version = ?diagnostics.latest_version,
            dirty_versions = ?diagnostics.dirty_versions,
            "迁移失败诊断摘要"
        );
        for query_error in diagnostics.query_errors {
            tracing::warn!(error = %query_error, "迁移失败诊断查询未完成");
        }
        pool.close().await;
        // 不把原始 `MigrateError` 作为 anyhow source 返回：CLI 终止时 anyhow 会再次
        // 打印 source 链，可能绕过上面的脱敏日志。完整（已脱敏）链已写入 tracing，
        // 返回值保留同样的版本/SQLx 错误文本并继续以非零码阻止 API。
        return Err(anyhow::anyhow!(
            "执行 SQLx migrations 失败（原始错误链）：{error_chain}"
        ));
    }

    let migration_count = MIGRATOR.iter().count();
    let latest_version = MIGRATOR.iter().map(|migration| migration.version).max();
    tracing::info!(
        migration_count,
        latest_version = ?latest_version,
        "数据库 migrations 已全部应用"
    );

    let bootstrap_result = match BootstrapAdminMode::from_env() {
        Ok(BootstrapAdminMode::Disabled) => None,
        Ok(BootstrapAdminMode::CreateAdmin) => Some(bootstrap_default_admin_from_env(&pool).await),
        Err(error) => Some(Err(error)),
    };
    pool.close().await;

    match bootstrap_result {
        None => tracing::info!("管理员引导模式未开启，已跳过账号创建"),
        Some(result) => match result.context("初始化引导管理员失败")? {
            BootstrapAdminOutcome::Created => {
                tracing::info!("一次性引导管理员已创建，首次登录必须修改口令");
            }
            BootstrapAdminOutcome::SkippedExistingAdmin => {
                tracing::info!("数据库已存在管理员，引导管理员创建已跳过");
            }
        },
    }

    Ok(())
}
