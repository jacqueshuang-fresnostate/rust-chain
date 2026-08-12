//! auth bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。

use crate::{
    architecture::ServiceLayer,
    config::Settings,
    error::{AppError, AppResult},
    modules::{
        auth::{
            ACTIVE_STATUS, ActorType, AdminCredentials, AdminRegistration, AgentCredentials,
            AgentRegistration, AuthActor, IssuedTokens, NewAdminActor, NewAgentActor, NewUserActor,
            ProjectRefreshTokenRepository, RefreshTokenRecord, StoredActorCredential,
            StoredProjectRefreshToken, StoredRefreshToken, TokenScope, UserCredentials,
            decode_claims,
            domain::{login_failure_key, login_locked_error},
            hash_password, hash_refresh_token, issue_token, map_sa_token_error, normalize_username,
            repository::AuthRepository,
            verify_password,
        },
        countries::normalize_country_code,
    },
};
use chrono::{DateTime, Duration, Utc};
use sa_token_core::SaTokenManager;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
/// 认证领域服务，统一编排三类主体的凭据验证、锁定策略和令牌签发。
///
/// 服务通过注入的认证仓储、Sa-Token 管理器和项目刷新令牌端口执行外部 I/O；
/// 不直接依赖 SQLx 或 Redis SDK，但登录、锁定、会话签发仍可因仓储或会话后端失败。
pub struct AuthService<R> {
    repository: R,
    settings: Arc<Settings>,
    auth_manager: Option<Arc<SaTokenManager>>,
    project_refresh_tokens: Option<Arc<dyn ProjectRefreshTokenRepository>>,
}

impl<R> ServiceLayer for AuthService<R> {}

impl<R: AuthRepository> AuthService<R> {
    /// 组装认证服务依赖；未提供项目刷新令牌端口时，Sa-Token 刷新记录回落到主仓储。
    ///
    /// 调用方负责保证 `auth_manager` 与刷新令牌端口配置一致；本构造函数不连接外部服务。
    pub fn new(
        repository: R,
        settings: Arc<Settings>,
        auth_manager: Option<Arc<SaTokenManager>>,
        project_refresh_tokens: Option<Arc<dyn ProjectRefreshTokenRepository>>,
    ) -> Self {
        Self {
            repository,
            settings,
            auth_manager,
            project_refresh_tokens,
        }
    }

    /// 校验国家与登录标识后先持久化用户，再签发首组访问与刷新令牌。
    ///
    /// 用户写入与会话签发不在同一事务；令牌后端失败时账号可能已创建，本方法不回滚或删除该账号。
    pub async fn register_user(&self, credentials: UserCredentials) -> AppResult<IssuedTokens> {
        let password = required_string(credentials.password, "password")?;
        let country_code = required_string(credentials.country_code, "country_code")?;
        let country_code = normalize_country_code(&country_code)?;
        let country = self
            .repository
            .find_registration_country(&country_code)
            .await?
            .ok_or_else(|| {
                AppError::Validation("country_code is not available for registration".to_owned())
            })?;
        let (email, phone) = user_identifier(credentials.email, credentials.phone)?;
        let actor = self
            .repository
            .create_user(NewUserActor {
                email,
                phone,
                password_hash: hash_password(&password)?,
                country_code: country.country_code,
                preferred_locale: country.default_locale,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    /// 校验普通用户凭据、更新失败锁定/最近登录状态后签发新会话。
    ///
    /// 账号不存在、密码错误和停用状态统一返回未授权；令牌持久化失败时，
    /// 前面已清除的失败计数和已记录的登录信息不会回滚。
    pub async fn login_user(&self, credentials: UserCredentials) -> AppResult<IssuedTokens> {
        let actor = self.verify_user_credentials(credentials).await?;
        self.issue_tokens(actor).await
    }

    /// 只验证普通用户凭据并返回活跃主体，不创建任何访问令牌。
    ///
    /// 该入口供二次验证流程复用，仍会执行失败计数、临时锁定和成功登录记录副作用。
    pub async fn verify_user_credentials(
        &self,
        credentials: UserCredentials,
    ) -> AppResult<AuthActor> {
        let password = required_string(credentials.password, "password")?;
        let identifier = user_login_identifier(
            credentials.email,
            credentials.phone,
            credentials.username,
            credentials.username_login_enabled,
        )?;
        let (identifier, stored) = match identifier {
            UserLoginIdentifier::Email(email) => {
                let stored = self.repository.find_user_by_email(&email).await?;
                (email, stored)
            }
            UserLoginIdentifier::Phone(phone) => {
                let stored = self.repository.find_user_by_phone(&phone).await?;
                (phone, stored)
            }
            UserLoginIdentifier::Username(username) => {
                let stored = self.repository.find_user_by_username(&username).await?;
                (username, stored)
            }
        };

        self.verify_with_lockout(ActorType::User, &identifier, stored, &password)
            .await
    }

    /// 管理员表非空时先验证请求主体仍为活跃管理员，表为空时走首管理员引导。
    ///
    /// “查空表—插入”未被同一事务或表锁包围，并发引导由唯一/外键约束决定结果；
    /// 管理员插入后令牌签发失败不会删除已创建账号。
    pub async fn register_admin(
        &self,
        requester_subject: Option<&str>,
        registration: AdminRegistration,
    ) -> AppResult<IssuedTokens> {
        // 首个管理员通过空表引导注册，此后必须由现有活跃管理员创建新管理员。
        if self.repository.has_any_admin().await? {
            let admin_id = requester_subject
                .ok_or(AppError::Unauthorized)?
                .strip_prefix("admin:")
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or(AppError::Unauthorized)?;
            self.repository
                .find_active_actor(&AuthActor::new(ActorType::Admin, admin_id, None))
                .await?
                .ok_or(AppError::Forbidden)?;
        }
        let username = required_string(registration.username, "username")?;
        let password = required_string(registration.password, "password")?;
        let role_id = registration
            .role_id
            .ok_or_else(|| AppError::Validation("role_id is required".to_owned()))?;
        let actor = self
            .repository
            .create_admin(NewAdminActor {
                username,
                password_hash: hash_password(&password)?,
                role_id,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    /// 验证管理员用户名和密码，并执行统一登录失败锁定策略。
    ///
    /// 本方法不签发令牌，便于后台二次验证在确认 TOTP 后再完成会话创建。
    pub async fn verify_admin_credentials(
        &self,
        credentials: AdminCredentials,
    ) -> AppResult<AuthActor> {
        let username = required_string(credentials.username, "username")?;
        let password = required_string(credentials.password, "password")?;
        let stored = self.repository.find_admin_by_username(&username).await?;

        self.verify_with_lockout(ActorType::Admin, &username, stored, &password)
            .await
    }

    /// 先将代理后台凭据绑定到已有 `agent_id`，再签发独立 `agent` 作用域会话。
    ///
    /// 凭据插入与会话签发分开提交；令牌后端失败时代理管理员记录仍可已存在。
    pub async fn register_agent(&self, registration: AgentRegistration) -> AppResult<IssuedTokens> {
        let username = required_string(registration.username, "username")?;
        let password = required_string(registration.password, "password")?;
        let agent_id = registration
            .agent_id
            .ok_or_else(|| AppError::Validation("agent_id is required".to_owned()))?;
        let actor = self
            .repository
            .create_agent(NewAgentActor {
                username,
                password_hash: hash_password(&password)?,
                agent_id,
            })
            .await?;

        self.issue_tokens(actor).await
    }

    /// 校验代理后台凭据并签发代理作用域令牌。
    ///
    /// 停用账号与错误密码走同一未授权分支，并统一累计登录失败次数。
    pub async fn login_agent(&self, credentials: AgentCredentials) -> AppResult<IssuedTokens> {
        let username = required_string(credentials.username, "username")?;
        let password = required_string(credentials.password, "password")?;
        let stored = self.repository.find_agent_by_username(&username).await?;
        let actor = self
            .verify_with_lockout(ActorType::Agent, &username, stored, &password)
            .await?;

        self.issue_tokens(actor).await
    }

    /// 使用刷新令牌校验作用域与主体活跃状态，成功后另外签发一组访问/刷新令牌。
    ///
    /// 当前实现不消费、不撤销传入的刷新令牌；在其过期或被主体级撤销前，重复提交可再签发会话。
    /// 过期、已撤销、作用域不匹配或主体停用统一按未授权处理，存储失败上抛。
    pub async fn refresh(
        &self,
        refresh_token: Option<String>,
        expected_scope: TokenScope,
    ) -> AppResult<IssuedTokens> {
        let refresh_token = required_string(refresh_token, "refresh_token")?;
        if self.auth_manager.is_some() {
            return self.refresh_sa_token(&refresh_token, expected_scope).await;
        }

        let claims = decode_claims(&self.settings, &refresh_token)?;
        if claims.scope != expected_scope {
            return Err(AppError::Unauthorized);
        }

        let token_hash = hash_refresh_token(&refresh_token)?;
        let stored = self
            .repository
            .find_refresh_token(&token_hash, Utc::now().naive_utc())
            .await?
            .ok_or(AppError::Unauthorized)?;
        let actor = AuthActor::new(stored.actor_type, stored.actor_id, stored.user_id);

        if stored.actor_type.scope() != claims.scope || actor.subject() != claims.sub {
            return Err(AppError::Unauthorized);
        }

        let actor = self
            .repository
            .find_active_actor(&actor)
            .await?
            .ok_or(AppError::Unauthorized)?;

        self.issue_tokens(actor).await
    }

    async fn refresh_sa_token(
        &self,
        refresh_token: &str,
        expected_scope: TokenScope,
    ) -> AppResult<IssuedTokens> {
        let stored = self
            .find_project_refresh_token(refresh_token)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if stored.scope != expected_scope || stored.actor_type.scope() != expected_scope {
            return Err(AppError::Unauthorized);
        }

        let actor = AuthActor::new(stored.actor_type, stored.actor_id, stored.user_id);
        let actor = self
            .repository
            .find_active_actor(&actor)
            .await?
            .ok_or(AppError::Unauthorized)?;

        self.issue_tokens(actor).await
    }

    /// 用户、管理员、代理共用同一条密码校验入口，统一执行失败计数与临时锁定。
    async fn verify_with_lockout(
        &self,
        actor_type: ActorType,
        identifier: &str,
        stored: Option<StoredActorCredential>,
        password: &str,
    ) -> AppResult<AuthActor> {
        let key = login_failure_key(identifier);
        if let Some(locked_until) = self.repository.find_login_lockout(actor_type, &key).await? {
            return Err(login_locked(locked_until));
        }

        let mut authenticated = None;
        if let Some(stored) = stored
            && stored.status == ACTIVE_STATUS
            && verify_password(&stored.password_hash, password)?
        {
            authenticated = Some(stored.actor);
        }

        let Some(actor) = authenticated else {
            // 账号不存在与密码错误共用同一条失败分支，既计入锁定又不泄露账号是否存在。
            let locked_until = self
                .repository
                .record_login_failure(actor_type, &key)
                .await?;
            return Err(locked_until.map_or(AppError::Unauthorized, login_locked));
        };

        self.repository
            .clear_login_failures(actor_type, &key)
            .await?;
        self.repository.record_login(&actor).await?;
        Ok(actor)
    }

    /// 为已完成口令/2FA 等额外校验的主体按当前运行模式签发会话。
    ///
    /// 本方法不重新查询主体状态；调用方须传入刚验证的权威主体。Sa-Token 模式先创建访问会话、
    /// 后写刷新令牌记录；后一步失败时访问会话可已存在，本方法不做补偿登出。
    pub async fn issue_tokens_for_actor(&self, actor: AuthActor) -> AppResult<IssuedTokens> {
        self.issue_tokens(actor).await
    }

    async fn issue_tokens(&self, actor: AuthActor) -> AppResult<IssuedTokens> {
        if let Some(manager) = &self.auth_manager {
            return self.issue_sa_tokens(manager, actor).await;
        }

        let scope = actor.actor_type.scope();
        let subject = actor.subject();
        let access_token = issue_token(
            &self.settings,
            subject.clone(),
            scope,
            self.settings.jwt_access_ttl_seconds,
        )?;
        let refresh_token = issue_token(
            &self.settings,
            subject,
            scope,
            self.settings.jwt_refresh_ttl_seconds,
        )?;
        let token_hash = hash_refresh_token(&refresh_token)?;
        let expires_at = Utc::now().naive_utc()
            + Duration::seconds(self.settings.jwt_refresh_ttl_seconds as i64);

        self.repository
            .store_refresh_token(StoredRefreshToken {
                actor_type: actor.actor_type,
                actor_id: actor.actor_id,
                user_id: actor.user_id,
                token_hash,
                expires_at,
            })
            .await?;

        Ok(IssuedTokens {
            access_token,
            refresh_token,
            token_type: "Bearer",
            scope,
        })
    }

    async fn issue_sa_tokens(
        &self,
        manager: &SaTokenManager,
        actor: AuthActor,
    ) -> AppResult<IssuedTokens> {
        let scope = actor.actor_type.scope();
        let access_token = manager
            .login_with_options(
                actor.actor_id.to_string(),
                Some(scope.as_login_type().to_owned()),
                Some("api".to_owned()),
                Some(json!({
                    "actor_type": actor.actor_type.as_str(),
                    "actor_id": actor.actor_id,
                    "user_id": actor.user_id,
                })),
                None,
                None,
            )
            .await
            .map_err(map_sa_token_error)?;
        let refresh_token = generate_refresh_token();
        let expires_at =
            Utc::now() + Duration::seconds(self.settings.jwt_refresh_ttl_seconds as i64);
        let record = StoredProjectRefreshToken {
            refresh_token: refresh_token.clone(),
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            user_id: actor.user_id,
            scope,
            expires_at,
        };

        if let Some(project_refresh_tokens) = &self.project_refresh_tokens {
            project_refresh_tokens
                .store_project_refresh_token(record)
                .await?;
        } else {
            self.repository
                .store_refresh_token(StoredRefreshToken {
                    actor_type: actor.actor_type,
                    actor_id: actor.actor_id,
                    user_id: actor.user_id,
                    token_hash: hash_refresh_token(&refresh_token)?,
                    expires_at: Utc::now().naive_utc()
                        + Duration::seconds(self.settings.jwt_refresh_ttl_seconds as i64),
                })
                .await?;
        }

        Ok(IssuedTokens {
            access_token: access_token.to_string(),
            refresh_token,
            token_type: "Bearer",
            scope,
        })
    }

    async fn find_project_refresh_token(
        &self,
        refresh_token: &str,
    ) -> AppResult<Option<RefreshTokenRecord>> {
        if let Some(project_refresh_tokens) = &self.project_refresh_tokens {
            return project_refresh_tokens
                .find_project_refresh_token(refresh_token, Utc::now())
                .await;
        }

        self.repository
            .find_refresh_token(&hash_refresh_token(refresh_token)?, Utc::now().naive_utc())
            .await
    }
}

fn login_locked(locked_until: DateTime<Utc>) -> AppError {
    login_locked_error((locked_until - Utc::now()).num_seconds())
}

fn generate_refresh_token() -> String {
    format!("refresh_{}", Uuid::now_v7().simple())
}

fn user_identifier(
    email: Option<String>,
    phone: Option<String>,
) -> AppResult<(Option<String>, Option<String>)> {
    let email = optional_string(email);
    let phone = optional_string(phone);

    if email.is_none() && phone.is_none() {
        Err(AppError::Validation(
            "email or phone is required".to_owned(),
        ))
    } else {
        Ok((email, phone))
    }
}

enum UserLoginIdentifier {
    Email(String),
    Phone(String),
    Username(String),
}

fn user_login_identifier(
    email: Option<String>,
    phone: Option<String>,
    username: Option<String>,
    username_login_enabled: bool,
) -> AppResult<UserLoginIdentifier> {
    if let Some(email) = optional_string(email) {
        return Ok(UserLoginIdentifier::Email(email));
    }
    if let Some(phone) = optional_string(phone) {
        return Ok(UserLoginIdentifier::Phone(phone));
    }
    if let Some(username) = optional_string(username) {
        if !username_login_enabled {
            return Err(AppError::Validation(
                "username login is disabled".to_owned(),
            ));
        }
        return Ok(UserLoginIdentifier::Username(normalize_username(
            &username,
        )?));
    }

    Err(AppError::Validation(
        "email, phone or username is required".to_owned(),
    ))
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}
