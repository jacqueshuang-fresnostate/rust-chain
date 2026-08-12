//! auth bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。

use crate::{
    error::AppResult,
    modules::auth::{
        ActiveCountryConfig, ActorType, AuthActor, NewAdminActor, NewAgentActor, NewUserActor,
        RefreshTokenRecord, StoredActorCredential, StoredProjectRefreshToken, StoredRefreshToken,
    },
};
use axum::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};

#[async_trait]
/// 认证主体仓储端口，统一承载用户、管理员和代理账号的凭据及登录状态持久化。
///
/// 各方法可由一个或多个独立存储语句组成，本端口不承诺方法内或跨方法事务；
/// 账号创建、失败计数、登录记录与令牌签发的部分成功边界由应用或服务层显式处理。
pub trait AuthRepository: Clone + Send + Sync + 'static {
    /// 单独持久化普通用户凭据并返回主体标识；不与后续令牌签发共用事务。
    async fn create_user(&self, actor: NewUserActor) -> AppResult<AuthActor>;
    /// 单独持久化管理员用户名、口令哈希与角色 ID；外键或唯一冲突必须上抛。
    async fn create_admin(&self, actor: NewAdminActor) -> AppResult<AuthActor>;
    /// 单独持久化代理后台凭据并绑定指定代理节点；不创建代理层级本身。
    async fn create_agent(&self, actor: NewAgentActor) -> AppResult<AuthActor>;
    /// 查询允许注册的国家配置；停用或禁止注册的国家必须按不存在处理。
    async fn find_registration_country(
        &self,
        country_code: &str,
    ) -> AppResult<Option<ActiveCountryConfig>>;
    /// 按规范化邮箱读取用户凭据，未命中时不得泄露额外账号状态。
    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<StoredActorCredential>>;
    /// 按规范化手机号读取用户凭据，未命中时返回 `None`。
    async fn find_user_by_phone(&self, phone: &str) -> AppResult<Option<StoredActorCredential>>;
    /// 按规范化用户名读取用户凭据，仅供已启用用户名登录的用例调用。
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    /// 按管理员用户名读取凭据及状态，不在仓储层执行密码比较。
    async fn find_admin_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    /// 判断系统是否已经存在管理员，用于限制首次管理员引导注册入口。
    async fn has_any_admin(&self) -> AppResult<bool>;
    /// 按代理后台用户名读取凭据，并保留代理主体类型供统一鉴权。
    async fn find_agent_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    /// 重新读取主体的有效状态；停用主体或失效代理层级返回 `None`。
    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>>;
    /// 在密码验证成功后记录适配器支持的登录元数据；MySQL 实现当前只更新代理管理员时间，
    /// 用户和平台管理员为成功空操作。实际 SQL 失败须使本次登录整体失败。
    async fn record_login(&self, actor: &AuthActor) -> AppResult<()>;
    /// 持久化 JWT 模式的刷新令牌摘要，严禁保存原始刷新令牌。
    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()>;
    /// 按摘要读取尚未过期且未撤销的 JWT 刷新令牌记录。
    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>>;
    /// 返回仍在生效的锁定截止时间，已过期的锁定视为不存在。
    async fn find_login_lockout(
        &self,
        actor_type: ActorType,
        identifier: &str,
    ) -> AppResult<Option<DateTime<Utc>>>;
    /// 累加失败次数并在触发阈值时返回锁定截止时间。
    async fn record_login_failure(
        &self,
        actor_type: ActorType,
        identifier: &str,
    ) -> AppResult<Option<DateTime<Utc>>>;
    /// 在认证成功后清除同一主体、同一标识的失败计数和临时锁定。
    async fn clear_login_failures(&self, actor_type: ActorType, identifier: &str) -> AppResult<()>;
}

#[async_trait]
/// 项目刷新令牌存储端口，隔离 Sa-Token 模式使用的 Redis 会话实现。
///
/// 原始令牌只允许在适配器内部转换为不可逆键；读取必须校验过期时间，撤销范围以
/// 适配器已登记的主体索引为准。端口只定义单次调用结果，不承诺记录键与索引键跨命令原子性。
pub trait ProjectRefreshTokenRepository: Send + Sync + 'static {
    /// 保存一枚项目刷新令牌及主体快照，并按记录中的有效期设置存储 TTL。
    async fn store_project_refresh_token(&self, token: StoredProjectRefreshToken) -> AppResult<()>;

    /// 读取仍在有效期内的项目刷新令牌；不存在、过期或内容损坏时不得返回有效主体。
    async fn find_project_refresh_token(
        &self,
        refresh_token: &str,
        now: DateTime<Utc>,
    ) -> AppResult<Option<RefreshTokenRecord>>;

    /// 撤销指定认证主体登记的全部项目刷新令牌，重复调用应保持幂等。
    async fn revoke_actor_refresh_tokens(&self, actor: &AuthActor) -> AppResult<()>;
}
