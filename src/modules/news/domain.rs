//! news bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 公开新闻查询的全部入参约束集中在此：分页边界、可选文本归一、分类白名单、国家代码与语言代码格式。
//! 这些校验都发生在拼装 SQL 之前，因为地区和语言条件最终会参与 JSON_SEARCH 与比较表达式，
//! 提前收敛取值既避免把前端自造的分类或畸形代码带进查询，也保证过滤语义可预期。
//! 本文件不访问数据库，也不感知新闻的发布状态机。

use crate::{
    architecture::DomainLayer,
    error::{AppError, AppResult},
};

/// 公开新闻列表的查询条件，可选字段为空表示该维度不参与过滤。
#[derive(Debug, Clone, Default)]
pub struct PublicNewsFilter {
    /// 新闻分类，取值限于五类白名单之一。
    pub category: Option<String>,
    /// 目标国家代码，匹配时会同时放行全局与未限定地区的新闻。
    pub country_code: Option<String>,
    /// 目标语言代码，按语言族在多语言内容项中做包含匹配。
    pub locale: Option<String>,
    /// 关键词，同时对标题与多语言内容原文做模糊匹配。
    pub keyword: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

impl DomainLayer for PublicNewsFilter {}

/// 将公开新闻页大小限制在 1 到 100，缺省返回 50，避免公共查询无界扫描。
/// 该接口无需登录即可访问，因此上限是硬约束而非建议值，超出部分直接夹断而不报错，
/// 以免正常客户端因页大小写大了就整个请求失败；下界为一保证不会退化成空查询。
pub fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 将公开新闻偏移限制在一万以内，避免深分页拖垮内容表查询。
/// 新闻查询带 JSON_SEARCH 与全文模糊匹配，偏移越大数据库需要跳过的行越多，成本近似线性上升，
/// 因此对未认证入口设硬顶；超出部分同样夹断而非报错，缺省从零开始。
pub fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(10_000)
}

/// 裁剪公开新闻可选查询文本，并把纯空白输入归一为空条件。
/// 前端在未选择筛选项时常会传空串或空格，若原样带入会生成恒不成立的等值条件导致列表莫名为空，
/// 因此这里统一折叠为未设置，让该维度彻底不参与 SQL 拼装；非空文本只去首尾空白，不改内部内容与大小写。
pub fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 仅接受后台公开合同定义的五类新闻分类，禁止公共查询使用前端自造分类。
/// 白名单依次是通用、行情、产品、系统与活动推广，比较严格区分大小写且不做去空白处理，
/// 调用方须先经可选文本归一；任何其他取值直接判为校验错误，不会退化成不过滤而返回全量新闻。
pub fn validate_news_category(value: &str) -> AppResult<String> {
    match value {
        "general" | "market" | "product" | "system" | "promotion" => Ok(value.to_owned()),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

/// 将新闻国家代码转为大写；GLOBAL 保留全局语义，其余值只允许安全的字母数字与分隔符。
/// 非法代码在构建 SQL 条件前失败，避免错误地区过滤或字符串拼接。
/// GLOBAL 表示面向所有地区的全站公告，转大写后即提前返回，不再受长度与字符白名单约束；
/// 其余代码限长二到十六，且只允许字母数字与连字符、下划线，据此排除通配符与引号等可被滥用的字符。
pub fn normalize_news_country_code(value: &str) -> AppResult<String> {
    let country_code = value.to_ascii_uppercase();
    if country_code == "GLOBAL" {
        return Ok(country_code);
    }
    if country_code.len() < 2
        || country_code.len() > 16
        || !country_code.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(AppError::Validation(
            "news country_code format is invalid".to_owned(),
        ));
    }
    Ok(country_code)
}

/// 校验语言代码格式：长度须在二到十六之间，且只允许 ASCII 字母数字与连字符。
/// 字符白名单是关键约束，该值随后会作为 JSON_SEARCH 的搜索目标进入查询，
/// 放行百分号或下划线会让调用方获得通配能力从而绕过语言过滤，放行引号等字符则扩大注入面。
/// 校验通过后原样返回，不做大小写归一，因此存储侧与查询侧必须使用一致的书写形式。
fn validate_news_locale(value: &str) -> AppResult<String> {
    if value.len() < 2
        || value.len() > 16
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::Validation(
            "news locale format is invalid".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

/// 为完整或短语言代码生成 JSON_SEARCH 匹配集合：完整代码回退语言族，短代码覆盖地区变体。
/// 语言格式非法时整体失败，不以任意内容或默认语言扩大公开查询。
pub fn news_locale_search_patterns(value: &str) -> AppResult<Vec<String>> {
    let locale = validate_news_locale(value)?;
    let mut patterns = vec![locale.clone()];
    if let Some((language, _region)) = locale.split_once('-') {
        patterns.push(language.to_owned());
    } else {
        patterns.push(format!("{locale}-%"));
    }
    Ok(patterns)
}
