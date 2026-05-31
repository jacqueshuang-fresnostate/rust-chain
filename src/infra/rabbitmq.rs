use crate::{config::Settings, error::AppResult};
use lapin::{Connection, ConnectionProperties};

pub async fn connect(settings: &Settings) -> AppResult<Connection> {
    Ok(Connection::connect(
        settings.exposed_rabbitmq_url(),
        ConnectionProperties::default(),
    )
    .await?)
}
