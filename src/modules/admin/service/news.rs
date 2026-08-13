//! 站内新闻内容的纯业务规则层，核心是那套自定义富文本文档格式的结构校验。
//!
//! 除标题、分类、状态、地域、语言等标量字段的规范化外，本文件用一组互相递归的私有函数
//! 逐层校验富文本：文档层确认版本号与默认语言、条目层确认语言与地域组合唯一、
//! 块层区分图片块与文本块并限制可用标签、叶子层限制可用格式标记。
//! 校验采用白名单策略，凡出现未知字段一律拒绝，从而防止未经审查的结构混进正文；
//! 但这里只管结构不管渲染安全，HTML 转义与展示策略仍由前端合同和录入权限负责。

use super::*;

/// 去除可选新闻图片 URL 的首尾空白，空串转为 None，并限制字段长度防止超出存储列。
/// 只校验文本形状，不下载图片或验证远端资源可达性。
pub(crate) fn validate_optional_image_url(
    value: Option<String>,
    field: &str,
) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

/// 去除新闻标题首尾空白并限制为数据库允许的最大字符数；空标题直接拒绝。
/// 上限 255 按字符数统计，中英文可用长度一致；该函数同时被富文本条目内的标题字段复用。
pub(crate) fn validate_news_title(value: &str) -> AppResult<String> {
    let Some(title) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news title is required".to_owned()));
    };
    if title.chars().count() > 255 {
        return Err(AppError::Validation("news title is too long".to_owned()));
    }
    Ok(title)
}

/// 规范化新闻分类为后台合同支持的稳定代码，空白或未支持分类返回校验错误。
/// 仅接受 general、market、product、system、promotion 五个取值，去空后区分大小写比对，不做大小写归一。
pub(crate) fn validate_news_category(value: &str) -> AppResult<String> {
    let Some(category) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news category is required".to_owned()));
    };
    match category.as_str() {
        "general" | "market" | "product" | "system" | "promotion" => Ok(category),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

/// 规范化新闻草稿/发布等生命周期状态；这里只校验目标值，不判断当前状态迁移或发布时间。
/// 三个取值 draft、published、archived 之间没有迁移限制，任意方向切换都会被本函数放行，
/// 首次发布时间的写入时机由应用层根据旧值判断，与此处无关。
pub(crate) fn validate_news_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "published" | "archived" => Ok(status),
        _ => Err(AppError::Validation("unsupported news status".to_owned())),
    }
}

/// 规范化可选新闻国家代码：None 或空白保持无筛选，其余委托严格国家代码规则。
/// 返回值只表示单个适用区域，不执行集合去重或国家配置查询；非法代码沿用内部校验错误。
pub(crate) fn normalize_optional_news_country_code(
    value: Option<String>,
) -> AppResult<Option<String>> {
    value
        .map(|value| normalize_news_country_code(&value))
        .transpose()
}

/// 将新闻国家代码去除空白并转为大写，接受 GLOBAL 或长度 2..=16 的 ASCII 字母数字及 `-_`。
/// 空白、超长或非法字符返回校验错误；函数不验证代码是否存在于国家配置表。
/// GLOBAL 是保留字面量，表示该条内容不限地区，会在长度与字符集校验之前直接短路放行。
/// 由于不查国家配置表，这里通过的代码可能对应一个尚未配置甚至已停用的国家，投放范围需另行核对。
pub(crate) fn normalize_news_country_code(value: &str) -> AppResult<String> {
    let Some(country_code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "news country_code is required".to_owned(),
        ));
    };
    let country_code = country_code.to_ascii_uppercase();
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

/// 规范化新闻 locale 并限制语言标签长度与字符格式，供标题、摘要和正文多语言文档使用。
/// 只接受长度 2 到 16 字节、由 ASCII 字母数字和横线组成的标签，因而 zh-CN 这类写法合法而带下划线的不合法。
/// 与国家代码不同，这里保留原始大小写不做归一，所以富文本条目里的语言标签必须与请求默认语言逐字一致才能匹配上。
pub(crate) fn validate_news_locale(value: &str) -> AppResult<String> {
    let Some(locale) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news locale is required".to_owned()));
    };
    if locale.len() < 2
        || locale.len() > 16
        || !locale
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::Validation(
            "news locale format is invalid".to_owned(),
        ));
    }
    Ok(locale)
}

/// 校验新闻富文本多语言对象，并保证默认 locale 对应内容存在且可用于公开详情页。
/// 函数只规范化 JSON 文档；HTML 安全渲染策略仍由前端合同和内容录入权限共同保证。
/// 文档顶层只允许 version、default_locale、items 三个键，version 必须恰好为 1，
/// 文档内声明的默认语言必须与请求传入的默认语言完全一致，否则视为两边配置打架而直接报错。
/// items 不得为空，每个条目只允许 locale、country_code、title、summary、content 五个键，
/// 语言与地域的组合在文档内必须唯一，且必须至少有一条命中默认语言，保证详情页总有可回落的内容。
/// 摘要为可选项，正文数组不得为空且必须至少产出一段非空文本；校验通过后原样返回入参而不做任何改写。
pub(crate) fn validate_news_content_document(
    value: Value,
    default_locale: &str,
) -> AppResult<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Validation("news content must be an object".to_owned()))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "version" | "default_locale" | "items"))
    {
        return Err(AppError::Validation(
            "news content field is unsupported".to_owned(),
        ));
    }
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "news content version must be 1".to_owned(),
        ));
    }
    let content_default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(validate_news_locale)
        .transpose()?
        .ok_or_else(|| {
            AppError::Validation("news content default_locale is required".to_owned())
        })?;
    if content_default_locale != default_locale {
        return Err(AppError::Validation(
            "news content default_locale must match request default_locale".to_owned(),
        ));
    }
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| AppError::Validation("news content items are required".to_owned()))?;
    let mut has_default_locale = false;
    let mut seen = HashSet::new();
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("news content item must be an object".to_owned())
        })?;
        if item_object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "locale" | "country_code" | "title" | "summary" | "content"
            )
        }) {
            return Err(AppError::Validation(
                "news content item field is unsupported".to_owned(),
            ));
        }
        let locale = required_news_content_string(item_object.get("locale"), "locale")?;
        let locale = validate_news_locale(locale)?;
        if locale == default_locale {
            has_default_locale = true;
        }
        let country_code =
            required_news_content_string(item_object.get("country_code"), "country_code")?;
        let country_code = normalize_news_country_code(country_code)?;
        if !seen.insert((locale, country_code)) {
            return Err(AppError::Validation(
                "news content locale and country_code must be unique".to_owned(),
            ));
        }
        validate_news_title(required_news_content_string(
            item_object.get("title"),
            "title",
        )?)?;
        if let Some(summary) = item_object.get("summary") {
            validate_news_summary(summary)?;
        }
        let content = item_object
            .get("content")
            .and_then(Value::as_array)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                AppError::Validation("news content item content is required".to_owned())
            })?;
        if !validate_news_rich_text_content(content)? {
            return Err(AppError::Validation(
                "news content body is required".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "news content default_locale must exist in items".to_owned(),
        ));
    }
    Ok(value)
}

/// 将后台新闻标题、图片、分类、状态、地域、默认语言、发布时间和管理员归属映射为审计快照。
/// 快照刻意不复制富文本正文，避免审计记录膨胀；应用层在新闻写事务中保存前后值。
/// 代价是仅改动正文的操作在审计前后值上看不出差异，只能靠更新人与更新时间的变化推断，
/// 因此正文改动的具体内容需要依赖操作原因说明。创建人与更新人分别记录，便于区分首发者和最后修改者。
pub(crate) fn admin_news_item_audit_json(news: &AdminNewsItemResponse) -> Value {
    json!({
        "id": news.id,
        "title": news.title,
        "banner_url": news.banner_url,
        "small_logo_url": news.small_logo_url,
        "category": news.category,
        "status": news.status,
        "country_code": news.country_code,
        "default_locale": news.default_locale,
        "published_at": news.published_at.map(|value| value.timestamp_millis()),
        "created_by_admin_id": news.created_by_admin_id,
        "updated_by_admin_id": news.updated_by_admin_id,
        "created_at": news.created_at.timestamp_millis(),
        "updated_at": news.updated_at.timestamp_millis(),
    })
}

/// 逐块校验富文本数组，并汇总出「整段内容里是否存在任何非空文本或图片」这一结论。
/// 注意布尔或运算写在右侧，因此即便前面的块已判定有内容，后续块仍会被完整校验而不会短路跳过。
/// 返回 false 表示结构合法但通篇为空白，是否接受由调用方按字段语义决定。
fn validate_news_rich_text_content(content: &[Value]) -> AppResult<bool> {
    let mut has_content = false;
    for node in content {
        has_content = validate_news_rich_text_block(node)? || has_content;
    }
    Ok(has_content)
}

/// 校验条目摘要，兼容纯字符串与富文本数组两种历史写法。
/// JSON null 视为未填写直接放行；字符串形态按 512 字符上限判定；
/// 数组形态先做完整的富文本结构校验，再只统计叶子节点纯文本的字符数并同样限制在 512 以内，
/// 因此标签与格式标记不计入长度。既不是字符串也不是非空数组的取值一律判为非法。
fn validate_news_summary(value: &Value) -> AppResult<()> {
    if value.is_null() {
        return Ok(());
    }
    if let Some(summary) = value.as_str() {
        if summary.chars().count() > 512 {
            return Err(AppError::Validation(
                "news content summary is too long".to_owned(),
            ));
        }
        return Ok(());
    }
    let summary = value
        .as_array()
        .filter(|summary| !summary.is_empty())
        .ok_or_else(|| {
            AppError::Validation("news content summary must be a string or rich text".to_owned())
        })?;
    validate_news_rich_text_content(summary)?;
    if news_rich_text_text_length(summary) > 512 {
        return Err(AppError::Validation(
            "news content summary is too long".to_owned(),
        ));
    }
    Ok(())
}

/// 统计富文本中所有叶子节点纯文本的字符总数，用于摘要长度限制。
/// 只下钻一层 children，因此嵌套更深的结构不会被计入；图片块没有 children 也自然不贡献长度。
/// 该函数不做校验，遇到形状不符的节点直接跳过，调用前应先通过结构校验。
fn news_rich_text_text_length(content: &[Value]) -> usize {
    content
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|object| object.get("children"))
        .filter_map(Value::as_array)
        .flat_map(|children| children.iter())
        .filter_map(Value::as_object)
        .filter_map(|leaf| leaf.get("text"))
        .filter_map(Value::as_str)
        .map(|text| text.chars().count())
        .sum()
}

/// 校验单个富文本块并返回该块是否贡献了实际内容。
/// 先按 type 分流：image 交给图片块专用校验，其余块只允许 type 与 children 两个键，
/// 且类型必须是 p、h1、h2、h3、blockquote 之一，children 必须存在且非空。
/// 块的返回值取所有子节点的或结果，因此整块只含空白文本时返回 false 而非报错。
fn validate_news_rich_text_block(node: &Value) -> AppResult<bool> {
    let object = node
        .as_object()
        .ok_or_else(invalid_news_rich_text_content)?;
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_news_rich_text_content)?;
    if node_type == "image" {
        return validate_news_rich_text_image_block(object);
    }
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "children"))
    {
        return Err(invalid_news_rich_text_content());
    }
    if !matches!(node_type, "p" | "h1" | "h2" | "h3" | "blockquote") {
        return Err(invalid_news_rich_text_content());
    }
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .filter(|children| !children.is_empty())
        .ok_or_else(invalid_news_rich_text_content)?;
    let mut has_text = false;
    for child in children {
        has_text = validate_news_rich_text_child(child)? || has_text;
    }
    Ok(has_text)
}

/// 校验图片块：只允许 type、url、alt 三个键，url 必填且复用新闻图片地址的长度规则。
/// alt 为可选，必须是字符串且不超过 256 字符。图片块与文本块的关键差异是它没有 children，
/// 且恒定返回 true，即一张图片本身就算作有效内容，不要求同段落再配文字。
fn validate_news_rich_text_image_block(object: &serde_json::Map<String, Value>) -> AppResult<bool> {
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "url" | "alt"))
    {
        return Err(invalid_news_rich_text_content());
    }
    let url = required_news_content_string(object.get("url"), "image url")?;
    validate_optional_image_url(Some(url.to_owned()), "news content image url")?;
    if let Some(alt) = object.get("alt") {
        let alt = alt.as_str().ok_or_else(invalid_news_rich_text_content)?;
        if alt.chars().count() > 256 {
            return Err(invalid_news_rich_text_content());
        }
    }
    Ok(true)
}

/// 校验富文本叶子节点：必须是对象且带字符串 text，键只允许 text 与 bold、italic、underline 三种格式标记。
/// 三个标记若出现必须是布尔值，出现其他类型或未知键一律判为非法节点。
/// 返回值表示该叶子去空白后是否仍有文本，空白叶子合法但不计作内容，由上层决定整段是否为空。
fn validate_news_rich_text_child(node: &Value) -> AppResult<bool> {
    let object = node
        .as_object()
        .ok_or_else(invalid_news_rich_text_content)?;
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(invalid_news_rich_text_content)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "text" | "bold" | "italic" | "underline"))
    {
        return Err(invalid_news_rich_text_content());
    }
    for mark in ["bold", "italic", "underline"] {
        if let Some(value) = object.get(mark)
            && !value.is_boolean()
        {
            return Err(invalid_news_rich_text_content());
        }
    }
    Ok(!text.trim().is_empty())
}

/// 构造富文本结构非法的统一校验错误，供块级与叶子级各个失败分支共用。
/// 错误文案刻意不区分具体是哪一层、哪个键出错，以免把内部文档结构细节透出到接口响应。
fn invalid_news_rich_text_content() -> AppError {
    AppError::Validation("news content node is invalid".to_owned())
}

/// 从富文本 JSON 中取出必填字符串字段，键缺失、类型不符或去空后为空都报同一类必填错误。
/// 返回的是去空白后的借用片段，因此调用方拿到的一定非空；错误文案带字段名以便定位到具体键。
fn required_news_content_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("news content {field} is required")))
}
