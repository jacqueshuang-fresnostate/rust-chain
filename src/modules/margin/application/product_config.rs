//! 杠杆产品配置用例。
//!
//! 覆盖用户侧的启用产品浏览，以及后台的产品创建、整体改配和启停三条写路径。
//! 写路径统一遵循同一套顺序：先在事务外做纯校验，再开事务锁产品旧快照，
//! 然后写配置并回读新快照，最后把 before/after 与管理员填写的变更原因写进同一条审计记录。
//! 配置变更只影响后续开仓，不会重算已有仓位的杠杆、维持保证金率或利息，也不触碰任何用户钱包。
//! 精度校验按资金列容量分档：杠杆和费率最多八位小数十位整数，保证金金额最多十八位小数二十位整数。

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

/// 查询用户可见的杠杆产品清单，硬编码只取 active 状态，停用产品不会出现在用户端。
/// 响应额外带上后端真实实现的能力集，让前端据此决定展示哪些下单类型和保证金模式开关。
/// 只读用例，不开事务、不锁钱包、不改任何状态；`limit` 已由路由层夹到安全区间。
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

/// 查询后台杠杆产品分页列表并同时返回总数，状态筛选固定传 None，因此覆盖 active 与 disabled 全量。
/// `limit` 与 `offset` 在这里才做归一化，分别夹到 1 到 100 和 0 到十万，防止管理端传入极端分页。
/// 行查询与 COUNT 共用同一组谓词，保证翻页时总数与列表口径一致；同样附带后端能力集供编辑表单使用。
/// 只读用例，失败直接上抛不返回部分结果，也不写审计日志。
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

/// 读取单个杠杆产品的完整配置，包含交易对符号、保证金币种符号等联表字段，供后台编辑页回填。
/// 这里虽然开了事务，但内部查询不带 FOR UPDATE，只是为了复用同一个按主键读取的适配器函数，
/// 因此不会阻塞并发的产品改配；产品不存在时返回 NotFound，全程没有写入和业务副作用。
pub(crate) async fn get_admin_margin_product(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    let mut tx = pool.begin().await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(product)
}

/// 新建杠杆产品：先在事务外完成字段校验、原因必填检查、状态归一化和持久化值组装，
/// 再开事务确认交易对与保证金币种真实存在，写入产品行，回读完整快照并追加一条创建审计。
/// 未传 `status` 时默认 active，未传小时利率时按八位精度的零处理，即建即可开仓且不计息。
/// 产品行与审计记录原子提交，任一步失败整体回滚，不会留下没有变更原因的已生效配置。
/// 本用例不发布事件、不接触任何用户钱包或仓位，新配置只对之后的开仓请求生效。
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

/// 全量改写杠杆产品配置：请求体是完整快照，缺省字段按创建时的同一套默认规则补齐，不做字段级增量合并。
/// 事务内第一步就对产品行加 FOR UPDATE，把 before 快照与后续更新绑定在同一版本上，避免并发改配互相覆盖。
/// 与创建路径的差别是这里 `status` 必填、允许改到 disabled，且审计同时记录 before 和 after 两份快照。
/// 交易对与保证金币种存在性在锁定之后重新确认，任一环节失败连同产品锁一起回滚，配置与审计不会分裂。
/// 调整杠杆档位、维持保证金率或小时利率只作用于后续开仓和后续计息，已有仓位的既存字段不被追溯修改。
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

/// 只切换杠杆产品的 active 与 disabled 状态，是三条写路径里唯一不校验杠杆档位和费率的入口。
/// 因此即使某个历史产品的配置已不满足当前校验规则，管理员仍能把它停用，不会被旧数据卡住。
/// 事务内先锁产品取 before 快照，改状态后回读 after，再连同必填的变更原因写入同一条审计。
/// 停用只让开仓路径的产品锁定判定为不可用，已有仓位仍可正常平仓、撤销并继续被利息 worker 计提。
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

/// 把创建请求摊平成字段列表后交给共享校验，创建与改配因此共用完全相同的合法性口径。
/// 与改配版本的唯一差别是 `status` 在创建请求里可缺省，此处按 Option 原样传下去，
/// 缺省时跳过状态枚举校验，真正的默认值 active 在调用方补齐。
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

/// 把改配请求摊平成字段列表后交给共享校验，保证改配不会绕过创建时的任何一条约束。
/// 改配请求的 `status` 是必填的普通字符串，这里包装成 Some 传入，因此状态枚举校验一定会执行。
/// 纯校验函数，在开事务和锁定产品之前调用，失败时数据库上没有任何痕迹。
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

/// 把已通过校验的请求字段组装成可直接绑定到 SQL 的持久化值，创建与改配共用同一份映射。
/// 归一化后的模式集合中第一个元素被取为产品默认模式，集合为空时兜底回落到 isolated。
/// 杠杆档位统一转成去掉多余尾零的字符串形式存 JSON 列，未显式给出时用最大杠杆生成单档。
/// 未提供小时利率时补一个八位精度的零，让该列始终有值，利息 worker 可以无条件参与计算。
/// 纯组装函数，返回值借用调用方持有的十进制引用，本身不写库也不决定事务边界。
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

/// 逐项校验杠杆产品配置快照的完整合法性，是创建与改配之前唯一的业务规则闸门。
/// 依次检查保证金模式集合与杠杆档位自洽、交易对与保证金币种主键非零、最大杠杆大于一、
/// 最小保证金为正、最大保证金不小于最小保证金、维持保证金率非负、小时利率非负，
/// 各十进制字段还要满足对应资金列的小数位与整数位容量，最后限制变更原因长度。
/// 任一条不满足即返回参数错误；纯函数不访问数据库，因此不会留下任何部分写入。
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

/// 把产品启停状态裁剪空白后限制为 active 或 disabled 两个字面量，空白值报必填。
/// 这与仓位状态的四值枚举是两套独立口径，产品只有启用和停用，没有终态概念。
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

/// 归一化产品支持的保证金模式集合，兼容只传单个 `margin_mode` 的旧版后台请求。
/// 两者都缺省时退化为仅支持 isolated 的单元素集合；显式传空数组则判为参数非法。
/// 每个元素既要通过字面量校验，也要确认后端风控已实现，杜绝配出用户点了必然失败的模式。
/// 用有序集合检测重复，同一模式出现两次直接报错，避免 JSON 列里存冗余项影响开仓匹配。
/// 返回值保持调用方给定的原始顺序，因此第一个元素会被上层取作产品默认模式。
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

/// 归一化可选杠杆档位列表并强制它与产品最大杠杆自洽，输出待存 JSON 列的字符串数组。
/// 未提供档位时按最大杠杆生成单档；显式传空数组判为参数非法，产品不能一个档位都没有。
/// 每个档位都按最大杠杆的同一套规则校验，必须大于一且满足费率列的小数位与整数位容量。
/// 档位先转成去尾零的规范字符串再查重，因此 10 与 10.0 会被判定为重复配置并报错。
/// 最后要求档位中的最大值与 `max_leverage` 精确相等，防止出现用户可选倍数超过产品上限的配置。
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

/// 把杠杆档位规范化成用于存储和比较的文本，先归一去除多余尾零，再去掉残留的 `.0` 后缀。
/// 这样 3、3.0、3.00 都会落成同一个 "3"，档位查重和开仓时的档位匹配才有稳定口径。
fn decimal_config_string(value: &BigDecimal) -> String {
    let normalized = value.normalized().to_string();
    normalized
        .strip_suffix(".0")
        .unwrap_or(&normalized)
        .to_owned()
}

/// 取出并裁剪管理员填写的变更原因，缺失或纯空白一律报必填，随后再复查长度上限。
/// 三条后台写路径都强制要求原因，保证审计记录里每次配置变更都有可追责的说明文本。
fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "margin product reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

/// 限制变更原因不超过五百一十二个字符，按 Unicode 字符数而非字节数统计，中文不会被误判超长。
/// 传 None 时直接通过，因此它只管长度不管必填，必填由 `required_reason` 单独负责。
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

/// 校验杠杆倍数严格大于一，等于一意味着不借款，不属于杠杆产品的合法配置。
/// 通过后再按费率列容量检查精度，最多八位小数、十位整数，超出报存储精度错误。
/// 最大杠杆和每个杠杆档位共用这条规则，保证档位不会出现小数位比列定义还长的取值。
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

/// 校验维持保证金率非负，允许配成零表示该产品不设维持保证金、不会因风险率触发强平。
/// 与杠杆共用费率列的精度上限，最多八位小数十位整数；该值直接参与强平线判定，越界会放大风控误差。
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

/// 校验产品的最小或最大保证金额度严格为正，并按资金列容量限制在十八位小数、二十位整数内。
/// 这里只管单个额度自身的合法性，最大不小于最小的相对关系由 `validate_product_fields` 另行检查。
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

/// 生成小时利率缺省值，标度固定为八位以匹配费率列定义，避免入库时被隐式补零或截断。
/// 用它兜底后该列始终非空，利息 worker 可以对所有产品统一走同一套计提公式，只是结果为零。
fn zero_rate() -> BigDecimal {
    BigDecimal::from(0).with_scale(8)
}

/// 裁剪产品图标地址并把空白折叠为 None，随后限制长度不超过两千零四十八个字符。
/// 只做长度约束，不校验协议或域名，`field` 仅用于拼出可定位的错误文案。
fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

/// 从后台路由透传下来的可选连接池中取出实例，缺失时报内部错误而不是继续往下走。
/// 三条产品写路径都在完成纯校验之后、开启事务之前调用它，确保配置缺失不会伪装成参数错误。
fn required_mysql_pool(pool: Option<&Pool<MySql>>) -> AppResult<&Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for margin routes".to_owned())
    })
}

/// 校验小时利率非负并满足费率列的八位小数、十位整数容量，零表示该产品免息。
/// 除后台配置外，开仓路径也会对锁定到的产品复查一次，因为历史数据可能早于当前校验规则，
/// 一旦利率越界就必须在写仓位和扣抵押之前失败，避免利息 worker 后续按非法费率反复计提。
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

/// 确认十进制值能被目标 `DECIMAL` 列无损存下，分别检查小数位数和整数位数两个上限。
/// 小数位直接取指数与 `max_scale` 比较，超过即报错，杜绝入库时被数据库静默四舍五入。
/// 整数位由有效数字个数减去标度推出，负标度按加法处理以覆盖 1E+3 这类科学计数形式；
/// 计算前剥掉符号和前导零，因此 0.5 的整数位算作零，不会被误判为占用一位。
/// 纯函数只做容量判定，不关心取值区间，正负号与业务上下限由各字段的专用校验负责。
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

/// 返回后端在杠杆上下文中真实实现的能力集：订单类型支持市价和限价，保证金模式支持逐仓和全仓。
/// 这份清单是硬编码的实现事实而非配置，用户侧和后台的产品列表都会附带它，
/// 前端据此决定是否渲染限价输入框和模式切换，禁止对客户端宣称尚未实现的下单类型。
pub(crate) fn margin_trading_capabilities() -> MarginTradingCapabilitiesResponse {
    // 两种保证金模式共用市价/限价能力，前端必须以此集合而不是本地假设决定可选项。
    MarginTradingCapabilitiesResponse {
        order_types: vec!["market".to_owned(), "limit".to_owned()],
        margin_modes: vec!["isolated".to_owned(), "cross".to_owned()],
    }
}
