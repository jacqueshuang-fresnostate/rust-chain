use crate::{config::Settings, error::AppResult};
use redis::{Client, aio::ConnectionManager};

pub async fn connect(settings: &Settings) -> AppResult<ConnectionManager> {
    let client = Client::open(settings.exposed_redis_url())?;
    Ok(ConnectionManager::new(client).await?)
}
