//! countries bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 国家配置的取值约束集中在此：国家代码统一大写并限定为纯 ASCII 字母，
//! 语言代码统一小写并限定在平台已提供文案的白名单内，支持语言列表去重后不得为空，
//! 且默认语言必须是支持语言之一，避免出现一个前端无法回退的孤立默认值。
//! 这些校验在写入配置和参与查询条件之前完成，是纯计算，不访问数据库。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

/// 平台已提供完整文案资源的语言集合，国家配置中的语言只能取自这里。
const ALLOWED_LOCALES: &[&str] = &["zh", "en"];

/// 对外可见的国家配置，仅包含已启用且开放注册的国家。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicCountry {
    /// 大写国家代码，注册与地区筛选均以此为准。
    pub country_code: String,
    /// 国家显示名称，由后台维护，不做多语言拆分。
    pub country_name: String,
    /// 该国默认语言，必定出现在支持语言列表中。
    pub default_locale: String,
    /// 该国可选语言，按后台配置的首次出现顺序排列且已去重。
    pub supported_locales: Vec<String>,
}

impl DomainLayer for PublicCountry {}

/// 规范化国家代码为大写 ASCII 字母，并限制为 2～8 个字符。
/// 空值或包含数字、符号的代码返回参数错误，不执行任何外部查询。
/// 统一大写是为了让存储与查询共用一种书写形式，否则同一国家会因大小写差异分裂成两条配置。
/// 纯字母的字符白名单同时承担安全职责：该代码会作为条件参与国家配置与新闻等地区过滤查询，
/// 提前排除引号、百分号、下划线之类字符可以杜绝通配绕过与拼接风险，因此校验必须早于任何 SQL 构造。
pub fn normalize_country_code(value: &str) -> AppResult<String> {
    let country_code = value.trim().to_ascii_uppercase();
    if country_code.is_empty() {
        return Err(AppError::Validation("country_code is required".to_owned()));
    }
    if country_code.len() < 2
        || country_code.len() > 8
        || !country_code
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(AppError::Validation(
            "country_code format is invalid".to_owned(),
        ));
    }
    Ok(country_code)
}

/// 规范化语言代码为小写，并只接受平台明确支持的语言集合。
/// 当前白名单仅有中文与英文，去空白与小写归一都在白名单比较之前完成，
/// 因此后台录入的大写或带空格写法同样可用，而未在白名单内的语言一律判为校验错误。
/// 白名单是硬约束而非建议：国家配置里的语言最终决定前端加载哪套文案包，
/// 放进一个没有翻译资源的语言会让该国用户看到空白界面，所以宁可在保存时就拒绝。
pub fn normalize_locale(value: &str) -> AppResult<String> {
    let locale = value.trim().to_ascii_lowercase();
    if !ALLOWED_LOCALES.contains(&locale.as_str()) {
        return Err(AppError::Validation("unsupported locale".to_owned()));
    }
    Ok(locale)
}

/// 逐项规范化并按首次出现顺序去重支持语言列表。
/// 任一语言非法或去重后为空时整体失败，避免保存部分有效的配置。
/// 保留首次出现顺序而非重新排序，是因为该顺序会直接决定前端语言切换器的展示次序，属于运营可控项。
/// 遇到非法语言立刻中断而不是跳过，确保运营在保存后看到的列表与提交内容完全一致，不会静默少掉一项。
pub fn normalize_supported_locales(values: Vec<String>) -> AppResult<Vec<String>> {
    let mut locales = Vec::new();
    for value in values {
        let locale = normalize_locale(&value)?;
        if !locales.contains(&locale) {
            locales.push(locale);
        }
    }
    if locales.is_empty() {
        return Err(AppError::Validation(
            "supported_locales is required".to_owned(),
        ));
    }
    Ok(locales)
}

/// 确认默认语言包含在已规范化的支持语言列表中。
/// 本函数只校验成员关系，不修改列表顺序或自动补入默认值。
/// 两侧必须都已经过语言归一后才比较，否则大小写差异会让本应通过的配置被误判为不一致。
/// 不自动补入是刻意的：默认语言落在支持列表之外说明运营配置本身有歧义，
/// 静默补齐会让用户拿到一门本不打算开放的语言，因此宁可让保存失败并要求显式修正。
pub fn ensure_default_locale_supported(
    default_locale: &str,
    supported_locales: &[String],
) -> AppResult<()> {
    if supported_locales
        .iter()
        .any(|locale| locale == default_locale)
    {
        Ok(())
    } else {
        Err(AppError::Validation(
            "default_locale must be included in supported_locales".to_owned(),
        ))
    }
}
