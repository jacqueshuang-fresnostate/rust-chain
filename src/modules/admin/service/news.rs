use super::*;

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

pub(crate) fn validate_news_title(value: &str) -> AppResult<String> {
    let Some(title) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news title is required".to_owned()));
    };
    if title.chars().count() > 255 {
        return Err(AppError::Validation("news title is too long".to_owned()));
    }
    Ok(title)
}

pub(crate) fn validate_news_category(value: &str) -> AppResult<String> {
    let Some(category) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news category is required".to_owned()));
    };
    match category.as_str() {
        "general" | "market" | "product" | "system" | "promotion" => Ok(category),
        _ => Err(AppError::Validation("unsupported news category".to_owned())),
    }
}

pub(crate) fn validate_news_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("news status is required".to_owned()));
    };
    match status.as_str() {
        "draft" | "published" | "archived" => Ok(status),
        _ => Err(AppError::Validation("unsupported news status".to_owned())),
    }
}

pub(crate) fn normalize_optional_news_country_code(
    value: Option<String>,
) -> AppResult<Option<String>> {
    value
        .map(|value| normalize_news_country_code(&value))
        .transpose()
}

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

fn validate_news_rich_text_content(content: &[Value]) -> AppResult<bool> {
    let mut has_content = false;
    for node in content {
        has_content = validate_news_rich_text_block(node)? || has_content;
    }
    Ok(has_content)
}

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

fn invalid_news_rich_text_content() -> AppError {
    AppError::Validation("news content node is invalid".to_owned())
}

fn required_news_content_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("news content {field} is required")))
}
