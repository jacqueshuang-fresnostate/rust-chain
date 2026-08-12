use super::*;

/// 校验后台人工充值必须指定资产、正数金额和非空原因，避免事务启动后才发现请求无效。
/// 不在此处查询用户/资产或修改钱包；余额、流水与管理员审计由应用事务原子写入。
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

/// 校验后台创建用户至少提供邮箱或手机号，并复核初始状态、语言和密码请求字段。
/// 联系方式唯一性及密码散列写入由应用事务负责；本函数不访问数据库，也不保留明文凭据。
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

/// 规范化后台允许设置的用户状态；空白或未知状态返回校验错误，不执行会话撤销或数据库写入。
pub(crate) fn validate_user_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "suspended" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported user status".to_owned())),
    }
}

/// 校验后台用户后生成不可逆密码散列，供创建或重置后台账号凭据时持久化。
/// 直接沿用认证模块的强度检查与散列错误；成功只返回散列文本，不创建用户、记录明文或开启事务。
pub(crate) fn hash_admin_user_password(password: &str) -> AppResult<String> {
    hash_password(password)
}

/// 将用户联系方式、邀请关系和状态映射为后台审计 JSON，不包含密码或认证密钥。
/// 快照实际包含用户 ID、邮箱、手机号、状态、KYC 等级和时间戳；应用层随用户写事务保存它。
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

/// 将人工充值编号、用户、资产、金额和变更后余额映射为资金审计 JSON。
/// 金额及 available/frozen/locked 统一格式化为 18 位小数；结果不含流水元数据，充值事务负责持久化审计。
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
