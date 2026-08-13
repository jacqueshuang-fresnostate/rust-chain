//! MySQL 连接池装配：交易所所有强一致业务数据都落在这里，包括账户、订单、钱包余额与资金流水账本。
//! 池参数在本文件写死而非走配置，最大十条连接、获取连接最多等待五秒，超时会以数据库错误形式反馈给调用方。
//! 每条新连接都会先把会话时区固定为 UTC，保证结算、到期判定与审计时间不随数据库主机的时区设置漂移。

use crate::{config::Settings, error::AppResult};
use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use std::time::Duration;

/// 创建 MySQL 连接池，并在每条新连接上固定 UTC 时区，确保结算、到期与审计时间不受数据库主机时区影响。
/// 连接或会话初始化失败必须阻止该连接进入池中；本入口不执行迁移，也不提供内存降级。
pub async fn connect(settings: &Settings) -> AppResult<Pool<MySql>> {
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(|connection, _metadata| {
            Box::pin(async move {
                sqlx::query("SET time_zone = '+00:00'")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect(settings.exposed_database_url())
        .await?;

    Ok(pool)
}
