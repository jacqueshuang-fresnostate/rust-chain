use super::*;

pub(crate) fn validate_admin_user_recharge(request: &AdminUserRechargeRequest) -> AppResult<()> {
    if request.asset_id == 0 {
        return Err(AppError::Validation("asset_id is required".to_owned()));
    }
    if request.amount <= 0 {
        return Err(AppError::Validation("amount must be positive".to_owned()));
    }
    required_admin_audit_reason(request.reason.clone())?;
    Ok(())
}

pub(crate) fn validate_create_admin_user_request(
    request: &CreateAdminUserRequest,
) -> AppResult<()> {
    if optional_string(request.email.clone()).is_none()
        && optional_string(request.phone.clone()).is_none()
    {
        return Err(AppError::Validation(
            "email or phone is required".to_owned(),
        ));
    }
    if let Some(email) = optional_string(request.email.clone())
        && (email.len() > 255 || !email.contains('@'))
    {
        return Err(AppError::Validation("email format is invalid".to_owned()));
    }
    if let Some(phone) = optional_string(request.phone.clone())
        && phone.len() > 32
    {
        return Err(AppError::Validation("phone is too long".to_owned()));
    }
    if optional_string(Some(request.password.clone())).is_none() {
        return Err(AppError::Validation("password is required".to_owned()));
    }
    if let Some(status) = request.status.as_deref() {
        validate_user_status(status)?;
    }
    if request.kyc_level.unwrap_or(0) < 0 {
        return Err(AppError::Validation(
            "kyc_level must be non-negative".to_owned(),
        ));
    }
    required_admin_audit_reason(request.reason.clone())?;
    Ok(())
}

pub(crate) fn validate_user_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported user status".to_owned())),
    }
}

pub(crate) fn hash_admin_user_password(password: &str) -> AppResult<String> {
    hash_password(password)
}

pub(crate) fn user_audit_json(user: &AdminUserResponse) -> Value {
    json!({
        "id": user.id,
        "email": user.email,
        "phone": user.phone,
        "status": user.status,
        "kyc_level": user.kyc_level,
        "created_at": user.created_at.timestamp_millis(),
        "updated_at": user.updated_at.timestamp_millis(),
    })
}

pub(crate) fn recharge_audit_json(recharge: &AdminUserRechargeResponse) -> Value {
    json!({
        "recharge_id": recharge.recharge_id,
        "user_id": recharge.user_id,
        "asset_id": recharge.asset_id,
        "asset_symbol": recharge.asset_symbol,
        "amount": format!("{:.18}", recharge.amount),
        "available": format!("{:.18}", recharge.available),
        "frozen": format!("{:.18}", recharge.frozen),
        "locked": format!("{:.18}", recharge.locked),
    })
}
