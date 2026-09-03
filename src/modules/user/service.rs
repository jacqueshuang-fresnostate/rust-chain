//! user bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 本文件集中用户上下文的输入校验口径与安全随机生成器，包括邮箱地址、邮件验证码、
//! 登录密码、资金密码、第三方绑定标识和用户邀请码，另外提供认证 subject 到用户 ID 的解析。
//! 所有校验函数均为纯函数且不做 I/O：它们只判断「格式是否可能合法」，
//! 真实的唯一性、可达性、是否已被占用等需要落库或发信才能确认的结论一律交给 application 与 infrastructure 层。
//! 生成类函数统一使用 `ring` 的系统安全随机源，失败时返回内部错误而不是退化为可预测值。

use crate::{
    error::{AppError, AppResult},
    modules::{
        security::ThirdPartyBindingPolicy,
        user::domain::{optional_string, required_string},
    },
};
use ring::rand::{SecureRandom, SystemRandom};

pub(crate) const EMAIL_BIND_PURPOSE: &str = "bind";
pub(crate) const TWO_FACTOR_RESET_PURPOSE: &str = "two_factor_reset";
pub(crate) const FUND_PASSWORD_RESET_PURPOSE: &str = "fund_password_reset";
pub(crate) const EMAIL_VERIFICATION_CODE_TTL_MINUTES: u32 = 10;
pub(crate) const EMAIL_VERIFICATION_CODE_COOLDOWN_SECONDS: i64 = 60;
pub(crate) const INVITE_CODE_LENGTH: usize = 6;
pub(crate) const INVITE_CODE_CREATE_ATTEMPTS: usize = 12;
const INVITE_CODE_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// 绑定邮箱前做轻量格式校验：去空白后要求恰好一个 `@`、本地部分与域名部分均非空、
/// 整串不含任何空白字符，且字节长度不超过 255 以匹配数据库列宽。
/// 采用「宽松但足够」的口径而非完整 RFC 解析，避免误杀合法的少见地址；
/// 违反任一条返回 `AppError::Validation`，消息统一为 `<field> is invalid`，字段名由调用方给出。
/// 只返回规范化字符串，不做大小写归一，也不判断域名是否存在或邮箱是否已被他人绑定；
/// 地址真实可达性由后续发信与验证码回填确认，占用冲突由 infrastructure 层的唯一索引兜底。
pub(crate) fn validate_email(value: &str, field: &str) -> AppResult<String> {
    let email = required_string(Some(value.to_owned()), field)?;
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if email.len() > 255
        || local.is_empty()
        || domain.is_empty()
        || parts.next().is_some()
        || email.chars().any(char::is_whitespace)
    {
        return Err(AppError::Validation(format!("{field} is invalid")));
    }
    Ok(email)
}

/// 校验用户提交的邮件验证码必须是恰好六位 ASCII 数字，并返回去空白后的规范值。
/// 这道前置门槛让明显格式错误的输入在进入数据库比对之前就被拒绝，
/// 既避免为无效输入消耗一次查询，也避免它计入验证码的失败尝试次数配额。
/// 失败时返回 `AppError::Validation`，消息固定为 `code is invalid`，不区分「长度不对」与「含非数字」以免泄露校验细节。
pub(crate) fn validate_email_code(value: &str) -> AppResult<String> {
    let code = required_string(Some(value.to_owned()), "code")?;
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code)
}

/// 使用系统安全随机源生成补零后的六位验证码。
/// 随机源失败时返回内部错误，调用方不得发送或持久化占位验证码。
pub(crate) fn generate_email_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 4];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("email verification code generation failed".to_owned()))?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}

/// 校验登录密码长度下限：按 Unicode 字符数而非字节数统计，要求至少八个字符，
/// 因此中文或表情符号密码不会因为单字符占多字节而被错误放宽或收紧。
/// 通过后原样返回密码内容（仅去除首尾空白），不做大小写转换或字符替换，以免破坏后续散列结果。
/// 不检查复杂度、历史重复或弱口令字典；密码本身绝不落日志，散列由 application 层负责。
pub(crate) fn validate_login_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.chars().count() < 8 {
        return Err(AppError::Validation(format!("{field} is too short")));
    }
    Ok(password)
}

/// 校验资金密码必须是恰好六位 ASCII 数字，长度按字节判定，任何字母、符号或内部空白都会被拒绝。
/// 资金密码与登录密码规则不同：这里是定长纯数字支付密码，用于提现等敏感操作的二次确认，
/// 因此不能沿用登录密码那套「至少八字符」的宽松口径。
/// 失败返回 `AppError::Validation`，消息为 `<field> must be 6 digits`；明文不写日志，散列在上层完成。
pub(crate) fn validate_fund_password(value: &str, field: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), field)?;
    if password.len() != 6 || !password.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation(format!("{field} must be 6 digits")));
    }
    Ok(password)
}

/// 把外部传入的第三方提供方名称收敛为平台已实现的白名单常量，目前只接受
/// `coinbase_wallet` 与 `telegram_account` 两种，其余一律返回 `AppError::Validation`。
/// 返回 `&'static str` 而非回传调用方的字符串，确保写库的提供方取值来自代码内的固定集合，
/// 杜绝任意字符串被持久化成新的伪提供方。匹配前只做首尾空白裁剪，大小写必须精确一致。
pub(crate) fn normalize_third_party_provider(value: &str) -> AppResult<&'static str> {
    match value.trim() {
        "coinbase_wallet" => Ok("coinbase_wallet"),
        "telegram_account" => Ok("telegram_account"),
        _ => Err(AppError::Validation("provider is invalid".to_owned())),
    }
}

/// 依据运营侧配置的第三方绑定策略，判断某个提供方的绑定入口当前是否开放。
/// 采用「默认关闭」语义：只有精确命中已知提供方且策略中对应开关为真才返回 `true`，
/// 未知或拼写错误的提供方一律视为关闭，避免新增提供方时因为漏配策略而被默认放行。
/// 本函数只读策略快照，不查库也不缓存，策略的加载与刷新由 security 上下文负责。
pub(crate) fn is_third_party_binding_enabled(
    policy: &ThirdPartyBindingPolicy,
    provider: &str,
) -> bool {
    match provider {
        "coinbase_wallet" => policy.coinbase_wallet_enabled,
        "telegram_account" => policy.telegram_account_enabled,
        _ => false,
    }
}

/// 校验第三方账号标识长度和空白字符，Telegram 使用更严格的 64 字节上限。
/// 本函数不验证远端账号是否存在，该可达性由具体第三方授权流程负责。
pub(crate) fn validate_third_party_identifier(provider: &str, value: &str) -> AppResult<String> {
    let identifier = required_string(Some(value.to_owned()), "account_identifier")?;
    let max_len = if provider == "telegram_account" {
        64
    } else {
        255
    };
    if identifier.len() > max_len || identifier.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(
            "account_identifier is invalid".to_owned(),
        ));
    }
    Ok(identifier)
}

/// 规范化第三方账号的可选展示名：空白折叠为 `None`，非空时限制不超过 255 个 Unicode 字符。
/// 长度按字符数而非字节数统计，因此中文昵称不会被过早截断判定为超长。
/// 缺省是合法输入而非错误，返回 `Ok(None)` 表示调用方应写入空值并在前端回落到账号标识展示。
/// 超长返回 `AppError::Validation`，消息为 `display_name is too long`；本函数不做敏感词或 HTML 转义。
pub(crate) fn normalize_third_party_display_name(
    value: Option<String>,
) -> AppResult<Option<String>> {
    let display_name = optional_string(value);
    if display_name
        .as_ref()
        .is_some_and(|value| value.chars().count() > 255)
    {
        return Err(AppError::Validation("display_name is too long".to_owned()));
    }
    Ok(display_name)
}

/// 使用系统安全随机源生成六位大写字母数字邀请码。
/// 普通用户码与代理码必须共用本入口和 `invite_codes.code` 的全局唯一索引，
/// 禁止按归属类型分配不同格式或可预测前缀。随机源失败返回内部错误；
/// 唯一性冲突由各写入用例在 `INVITE_CODE_CREATE_ATTEMPTS` 次内换码重试。
pub(crate) fn generate_invite_code() -> AppResult<String> {
    let rng = SystemRandom::new();
    let uniform_bound =
        (usize::from(u8::MAX) + 1) / INVITE_CODE_ALPHABET.len() * INVITE_CODE_ALPHABET.len();
    let mut code = String::with_capacity(INVITE_CODE_LENGTH);

    // 256 不能被 36 整除；丢弃尾部四个取值后再取模，避免部分字符拥有更高概率。
    while code.len() < INVITE_CODE_LENGTH {
        let mut bytes = [0_u8; INVITE_CODE_LENGTH];
        rng.fill(&mut bytes)
            .map_err(|_| AppError::Internal("invite code generation failed".to_owned()))?;
        for byte in bytes {
            let byte = usize::from(byte);
            if byte >= uniform_bound {
                continue;
            }
            code.push(INVITE_CODE_ALPHABET[byte % INVITE_CODE_ALPHABET.len()] as char);
            if code.len() == INVITE_CODE_LENGTH {
                break;
            }
        }
    }

    Ok(code)
}

/// 判定一个字符串是否符合本平台自生成邀请码的形态：长度恰为 `INVITE_CODE_LENGTH`，
/// 且每个字符都落在大写字母或阿拉伯数字范围内，与 `generate_invite_code` 使用的字母表口径一致。
/// 注意这里不排除易混淆字符，也不校验该邀请码是否真实存在或仍然可用，只做形态判定；
/// 返回布尔值而非 `Result`，供调用方在「是否走邀请码分支」这类判断中直接使用。
pub(crate) fn is_valid_generated_invite_code(code: &str) -> bool {
    code.len() == INVITE_CODE_LENGTH
        && code
            .chars()
            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
}

/// 规范化用户填写的邀请码：仅裁剪首尾空白并拒绝空串，刻意不做大小写转换或字符集过滤。
/// 之所以比 `is_valid_generated_invite_code` 宽松，是因为绑定入口同时接受平台自生成的邀请码
/// 与代理商推广链接携带的其他码型，收紧字符集会误伤后者。
/// 因此这里只保证「有内容可查」，该码是否存在、是否属于有效代理、剩余可用次数是否耗尽，
/// 全部由 infrastructure 层在事务内锁行校验；失败返回 `AppError::Validation` 且消息为 `code is required`。
pub(crate) fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("code is required".to_owned()));
    }
    Ok(code.to_owned())
}

/// 从访问令牌解析出的认证主体串中提取用户数字 ID，只接受 `user:<id>` 这一种前缀。
/// 前缀是权限边界的一部分：管理员或其他主体类型的 subject 不带该前缀，会在此被挡下，
/// 因此本函数同时承担「确认调用者是普通用户自服务身份」的职责。
/// 前缀不符、剩余部分为空或不能解析为 `u64` 时统一返回 `AppError::Unauthorized` 而非校验错误，
/// 避免通过错误类型区分「格式错」与「非用户主体」而泄露令牌结构信息。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
