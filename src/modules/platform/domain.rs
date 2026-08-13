//! platform bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 平台品牌配置的全部校验规则集中在此：站点名称必填且限长、Logo 地址限定协议形态、
//! 图表提供方限定为两个受支持的取值。校验产出的是一个已归一化的值对象，
//! 空白折叠与大小写归一都在此完成，写入侧因此不必再处理原始输入。
//! 本文件为纯计算，不读写数据库、不发布事件，也不感知配置的版本或审计。

use crate::architecture::DomainLayer;
use chrono::{DateTime, Utc};

/// 平台品牌配置的固定行名，整张表按业务约定只维护这一行全局配置。
pub const DEFAULT_CONFIG_NAME: &str = "default";
/// 默认图表提供方，未显式配置时由数据库默认值落到该取值。
pub const DEFAULT_CHART_PROVIDER: &str = "klinecharts";
/// 另一个受支持的图表提供方，切换后前端加载对应的行情图表组件。
pub const TRADINGVIEW_CHART_PROVIDER: &str = "tradingview";

/// 品牌配置的领域校验错误，只携带一条稳定文案，由应用层转成统一的参数错误。
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("validation error: {message}")]
pub struct PlatformBrandValidationError {
    message: String,
}

impl PlatformBrandValidationError {
    /// 用给定文案构造领域校验错误，文案会原样呈现给调用方，因此只应描述哪个字段不合规，
    /// 不得拼入用户提交的原始内容，避免把非法输入回显到接口响应里。
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// 消费领域校验错误并提取稳定消息，供应用层映射为统一参数错误。
    /// 按值接收是刻意的：错误一旦被取走文案就完成了跨层转换，不应再被复用或二次上报，
    /// 领域层因而不必对外暴露内部字段，也不会出现同一错误被包装成多种响应的情况。
    pub(crate) fn into_message(self) -> String {
        self.message
    }
}

/// 更新品牌配置的原始意图，字段取自后台请求体，尚未经过任何归一或校验。
#[derive(Debug)]
pub struct PlatformBrandCommand {
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: Option<String>,
}

/// 品牌配置的完整快照，对应配置表中那唯一一行，供后台回显与前端初始化使用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformBrand {
    pub id: u64,
    /// 配置行名，固定为 default，用于在表内定位这行全局配置。
    pub name: String,
    /// 对外展示的站点名称。
    pub platform_name: String,
    pub logo_url: Option<String>,
    /// 生效的图表提供方，读取时必有值，未配置过则为数据库默认值。
    pub chart_provider: String,
    /// 最后一次修改该配置的管理员，从未被后台改过时为空。
    pub updated_by: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DomainLayer for PlatformBrand {}

/// 校验通过后的品牌配置，文本已去空白、图表提供方已转小写。
/// 可选字段为空表示本次不修改该项，写入侧会保留数据库现有取值而非清空。
#[derive(Debug)]
pub struct ValidatedPlatformBrand {
    pub platform_name: String,
    pub logo_url: Option<String>,
    pub chart_provider: Option<String>,
}

impl DomainLayer for ValidatedPlatformBrand {}

/// 校验并规范化平台名称、Logo 地址和图表提供方配置。
/// 所有规则均为纯计算；任一字段非法时整体失败，不产生部分配置或持久化副作用。
pub fn validate_platform_brand(
    command: PlatformBrandCommand,
) -> Result<ValidatedPlatformBrand, PlatformBrandValidationError> {
    Ok(ValidatedPlatformBrand {
        platform_name: required_string(Some(command.platform_name), "platform_name", 128)?,
        logo_url: validate_logo_url(command.logo_url)?,
        chart_provider: validate_chart_provider(command.chart_provider)?,
    })
}

/// 校验图表提供方取值，仅接受内置的两种行情图表实现，返回结果已统一为小写。
/// 缺省或纯空白视为不修改该项，交由写入侧保留数据库现值，这与显式传入非法值必须报错是两回事。
/// 大小写归一在白名单比较之前完成，因此后台录入的大写形式同样可用；
/// 白名单之外的取值一律拒绝，避免前端拿到一个无对应实现的提供方名而整个行情页无法渲染。
fn validate_chart_provider(
    value: Option<String>,
) -> Result<Option<String>, PlatformBrandValidationError> {
    let Some(provider) = optional_string(value) else {
        return Ok(None);
    };
    let provider = provider.to_ascii_lowercase();
    if matches!(
        provider.as_str(),
        DEFAULT_CHART_PROVIDER | TRADINGVIEW_CHART_PROVIDER
    ) {
        Ok(Some(provider))
    } else {
        Err(PlatformBrandValidationError::new(
            "chart_provider must be klinecharts or tradingview",
        ))
    }
}

/// 校验站点 Logo 地址，缺省或纯空白表示不设置该项，其余情况按三档规则依次判定。
/// 先限长两千零四十八个字符以容纳内联图片而又不至于撑爆配置行；
/// 再拒绝任何控制字符与空白字符，因为该地址会直接落进页面属性，混入这类字符可能被用来截断或注入标记；
/// 最后要求以 https、http、根相对斜杠或 data:image 前缀开头，把取值限制在浏览器可安全加载的形态内，
/// 顺带排除 javascript 之类的伪协议。校验通过时返回原始大小写文本，只有前缀判定过程使用小写副本。
fn validate_logo_url(
    value: Option<String>,
) -> Result<Option<String>, PlatformBrandValidationError> {
    let Some(logo_url) = optional_string(value) else {
        return Ok(None);
    };
    if logo_url.chars().count() > 2048 {
        return Err(PlatformBrandValidationError::new("logo_url is too long"));
    }
    if logo_url.chars().any(char::is_control) || logo_url.chars().any(char::is_whitespace) {
        return Err(PlatformBrandValidationError::new(
            "logo_url format is invalid",
        ));
    }
    let lower = logo_url.to_ascii_lowercase();
    if lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("/")
        || lower.starts_with("data:image/")
    {
        Ok(Some(logo_url))
    } else {
        Err(PlatformBrandValidationError::new(
            "logo_url must be http(s), root-relative, or data:image",
        ))
    }
}

/// 校验一个必填文本字段：去空白后不得为空，且字符数不得超过给定上限。
/// 缺省与纯空白同等对待，都报字段必填，避免用空格绕过必填约束写出一个视觉上为空的站点名。
/// 长度按 Unicode 字符计数而非字节，中文站点名因此不会被字节口径误判为超长。
/// 错误文案只带字段名不带原值，两类失败分别提示必填与超长，便于前端定位。
fn required_string(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> Result<String, PlatformBrandValidationError> {
    let Some(value) = optional_string(value) else {
        return Err(PlatformBrandValidationError::new(format!(
            "{field} is required"
        )));
    };
    if value.chars().count() > max_chars {
        return Err(PlatformBrandValidationError::new(format!(
            "{field} is too long"
        )));
    }
    Ok(value)
}

/// 去除文本首尾空白并把空串折叠为未设置，是本文件其余校验共用的第一道归一。
/// 这样一来后台把输入框清空提交空串，与根本不传该字段，会得到完全一致的处理结果。
fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
