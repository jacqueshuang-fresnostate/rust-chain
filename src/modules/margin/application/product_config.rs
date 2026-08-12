use super::support::{
    MARGIN_AMOUNT_MAX_INTEGER_DIGITS, MARGIN_AMOUNT_MAX_SCALE, MARGIN_AUDIT_REASON_MAX_LEN,
    MARGIN_RATE_MAX_INTEGER_DIGITS, MARGIN_RATE_MAX_SCALE, ensure_supported_user_margin_mode,
    normalized_margin_mode, optional_string, route_limit, route_offset,
};
use crate::{
    error::{AppError, AppResult},
    modules::margin::{
        infrastructure::{
            MarginProductUpsertValues, ensure_asset_exists, ensure_pair_exists,
            insert_admin_audit_log, insert_margin_product,
            list_admin_margin_products as list_admin_margin_products_from_store,
            list_margin_products, load_product_by_id, lock_product_by_id, update_margin_product,
            update_margin_product_status as update_margin_product_status_row,
        },
        presentation::{
            AdminMarginProductsQuery, AdminMarginProductsResponse, CreateMarginProductRequest,
            MarginProductResponse, MarginProductsResponse, MarginTradingCapabilitiesResponse,
            UpdateMarginProductRequest, UpdateMarginProductStatusRequest,
        },
        service::margin_product_audit_json,
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};
use std::collections::BTreeSet;

/// 查询用户可见的启用保证金产品并附带真实能力集；该只读用例不锁钱包或修改状态。
pub(crate) async fn list_active_margin_products(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<MarginProductsResponse> {
    let products = list_margin_products(pool, Some("active"), limit).await?;
    Ok(MarginProductsResponse {
        products,
        capabilities: margin_trading_capabilities(),
    })
}

/// 按后台筛选和分页查询保证金产品及总数；只读失败直接返回且不写审计。
pub(crate) async fn list_admin_margin_products(
    pool: &Pool<MySql>,
    query: AdminMarginProductsQuery,
) -> AppResult<AdminMarginProductsResponse> {
    let (products, total) = list_admin_margin_products_from_store(
        pool,
        None,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminMarginProductsResponse {
        products,
        capabilities: margin_trading_capabilities(),
        total,
    })
}

/// 读取指定保证金产品完整配置；记录缺失返回 NotFound，不创建事务或业务副作用。
pub(crate) async fn get_admin_margin_product(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    let mut tx = pool.begin().await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(product)
}

/// 校验交易对、资产、费率和能力配置后，在事务内创建保证金产品及后台审计。
/// 产品写入与审计原子提交；配置冲突或数据库失败整体回滚，不影响用户资金。
pub(crate) async fn create_margin_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    request: CreateMarginProductRequest,
) -> AppResult<MarginProductResponse> {
    validate_create_product_request(&request)?;
    let reason = required_reason(request.reason)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let values = margin_product_upsert_values(
        request.pair_id,
        request.margin_asset,
        request.logo_url,
        request.margin_mode.as_deref(),
        request.margin_modes.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate,
        &status,
    )?;
    let pool = required_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    ensure_pair_exists(&mut tx, request.pair_id).await?;
    ensure_asset_exists(&mut tx, request.margin_asset).await?;
    // 产品配置和后台审计必须同事务提交，避免配置已生效但没有审计原因。
    let product_id = insert_margin_product(&mut tx, &values).await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log(
        &mut tx,
        admin_id,
        "margin_product.create",
        product.id,
        None,
        Some(margin_product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(product)
}

/// 先锁定产品旧快照，再校验并更新配置，同时写入 before/after 后台审计。
/// 产品更新和审计同事务提交；并发状态变化或任一写入失败会整体回滚。
pub(crate) async fn update_margin_product_config(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateMarginProductRequest,
) -> AppResult<MarginProductResponse> {
    validate_update_product_request(&request)?;
    let reason = required_reason(request.reason)?;
    let status = normalized_product_status(&request.status)?;
    let values = margin_product_upsert_values(
        request.pair_id,
        request.margin_asset,
        request.logo_url,
        request.margin_mode.as_deref(),
        request.margin_modes.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate,
        &status,
    )?;
    let pool = required_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    ensure_pair_exists(&mut tx, request.pair_id).await?;
    ensure_asset_exists(&mut tx, request.margin_asset).await?;
    update_margin_product(&mut tx, product_id, &values).await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log(
        &mut tx,
        admin_id,
        "margin_product.update",
        product_id,
        Some(margin_product_audit_json(&before)),
        Some(margin_product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 在调用方事务内更新产品启停状态，调用方随后以同一事务写入审计记录。
pub(crate) async fn update_margin_product_status(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateMarginProductStatusRequest,
) -> AppResult<MarginProductResponse> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let pool = required_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    update_margin_product_status_row(&mut tx, product_id, &status).await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log(
        &mut tx,
        admin_id,
        "margin_product.update_status",
        product_id,
        Some(margin_product_audit_json(&before)),
        Some(margin_product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

fn validate_create_product_request(request: &CreateMarginProductRequest) -> AppResult<()> {
    validate_product_fields(
        request.pair_id,
        request.margin_asset,
        request.margin_modes.as_deref(),
        request.margin_mode.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate.as_ref(),
        request.status.as_deref(),
        request.reason.as_deref(),
    )
}

fn validate_update_product_request(request: &UpdateMarginProductRequest) -> AppResult<()> {
    validate_product_fields(
        request.pair_id,
        request.margin_asset,
        request.margin_modes.as_deref(),
        request.margin_mode.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate.as_ref(),
        Some(request.status.as_str()),
        request.reason.as_deref(),
    )
}

#[allow(clippy::too_many_arguments)] // 将完整请求快照映射为持久化值，避免更新路径遗漏字段。
fn margin_product_upsert_values<'a>(
    pair_id: u64,
    margin_asset: u64,
    logo_url: Option<String>,
    margin_mode: Option<&str>,
    margin_modes: Option<&[String]>,
    leverage_levels: Option<&[BigDecimal]>,
    max_leverage: &'a BigDecimal,
    min_margin: &'a BigDecimal,
    max_margin: Option<&'a BigDecimal>,
    maintenance_margin_rate: &'a BigDecimal,
    hourly_interest_rate: Option<BigDecimal>,
    status: &'a str,
) -> AppResult<MarginProductUpsertValues<'a>> {
    let margin_modes = validated_margin_modes(margin_modes, margin_mode)?;
    let margin_mode = margin_modes
        .first()
        .cloned()
        .unwrap_or_else(|| "isolated".to_owned());
    let leverage_levels = validated_leverage_levels(max_leverage, leverage_levels)?;
    Ok(MarginProductUpsertValues {
        pair_id,
        margin_asset,
        logo_url: optional_image_url(logo_url, "margin product logo_url")?,
        margin_mode,
        margin_modes,
        leverage_levels,
        max_leverage,
        min_margin,
        max_margin,
        maintenance_margin_rate,
        hourly_interest_rate: hourly_interest_rate.unwrap_or_else(zero_rate),
        status,
    })
}

#[allow(clippy::too_many_arguments)] // 纯函数校验完整保证金产品快照，显式字段便于审计约束。
fn validate_product_fields(
    pair_id: u64,
    margin_asset: u64,
    margin_modes: Option<&[String]>,
    margin_mode: Option<&str>,
    leverage_levels: Option<&[BigDecimal]>,
    max_leverage: &BigDecimal,
    min_margin: &BigDecimal,
    max_margin: Option<&BigDecimal>,
    maintenance_margin_rate: &BigDecimal,
    hourly_interest_rate: Option<&BigDecimal>,
    status: Option<&str>,
    reason: Option<&str>,
) -> AppResult<()> {
    validated_margin_modes(margin_modes, margin_mode)?;
    validated_leverage_levels(max_leverage, leverage_levels)?;
    if pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    if margin_asset == 0 {
        return Err(AppError::Validation("margin_asset is required".to_owned()));
    }
    validate_max_leverage(max_leverage)?;
    validate_margin_amount(min_margin)?;
    if let Some(max_margin) = max_margin {
        validate_margin_amount(max_margin)?;
        if max_margin < min_margin {
            return Err(AppError::Validation(
                "margin product max_margin must be greater than or equal to min_margin".to_owned(),
            ));
        }
    }
    validate_maintenance_margin_rate(maintenance_margin_rate)?;
    if let Some(hourly_interest_rate) = hourly_interest_rate {
        validate_hourly_interest_rate(hourly_interest_rate)?;
    }
    if let Some(status) = status {
        normalized_product_status(status)?;
    }
    validate_reason_len(reason)?;
    Ok(())
}

fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "margin product status must be active or disabled".to_owned(),
        )),
    }
}

fn validated_margin_modes(
    margin_modes: Option<&[String]>,
    legacy_margin_mode: Option<&str>,
) -> AppResult<Vec<String>> {
    let raw_modes: Vec<String> = match margin_modes {
        Some(modes) => modes.to_vec(),
        None => vec![legacy_margin_mode.unwrap_or("isolated").to_owned()],
    };
    if raw_modes.is_empty() {
        return Err(AppError::Validation(
            "margin product margin_modes must not be empty".to_owned(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut modes = Vec::with_capacity(raw_modes.len());
    for raw_mode in raw_modes {
        let mode = normalized_margin_mode(&raw_mode)?;
        // 杠杆产品配置必须与实际风控能力同步，不能让后台配置出用户无法使用的 cross。
        ensure_supported_user_margin_mode(&mode)?;
        if !seen.insert(mode.clone()) {
            return Err(AppError::Validation(
                "margin product margin_modes must not contain duplicates".to_owned(),
            ));
        }
        modes.push(mode);
    }

    Ok(modes)
}

fn validated_leverage_levels(
    max_leverage: &BigDecimal,
    leverage_levels: Option<&[BigDecimal]>,
) -> AppResult<Vec<String>> {
    validate_max_leverage(max_leverage)?;
    let Some(levels) = leverage_levels else {
        return Ok(vec![decimal_config_string(max_leverage)]);
    };
    if levels.is_empty() {
        return Err(AppError::Validation(
            "margin product leverage_levels must not be empty".to_owned(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(levels.len());
    for level in levels {
        validate_max_leverage(level)?;
        let level_text = decimal_config_string(level);
        if !seen.insert(level_text.clone()) {
            return Err(AppError::Validation(
                "margin product leverage_levels must not contain duplicates".to_owned(),
            ));
        }
        normalized.push(level_text);
    }

    let max_level = levels
        .iter()
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        .ok_or_else(|| {
            AppError::Validation("margin product leverage_levels must not be empty".to_owned())
        })?;
    if max_level != max_leverage {
        return Err(AppError::Validation(
            "margin product max_leverage must match maximum leverage level".to_owned(),
        ));
    }

    Ok(normalized)
}

fn decimal_config_string(value: &BigDecimal) -> String {
    let normalized = value.normalized().to_string();
    normalized
        .strip_suffix(".0")
        .unwrap_or(&normalized)
        .to_owned()
}

fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "margin product reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

fn validate_reason_len(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > MARGIN_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "margin product reason is too long".to_owned(),
        ));
    }
    Ok(())
}

fn validate_max_leverage(leverage: &BigDecimal) -> AppResult<()> {
    if leverage <= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "margin product max_leverage must be greater than 1".to_owned(),
        ));
    }
    validate_decimal_storage(
        leverage,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product max_leverage",
    )
}

fn validate_maintenance_margin_rate(rate: &BigDecimal) -> AppResult<()> {
    if rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product maintenance_margin_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        rate,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product maintenance_margin_rate",
    )
}

fn validate_margin_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product margin amount must be positive".to_owned(),
        ));
    }
    validate_decimal_storage(
        amount,
        MARGIN_AMOUNT_MAX_SCALE,
        MARGIN_AMOUNT_MAX_INTEGER_DIGITS,
        "margin product margin amount",
    )
}

fn zero_rate() -> BigDecimal {
    BigDecimal::from(0).with_scale(8)
}

fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

fn required_mysql_pool(pool: Option<&Pool<MySql>>) -> AppResult<&Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for margin routes".to_owned())
    })
}

/// 校验小时利率的非负性与数据库精度上限；失败时产品配置不得进入事务写入。
pub(super) fn validate_hourly_interest_rate(rate: &BigDecimal) -> AppResult<()> {
    if rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product hourly_interest_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        rate,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product hourly_interest_rate",
    )
}

fn validate_decimal_storage(
    value: &BigDecimal,
    max_scale: i64,
    max_integer_digits: usize,
    label: &str,
) -> AppResult<()> {
    let (digits, scale) = value.as_bigint_and_exponent();
    if scale > max_scale {
        return Err(AppError::Validation(format!(
            "{label} supports at most {max_scale} decimal places"
        )));
    }

    let significant_digits = digits
        .to_str_radix(10)
        .trim_start_matches('-')
        .trim_start_matches('0')
        .len();
    let integer_digits = if scale >= 0 {
        significant_digits.saturating_sub(scale as usize)
    } else {
        significant_digits.saturating_add(scale.unsigned_abs() as usize)
    };
    if integer_digits > max_integer_digits {
        return Err(AppError::Validation(format!(
            "{label} exceeds decimal storage precision"
        )));
    }
    Ok(())
}

/// 返回后端真实实现的市价单与逐仓、全仓能力集合，禁止对客户端宣称未实现订单类型。
pub(crate) fn margin_trading_capabilities() -> MarginTradingCapabilitiesResponse {
    // 两种模式都只支持市价开仓，前端应依据能力集显示模式切换。
    MarginTradingCapabilitiesResponse {
        order_types: vec!["market".to_owned()],
        margin_modes: vec!["isolated".to_owned(), "cross".to_owned()],
    }
}
