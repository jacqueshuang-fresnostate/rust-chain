//! user bounded context HTTP routing layer.
//!
//! 传输层：声明用户自服务的 HTTP 路由表，把请求体解出的参数转交给 application 层用例。
//! 本文件覆盖 `/user/*` 与 `/referral/*` 两组前缀，包含资料与头像、用户名、KYC 提交与查询、
//! 二次验证、第三方绑定、邮箱绑定、登录密码与资金密码，以及邀请码相关端点。
//! 权限边界统一为用户自服务：每个处理器都通过 `UserAuth` 提取器要求携带有效的用户访问令牌，
//! 再由 `user_id_from_subject` 从令牌主体解析出用户 ID。
//! 关键约束是操作对象只能来自令牌，任何端点都不接受请求体或路径参数指定的用户 ID，
//! 因此不存在用户之间越权读写的入口；管理员侧的 KYC 审核、策略配置等能力在 admin 上下文另行暴露。
//! 本层不承载业务规则：不做校验、不开事务、不写审计，只负责依赖装配、参数搬运与 JSON 序列化，
//! 所有校验与失败语义由被调用的 application 用例定义并以 `AppError` 形式向上冒泡。

use crate::{
    error::AppResult,
    modules::user::service::user_id_from_subject,
    modules::user::{
        application::{
            bind_user_email, bind_user_referral_code, bind_user_third_party_account,
            change_user_fund_password, change_user_password, confirm_user_two_factor,
            create_user_fund_password, get_user_kyc_status, get_user_profile,
            get_user_referral_code, get_user_third_party_bindings, get_user_two_factor_status,
            list_user_invites, reset_user_fund_password, reset_user_two_factor_with_email_code,
            send_user_email_bind_code, send_user_fund_password_reset_code,
            send_user_two_factor_reset_code, setup_user_two_factor, submit_user_kyc_submission,
            update_user_login_two_factor, update_user_username, upload_user_avatar,
        },
        presentation::{
            BindEmailCodeRequest, BindEmailCodeResponse, BindEmailRequest, BindEmailResponse,
            BindReferralCodeRequest, BindThirdPartyAccountRequest, ChangeFundPasswordRequest,
            ChangePasswordRequest, ConfirmTwoFactorRequest, CreateFundPasswordRequest,
            FundPasswordResponse, MyInvitesResponse, ReferralBindingResponse, ReferralCodeResponse,
            ResetFundPasswordRequest, ResetTwoFactorRequest, SetupTwoFactorResponse,
            ThirdPartyBindingStatusResponse, TokenResponse, UpdateLoginTwoFactorRequest,
            UpdateUsernameRequest, UpdateUsernameResponse, UserAvatarResponse, UserProfileResponse,
            UserTwoFactorStatusResponse,
        },
    },
    modules::{
        admin::{presentation::multipart_file_input, service::MAX_UPLOAD_BODY_SIZE_BYTES},
        auth::UserAuth,
        kyc::{KycStatusResponse, KycSubmissionResponse, SubmitKycRequest},
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, State},
    routing::{get, patch, post},
};

/// 从 HTTP 状态中取得用户资料与安全用例使用的数据库连接池。
///
/// 本函数仅完成传输层依赖装配，不包含用户业务规则；缺失连接池时返回稳定内部错误。
fn mysql_pool(state: &AppState) -> AppResult<crate::state::MySqlPool> {
    state.mysql.clone().ok_or_else(|| {
        crate::error::AppError::Internal("mysql pool is not configured for user routes".to_owned())
    })
}

/// 装配用户自服务的全部路由并返回可挂载到应用根路由的子路由器。
/// 路径分两组：账号相关集中在 `/user/*`，邀请与推广相关放在 `/referral/*`。
/// 方法语义遵循同一约定：`get` 用于只读查询，`post` 用于创建或触发一次性动作（发码、提交、重置），
/// `patch` 用于修改既有资源（用户名、登录密码、资金密码、登录二次验证开关）。
/// 三条路径复用同一 URL 承载两个方法：第三方绑定的查询与新增、资金密码的创建与修改，
/// 由 HTTP 方法区分意图，避免为语义相近的操作再造路径。
/// 头像上传是唯一带请求体大小限制的端点，单独叠加 `DefaultBodyLimit`，
/// 上限取自 admin 上下文的统一上传阈值，超限请求在进入处理器之前就被框架拒绝。
/// 本函数只做声明，不含任何鉴权判断，鉴权由各处理器的 `UserAuth` 提取器逐个施加。
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/user/profile", get(profile))
        .route("/user/username", patch(update_username))
        .route(
            "/user/avatar",
            post(upload_avatar).layer(DefaultBodyLimit::max(MAX_UPLOAD_BODY_SIZE_BYTES)),
        )
        .route("/user/kyc", get(get_kyc_status))
        .route("/user/kyc/submissions", post(submit_kyc_submission))
        .route("/user/2fa", get(get_two_factor_status))
        .route("/user/2fa/setup", post(setup_two_factor))
        .route("/user/2fa/confirm", post(confirm_two_factor))
        .route("/user/2fa/login", patch(update_login_two_factor))
        .route("/user/2fa/reset-code", post(send_two_factor_reset_code))
        .route("/user/2fa/reset", post(reset_two_factor))
        .route(
            "/user/third-party-bindings",
            get(get_third_party_bindings).post(bind_third_party_account),
        )
        .route("/user/email/bind-code", post(send_email_bind_code))
        .route("/user/email/bind", post(bind_email))
        .route("/user/password", patch(change_password))
        .route(
            "/user/fund-password",
            post(create_fund_password).patch(change_fund_password),
        )
        .route(
            "/user/fund-password/reset-code",
            post(send_fund_password_reset_code),
        )
        .route("/user/fund-password/reset", post(reset_fund_password))
        .route("/referral/my-code", get(my_referral_code))
        .route("/referral/bind", post(bind_referral_code))
        .route("/referral/my-invites", get(my_invites))
}

/// 处理 `GET /user/profile`，返回令牌持有者本人的资料快照。
/// 响应含邮箱、手机号、头像、国家与本地化配置、账号状态、KYC 等级及资金密码是否已设置，
/// 其中不含任何密码哈希或 TOTP 密钥。
/// 只读端点，无请求体；用户 ID 只取自令牌，无法查询他人资料。
async fn profile(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserProfileResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let profile = get_user_profile(&pool, user_id).await?;

    Ok(Json(profile))
}

/// 处理 `PATCH /user/username`，修改本人的登录用户名。
/// 请求体只带新用户名原文，规范化与重名判定都在用例内完成，本层原样透传。
/// 与他人重名时用例返回冲突错误；成功响应回传已规范化的权威用户名，供前端覆盖本地显示。
/// 改名不影响现有登录态，本端点不签发也不撤销任何令牌。
async fn update_username(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<UpdateUsernameRequest>,
) -> AppResult<Json<UpdateUsernameResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = update_user_username(&pool, user_id, request.username).await?;

    Ok(Json(response))
}

/// 处理 `POST /user/avatar`，接收 multipart 表单上传的头像图片并更新到本人资料。
/// 这是本文件唯一的非 JSON 入口，请求体大小已在路由声明处按统一上传上限限制，
/// 超限请求由框架直接拒绝，不会进入本函数。
/// multipart 解析、字段提取与图片类型判定复用 admin 上下文的公共实现，本层不自行解析字节流。
/// 上传对象先落存储再回写资料 URL，因此中途失败可能留下未被引用的孤儿对象，本端点不做补偿清理。
/// 响应同时返回新的头像地址与上传记录元数据。
async fn upload_avatar(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    multipart: Multipart,
) -> AppResult<Json<UserAvatarResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let input = multipart_file_input(multipart).await?;
    let response = upload_user_avatar(&state, &pool, user_id, input).await?;
    Ok(Json(response))
}

/// 处理 `GET /user/kyc`，返回平台当前的实名认证配置与本人最新一份申请。
/// 配置部分告诉前端需要采集哪些证件类型与图片；申请部分携带审核状态，未提交过时为空。
/// 响应包含本人证件信息与材料地址，属于高敏感数据，只允许交付给令牌持有者本人。
/// 该端点在配置缺失时会由下游写入一份默认配置，因此并非严格只读。
async fn get_kyc_status(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<KycStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = get_user_kyc_status(&pool, user_id).await?;
    Ok(Json(response))
}

/// 处理 `POST /user/kyc/submissions`，提交一份实名认证申请等待人工审核。
/// 请求体以 JSON 承载姓名、证件号与 Base64 编码的证件图片，图片大小与编码合法性由 kyc 用例校验。
/// 已有待审申请或已通过认证时用例会拒绝重复提交，前端应据 `GET /user/kyc` 的状态决定是否展示入口。
/// 提交只把材料落库并置为待审，本端点不触发任何自动审核，状态流转由后台审核动作驱动。
/// 请求与响应都涉及证件原文，禁止在传输层记录请求体或响应体。
async fn submit_kyc_submission(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<SubmitKycRequest>,
) -> AppResult<Json<KycSubmissionResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let submission = submit_user_kyc_submission(&pool, user_id, request).await?;
    Ok(Json(submission))
}

/// 处理 `GET /user/2fa`，返回二次验证的完整状态视图。
/// 同时给出用户侧设置（TOTP 是否已绑定、登录二次验证是否开启）与平台侧策略
/// （全局登录二次验证模式、各支付动作要求的验证方式、第三方绑定开关）。
/// 其中 `can_toggle_login_2fa` 直接告诉前端登录开关是否可交互，无需前端自行推导策略含义。
/// 只读端点，任何情况下都不返回 TOTP 密钥明文或密文。
async fn get_two_factor_status(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserTwoFactorStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let status = get_user_two_factor_status(&pool, user_id).await?;
    Ok(Json(status))
}

/// 处理 `GET /user/third-party-bindings`，列出本人已绑定的第三方账号并附带后台入口开关。
/// 与同路径的 POST 共享响应结构，使前端在查询与绑定后拿到形状一致的数据，无需分别处理。
/// 返回的账号标识是绑定时留存的本地快照，不反映第三方侧的最新状态，也不含任何第三方令牌。
/// 只读端点，无请求体。
async fn get_third_party_bindings(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<ThirdPartyBindingStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = get_user_third_party_bindings(&pool, user_id).await?;
    Ok(Json(response))
}

/// 处理 `POST /user/third-party-bindings`，登记或覆盖一个第三方账号绑定。
/// 请求体拆出提供方、账号标识与可选展示名三项分别传给用例，展示名缺省时前端应回落展示账号标识。
/// 提供方必须落在白名单内且对应入口在后台策略中处于开启状态，否则用例返回安全禁止错误。
/// 同一提供方重复提交是覆盖既有绑定而非新增记录，因此本端点可安全重试。
/// 只登记用户自报的标识，服务端不会调用第三方接口验证账号归属。
/// 成功后返回与查询端点一致的完整绑定列表加策略视图。
async fn bind_third_party_account(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindThirdPartyAccountRequest>,
) -> AppResult<Json<ThirdPartyBindingStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = bind_user_third_party_account(
        &pool,
        user_id,
        request.provider,
        request.account_identifier,
        request.display_name,
    )
    .await?;
    Ok(Json(response))
}

/// 处理 `POST /user/2fa/setup`，是绑定 TOTP 的第一步，生成新密钥并以待确认状态保存。
/// 响应回传密钥明文与可供验证器扫码的 otpauth URI，这是全流程中密钥唯一一次对外出现，
/// 前端只能直接展示给用户，不得记录、缓存或上报。
/// 此时二次验证尚未生效，必须继续调用确认端点提交动态码才算绑定完成。
/// 已绑定用户调用会被拒绝，换绑需先走重置流程。
async fn setup_two_factor(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<SetupTwoFactorResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let setup = setup_user_two_factor(&state, &pool, user_id).await?;
    Ok(Json(setup))
}

/// 处理 `POST /user/2fa/confirm`，是绑定 TOTP 的第二步，用动态码确认并正式启用密钥。
/// 请求体只带六位动态码，服务端据此校验用户确实已把生成步骤下发的密钥导入验证器。
/// 校验允许一定时钟漂移；格式错误与码值错误返回同一个错误码，无法据此区分。
/// 未先调用生成端点时会被要求先生成密钥。
/// 成功后返回最新的二次验证状态视图，其中不含密钥内容。
async fn confirm_two_factor(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ConfirmTwoFactorRequest>,
) -> AppResult<Json<UserTwoFactorStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let status = confirm_user_two_factor(&state, &pool, user_id, request.totp_code).await?;
    Ok(Json(status))
}

/// 处理 `PATCH /user/2fa/login`，开关「登录时要求二次验证」这一项设置。
/// 请求体只带布尔开关。是否允许改动取决于后台策略：仅当策略为「由用户自行决定」时可改，
/// 强制开启或强制关闭模式下用例直接拒绝，前端应据状态查询中的可切换标志置灰控件。
/// 开启前必须已完成 TOTP 绑定，否则会因缺少验证手段而被拒绝。
/// 本端点只改登录场景的开关，不影响支付类动作的验证要求，也不撤销任何现有会话。
async fn update_login_two_factor(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<UpdateLoginTwoFactorRequest>,
) -> AppResult<Json<UserTwoFactorStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let status = update_user_login_two_factor(&pool, user_id, request.enabled).await?;
    Ok(Json(status))
}

/// 处理 `POST /user/2fa/reset-code`，向本人已验证邮箱发送用于解绑 TOTP 的验证码。
/// 无请求体，收件地址不可指定，服务端只会发往数据库中记录的已验证邮箱，这是防止重置码被劫持的关键。
/// 未绑定或未验证邮箱的用户无法使用此恢复通道。
/// 该用途的验证码有独立的发送冷却窗口，与邮箱绑定码、资金密码重置码互不干扰。
/// 响应只返回是否已发送与过期时间，不含验证码本身。
async fn send_two_factor_reset_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<BindEmailCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = send_user_two_factor_reset_code(&state, &pool, user_id).await?;
    Ok(Json(response))
}

/// 处理 `POST /user/2fa/reset`，凭邮箱验证码解除 TOTP 绑定，供验证器丢失的用户自助恢复。
/// 请求体只带邮件验证码，不需要也无法提供动态码，这正是该通道存在的意义。
/// 只接受二次验证重置用途的码，用其他用途的有效码调用同样会失败。
/// 重置后 TOTP 与登录二次验证开关一并清空，用户需重新走生成与确认流程；现有登录会话不受影响。
/// 成功返回重置后的二次验证状态视图。
async fn reset_two_factor(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ResetTwoFactorRequest>,
) -> AppResult<Json<UserTwoFactorStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let status = reset_user_two_factor_with_email_code(&pool, user_id, request.code).await?;
    Ok(Json(status))
}

/// 处理 `POST /user/email/bind-code`，向请求体中指定的待绑定邮箱发送验证码。
/// 与两个重置类发码端点的根本差异是收件地址由用户提交，因此用例必须先确认该地址未被他人占用，
/// 并在发送前作废同用途的旧码，保证同时只有最新一枚有效。
/// 发送受冷却窗口限制，频繁调用会返回校验错误。
/// 验证码落库后才实际投递邮件，投递失败不会回滚已记录的冷却状态。
/// 响应只返回发送标志与过期时间。
async fn send_email_bind_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindEmailCodeRequest>,
) -> AppResult<Json<BindEmailCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = send_user_email_bind_code(&state, &pool, user_id, request.email).await?;
    Ok(Json(response))
}

/// 处理 `POST /user/email/bind`，用收到的验证码把邮箱正式绑定到本人账号，也用于更换邮箱。
/// 请求体需同时带邮箱与验证码，两者必须与发码时的组合一致，只对得上其中一项不会通过。
/// 验证码错误会消耗有限的尝试次数，次数用尽后该码即使未过期也失效，需重新发码。
/// 成功后邮箱与验证时间一起写入，账号自此可作为重置密码等流程的可信联系方式。
/// 响应回传绑定后的邮箱与验证时间。
async fn bind_email(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindEmailRequest>,
) -> AppResult<Json<BindEmailResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = bind_user_email(&pool, user_id, request.email, request.code).await?;
    Ok(Json(response))
}

/// 处理 `PATCH /user/password`，凭旧登录密码修改为新登录密码。
/// 这是本文件中唯一会改变登录态的端点：改密成功后服务端撤销全部旧会话与刷新令牌，
/// 并在响应中直接返回一对新的访问与刷新令牌，前端必须用其替换本地凭证，否则后续请求将失效。
/// 新旧密码不得相同，新密码需满足长度策略；旧密码错误与账号被停用返回同一未授权错误。
/// 密码明文仅在请求体中出现，禁止在传输层记录请求内容。
async fn change_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ChangePasswordRequest>,
) -> AppResult<Json<TokenResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = change_user_password(
        &state,
        &pool,
        user_id,
        request.old_password,
        request.new_password,
    )
    .await?;
    Ok(Json(response))
}

/// 处理 `POST /user/fund-password`，首次设置六位数字资金密码。
/// 请求体需同时提供登录密码与新资金密码：前者用于确认操作者身份，后者是待设置的支付口令。
/// 二者不得相同，否则资金密码将失去作为独立第二道确认的意义。
/// 已设置过资金密码时返回冲突错误而非覆盖，修改须改用同路径的 PATCH 方法。
/// 响应只回传 `fund_password_set` 标志，不返回任何口令内容。
async fn create_fund_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateFundPasswordRequest>,
) -> AppResult<Json<FundPasswordResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = create_user_fund_password(
        &pool,
        user_id,
        request.login_password,
        request.fund_password,
    )
    .await?;
    Ok(Json(response))
}

/// 处理 `PATCH /user/fund-password`，凭旧资金密码修改为新资金密码。
/// 与同路径的 POST 相比，这里不需要登录密码，仅以旧资金密码本身完成鉴权；
/// 两个端点共用路径而以 HTTP 方法区分「首次创建」与「修改既有」。
/// 新旧资金密码都必须是六位数字且不得相同；从未设置过时返回未找到，提示应改走创建端点。
/// 响应形状与创建端点一致，只回传是否已设置的标志。
async fn change_fund_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ChangeFundPasswordRequest>,
) -> AppResult<Json<FundPasswordResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = change_user_fund_password(
        &pool,
        user_id,
        request.old_fund_password,
        request.new_fund_password,
    )
    .await?;
    Ok(Json(response))
}

/// 处理 `POST /user/fund-password/reset-code`，向本人已验证邮箱发送资金密码重置码，用于遗忘口令的场景。
/// 无请求体，收件地址同样只取数据库中的已验证邮箱，不接受调用方指定。
/// 与二次验证重置发码的差别在于这里多一道前置条件：必须确实已设置过资金密码才会发送，
/// 未设置的账号应走创建端点而非重置流程。
/// 该用途拥有独立的冷却窗口；邮件投递失败不会回滚已写入的冷却记录。
async fn send_fund_password_reset_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<BindEmailCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = send_user_fund_password_reset_code(&state, &pool, user_id).await?;
    Ok(Json(response))
}

/// 处理 `POST /user/fund-password/reset`，凭邮箱验证码在不知道旧口令的情况下重设资金密码。
/// 请求体带验证码与新资金密码，只接受资金密码重置用途的码。
/// 与修改端点的区别是这里用邮箱控制权替代旧口令作为身份凭据，因此账号必须已绑定并验证过邮箱。
/// 验证码错误会消耗尝试次数；成功时新口令写入与验证码核销在同一事务内完成，不会出现只生效一半的情况。
/// 响应只回传是否已设置的标志。
async fn reset_fund_password(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ResetFundPasswordRequest>,
) -> AppResult<Json<FundPasswordResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response =
        reset_user_fund_password(&pool, user_id, request.code, request.new_fund_password).await?;
    Ok(Json(response))
}

/// 处理 `GET /referral/my-code`，返回本人用于推广的自有邀请码。
/// 虽是 GET 语义，但并非纯只读：用户尚无邀请码或既有码不符合当前格式规范时，
/// 用例会即时生成一枚唯一码并落库后再返回，因此首次调用带有写副作用。
/// 同一用户重复调用会稳定拿到同一枚码，不会每次生成新码。
/// 响应附带该码的使用上限、已用次数与所属代理归属，供前端展示推广进度。
async fn my_referral_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<ReferralCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let code = get_user_referral_code(&pool, user_id).await?;
    Ok(Json(code))
}

/// 处理 `POST /referral/bind`，把本人挂靠到某个邀请码之下，建立直接邀请人与代理归属关系。
/// 请求体只带邀请码。绑定是一次性的：已绑定用户重复调用会原样返回既有关系且不再消耗邀请码次数，
/// 因此该端点可安全重试，但无法用来更换邀请人。
/// 邀请码可属于用户或代理，两种归属派生出不同的代理链；禁止绑定自己的码，
/// 邀请码用尽次数或所属代理链上任一层级被停用都会被拒绝。
/// 响应返回绑定后的推荐关系，含直接邀请人、根代理、层级深度与物化路径。
async fn bind_referral_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<BindReferralCodeRequest>,
) -> AppResult<Json<ReferralBindingResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let binding = bind_user_referral_code(&pool, user_id, request.code).await?;
    Ok(Json(binding))
}

/// 处理 `GET /referral/my-invites`，列出把本人登记为直接邀请人的下级用户。
/// 只返回第一层直邀关系，不展开更深层级，因此该列表不能用于核对多级返佣。
/// 结果不分页且最多一百条，按加入时间升序排列，邀请人数超出后只能看到最早的一批。
/// 响应含下级的邮箱与手机号，属于他人隐私字段，仅限邀请人本人查看。
async fn my_invites(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MyInvitesResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = list_user_invites(&pool, user_id).await?;
    Ok(Json(response))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_user_routes_tests.rs"]
mod tests;
