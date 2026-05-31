use crate::{config::Settings, error::AppResult};
use sqlx::{MySql, Pool, mysql::MySqlPoolOptions};
use std::time::Duration;

pub async fn connect(settings: &Settings) -> AppResult<Pool<MySql>> {
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(settings.exposed_database_url())
        .await?;

    Ok(pool)
}
