//! auth bounded context HTTP routes.
//!
//! 路由层：把认证限界上下文的用例挂载到用户端、管理后台和代理后台三套 Axum 路由表上。
//! 三张表各自独立注册，相同路径在不同表下由挂载前缀区分，对应三种互不通用的令牌作用域。
//! 本文件只负责请求提取、调用用例和封装响应，不含任何校验、事务或业务分支：
//! 人机校验、口令比对、失败锁定、二次验证挑战与令牌签发全部由应用层完成，改动这里不会改变安全语义。
//! 除后台四个二次验证管理入口通过 `AdminAuth` 提取器要求已登录管理员外，其余路径均为匿名可达，
//! 因此账号枚举防护、发送冷却与爆破限制都必须落在应用层，路由层不提供额外保护。

use crate::{
    error::AppResult,
    modules::auth::{
        AdminAuth, AdminCredentials, AdminRegistration, AgentCredentials, TokenScope,
        application::{
            confirm_admin_two_factor, confirm_login_two_factor_setup_and_issue_tokens,
            disable_admin_two_factor, get_admin_two_factor_status, load_login_config,
            load_register_config, login_admin_with_turnstile, login_agent_with_turnstile,
            login_user_with_optional_two_factor_response, mysql_pool, refresh_actor_tokens,
            register_admin_actor, register_user_with_email_code_response,
            reject_agent_registration, reset_login_two_factor_with_email_code,
            reset_password_with_email_code, send_login_two_factor_reset_email_code,
            send_password_reset_email_code, send_registration_email_code, setup_admin_two_factor,
            setup_login_two_factor_challenge, verify_admin_login_two_factor,
            verify_login_two_factor_and_issue_tokens,
        },
        presentation::{
            AdminAuthRequest, AdminLoginResponse, AdminTwoFactorCodeRequest,
            AdminTwoFactorSetupResponse, AdminTwoFactorStatusResponse, AgentAuthRequest,
            LoginConfigResponse, LoginTransportContext, LoginTwoFactorCodeResponse,
            LoginTwoFactorRequest, LoginTwoFactorResetCodeRequest, LoginTwoFactorResetRequest,
            LoginTwoFactorResetResponse, LoginTwoFactorSetupConfirmRequest,
            LoginTwoFactorSetupRequest, LoginTwoFactorSetupResponse, PasswordResetCodeRequest,
            PasswordResetCodeResponse, PasswordResetRequest, PasswordResetResponse, RefreshRequest,
            RegisterConfigResponse, RegisterEmailCodeRequest, RegisterEmailCodeResponse,
            TokenResponse, UserAuthRequest, UserLoginResponse,
        },
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;

/// 组装用户端认证路由表，覆盖注册与登录配置查询、邮件验证码、注册、登录、二次验证和令牌刷新。
/// 表中所有入口都不经过身份提取器，属于匿名可达：防止账号枚举与爆破完全依靠应用层自带的
/// 失败锁定、发送冷却和统一错误响应，路由层不再叠加限制。
/// 二次验证被拆成校验、首次绑定、绑定确认和重置四条独立路径，它们共享登录阶段签发的挑战标识，
/// 因此挑战本身的有效期和消费语义决定了这组接口的重放边界。
pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register/config", get(get_register_config))
        .route("/auth/login/config", get(get_login_config))
        .route("/auth/register/email-code", post(send_register_email_code))
        .route("/auth/register", post(user_register))
        .route("/auth/password/reset-code", post(send_password_reset_code))
        .route("/auth/password/reset", post(reset_password))
        .route("/auth/login", post(user_login))
        .route("/auth/login/2fa", post(user_login_two_factor))
        .route("/auth/login/2fa/setup", post(user_login_two_factor_setup))
        .route(
            "/auth/login/2fa/setup/confirm",
            post(user_login_two_factor_setup_confirm),
        )
        .route(
            "/auth/login/2fa/reset-code",
            post(send_login_two_factor_reset_code),
        )
        .route("/auth/login/2fa/reset", post(reset_login_two_factor))
        .route("/auth/refresh", post(user_refresh))
}

/// 组装管理后台认证路由表，包含管理员注册、登录、登录二次验证、令牌刷新以及四个二次验证管理入口。
/// 注册入口只在系统尚无任何管理员时允许匿名调用，此后必须携带有效的管理员令牌，该判断在应用层完成。
/// 四个二次验证管理入口通过提取器要求已登录管理员，是本表中仅有的需要鉴权的路径。
/// 登录配置查询与用户端复用同一个处理函数，响应内容不随调用方身份变化，不会泄露后台专有策略。
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(admin_register))
        .route("/auth/login/config", get(get_login_config))
        .route("/auth/login", post(admin_login))
        .route("/auth/login/2fa", post(admin_login_two_factor))
        .route("/auth/2fa", get(admin_two_factor_status))
        .route("/auth/2fa/setup", post(admin_two_factor_setup))
        .route("/auth/2fa/confirm", post(admin_two_factor_confirm))
        .route("/auth/2fa/disable", post(admin_two_factor_disable))
        .route("/auth/refresh", post(admin_refresh))
}

/// 组装代理后台认证路由表，只开放注册、登录和刷新三条路径，其中注册入口固定返回拒绝。
/// 代理账号必须由平台后台按审核流程创建，这里保留注册路径只是为了给出明确的禁止访问答复，
/// 避免调用方把缺失路径误当作临时故障反复重试。登录与刷新签发的是独立的代理作用域令牌，
/// 无法用于用户端或管理后台的任何接口。
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(agent_register))
        .route("/auth/login", post(agent_login))
        .route("/auth/refresh", post(agent_refresh))
}

/// 返回注册页所需的两个开关：邮件验证码是否必填、邀请码是否必填，供前端决定展示哪些输入项。
/// 这里只是把应用层读到的安全策略原样透出，前端隐藏某个字段不会放松后端校验，
/// 真正的强制判定仍在注册用例中执行。响应不含任何账号信息或密钥，可匿名访问。
async fn get_register_config(
    State(state): State<AppState>,
) -> AppResult<Json<RegisterConfigResponse>> {
    let config = load_register_config(&mysql_pool(&state)?).await?;

    Ok(Json(RegisterConfigResponse {
        email_code_required: config.email_code_required,
        invite_code_required: config.invite_code_required,
    }))
}

/// 返回登录页所需的开关：是否允许用户名登录、Turnstile 是否启用，以及启用时的站点公钥。
/// 站点公钥本就是要嵌进前端页面的公开值，服务端密钥和站点校验接口地址不会出现在响应中。
/// 该接口被用户端与管理后台共用且匿名可访问，响应内容不随调用方身份变化。
async fn get_login_config(State(state): State<AppState>) -> AppResult<Json<LoginConfigResponse>> {
    let config = load_login_config(&state).await?;

    Ok(Json(LoginConfigResponse {
        username_login_enabled: config.username_login_enabled,
        cf_turnstile_enabled: config.cf_turnstile_enabled,
        cf_turnstile_site_key: config.cf_turnstile_site_key,
    }))
}

/// 向待注册邮箱发送验证码，用例内部会拒绝已被占用的邮箱并施加发送冷却。
/// 响应只回传发送标志和按当前时刻算出的剩余有效秒数，秒数经下限夹取不会为负，验证码本身只出现在邮件里。
/// 邮箱已注册时用例返回冲突错误，因此该入口可以被用来判断某个邮箱是否已注册，
/// 对枚举的防护须依赖上游限流，路由层不做遮蔽。
async fn send_register_email_code(
    State(state): State<AppState>,
    Json(request): Json<RegisterEmailCodeRequest>,
) -> AppResult<Json<RegisterEmailCodeResponse>> {
    let pool = mysql_pool(&state)?;
    let expires_at = send_registration_email_code(&state, &pool, request.email).await?;

    Ok(Json(RegisterEmailCodeResponse {
        sent: true,
        expires_in_seconds: (expires_at - Utc::now()).num_seconds().max(0),
    }))
}

/// 向已注册且完成邮箱验证的账号发送密码重置验证码，响应结构与注册验证码一致。
/// 用例对未注册邮箱返回校验错误而不是静默成功，因此该入口同样会暴露邮箱是否已注册。
/// 验证码只以哈希入库，接口返回的仅有发送标志与剩余有效秒数；重置码按用途隔离，
/// 与注册验证码互不通用，也各自独立计算发送冷却。
async fn send_password_reset_code(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetCodeRequest>,
) -> AppResult<Json<PasswordResetCodeResponse>> {
    let pool = mysql_pool(&state)?;
    let expires_at = send_password_reset_email_code(&state, &pool, request.email).await?;

    Ok(Json(PasswordResetCodeResponse {
        sent: true,
        expires_in_seconds: (expires_at - Utc::now()).num_seconds().max(0),
    }))
}

/// 用邮箱、验证码和新口令完成密码重置，成功后固定返回需要重新登录的标志。
/// 用例会消费验证码、更新口令哈希，并撤销该用户既有的刷新令牌与会话，因此旧令牌在返回后即失效。
/// 路由层不做任何回退：若会话撤销阶段失败，新口令已经生效而错误会原样上抛，客户端应按需重新登录。
/// 请求体中的明文口令只在用例内部散列使用，不会出现在响应或日志中。
async fn reset_password(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetRequest>,
) -> AppResult<Json<PasswordResetResponse>> {
    let pool = mysql_pool(&state)?;
    reset_password_with_email_code(&state, &pool, request.email, request.code, request.password)
        .await?;

    Ok(Json(PasswordResetResponse {
        reset: true,
        requires_relogin: true,
    }))
}

/// 完成邮件验证码注册并直接返回首组令牌，让用户注册后无需再走一次登录。
/// 用例在单个事务内锁国家配置、消费验证码、写入用户与邀请关系，事务提交后才签发令牌，
/// 因此响应成功即代表账号确实已经落库。验证码错误只提交试错计数并返回校验错误，不会创建账号。
/// 该入口匿名可达且不经过 Turnstile，防刷完全依赖验证码发送侧的冷却与试错上限。
async fn user_register(
    State(state): State<AppState>,
    Json(request): Json<UserAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens = register_user_with_email_code_response(&state, &pool, request).await?;

    Ok(Json(tokens))
}

/// 用户登录入口，先把请求头归一化成传输上下文以支撑 Turnstile 判定，再交由应用层编排后续流程。
/// 响应是三选一的联合体：直接返回令牌、返回需要二次验证的挑战，或返回需要首次绑定的挑战，
/// 三种结果共用同一个状态码，客户端必须按字段判别而不能只看 HTTP 状态。
/// 口令错误、账号不存在与账号停用统一返回未授权，无法据此区分账号是否存在；
/// 连续失败会按规范化后的登录标识累计计数并触发临时锁定。
async fn user_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UserAuthRequest>,
) -> AppResult<Json<UserLoginResponse>> {
    let transport = LoginTransportContext::from_headers(&headers);
    Ok(Json(
        login_user_with_optional_two_factor_response(&state, request, transport).await?,
    ))
}

/// 用用户作用域的刷新令牌换取新一组令牌，作用域在应用层校验，管理员或代理令牌会被拒绝。
/// 当前实现不消费传入的刷新令牌，在其自然过期或该主体会话被整体撤销之前，同一枚令牌可以反复兑换，
/// 因此客户端泄露刷新令牌等同于泄露了直至过期为止的会话。请求体缺少令牌时按校验错误处理。
async fn user_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = refresh_actor_tokens(&state, request.refresh_token, TokenScope::User).await?;

    Ok(Json(tokens.into()))
}

/// 提交登录挑战标识与动态码完成二次验证，通过后才签发真正的访问与刷新令牌。
/// 挑战由口令登录阶段创建并自带有效期，动态码错误不会消费挑战，但挑战自身的过期规则依旧生效。
/// 挑战的可用性检查与消费不是同一个原子操作，并发提交同一挑战存在竞态，本接口不提供严格的一次性保证。
/// 请求只接受挑战标识而不接受用户 ID，调用方无法借此为任意账号签发令牌。
async fn user_login_two_factor(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens = verify_login_two_factor_and_issue_tokens(
        &state,
        &pool,
        request.challenge_id,
        request.totp_code,
    )
    .await?;

    Ok(Json(tokens.into()))
}

/// 在首次绑定挑战下生成新的 TOTP 密钥，返回明文密钥、导入用的 otpauth 链接和挑战剩余秒数。
/// 密钥加密后先存为待确认值，此刻尚未启用；本步骤不消费挑战，客户端可在挑战有效期内重复调用换取新密钥。
/// 响应中的明文密钥是整条链路上唯一一次外发，只能直接渲染给当前用户，不得写入日志或前端持久化存储。
/// 已经绑定过二次验证的账号调用本接口会被拒绝，避免既有密钥被静默替换。
async fn user_login_two_factor_setup(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorSetupRequest>,
) -> AppResult<Json<LoginTwoFactorSetupResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(
        setup_login_two_factor_challenge(&state, &pool, request.setup_challenge_id).await?,
    ))
}

/// 用刚生成的待确认密钥提交一次动态码确认绑定，确认通过后启用二次验证并直接签发登录令牌。
/// 启用绑定、消费挑战与签发令牌分处不同语句：消费采用带条件的更新，竞争失败时二次验证可能已经启用；
/// 而消费成功后若令牌后端失败，挑战不会被恢复，用户需要重新走一遍口令登录。
/// 动态码错误既不改变绑定状态，也不消费挑战。
async fn user_login_two_factor_setup_confirm(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorSetupConfirmRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens = confirm_login_two_factor_setup_and_issue_tokens(
        &state,
        &pool,
        request.setup_challenge_id,
        request.totp_code,
    )
    .await?;

    Ok(Json(tokens.into()))
}

/// 在有效的登录挑战下，向该账号已验证的邮箱发送用于重置二次验证的专用验证码。
/// 这是第二因子的找回通道，因此挑战必须是口令登录阶段产生的登录挑战，首次绑定挑战走不通这里。
/// 验证码按用途隔离，与注册和密码重置的验证码互不通用，各自独立计算发送冷却。
/// 响应只含发送标志与剩余有效秒数；邮件在记录提交之后才发出，发送失败时冷却已经生效。
async fn send_login_two_factor_reset_code(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorResetCodeRequest>,
) -> AppResult<Json<LoginTwoFactorCodeResponse>> {
    let pool = mysql_pool(&state)?;
    let expires_at =
        send_login_two_factor_reset_email_code(&state, &pool, request.challenge_id).await?;

    Ok(Json(LoginTwoFactorCodeResponse {
        sent: true,
        expires_in_seconds: (expires_at - Utc::now()).num_seconds().max(0),
    }))
}

/// 用邮箱验证码清除账号已绑定的二次验证，成功后固定返回需要重新登录的标志。
/// 清除后账号回到未绑定状态，下次登录按策略重新走绑定流程，本接口自身不签发任何令牌。
/// 验证码消费、清除绑定与消费挑战分三步执行且不共享事务，中途失败会留下部分已完成的步骤。
/// 验证码错误只在其自身事务内累加试错次数，不会清除绑定。
async fn reset_login_two_factor(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorResetRequest>,
) -> AppResult<Json<LoginTwoFactorResetResponse>> {
    let pool = mysql_pool(&state)?;
    reset_login_two_factor_with_email_code(&pool, request.challenge_id, request.code).await?;

    Ok(Json(LoginTwoFactorResetResponse {
        reset: true,
        requires_relogin: true,
    }))
}

/// 创建管理员账号并返回首组后台令牌，请求头中是否携带 Bearer 令牌决定这次调用走引导还是常规路径。
/// 管理员表为空时允许匿名完成首次引导；一旦已存在管理员，就必须携带有效且仍然活跃的管理员令牌。
/// 该判断完全在应用层完成，路由层只原样透传请求头，因此不能指望中间件替代这道检查。
/// 引导路径的查空表与插入不在同一事务内，并发引导的最终结果由用户名唯一约束裁定。
async fn admin_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdminAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = register_admin_actor(
        &state,
        &headers,
        AdminRegistration {
            username: request.username,
            password: request.password,
            role_id: request.role_id,
        },
    )
    .await?;

    Ok(Json(tokens.into()))
}

/// 管理后台登录入口，先按运行时策略执行 Turnstile 校验，再进入口令与二次验证编排。
/// 响应是二选一的联合体：未绑定二次验证的管理员直接拿到令牌，已绑定的只拿到挑战标识和有效期。
/// 出于兼容存量账号的考虑，后台二次验证按账号自愿绑定，未绑定并不会阻断登录。
/// 口令错误与账号不存在共用同一条失败分支并计入锁定，后台账号不会因响应差异被枚举出来。
async fn admin_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdminAuthRequest>,
) -> AppResult<Json<AdminLoginResponse>> {
    let transport = LoginTransportContext::from_headers(&headers);
    let response = login_admin_with_turnstile(
        &state,
        AdminCredentials {
            username: request.username,
            password: request.password,
        },
        request.cf_turnstile_token,
        transport,
    )
    .await?;

    Ok(Json(response))
}

/// 管理员在登录挑战下提交动态码完成二次验证并换取后台令牌。
/// 动态码错误会累加该挑战的试错次数，次数用尽后挑战作废，攻击者必须重新通过口令登录才能再试。
/// 与用户端一样，挑战的可用性检查与消费不是同一个原子操作，并发提交存在竞态；
/// 消费成功之后若令牌签发失败，挑战不会被恢复。
async fn admin_login_two_factor(
    State(state): State<AppState>,
    Json(request): Json<LoginTwoFactorRequest>,
) -> AppResult<Json<TokenResponse>> {
    let pool = mysql_pool(&state)?;
    let tokens =
        verify_admin_login_two_factor(&state, &pool, request.challenge_id, request.totp_code)
            .await?;

    Ok(Json(tokens))
}

/// 查询当前登录管理员本人的二次验证绑定状态，只返回是否已启用这一个布尔值。
/// 管理员 ID 取自令牌声明而不是请求参数，因此无法用本接口查询其他管理员的绑定情况。
/// 响应不含密钥、待确认密钥和绑定时间，避免后台页面把第二因子的存在细节无意扩散出去。
async fn admin_two_factor_status(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
) -> AppResult<Json<AdminTwoFactorStatusResponse>> {
    let pool = mysql_pool(&state)?;
    let status = get_admin_two_factor_status(&pool, &claims.sub).await?;

    Ok(Json(status))
}

/// 为当前登录管理员生成新的 TOTP 密钥，返回明文密钥与用于扫码导入的 otpauth 链接。
/// 密钥加密后先存为待确认值，在确认之前二次验证并未启用，此时账号仍按原有方式登录。
/// 已经绑定的管理员调用会被拒绝，防止密钥被静默替换；状态检查与写入相互分离，并发调用可能互相覆盖待确认值。
/// 响应含明文密钥，只能回给发起请求的管理员本人，且不得写入任何日志或审计记录。
async fn admin_two_factor_setup(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
) -> AppResult<Json<AdminTwoFactorSetupResponse>> {
    let pool = mysql_pool(&state)?;
    let setup = setup_admin_two_factor(&state, &pool, &claims.sub).await?;

    Ok(Json(setup))
}

/// 管理员提交一次动态码确认绑定，校验通过后把待确认密钥正式启用为第二因子。
/// 尚未生成密钥或已经绑定都会被拒绝；动态码错误不改变任何状态，也不消耗待确认密钥。
/// 启用之后本接口不撤销该管理员已有的会话，此前签发的令牌在自然过期前仍然可用。
async fn admin_two_factor_confirm(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
    Json(request): Json<AdminTwoFactorCodeRequest>,
) -> AppResult<Json<AdminTwoFactorStatusResponse>> {
    let pool = mysql_pool(&state)?;
    let status = confirm_admin_two_factor(&state, &pool, &claims.sub, request.totp_code).await?;

    Ok(Json(status))
}

/// 解除当前登录管理员的二次验证绑定，必须先提交一次有效动态码才允许执行。
/// 这道校验正是本接口的安全边界：仅持有后台令牌不足以关闭第二因子，会话被劫持者仍过不了动态码这一关。
/// 动态码错误时绑定保持原样；清除操作幂等，并且不会顺带撤销该管理员的现有会话。
async fn admin_two_factor_disable(
    State(state): State<AppState>,
    AdminAuth(claims): AdminAuth,
    Json(request): Json<AdminTwoFactorCodeRequest>,
) -> AppResult<Json<AdminTwoFactorStatusResponse>> {
    let pool = mysql_pool(&state)?;
    let status = disable_admin_two_factor(&state, &pool, &claims.sub, request.totp_code).await?;

    Ok(Json(status))
}

/// 用管理员作用域的刷新令牌换取新一组后台令牌，作用域不符会被直接拒绝。
/// 与用户端刷新一样，传入的刷新令牌不会被消费，在其过期或该管理员会话被整体撤销之前可以重复使用。
/// 刷新过程会重新确认管理员账号仍然活跃，因此账号被停用后无法再靠旧刷新令牌延续后台会话。
async fn admin_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = refresh_actor_tokens(&state, request.refresh_token, TokenScope::Admin).await?;

    Ok(Json(tokens.into()))
}

/// 固定拒绝公开的代理注册请求，反序列化后的请求体不做任何使用，也完全不触碰数据库。
/// 代理账号只能由平台后台按审核与层级派生规则创建，保留这条路径是为了给出明确的禁止访问答复。
/// 因为不查询任何存储，本入口不会泄露用户名是否已被占用，也不产生失败计数或审计记录。
async fn agent_register(
    State(_state): State<AppState>,
    Json(_request): Json<AgentAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = reject_agent_registration()?;

    Ok(Json(tokens.into()))
}

/// 代理后台登录入口，先按运行时策略执行 Turnstile 校验，再验证代理管理员的用户名与口令。
/// 登录同时要求所属代理公司及其整条上级链路都处于活跃状态，任一层级被停用都会让下级后台无法登录，
/// 因而冻结上级无需逐个改写下级账号。校验通过后签发独立的代理作用域令牌，不能用于用户端或管理后台接口。
/// 账号停用与口令错误走同一条失败分支，并计入与其他主体一致的失败锁定策略。
async fn agent_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AgentAuthRequest>,
) -> AppResult<Json<TokenResponse>> {
    let transport = LoginTransportContext::from_headers(&headers);
    let tokens = login_agent_with_turnstile(
        &state,
        AgentCredentials {
            username: request.username,
            password: request.password,
        },
        request.cf_turnstile_token,
        transport,
    )
    .await?;

    Ok(Json(tokens.into()))
}

/// 用代理作用域的刷新令牌换取新一组代理令牌，其他作用域的刷新令牌一律拒绝。
/// 刷新会重新回查代理管理员及其代理层级是否仍然活跃，因此上级代理被冻结后下级无法再续期，
/// 但已经签发且尚未过期的访问令牌仍可继续使用到自然到期，需要立即阻断时必须撤销该主体的会话。
async fn agent_refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> AppResult<Json<TokenResponse>> {
    let tokens = refresh_actor_tokens(&state, request.refresh_token, TokenScope::Agent).await?;

    Ok(Json(tokens.into()))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_routes_tests.rs"]
mod tests;
