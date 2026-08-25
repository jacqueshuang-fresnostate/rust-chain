//! 数据库迁移与首个管理员引导的独立可执行程序，部署流水线应在启动 API 服务之前先运行它。
//! 它把 `migrations` 目录在编译期嵌入二进制，因此运行时不依赖源码目录，可以直接在镜像里执行。
//! 迁移与引导都具备幂等性：已应用的 migration 会被跳过，库中已存在管理员时引导也只跳过而不覆盖。
//! 与 API 服务不同，这里只连接 MySQL，不接触 Mongo、Redis 或消息队列。

use anyhow::Context;
use exchange_api::bootstrap::{
    BootstrapAdminConfig, BootstrapAdminMode, BootstrapAdminOutcome, bootstrap_default_admin,
};
use sqlx::mysql::MySqlPoolOptions;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// 依次执行数据库结构迁移与默认管理员引导，任一步失败都带中文上下文向上返回并让进程以非零码退出。
/// 连接串只从 `DATABASE_URL` 读取，先尝试加载 `.env` 但忽略其缺失；连接池限制为单连接，避免迁移期间并发改表。
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
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .context("连接 MySQL 失败")?;

    MIGRATOR
        .run(&pool)
        .await
        .context("执行 SQLx migrations 失败")?;
    tracing::info!("数据库 migrations 已全部应用");

    let bootstrap_result = match BootstrapAdminMode::from_env() {
        Ok(BootstrapAdminMode::Disabled) => None,
        Ok(BootstrapAdminMode::CreateAdmin) => Some(match BootstrapAdminConfig::from_env() {
            Ok(config) => bootstrap_default_admin(&pool, &config).await,
            Err(error) => Err(error),
        }),
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
