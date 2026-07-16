//! auth bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。

use crate::{
    error::AppResult,
    modules::auth::{
        ActiveCountryConfig, AuthActor, NewAdminActor, NewAgentActor, NewUserActor,
        RefreshTokenRecord, StoredActorCredential, StoredRefreshToken,
    },
};
use axum::async_trait;
use chrono::NaiveDateTime;

#[async_trait]
pub trait AuthRepository: Clone + Send + Sync + 'static {
    async fn create_user(&self, actor: NewUserActor) -> AppResult<AuthActor>;
    async fn create_admin(&self, actor: NewAdminActor) -> AppResult<AuthActor>;
    async fn create_agent(&self, actor: NewAgentActor) -> AppResult<AuthActor>;
    async fn find_registration_country(
        &self,
        country_code: &str,
    ) -> AppResult<Option<ActiveCountryConfig>>;
    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<StoredActorCredential>>;
    async fn find_user_by_phone(&self, phone: &str) -> AppResult<Option<StoredActorCredential>>;
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    async fn find_admin_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    async fn find_agent_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>>;
    async fn record_login(&self, actor: &AuthActor) -> AppResult<()>;
    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()>;
    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>>;
}
