use anyhow::Context;
use exchange_api::bootstrap::{
    BootstrapAdminConfig, BootstrapAdminOutcome, bootstrap_default_admin,
};
use sqlx::mysql::MySqlPoolOptions;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

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

    let bootstrap_result = match BootstrapAdminConfig::from_env() {
        Ok(config) => bootstrap_default_admin(&pool, &config).await,
        Err(error) => Err(error),
    };
    pool.close().await;

    match bootstrap_result.context("初始化默认管理员失败")? {
        BootstrapAdminOutcome::Created => {
            tracing::info!("默认管理员已创建");
        }
        BootstrapAdminOutcome::SkippedExistingAdmin => {
            tracing::info!("数据库已存在管理员，默认管理员引导已跳过");
        }
    }

    Ok(())
}
