//! auth bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//!
//! 本文件承载认证限界上下文中不触碰数据库、Redis 和 HTTP 的纯判定：登录失败锁定的阈值与时间窗口常量、
//! 单次登录是否需要调用 Turnstile 站点校验的策略，以及注册、邮件码和重置口令链路的输入规范化与格式校验。
//! 这里只做格式与策略判断；账号是否存在、验证码是否已被消费、邀请码是否还有余量等依赖存储时序的结论，
//! 一律留给仓储与应用层在事务内判定，避免领域规则被外部并发状态污染。
//! 本层校验失败返回的错误只描述输入格式问题，不携带账号存在与否的线索，因此可以安全地直接回传客户端。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

/// 连续密码失败次数达到该阈值即触发临时锁定，与邮件验证码的尝试上限保持一致。
pub(crate) const LOGIN_FAILURE_LIMIT: u32 = 5;
/// 失败计数窗口：窗口过期后计数自动归零，无需人工干预。
pub(crate) const LOGIN_FAILURE_WINDOW_SECONDS: i64 = 900;
/// 锁定时长：到期后自动解锁，避免计数卡死造成永久拒绝登录。
pub(crate) const LOGIN_LOCKOUT_SECONDS: i64 = 900;

const LOGIN_FAILURE_KEY_MAX_CHARS: usize = 191;

#[derive(Debug)]
pub struct AuthValidationRules;

impl DomainLayer for AuthValidationRules {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoginTurnstilePolicy {
    enabled: bool,
    enforce_token: bool,
}

impl LoginTurnstilePolicy {
    /// 由运行时配置推导登录前置人机校验策略，只有服务端密钥与站点公钥同时存在才视为已启用。
    /// 缺少任意一把密钥说明部署尚未配置完整，此时策略退化为不校验，登录不会因为配置缺失而被拒绝。
    /// `enforce_token` 只在已启用的前提下生效，单独打开它不会让未配置密钥的环境开始校验。
    pub(crate) fn new(has_secret: bool, has_site_key: bool, enforce_token: bool) -> Self {
        Self {
            enabled: has_secret && has_site_key,
            enforce_token,
        }
    }

    /// 返回本次部署是否具备完整的 Turnstile 校验能力，供登录配置接口向前端下发开关和站点公钥。
    /// 该标志只反映密钥是否齐备，不代表当前这一次登录必定需要校验；具体是否回源核验由
    /// `requires_verification` 结合强制策略与 `cf_clearance` 判断。
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// 判定本次登录请求是否必须先向 Cloudflare 站点校验接口核验挑战令牌。
    /// 未启用时恒不校验；启用后若开启强制标志则每次都校验，未开启强制标志时只在请求缺少
    /// `cf_clearance` Cookie 时校验，即已通过边缘挑战的会话可以跳过一次回源核验以降低延迟。
    /// 本判定属于人机识别关卡，与口令校验相互独立，调用方不得因为这里返回假就跳过凭据验证。
    pub(crate) fn requires_verification(&self, has_cf_clearance: bool) -> bool {
        self.enabled && (self.enforce_token || !has_cf_clearance)
    }
}

/// 把登录标识规范化为失败计数与锁定记录使用的键，防止同一账号被拆成多个互不相干的计数桶。
/// 先去掉首尾空白再统一转小写，使同一邮箱的大小写变体命中同一条锁定记录；随后按字符截断到 191 个字符，
/// 既对齐存储列宽，也避免攻击者用超长标识撑爆索引或制造永不重复的键来绕过锁定阈值。
/// 本函数不判断标识属于邮箱、手机号还是用户名，也不检查它是否对应真实账号，因此对不存在的账号同样会计数。
pub fn login_failure_key(identifier: &str) -> String {
    identifier
        .trim()
        .to_lowercase()
        .chars()
        .take(LOGIN_FAILURE_KEY_MAX_CHARS)
        .collect()
}

/// 构造账号被临时锁定时统一返回的安全校验错误，向客户端说明还需等待多久才能重试。
/// 剩余秒数先夹到非负再按分钟向上取整，并至少显示一分钟，避免时钟回拨或临界值算出零分钟等无意义提示。
/// 提示文本只含等待时长，不含账号是否存在、已累计几次失败或锁定起始时间，因此无法被用来枚举账号。
/// 调用方须原样上抛该错误，不要拼接标识或失败次数，否则会把这里刻意隐藏的信息重新泄露出去。
pub(crate) fn login_locked_error(retry_after_seconds: i64) -> AppError {
    let minutes = (retry_after_seconds.max(0) as u64).div_ceil(60).max(1);
    AppError::security_validation(
        "login_temporarily_locked",
        format!("登录失败次数过多，请在 {minutes} 分钟后重试"),
    )
}

/// 校验邮件验证码去除首尾空白后恰好是六位 ASCII 数字，并返回规范化后的值供后续比对哈希。
/// 这是验证码进入仓储前的唯一格式关卡：长度不符或含非数字字符直接返回校验错误，使明显伪造的输入
/// 既不占用数据库查询，也不消耗该验证码有限的试错次数，避免攻击者用垃圾串把合法用户的验证码刷废。
/// 本函数不比对哈希、不检查过期时间与尝试上限，是否命中真实验证码由事务内加锁的消费逻辑判定。
pub(crate) fn validate_email_code(value: &str) -> AppResult<String> {
    let code = value.trim();
    if code.len() != 6 || !code.chars().all(|char| char.is_ascii_digit()) {
        return Err(AppError::Validation("code is invalid".to_owned()));
    }
    Ok(code.to_owned())
}

/// 校验注册邮箱必填、字节长度不超过 255 且至少包含一个 `@`，并统一转为 ASCII 小写后返回。
/// 统一小写保证同一邮箱在唯一索引、验证码记录和发送冷却判断中始终折叠成同一个键，
/// 否则大小写变体可以绕开占用检查、重复领取验证码，或在注册与重置之间指向不同记录。
/// 长度上限对齐存储列宽，使超长输入在开启事务之前就被拒绝。这里只做语法粗筛，不做投递或 MX 探测，
/// 邮箱是否已被占用、是否已完成验证都由注册事务内的检查决定。
pub(crate) fn validate_registration_email(value: Option<String>) -> AppResult<String> {
    let email = required_string(value, "email")?;
    if email.len() > 255 || !email.contains('@') {
        return Err(AppError::Validation("email format is invalid".to_owned()));
    }
    Ok(email.to_ascii_lowercase())
}

/// 校验重置口令去除首尾空白后长度在 6 到 20 之间，长度按 Unicode 字符计数而非字节，兼容非 ASCII 口令。
/// 返回的明文只应在内存中直接交给 Argon2 散列，不得写入日志、事件或数据库；调用方须尽早完成散列后丢弃。
/// 本函数只判定长度，不做复杂度、常见口令字典或历史口令重复检查，也不验证调用方是否已通过邮件码校验，
/// 该前置授权关卡由重置用例负责，不能因为这里返回成功就跳过。
pub(crate) fn validate_reset_password(value: &str) -> AppResult<String> {
    let password = required_string(Some(value.to_owned()), "password")?;
    if !(6..=20).contains(&password.chars().count()) {
        return Err(AppError::Validation(
            "password must be 6-20 characters long".to_owned(),
        ));
    }
    Ok(password)
}

/// 去除邀请码首尾空白并拒绝空串，为后续按码加锁查询提供稳定的键，不改变大小写。
/// 这里只做输入整形：邀请码是否存在、是否处于启用状态、剩余可用次数是否耗尽、归属方是否仍然活跃，
/// 都必须在注册事务内以行锁读取后判定，否则并发注册可能同时通过额度检查而导致邀请码超发。
/// 因此调用方不能把本函数的成功返回理解为邀请码可用。
pub(crate) fn normalize_invite_code(code: &str) -> AppResult<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("invite_code is required".to_owned()));
    }
    Ok(code.to_owned())
}

/// 取出必填字符串参数，缺失或仅含空白时返回带调用方字段名的校验错误。
/// 复用 `optional_string` 的折叠规则，因此纯空白输入等同于未提供，避免空串被当成合法值写入账号字段。
/// 错误消息只嵌入调用方给出的字段名，不回显用户提交的内容，防止把请求体原样反射进响应。
pub(crate) fn required_string(value: Option<String>, field: &str) -> AppResult<String> {
    optional_string(value).ok_or_else(|| AppError::Validation(format!("{field} is required")))
}

/// 把可选字符串折叠成「确有内容」或 `None` 两种状态，先去首尾空白，再将空串视为未提供。
/// 认证入口大量依赖这条折叠规则来判断调用方到底提交了邮箱、手机号还是用户名；若不折叠，
/// 前端传来的空字符串会被当作已填写的登录标识，进而以空值查库并落到错误的失败分支上。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_auth_domain_tests.rs"]
mod tests;
