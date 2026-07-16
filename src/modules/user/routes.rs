use crate::{
    error::AppResult,
    modules::user::service::{mysql_pool, user_id_from_subject},
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
        admin::{infrastructure::multipart_file_input, service::MAX_UPLOAD_BODY_SIZE_BYTES},
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

async fn profile(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserProfileResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let profile = get_user_profile(&pool, user_id).await?;

    Ok(Json(profile))
}

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

async fn get_kyc_status(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<KycStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = get_user_kyc_status(&pool, user_id).await?;
    Ok(Json(response))
}

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

async fn get_two_factor_status(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserTwoFactorStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let status = get_user_two_factor_status(&pool, user_id).await?;
    Ok(Json(status))
}

async fn get_third_party_bindings(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<ThirdPartyBindingStatusResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = get_user_third_party_bindings(&pool, user_id).await?;
    Ok(Json(response))
}

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

async fn setup_two_factor(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<SetupTwoFactorResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let setup = setup_user_two_factor(&state, &pool, user_id).await?;
    Ok(Json(setup))
}

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

async fn send_two_factor_reset_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<BindEmailCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = send_user_two_factor_reset_code(&state, &pool, user_id).await?;
    Ok(Json(response))
}

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

async fn send_fund_password_reset_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<BindEmailCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let response = send_user_fund_password_reset_code(&state, &pool, user_id).await?;
    Ok(Json(response))
}

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

async fn my_referral_code(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
) -> AppResult<Json<ReferralCodeResponse>> {
    let user_id = user_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let code = get_user_referral_code(&pool, user_id).await?;
    Ok(Json(code))
}

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
