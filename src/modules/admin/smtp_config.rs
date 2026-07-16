use crate::{
    error::AppResult,
    infra::email::{EmailSender, SmtpEmailConfig},
    modules::admin::application::{
        create_admin_smtp_config, get_admin_smtp_config, list_admin_smtp_configs,
        load_enabled_admin_smtp_config, save_admin_smtp_config, save_admin_smtp_delivery_settings,
        send_admin_smtp_test_with_sender, update_admin_smtp_config,
    },
};
use sqlx::{MySql, Pool};

pub use crate::modules::admin::presentation::{
    SaveSmtpConfigRequest, SaveSmtpDeliverySettingsRequest, SendSmtpTestRequest,
    SendSmtpTestResponse, SmtpConfigListResponse, SmtpConfigResponse, SmtpDeliverySettingsResponse,
};

pub async fn load_smtp_config(pool: &Pool<MySql>) -> AppResult<Option<SmtpConfigResponse>> {
    get_admin_smtp_config(Some(pool.clone())).await
}

pub async fn list_smtp_configs(pool: &Pool<MySql>) -> AppResult<SmtpConfigListResponse> {
    list_admin_smtp_configs(Some(pool.clone())).await
}

pub async fn save_smtp_delivery_settings(
    pool: &Pool<MySql>,
    admin_id: u64,
    request: SaveSmtpDeliverySettingsRequest,
) -> AppResult<SmtpDeliverySettingsResponse> {
    save_admin_smtp_delivery_settings(Some(pool.clone()), admin_id, request).await
}

pub async fn save_smtp_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    save_admin_smtp_config(Some(pool.clone()), admin_id, key, request).await
}

pub async fn create_smtp_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    create_admin_smtp_config(Some(pool.clone()), admin_id, key, request).await
}

pub async fn update_smtp_config(
    pool: &Pool<MySql>,
    admin_id: u64,
    config_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    update_admin_smtp_config(Some(pool.clone()), admin_id, config_id, key, request).await
}

pub async fn send_smtp_test_email(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    sender: &dyn EmailSender,
    request: SendSmtpTestRequest,
) -> AppResult<SendSmtpTestResponse> {
    send_admin_smtp_test_with_sender(pool, admin_id, key, sender, request).await
}

pub async fn load_enabled_smtp_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_enabled_admin_smtp_config(pool, key).await
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_smtp_config_tests.rs"]
mod tests;
