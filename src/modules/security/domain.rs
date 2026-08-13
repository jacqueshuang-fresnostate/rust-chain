//! security bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件定义安全策略的值对象与默认口径（登录二次验证模式、各资金动作的支付校验要求、
//! 第三方绑定开关），以及用户与管理员二次验证的领域快照和登录挑战模型。
//! 另一半职责是 TOTP 的纯算法实现：密钥生成、Base32 编解码、HOTP 计算、动态码校验与 otpauth 导入链接构造，
//! 参数固定为 SHA1、六位数字、三十秒时间步，与主流验证器应用兼容。
//! 时间语义：动态码校验的当前时间由调用方注入，容许前后各一个时间步的漂移；
//! 计数器由 Unix 秒整除时间步得到，因此同一时间步内重复提交的是同一枚码。
//! 防重放不在本层实现：本层的校验函数是纯函数，同一枚码在有效期内可被反复验证通过，
//! 真正的一次性语义由 infrastructure 层的挑战记录消费与 `last_verified_at` 落库共同保证。
//! 密钥红线：所有涉及 TOTP 密钥的函数只在内存中处理明文，绝不写日志、指标或审计字段，
//! 密钥的加密存储由 application 与 infrastructure 层负责。
//! 历史兼容：策略 JSON 曾以字符串包装整体存储，布尔字段也曾写成 `0/1` 或 `"true"` 等形式，
//! 解码入口会先做形态归一再反序列化，归一只在内存中进行，不回写数据库。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::Sha1;

pub const TOTP_STEP_SECONDS: u64 = 30;
pub const TOTP_DIGITS: u32 = 6;
/// 单个管理员登录挑战允许的验证码错误次数，用尽即作废，防止对挑战暴力试码。
pub const ADMIN_LOGIN_TWO_FACTOR_ATTEMPT_LIMIT: u32 = 5;

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const DEFAULT_TOTP_SECRET_BYTES: usize = 20;

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginTwoFactorMode {
    None,
    UserEnabled,
    Mandatory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityVerificationMethod {
    FundPassword,
    TwoFactor,
    FundPasswordAndTwoFactor,
}

impl SecurityVerificationMethod {
    /// 返回该校验方式的稳定存储字符串，与策略 JSON 中的取值和后台下拉选项一一对应。
    /// 这些字面量属于持久化契约，一旦有历史数据写入就不能再改动，
    /// 否则既有策略记录反序列化时会落到未知分支。
    /// 组合方式 `fund_password_and_two_factor` 表示两道校验都要通过，而非二选一。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FundPassword => "fund_password",
            Self::TwoFactor => "two_factor",
            Self::FundPasswordAndTwoFactor => "fund_password_and_two_factor",
        }
    }

    /// 判断该校验方式是否要求提交资金密码，单独方式与组合方式都会命中。
    /// 与 `requires_two_factor` 是两个独立判定而非互斥选择：组合方式下两者同时返回真，
    /// 调用方必须分别校验两项，不能因为满足其一就放行。
    pub(crate) fn requires_fund_password(self) -> bool {
        matches!(self, Self::FundPassword | Self::FundPasswordAndTwoFactor)
    }

    /// 判断该校验方式是否要求提交 TOTP 动态码，单独方式与组合方式都会命中。
    /// 返回真只说明策略要求这道校验，不代表用户已经绑定过 TOTP；
    /// 未绑定却命中此要求时应由调用方引导用户先完成绑定，而不是跳过校验。
    pub(crate) fn requires_two_factor(self) -> bool {
        matches!(self, Self::TwoFactor | Self::FundPasswordAndTwoFactor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityAction {
    Withdraw,
    SpotOrder,
    Convert,
    EarnSubscribe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentPolicy {
    pub enabled: bool,
    pub method: SecurityVerificationMethod,
}

impl PaymentPolicy {
    /// 构造一条「该动作无需额外安全校验」的策略，用作默认配置中低风险动作的初始值。
    /// `method` 在关闭状态下不参与判定，这里填资金密码只是占位，避免引入可空字段；
    /// 因此运营侧后续把 `enabled` 打开时，若未同时选择方式就会落到资金密码这一档。
    fn disabled() -> Self {
        Self {
            enabled: false,
            method: SecurityVerificationMethod::FundPassword,
        }
    }

    /// 构造一条「启用且要求资金密码」的策略，是默认配置中提现动作的初始值。
    /// 选资金密码而非 TOTP 作为默认，是因为资金密码为平台内可强制设置的凭证，
    /// 而 TOTP 依赖用户自行绑定外部验证器，默认要求它会让未绑定用户直接无法提现。
    fn fund_password_required() -> Self {
        Self {
            enabled: true,
            method: SecurityVerificationMethod::FundPassword,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentPolicies {
    pub withdraw: PaymentPolicy,
    pub spot_order: PaymentPolicy,
    pub convert: PaymentPolicy,
    pub earn_subscribe: PaymentPolicy,
}

impl PaymentPolicies {
    /// 按资金动作取出对应的支付校验策略，是「这笔操作要不要二次确认」的唯一权威查询入口。
    /// 四类动作各自持有独立策略，运营可以只对提现开启校验而放行现货下单，互不牵连。
    /// 用穷尽匹配而非映射表，保证新增动作类型时编译器会强制在此补齐分支，不会静默漏配。
    /// 返回引用且不复制，调用方只读判定，本方法无任何状态变更。
    pub fn policy_for(&self, action: SecurityAction) -> &PaymentPolicy {
        match action {
            SecurityAction::Withdraw => &self.withdraw,
            SecurityAction::SpotOrder => &self.spot_order,
            SecurityAction::Convert => &self.convert,
            SecurityAction::EarnSubscribe => &self.earn_subscribe,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ThirdPartyBindingPolicy {
    #[serde(default)]
    pub coinbase_wallet_enabled: bool,
    #[serde(default)]
    pub telegram_account_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSecurityPolicy {
    pub login_2fa_mode: LoginTwoFactorMode,
    #[serde(default)]
    pub registration_invite_required: bool,
    #[serde(default)]
    pub username_login_enabled: bool,
    pub payment_policies: PaymentPolicies,
    #[serde(default)]
    pub third_party_bindings: ThirdPartyBindingPolicy,
}

impl DomainLayer for UserSecurityPolicy {}

impl Default for UserSecurityPolicy {
    /// 给出平台安全策略的初始口径，在数据库尚无配置或配置为空时兜底使用。
    /// 取值遵循「资金动作从严、账号功能从宽」的取舍：
    /// 提现默认要求资金密码，现货下单、闪兑与理财申购默认不额外校验，避免高频交易被口令打断。
    /// 登录二次验证默认为用户自选模式，即平台不强制，由用户按需开启。
    /// 注册邀请码与用户名登录默认关闭，第三方绑定入口也全部默认关闭，
    /// 这三项都属于开了才生效的增量能力，默认关闭可确保未显式配置时不会意外放开入口。
    fn default() -> Self {
        Self {
            login_2fa_mode: LoginTwoFactorMode::UserEnabled,
            registration_invite_required: false,
            username_login_enabled: false,
            payment_policies: PaymentPolicies {
                withdraw: PaymentPolicy::fund_password_required(),
                spot_order: PaymentPolicy::disabled(),
                convert: PaymentPolicy::disabled(),
                earn_subscribe: PaymentPolicy::disabled(),
            },
            third_party_bindings: ThirdPartyBindingPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserTwoFactorSettings {
    pub user_id: u64,
    pub totp_secret_encrypted: Option<String>,
    pub totp_enabled: bool,
    pub login_2fa_enabled: bool,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_verified_at: Option<DateTime<Utc>>,
}

impl UserTwoFactorSettings {
    /// 构造用户尚未绑定二次验证时的零值快照，供仓储在查不到设置行时返回，避免上层处理空值。
    /// 密钥、确认时间与最近验证时间全部为空，两个开关均为关闭，
    /// 因此该快照在语义上等价于「从未进入过绑定流程」，可安全用于状态展示与开关判定。
    /// 只在内存中构造，不写库；调用方若需要持久化必须另行执行写入。
    pub(crate) fn empty(user_id: u64) -> Self {
        Self {
            user_id,
            totp_secret_encrypted: None,
            totp_enabled: false,
            login_2fa_enabled: false,
            confirmed_at: None,
            last_verified_at: None,
        }
    }
}

impl DomainLayer for UserTwoFactorSettings {}

#[derive(Debug, Clone, Serialize)]
pub struct AdminTwoFactorSettings {
    pub admin_id: u64,
    pub totp_secret_encrypted: Option<String>,
    pub totp_enabled: bool,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_verified_at: Option<DateTime<Utc>>,
}

impl AdminTwoFactorSettings {
    /// 构造管理员尚未绑定二次验证时的零值快照，用途与用户版本对应但字段少一项。
    /// 管理员没有 `login_2fa_enabled` 开关：后台登录是否要求二次验证由系统策略统一决定，
    /// 不允许管理员自行关闭，这正是两个结构体不能合并的原因。
    /// 同样不含任何密钥且不触发写入。
    pub(crate) fn empty(admin_id: u64) -> Self {
        Self {
            admin_id,
            totp_secret_encrypted: None,
            totp_enabled: false,
            confirmed_at: None,
            last_verified_at: None,
        }
    }
}

impl DomainLayer for AdminTwoFactorSettings {}

#[derive(Debug, Clone)]
pub struct AdminLoginTwoFactorChallenge {
    pub challenge_id: String,
    pub admin_id: u64,
    pub attempt_count: u32,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginTwoFactorChallengeType {
    LoginTwoFactor,
    SetupTwoFactor,
}

impl LoginTwoFactorChallengeType {
    /// 返回挑战类型的稳定存储字符串，用于写入挑战记录并在回读时区分两条流程。
    /// `login_2fa` 表示已绑定用户登录时的动态码验证，`setup_2fa` 表示登录过程中被要求首次完成绑定；
    /// 二者的后续处理完全不同，因此挑战必须携带类型，不能仅凭用户状态推断。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoginTwoFactor => "login_2fa",
            Self::SetupTwoFactor => "setup_2fa",
        }
    }

    /// 把数据库中的挑战类型字符串还原为枚举，是 `as_str` 的逆向映射，两者必须成对维护。
    /// 未知取值一律返回 `AppError::Validation` 而不是回落到任一默认类型：
    /// 挑战类型直接决定登录流程走验证分支还是绑定分支，静默降级可能让本该强制绑定的会话被放行。
    /// 精确匹配且不做大小写归一，写入端只会产生这两个字面量。
    pub(crate) fn from_storage(value: &str) -> AppResult<Self> {
        match value {
            "login_2fa" => Ok(Self::LoginTwoFactor),
            "setup_2fa" => Ok(Self::SetupTwoFactor),
            _ => Err(AppError::Validation("invalid challenge type".to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoginTwoFactorChallenge {
    pub challenge_id: String,
    pub user_id: u64,
    pub challenge_type: LoginTwoFactorChallengeType,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreatedLoginTwoFactorChallenge {
    pub challenge_id: String,
    pub expires_at: DateTime<Utc>,
    pub expires_in_seconds: i64,
}

pub struct SecurityVerificationInput<'a> {
    pub fund_password: Option<&'a str>,
    pub totp_code: Option<&'a str>,
}

/// 使用系统安全随机源生成 20 字节 TOTP 密钥并输出无填充 Base32；返回值等同第二因子凭证，
/// 只可进入加密存储和一次性绑定响应，不得写入普通日志、指标或审计字段。
pub fn generate_totp_secret() -> AppResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; DEFAULT_TOTP_SECRET_BYTES];
    rng.fill(&mut bytes)
        .map_err(|_| AppError::Internal("failed to generate TOTP secret".to_owned()))?;
    Ok(base32_encode_no_padding(&bytes))
}

/// 对发行方和账号做 URI 编码，构造 SHA1、六位、三十秒周期的 TOTP 导入链接。
/// 返回 URI 内含原始 TOTP secret，调用方须按敏感凭证处理并仅交付当前已认证主体。
pub fn totp_otpauth_uri(issuer: &str, account: &str, secret: &str) -> String {
    let label = format!("{}:{}", issuer.trim(), account.trim());
    format!(
        "otpauth://totp/{}?secret={}&issuer={}&algorithm=SHA1&digits={}&period={}",
        percent_encode(&label),
        secret,
        percent_encode(issuer.trim()),
        TOTP_DIGITS,
        TOTP_STEP_SECONDS
    )
}

/// 校验一枚六位 TOTP 动态码是否与给定密钥在当前时刻匹配。
/// 时间窗口是关键语义：除当前时间步外，还接受前一步与后一步共三个候选码，
/// 因此在三十秒步长下实际容错约正负三十秒，用于吸收用户设备与服务器之间的时钟偏差。
/// 这也意味着一枚码的可用时长最长可达约九十秒，防重放不能依赖时间窗口本身，
/// 必须由调用方在业务侧记录已消费的挑战或最近验证时间。
/// 格式不符（长度不对或含非数字）直接返回 `Ok(false)` 而非报错，与码值错误表现一致，
/// 使调用方无法据返回值区分两种失败。
/// 时间戳取负时截断为零，避免早于纪元的时间导致计数器回绕；边界步长用饱和加减防止溢出。
/// 只有 Base32 密钥本身损坏才返回 `AppError::Validation`，这属于数据异常而非用户输入问题。
/// 纯函数，不落库也不更新任何验证时间戳。
pub fn verify_totp_code(secret_base32: &str, code: &str, now: DateTime<Utc>) -> AppResult<bool> {
    let code = code.trim();
    if code.len() != TOTP_DIGITS as usize || !code.chars().all(|value| value.is_ascii_digit()) {
        return Ok(false);
    }
    let secret = base32_decode_no_padding(secret_base32)?;
    let timestamp = now.timestamp().max(0) as u64;
    for offset in [-1_i64, 0, 1] {
        let candidate_timestamp = if offset.is_negative() {
            timestamp.saturating_sub(TOTP_STEP_SECONDS)
        } else if offset.is_positive() {
            timestamp.saturating_add(TOTP_STEP_SECONDS)
        } else {
            timestamp
        };
        if totp_code_for_time(&secret, candidate_timestamp, TOTP_STEP_SECONDS, TOTP_DIGITS) == code
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 将时间戳按步长换算为计数器并生成指定位数 HOTP。
/// 调用方必须传入非零 `step_seconds`，否则整数除法会 panic；`digits` 也须位于 `u32` 十次幂范围内。
pub fn totp_code_for_time(secret: &[u8], timestamp: u64, step_seconds: u64, digits: u32) -> String {
    let counter = timestamp / step_seconds;
    hotp_code(secret, counter, digits)
}

/// 把二进制密钥编码为 RFC 4648 大写 Base32 文本，刻意省略末尾 `=` 填充。
/// 省略填充是为了兼容主流验证器：多数应用在扫码或手工录入时不接受带填充的密钥串。
/// 实现按每五位取一个字母滚动输出，末尾不足五位时左移补零凑满一个字符，
/// 因此二十字节密钥固定产出三十二个字符。
/// 空输入返回空字符串，不视为错误。
/// 输出等同于第二因子凭证明文，调用方只能交给加密存储或一次性绑定响应，不得记录。
pub fn base32_encode_no_padding(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut buffer = 0_u32;
    let mut bits_left = 0_u8;

    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits_left += 8;
        while bits_left >= 5 {
            let index = ((buffer >> (bits_left - 5)) & 0b11111) as usize;
            output.push(BASE32_ALPHABET[index] as char);
            bits_left -= 5;
        }
    }

    if bits_left > 0 {
        let index = ((buffer << (5 - bits_left)) & 0b11111) as usize;
        output.push(BASE32_ALPHABET[index] as char);
    }

    output
}

/// 把 Base32 文本解码回密钥字节，容错口径比编码端更宽以适应各种来源的密钥串。
/// 具体宽容之处：首尾空白被裁剪，字母大小写均可，`=` 出现在任意位置都被跳过而非仅限末尾。
/// 之所以不严格要求无填充，是因为解码端可能收到从其他系统迁移来的带填充密钥。
/// 除此之外的任何字符立即返回 `AppError::Validation`，不做静默忽略，避免把脏数据解成一段错误密钥。
/// 每凑满八位输出一个字节，末尾不足八位的余数位被丢弃，这与编码端的补零规则互为逆操作。
/// 不校验解出的字节数是否符合业务预期长度，长度判定由调用方按需要另行处理。
pub fn base32_decode_no_padding(value: &str) -> AppResult<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0_u32;
    let mut bits_left = 0_u8;

    for character in value.trim().chars() {
        if character == '=' {
            continue;
        }
        let index = base32_value(character).ok_or_else(|| {
            AppError::Validation("TOTP secret contains invalid base32 character".to_owned())
        })?;
        buffer = (buffer << 5) | u32::from(index);
        bits_left += 5;
        if bits_left >= 8 {
            output.push(((buffer >> (bits_left - 8)) & 0xff) as u8);
            bits_left -= 8;
        }
    }

    Ok(output)
}

/// 解码安全策略 JSON，兼容字符串包装对象及历史 `0/1` 布尔形式。
/// 空值使用安全默认策略，结构损坏返回内部错误；过程不回写数据库。
pub fn decode_security_policy_value(value: Value) -> AppResult<UserSecurityPolicy> {
    let mut value = match value {
        Value::Null => return Ok(UserSecurityPolicy::default()),
        Value::String(text) if text.trim_start().starts_with('{') => {
            serde_json::from_str::<Value>(&text).map_err(|error| {
                AppError::Internal(format!(
                    "failed to parse user security policy JSON: {error}"
                ))
            })?
        }
        other => other,
    };
    normalize_security_policy_bool_fields(&mut value);
    serde_json::from_value(value).map_err(|error| {
        AppError::Internal(format!("failed to decode user security policy: {error}"))
    })
}

/// 从安全校验输入中取出一个必填字段，裁剪空白后要求非空。
/// 缺失与空串合并处理，两者都返回带 `security_verification_required` 码的安全校验错误，
/// 而非普通的字段校验错误：前端据此统一弹出安全验证面板，无需针对每个字段单独判断。
/// 错误文案不指明缺的是资金密码还是动态码，避免据此反推账号启用了哪些校验方式。
pub(crate) fn required_security_field(value: Option<&str>) -> AppResult<&str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::security_validation("security_verification_required", "请完成安全校验")
        })
}

/// 构造「登录二次验证挑战已失效」这一固定错误，集中定义错误码与提示文案。
/// 收敛到一处是为了让挑战不存在、已过期、已被消费、尝试次数用尽这几种情况返回完全相同的响应，
/// 攻击者无法据错误差异判断某个挑战 ID 是否真实存在或还剩几次机会。
/// 错误码 `login_2fa_challenge_expired` 是前端识别「需重新登录」的稳定契约，不应随文案调整。
pub(crate) fn login_challenge_expired() -> AppError {
    AppError::security_validation("login_2fa_challenge_expired", "登录验证已过期，请重新登录")
}

/// 就地递归遍历策略 JSON，把历史遗留的非布尔写法归一成真正的布尔值，为严格反序列化扫清障碍。
/// 早期版本把开关写成数字 `0/1` 或字符串 `"true"`，而策略结构体带 `deny_unknown_fields`
/// 且字段类型为 `bool`，不做归一这些历史记录会直接解码失败。
/// 遍历同时下探对象与数组，因此嵌套在支付策略内部的开关也能被覆盖。
/// 命中布尔键且能成功折算时替换该节点并跳过下探（布尔是叶子节点，无需继续递归）；
/// 折算失败则保留原值继续下探，把最终的类型错误交给反序列化阶段报出，而不是在此吞掉。
/// 只修改传入的内存中 JSON，不回写数据库，因此每次读取都会重新归一一遍。
fn normalize_security_policy_bool_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if is_security_policy_bool_key(key)
                    && let Some(normalized) = coerce_legacy_bool_value(child)
                {
                    *child = Value::Bool(normalized);
                    continue;
                }
                normalize_security_policy_bool_fields(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_security_policy_bool_fields(item);
            }
        }
        _ => {}
    }
}

/// 按键名判断某个 JSON 字段是否属于应当归一为布尔的开关项。
/// 命中三类：支付策略内的 `enabled`、以 `_enabled` 结尾的各类开关（登录、第三方绑定、用户名登录等），
/// 以及显式列出的 `registration_invite_required`——它是唯一不带 `enabled` 后缀的布尔字段。
/// 用后缀匹配而非硬编码全量键名，可让后续新增的 `_enabled` 开关自动获得历史兼容能力。
fn is_security_policy_bool_key(key: &str) -> bool {
    key == "enabled" || key.ends_with("_enabled") || key == "registration_invite_required"
}

/// 把一个 JSON 节点尝试折算为布尔值，覆盖历史上出现过的三种写法。
/// 已是布尔的原样返回；数字只接受 `0` 与 `1`，其余数值返回 `None` 而不按非零即真处理，
/// 因为出现 `2` 之类的取值说明数据本身有问题，不应猜测其意图。
/// 字符串先裁空白再转小写，接受 `0/false/no/off` 与 `1/true/yes/on` 两组常见写法。
/// 返回 `None` 表示无法安全折算，调用方会保留原值并让后续反序列化报出明确的类型错误，
/// 这一保守取舍确保不会把含义不明的数据静默解释成「开启」。
fn coerce_legacy_bool_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .and_then(|value| match value {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            }),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" => Some(false),
            "1" | "true" | "yes" | "on" => Some(true),
            _ => None,
        },
        _ => None,
    }
}

/// 按 RFC 4226 计算一枚 HOTP 动态码，是 TOTP 的底层算法，区别只在于计数器来源。
/// 步骤为：以密钥作 HMAC-SHA1 密钥、对计数器的八字节大端表示求 MAC，
/// 取摘要末字节低四位作偏移量，从该偏移读四字节并抹掉最高位得到三十一位整数，
/// 最后对十的 `digits` 次幂取模并左侧补零到固定位数。
/// 抹掉最高位是规范要求，用于消除不同平台对有符号整数解释的差异。
/// 两个 panic 前提：`HmacSha1` 接受任意长度密钥故构造不会失败；
/// `digits` 必须使十的该次幂不超过 `u32` 上限，即不得大于九，越界会在幂运算处溢出 panic。
/// 调用方均传入常量 `TOTP_DIGITS`，因此该边界在本仓库内不会被触及。
fn hotp_code(secret: &[u8], counter: u64, digits: u32) -> String {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC supports variable key length");
    mac.update(&counter.to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let offset = usize::from(hash[hash.len() - 1] & 0x0f);
    let binary = ((u32::from(hash[offset]) & 0x7f) << 24)
        | (u32::from(hash[offset + 1]) << 16)
        | (u32::from(hash[offset + 2]) << 8)
        | u32::from(hash[offset + 3]);
    let modulus = 10_u32.pow(digits);
    format!("{:0width$}", binary % modulus, width = digits as usize)
}

/// 把单个 Base32 字符映射为它代表的五位数值，大小写字母等价处理。
/// 字母 A 到 Z 对应 0 到 25，数字 2 到 7 对应 26 到 31，这正是 RFC 4648 字母表的排列。
/// 注意字母表刻意不含 0、1、8、9：它们与 O、I、B 等字母形态相近，排除后可降低人工抄录密钥时的误读率。
/// 任何不在表内的字符返回 `None`，由调用方转成校验错误。
fn base32_value(character: char) -> Option<u8> {
    match character.to_ascii_uppercase() {
        'A'..='Z' => Some(character.to_ascii_uppercase() as u8 - b'A'),
        '2'..='7' => Some(character as u8 - b'2' + 26),
        _ => None,
    }
}

/// 对字符串做百分号编码，用于把发行方名称与账号标签安全嵌入 otpauth 链接。
/// 保留集仅限 RFC 3986 定义的未保留字符（字母、数字与 `-._~`），其余字节一律转成 `%XX` 大写十六进制。
/// 采取这种最小保留集是有意为之：账号标签常含邮箱的 `@`、路径分隔用的 `:` 以及中文用户名，
/// 不编码会破坏 URI 结构或让验证器解析出错。
/// 按字节而非字符遍历，因此多字节 UTF-8 会被逐字节编码，符合 URI 规范对非 ASCII 的处理要求。
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
