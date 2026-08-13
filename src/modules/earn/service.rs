//! earn bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。
//!
//! 本文件全部是无 I/O 的纯函数，承担理财的准入判定与数据归一，可脱离数据库单测。
//! 内容分四块：鉴权主体解析与分页归一；产品和分类配置的字段校验；
//! 多语言名称与富文本介绍的结构校验；以及面向赎回的费率与金额换算入口。
//! 数值校验统一走「先判正负与区间、再判小数位与整数位」两步，
//! 后一步对齐数据库列的存储精度，防止落库时被静默截断造成账实不符。
//! 富文本采用白名单策略：块级节点只允许 p、h1、h2、h3、blockquote，
//! 叶子节点只允许 text 加 bold、italic、underline 三个布尔标记，出现其他键一律整体拒绝。

use crate::{
    error::{AppError, AppResult},
    modules::earn::{
        presentation::{
            CreateEarnProductRequest, EarnCategoryResponse, EarnProductResponse,
            EarnSubscriptionResponse, UpdateEarnProductRequest,
        },
        redemption::{
            EARLY_REDEEM_FEE_BASIS_NONE, EARLY_REDEEM_FEE_BASIS_PRINCIPAL,
            EARLY_REDEEM_FEE_BASIS_PROFIT, EarnRedemptionAmounts, EarnRedemptionTerms,
            calculate_earn_redemption_amounts,
        },
        repository::{EarnProductFeeConfig, EarnProductRuleRow},
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

/// 从鉴权主体解析管理员编号，只接受 `admin:{u64}` 形式，其余一律按未授权拒绝。
/// 该编号会作为审计日志的操作主体落库，因此不能取自请求体，只能来自已验签的令牌。
/// 前缀不符与数字溢出走同一失败分支，不向调用方泄露主体的具体格式问题。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从鉴权主体解析用户编号，只接受 `user:{u64}` 形式，前缀与管理员版本不同因而无法互相冒充。
/// 理财所有涉及用户资金与订阅归属的入口都必须先过这一步，用户维度不得来自请求体。
/// 解析结果同时用于订阅归属过滤和事件广播的目标用户。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 归一理财各列表接口的分页量，缺省 50，越界夹紧到 1..=100 而不是返回参数错误。
/// 产品、订阅、分类三类列表共用同一口径，用户端与管理端也不作区分。
/// 上限用于防止单次查询拉走整表，调用方不得绕过本函数直接使用原始 limit。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 归一后台分页偏移，缺省 0 并硬性截断到十万。
/// 设上限是因为 MySQL 的 LIMIT OFFSET 需要先扫描并丢弃前 offset 行，
/// 订阅这类持续增长的大表在超大偏移下会退化为全表扫描加文件排序。
/// 超限时静默截断而非报错，代价是极深分页会重复返回同一页数据。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 裁剪可选文本并把纯空白归一为空值，使「未传该字段」与「传了空串」得到一致语义。
/// 理财的筛选条件、名称、分类代码和审计原因都经由本函数归一，
/// 避免空串被当作有效过滤条件写进 SQL，或被当作合法名称落库。
/// 只做裁剪与判空，不校验长度、字符集或业务枚举。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 把产品当前 APR、期限、额度、分类及全部费用配置映射为管理员审计快照。
/// 该快照只记录配置变更；既有订阅仍使用申购时复制的费用字段，不会被审计映射改写。
pub(crate) fn product_audit_json(product: &EarnProductResponse) -> Value {
    json!({
        "id": product.id,
        "asset_id": product.asset_id,
        "asset_symbol": product.asset_symbol,
        "name": product.name,
        "banner_url": product.banner_url,
        "small_logo_url": product.small_logo_url,
        "category": product.category,
        "category_name": product.category_name,
        "category_name_json": product.category_name_json.as_ref().map(|value| value.0.clone()),
        "introduction_json": product.introduction_json.0.clone(),
        "term_days": product.term_days,
        "apr_rate": product.apr_rate,
        "redemption_fee_rate": product.redemption_fee_rate,
        "maturity_profit_fee_rate": product.maturity_profit_fee_rate,
        "early_redeem_fee_basis": product.early_redeem_fee_basis,
        "early_redeem_fee_rate": product.early_redeem_fee_rate,
        "min_subscribe": product.min_subscribe,
        "max_subscribe": product.max_subscribe,
        "status": product.status,
    })
}

/// 把分类的代码、多语言名称、展示名、排序权重和状态摊平为审计快照 JSON。
/// 创建时只写后快照，更新与状态切换时前后各写一份，便于回溯是谁在何时改了哪一项。
/// 快照记录的是分类配置本身，不包含引用该分类的产品清单，也不触发任何写入。
pub(crate) fn category_audit_json(category: &EarnCategoryResponse) -> Value {
    json!({
        "id": category.id,
        "code": category.code,
        "name_json": category.name_json.0.clone(),
        "default_name": category.default_name,
        "sort_order": category.sort_order,
        "status": category.status,
    })
}

/// 集中校验产品配置的所有标量与结构字段，创建与更新共用以保证两条入口口径一致。
/// 顺序为：资产编号非零、名称裁剪后非空且不超过 128 字符、期限落在 1..=3650、
/// APR 非负且符合存储精度、最小额为正、最大额若存在须为正且不小于最小额。
/// 随后校验可选状态枚举、分类代码字符集与长度，最后校验富文本介绍的完整结构与审计原因长度。
/// 分类与介绍两项即使调用方未提供也会走缺省生成再校验，因此缺省值本身必定合法。
/// 刻意接收展开的标量参数而非整个请求结构，是为了让创建与更新无法各自漏掉某个字段的校验。
/// 全程无 I/O：不查资产是否存在、不查分类是否启用，这两项在应用层事务内另行确认。
#[allow(clippy::too_many_arguments)] // 纯函数校验完整产品快照，显式参数防止局部更新绕过字段约束。
fn validate_product_request_fields(
    asset_id: u64,
    name: &str,
    term_days: u32,
    apr_rate: &BigDecimal,
    min_subscribe: &BigDecimal,
    max_subscribe: Option<&BigDecimal>,
    status: Option<&str>,
    category: Option<&str>,
    introduction_json: Option<Value>,
    reason: Option<&str>,
) -> AppResult<()> {
    if asset_id == 0 {
        return Err(AppError::Validation("asset_id is required".to_owned()));
    }
    let Some(name) = optional_string(Some(name.to_owned())) else {
        return Err(AppError::Validation(
            "earn product name is required".to_owned(),
        ));
    };
    if name.chars().count() > EARN_PRODUCT_NAME_MAX_LEN {
        return Err(AppError::Validation(
            "earn product name is too long".to_owned(),
        ));
    }
    validate_term_days(term_days)?;
    validate_apr_rate(apr_rate)?;
    validate_amount(min_subscribe)?;
    if let Some(max_subscribe) = max_subscribe {
        validate_amount(max_subscribe)?;
        if max_subscribe < min_subscribe {
            return Err(AppError::Validation(
                "earn product max_subscribe must be greater than or equal to min_subscribe"
                    .to_owned(),
            ));
        }
    }
    if let Some(status) = status {
        normalized_product_status(status)?;
    }
    normalized_product_category(category)?;
    normalized_introduction_json(introduction_json, &name)?;
    validate_optional_reason(reason)?;
    Ok(())
}

/// 校验新建产品请求的全部字段，status 在此为可选，未提供时跳过枚举校验并在后续按 active 处理。
/// 只做字段级判定，不校验费率取值，费率归一与范围检查由 `product_fee_config_from_create_request` 负责。
/// 该纯规则不访问数据库；失败时应用层不得创建产品，也不得写入任何审计记录。
pub(crate) fn validate_create_product_request(request: &CreateEarnProductRequest) -> AppResult<()> {
    validate_product_request_fields(
        request.asset_id,
        &request.name,
        request.term_days,
        &request.apr_rate,
        &request.min_subscribe,
        request.max_subscribe.as_ref(),
        request.status.as_deref(),
        request.category.as_deref(),
        request.introduction_json.clone(),
        request.reason.as_deref(),
    )
}

/// 校验整体更新产品请求，status 在此为必填因而一定会走枚举校验，其余口径与创建一致。
/// 要求传入完整快照而非局部字段，防止只改一两项时绕过额度区间或富文本结构约束。
/// 校验通过不代表配置一定能落库，资产存在性与分类是否启用仍在事务内确认。
/// 该纯规则不修改既有产品、不改写订阅的费率快照，也不触碰用户钱包。
pub(crate) fn validate_update_product_request(request: &UpdateEarnProductRequest) -> AppResult<()> {
    validate_product_request_fields(
        request.asset_id,
        &request.name,
        request.term_days,
        &request.apr_rate,
        &request.min_subscribe,
        request.max_subscribe.as_ref(),
        Some(request.status.as_str()),
        request.category.as_deref(),
        request.introduction_json.clone(),
        request.reason.as_deref(),
    )?;
    Ok(())
}

/// 为新产品补齐赎回费、到期收益费和提前赎回费规则，并校验各费率位于 0..=1、最多 8 位小数。
/// early basis 为 none 时强制提前赎回费率归零；该纯规则不写产品或历史订阅。
pub(crate) fn product_fee_config_from_create_request(
    request: &CreateEarnProductRequest,
) -> AppResult<EarnProductFeeConfig> {
    normalized_product_fee_config(
        request.redemption_fee_rate.as_ref(),
        request.maturity_profit_fee_rate.as_ref(),
        request.early_redeem_fee_basis.as_deref(),
        request.early_redeem_fee_rate.as_ref(),
    )
}

/// 规范更新产品的全部费用字段；费率位于 0..=1 且最多 8 位小数，none 基准强制提前赎回费率归零。
/// 返回值供新配置和未来订阅快照使用，不重算既有订阅费用。
pub(crate) fn product_fee_config_from_update_request(
    request: &UpdateEarnProductRequest,
) -> AppResult<EarnProductFeeConfig> {
    normalized_product_fee_config(
        request.redemption_fee_rate.as_ref(),
        request.maturity_profit_fee_rate.as_ref(),
        request.early_redeem_fee_basis.as_deref(),
        request.early_redeem_fee_rate.as_ref(),
    )
}

/// 把四个可选费用字段归一为一份完整配置，是创建与更新两条入口的共同实现。
/// 三个费率缺省一律补零，基准缺省补 none，因此调用方不传费用字段等价于配置一个免费产品。
/// 基准为 none 时提前赎回费率被强制归零而非报错，避免留下永远不会生效却容易误读的配置。
/// 三个费率随后逐一校验：必须落在 0..=1 闭区间，最多 8 位小数且整数位不超过 1 位。
/// 上界取 1 是因为费率超过 100% 会让净到账额被截断为零，等同于没收本金。
/// 返回值即产品的费率定稿，后续申购会把它逐字复制进订阅快照；本函数不写库也不影响既有订阅。
fn normalized_product_fee_config(
    redemption_fee_rate: Option<&BigDecimal>,
    maturity_profit_fee_rate: Option<&BigDecimal>,
    early_redeem_fee_basis: Option<&str>,
    early_redeem_fee_rate: Option<&BigDecimal>,
) -> AppResult<EarnProductFeeConfig> {
    let redemption_fee_rate = redemption_fee_rate
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0));
    let maturity_profit_fee_rate = maturity_profit_fee_rate
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0));
    let early_redeem_fee_basis = normalized_early_redeem_fee_basis(early_redeem_fee_basis)?;
    let early_redeem_fee_rate = if early_redeem_fee_basis == EARLY_REDEEM_FEE_BASIS_NONE {
        BigDecimal::from(0)
    } else {
        early_redeem_fee_rate
            .cloned()
            .unwrap_or_else(|| BigDecimal::from(0))
    };

    validate_fee_rate(&redemption_fee_rate, "earn product redemption_fee_rate")?;
    validate_fee_rate(
        &maturity_profit_fee_rate,
        "earn product maturity_profit_fee_rate",
    )?;
    validate_fee_rate(&early_redeem_fee_rate, "earn product early_redeem_fee_rate")?;

    Ok(EarnProductFeeConfig {
        redemption_fee_rate,
        maturity_profit_fee_rate,
        early_redeem_fee_basis,
        early_redeem_fee_rate,
    })
}

/// 归一提前赎回费基准，未提供或裁剪后为空时回退到 none 表示不收提前赎回费。
/// 只接受 none、principal、profit 三种取值，分别对应不收费、按本金计费、按毛收益计费。
/// 未知取值返回参数错误，因为赎回算式对基准做穷举匹配，无法识别的值会静默变成不收费。
fn normalized_early_redeem_fee_basis(value: Option<&str>) -> AppResult<String> {
    let basis = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(EARLY_REDEEM_FEE_BASIS_NONE);
    match basis {
        EARLY_REDEEM_FEE_BASIS_NONE
        | EARLY_REDEEM_FEE_BASIS_PRINCIPAL
        | EARLY_REDEEM_FEE_BASIS_PROFIT => Ok(basis.to_owned()),
        _ => Err(AppError::Validation(
            "earn product early_redeem_fee_basis must be none, principal, or profit".to_owned(),
        )),
    }
}

/// 只校验审计原因的长度上限，不判断是否提供，因此可用于「可选原因」场景。
/// 按裁剪后的字符数计而非字节数计，上限 512，中英文使用同一口径。
/// 未提供时直接放行；是否必填由 `required_reason` 在写入路径上单独把关。
fn validate_optional_reason(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > EARN_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "earn product reason is too long".to_owned(),
        ));
    }
    Ok(())
}

/// 在管理端写入路径上把审计原因当作必填项处理：裁剪后为空即拒绝，超过 512 字符也拒绝。
/// 五个配置写接口全部经过本函数，因此不存在没有变更说明的后台改动。
/// 返回裁剪后的文本供审计日志落库，调用方不应再使用原始未裁剪值。
pub(crate) fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "earn product reason is required".to_owned(),
        ));
    };
    validate_optional_reason(Some(reason.as_str()))?;
    Ok(reason)
}

/// 归一并校验理财产品的上下架状态，只接受 active 与 disabled，裁剪后为空按必填缺失拒绝。
/// disabled 会让产品从用户端列表消失并阻断新申购，但不影响存量订阅的计息与赎回。
/// 该状态与订阅状态是两套取值空间，订阅只有 subscribed 与 redeemed 两种。
pub(crate) fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "earn product status must be active or disabled".to_owned(),
        )),
    }
}

/// 归一并校验分类的启停状态，取值同样限于 active 与 disabled。
/// 与产品状态逻辑一致但错误消息不同，便于前端区分是哪个对象的状态非法。
/// 分类置为 disabled 后新产品不能再引用它，但已引用它的存量产品照常展示与申购。
pub(crate) fn normalized_category_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product category status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "earn product category status must be active or disabled".to_owned(),
        )),
    }
}

/// 校验新建分类时的代码：裁剪后必须非空，长度不超过 64 字符，且只含字母、数字、下划线和连字符。
/// 与产品侧的分类字段不同，这里没有缺省回退，因为分类代码是产品引用它的稳定标识，必须由运营显式指定。
/// 代码一经创建即不可修改，更新接口的请求体也不接受该字段。
pub(crate) fn normalized_required_category_code(value: &str) -> AppResult<String> {
    let Some(code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "earn product category code is required".to_owned(),
        ));
    };
    validate_category_code(&code, "earn product category code")?;
    Ok(code)
}

/// 归一产品引用的分类代码，未填写或裁剪后为空时回退到内置的 fixed_term。
/// 该回退是为兼容早期没有分类概念的产品数据，使旧接口调用方无需改造即可继续创建产品。
/// 回退值同样要过字符集与长度校验，因此 `unreachable!` 分支在逻辑上不可达，只是让 `let else` 类型收敛。
/// 校验通过不代表该分类真实存在或处于启用状态，这一点由应用层在事务内另行确认。
pub(crate) fn normalized_product_category(value: Option<&str>) -> AppResult<String> {
    let Some(category) = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| Some("fixed_term".to_owned()))
    else {
        unreachable!("default earn product category is always present");
    };
    validate_category_code(&category, "earn product category")?;
    Ok(category)
}

/// 校验分类代码的长度与字符集，分类自身和产品引用两处共用同一规则。
/// 只允许 ASCII 字母数字加下划线与连字符，从而保证代码可安全用于 URL 与配置文件。
/// 长度上限 64 字符，按字符数计；label 参数决定错误消息指向哪个字段。
fn validate_category_code(value: &str, label: &str) -> AppResult<()> {
    if value.chars().count() > EARN_PRODUCT_CATEGORY_MAX_LEN {
        return Err(AppError::Validation(format!("{label} is too long")));
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return Err(AppError::Validation(format!(
            "{label} supports only letters, numbers, underscore, and hyphen"
        )));
    }
    Ok(())
}

/// 在管理端未提供多语言名称时，用分类代码兜底生成一份合法的名称结构。
/// 结构固定为 version 1、默认语言 zh-CN，只含一条 locale 为 zh-CN、country 为 CN 的条目，标题即代码本身。
/// 生成形态与校验函数的要求严格对齐，因此兜底值必然能通过后续校验。
fn default_category_name_json(code: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": code
            }
        ]
    })
}

/// 把可选的分类多语言名称归一为一份必定通过校验的结构，缺省时按分类代码生成中文兜底。
/// 调用方显式传入的结构不会被补齐或修正，只做整体校验，任何一项不合规都直接拒绝。
/// 更新分类时传入的 code 来自事务内锁定到的旧行，因此兜底标题用的是当前真实代码而非请求体。
pub(crate) fn normalized_category_name_json(value: Option<Value>, code: &str) -> AppResult<Value> {
    let name_json = value.unwrap_or_else(|| default_category_name_json(code));
    validate_category_name_json(&name_json)?;
    Ok(name_json)
}

/// 逐层校验分类多语言名称结构，任一项不合规都返回参数错误并阻止分类配置落库。
/// 顶层必须是对象，version 必须恰为 1，default_locale 裁剪后非空，items 必须是非空数组。
/// 每个条目必须是对象且 locale、country、title 三项裁剪后均非空，标题按字符数不超过 128。
/// 遍历中还要确认 default_locale 至少对应一个条目，否则前端取默认标题时会落空。
/// 与产品介绍结构的区别在于条目不含 content 字段，因此不涉及富文本节点校验。
/// 本函数只做校验，不补齐缺省字段、不改写传入结构，也不访问数据库。
fn validate_category_name_json(value: &Value) -> AppResult<()> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("earn product category name_json must be an object".to_owned())
    })?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "earn product category name_json version must be 1".to_owned(),
        ));
    }
    let default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "earn product category name_json default_locale is required".to_owned(),
            )
        })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            AppError::Validation("earn product category name_json items are required".to_owned())
        })?;
    let mut has_default_locale = false;
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation(
                "earn product category name_json item must be an object".to_owned(),
            )
        })?;
        let locale = required_category_name_string(item_object.get("locale"), "locale")?;
        if locale == default_locale {
            has_default_locale = true;
        }
        required_category_name_string(item_object.get("country"), "country")?;
        let title = required_category_name_string(item_object.get("title"), "title")?;
        if title.chars().count() > EARN_CATEGORY_TITLE_MAX_LEN {
            return Err(AppError::Validation(
                "earn product category name_json title is too long".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "earn product category name_json default_locale must exist in items".to_owned(),
        ));
    }
    Ok(())
}

/// 从分类名称条目中取出一个必填字符串字段，缺失、类型不符或裁剪后为空都归为同一种参数错误。
/// 返回裁剪后的借用切片，生命周期绑定原始 JSON，调用方无需额外分配。
/// 错误消息带上字段名以区分是 locale、country 还是 title 不合规。
fn required_category_name_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(format!(
                "earn product category name_json {field} is required"
            ))
        })
}

/// 在管理端未提供产品介绍时，用产品名兜底生成一份合法的多语言富文本。
/// 结构为 version 1、默认语言 zh-CN，单条 zh-CN 条目，正文是一个仅含产品名文本的段落节点。
/// 相比分类名称结构多出 content 数组，其节点形态严格符合白名单，因此兜底值必然通过校验。
fn default_introduction_json(product_name: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": product_name,
                "content": [
                    { "type": "p", "children": [{ "text": product_name }] }
                ]
            }
        ]
    })
}

/// 补充默认产品介绍结构，并校验受控富文本节点与多语言条目。
/// 无效节点整体拒绝，避免未支持结构进入存储和前台渲染。
pub(crate) fn normalized_introduction_json(
    value: Option<Value>,
    product_name: &str,
) -> AppResult<Value> {
    let introduction = value.unwrap_or_else(|| default_introduction_json(product_name));
    validate_introduction_json(&introduction)?;
    Ok(introduction)
}

/// 逐层校验产品介绍的多语言富文本结构，任一项不合规都阻止产品配置落库。
/// 顶层要求与分类名称一致：对象、version 为 1、default_locale 非空、items 非空数组。
/// 每个条目除 locale、country、title 三项必填外，还必须带非空的 content 数组作为正文。
/// 标题按字符数限 128；正文交由白名单校验，出现未支持的节点类型或多余键即整体拒绝。
/// 遍历中同样确认 default_locale 至少对应一个条目，避免前端取不到默认语言版本。
/// 整体拒绝而非静默丢弃非法节点，是为了不让未经支持的结构进入存储并在前台渲染时出错。
fn validate_introduction_json(value: &Value) -> AppResult<()> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("earn product introduction must be an object".to_owned())
    })?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "earn product introduction version must be 1".to_owned(),
        ));
    }
    let default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("earn product introduction default_locale is required".to_owned())
        })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            AppError::Validation("earn product introduction items are required".to_owned())
        })?;
    let mut has_default_locale = false;
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("earn product introduction item must be an object".to_owned())
        })?;
        let locale = required_intro_string(item_object.get("locale"), "locale")?;
        if locale == default_locale {
            has_default_locale = true;
        }
        required_intro_string(item_object.get("country"), "country")?;
        let title = required_intro_string(item_object.get("title"), "title")?;
        if title.chars().count() > EARN_INTRO_TITLE_MAX_LEN {
            return Err(AppError::Validation(
                "earn product introduction title is too long".to_owned(),
            ));
        }
        let content = item_object
            .get("content")
            .and_then(Value::as_array)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                AppError::Validation("earn product introduction content is required".to_owned())
            })?;
        validate_plate_content(content)?;
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "earn product introduction default_locale must exist in items".to_owned(),
        ));
    }
    Ok(())
}

/// 遍历一条介绍条目的正文数组，要求其中每个元素都是合法的块级节点。
/// 数组非空由调用方保证，本函数不再重复判空，遇到第一个非法节点即中止并返回错误。
/// 只做校验不做清洗，不会剔除非法节点后放行剩余内容。
fn validate_plate_content(content: &[Value]) -> AppResult<()> {
    for node in content {
        validate_plate_block_node(node)?;
    }
    Ok(())
}

/// 校验单个块级富文本节点，采用白名单策略：只认 type 和 children 两个键，出现其他键立即拒绝。
/// type 必须是 p、h1、h2、h3、blockquote 之一，不支持列表、图片、链接等其他块级结构。
/// children 必须是非空数组，其中每个元素再交由叶子节点校验。
/// 键白名单先于类型判断执行，因此携带额外属性的合法类型节点同样会被拒绝。
/// 所有失败共用同一条错误消息，不区分具体原因，避免向调用方暴露内部校验细节。
fn validate_plate_block_node(node: &Value) -> AppResult<()> {
    let object = node.as_object().ok_or_else(invalid_plate_content)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "children"))
    {
        return Err(invalid_plate_content());
    }
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_plate_content)?;
    if !matches!(node_type, "p" | "h1" | "h2" | "h3" | "blockquote") {
        return Err(invalid_plate_content());
    }
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .filter(|children| !children.is_empty())
        .ok_or_else(invalid_plate_content)?;
    for child in children {
        validate_plate_child_node(child)?;
    }
    Ok(())
}

/// 校验单个叶子节点：必须是对象，必须带字符串类型的 text 字段，允许为空字符串。
/// 键白名单限于 text、bold、italic、underline，出现其他键即拒绝，从而排除内联链接等未支持结构。
/// 三个格式标记若存在则必须是布尔值，字符串形式的 "true" 同样会被拒绝。
/// 与块级节点共用同一条错误消息，不单独区分是哪一项不合规。
fn validate_plate_child_node(node: &Value) -> AppResult<()> {
    let object = node.as_object().ok_or_else(invalid_plate_content)?;
    if !object.get("text").is_some_and(Value::is_string) {
        return Err(invalid_plate_content());
    }
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "text" | "bold" | "italic" | "underline"))
    {
        return Err(invalid_plate_content());
    }
    for mark in ["bold", "italic", "underline"] {
        if let Some(value) = object.get(mark)
            && !value.is_boolean()
        {
            return Err(invalid_plate_content());
        }
    }
    Ok(())
}

/// 构造富文本校验的统一错误。所有节点级失败共用同一消息，
/// 既避免在多处重复字面量，也不向调用方泄露具体是哪个键或哪种类型不合规。
fn invalid_plate_content() -> AppError {
    AppError::Validation("earn product introduction content node is invalid".to_owned())
}

/// 从介绍条目中取出一个必填字符串字段，缺失、类型不符或裁剪后为空都归为同一种参数错误。
/// 与分类版本逻辑相同但错误消息前缀不同，便于定位是介绍还是分类名称出了问题。
/// 返回裁剪后的借用切片，生命周期绑定原始 JSON。
fn required_intro_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation(format!("earn product introduction {field} is required"))
        })
}

/// 校验申购金额落在已锁定产品的额度区间内，下界闭区间，上界在配置了最大额时同样闭区间。
/// 最大额为空表示不设上限，此时只检查下界；两条判定都返回参数错误而非静默夹紧。
/// 传入的产品来自事务内 FOR UPDATE 锁定的行，因此区间值与随后写入订阅的快照必定同源。
/// 本函数在插入订阅和扣减 available 之前执行，只比较数值，不按资产 precision_scale 做截断。
pub(crate) fn validate_product_amount(
    amount: &BigDecimal,
    product: &EarnProductRuleRow,
) -> AppResult<()> {
    if amount < &product.min_subscribe {
        return Err(AppError::Validation(
            "earn subscription amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_subscribe) = &product.max_subscribe
        && amount > max_subscribe
    {
        return Err(AppError::Validation(
            "earn subscription amount exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

/// 产品期限上限，约合十年，防止配置出实际上无法到期的超长产品。
const EARN_PRODUCT_MAX_TERM_DAYS: u32 = 3_650;
/// 产品名长度上限，按字符数计。
const EARN_PRODUCT_NAME_MAX_LEN: usize = 128;
/// 分类代码长度上限，按字符数计。
const EARN_PRODUCT_CATEGORY_MAX_LEN: usize = 64;
/// 分类多语言标题长度上限。
const EARN_CATEGORY_TITLE_MAX_LEN: usize = 128;
/// 产品介绍多语言标题长度上限。
const EARN_INTRO_TITLE_MAX_LEN: usize = 128;
/// 管理员审计原因长度上限。
const EARN_AUDIT_REASON_MAX_LEN: usize = 512;
/// APR 允许的最大小数位，与数据库列的 scale 一致。
const EARN_APR_MAX_SCALE: i64 = 8;
/// APR 允许的最大整数位。
const EARN_APR_MAX_INTEGER_DIGITS: usize = 10;
/// 三项费率允许的最大小数位。
const EARN_FEE_RATE_MAX_SCALE: i64 = 8;
/// 费率允许的最大整数位；取 1 是因为费率上界为 1，只需容纳个位。
const EARN_FEE_RATE_MAX_INTEGER_DIGITS: usize = 1;
/// 申购金额允许的最大小数位，与钱包账本列精度一致。
const EARN_AMOUNT_MAX_SCALE: i64 = 18;
/// 申购金额允许的最大整数位。
const EARN_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;

/// 按调用时刻的 UTC 当前时间加产品期限天数算出订阅到期时刻，结果直接写入订阅行。
/// 时间基准取自系统时钟而非请求参数，因此同一批并发申购的到期时刻可能相差毫秒级。
/// 该时刻此后即固化，是提前赎回与到期赎回两种计费口径的分界，也是自动赎回任务的扫描依据。
/// 日期加法溢出时返回参数错误并阻止创建订阅，不会退化成一个错误的到期时间。
pub(crate) fn earn_matures_at(term_days: u32) -> AppResult<DateTime<Utc>> {
    Utc::now()
        .checked_add_signed(chrono::TimeDelta::days(term_days as i64))
        .ok_or_else(|| {
            AppError::Validation("earn product term_days exceeds supported maximum".to_owned())
        })
}

/// 校验产品期限落在 1 到 3650 天之间，零和超上限分别返回不同的错误消息。
/// 期限为零会让实际天数计息的分母为零，因此必须在配置阶段就挡下。
/// 上限约合十年，既避免日期运算溢出，也防止配置出实际上无法到期的产品。
fn validate_term_days(term_days: u32) -> AppResult<()> {
    if term_days == 0 {
        return Err(AppError::Validation(
            "earn product term_days must be positive".to_owned(),
        ));
    }
    if term_days > EARN_PRODUCT_MAX_TERM_DAYS {
        return Err(AppError::Validation(
            "earn product term_days exceeds supported maximum".to_owned(),
        ));
    }
    Ok(())
}

/// 校验年化收益率非负，再确认其符合数据库列的存储精度：最多 8 位小数、整数位不超过 10 位。
/// 只设下界不设上界，超高收益率是否合理由运营流程把关而非代码判定。
/// 零值合法，表示配置一个不产生收益的产品。
fn validate_apr_rate(apr_rate: &BigDecimal) -> AppResult<()> {
    if apr_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "earn product apr_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        apr_rate,
        EARN_APR_MAX_SCALE,
        EARN_APR_MAX_INTEGER_DIGITS,
        "earn product apr_rate",
    )
}

/// 校验单项费率落在 0..=1 闭区间，再确认其符合最多 8 位小数、整数位不超过 1 位的存储精度。
/// 与 APR 的差别在于这里有明确上界：费率超过 100% 会把净到账额吃到零，等同没收本金。
/// 上界取闭区间，因此允许配置 100% 费率，此种极端配置由运营自行承担后果。
/// label 参数决定错误消息指向三项费率中的哪一项。
fn validate_fee_rate(fee_rate: &BigDecimal, label: &str) -> AppResult<()> {
    if fee_rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{label} must be non-negative"
        )));
    }
    if fee_rate > &BigDecimal::from(1) {
        return Err(AppError::Validation(format!(
            "{label} must be less than or equal to 1"
        )));
    }
    validate_decimal_storage(
        fee_rate,
        EARN_FEE_RATE_MAX_SCALE,
        EARN_FEE_RATE_MAX_INTEGER_DIGITS,
        label,
    )
}

/// 校验申购金额严格为正，再确认其符合数据库列的存储精度：最多 18 位小数、整数位不超过 20 位。
/// 这是存储口径而非资产口径：本函数不读取资产的 precision_scale，
/// 因此小数位少于 18 位的资产也可能收到超出其自身精度的申购额。
/// 超精度一律拒绝而非隐式截断，保证用户提交的金额与最终扣款、落库金额完全一致。
/// 产品额度区间的判定由 `validate_product_amount` 在锁定产品后单独执行。
pub(crate) fn validate_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "earn subscription amount must be positive".to_owned(),
        ));
    }

    validate_decimal_storage(
        amount,
        EARN_AMOUNT_MAX_SCALE,
        EARN_AMOUNT_MAX_INTEGER_DIGITS,
        "earn subscription amount",
    )
}

/// 判定一个十进制数能否无损存入指定精度的数据库列，APR、费率和金额三类校验共用该实现。
/// 先比较 scale：超过允许的小数位即拒绝，注意此处不做归一化，因此 `1.500` 会按 3 位小数计。
/// 再由有效数字位数反推整数位数：去掉符号与前导零后的长度减去 scale 即整数位。
/// scale 为负表示该数以 10 的幂为单位存储，此时整数位改为有效数字加上 scale 的绝对值。
/// 两项任一超限都返回参数错误，绝不静默截断，以免落库值与用户提交值不一致。
/// label 参数决定错误消息指向哪个字段；本函数只判定不修改传入值。
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

/// 裁剪并校验申购幂等键：裁剪后不得为空，且字节长度不超过 255。
/// 长度按字节而非字符计，因此含中文的键实际可用字符数少于 255。
/// 归一后的键参与用户维度唯一约束，是理财申购防重复扣款的唯一依据。
/// 与借贷不同，理财在重放时会继续核对产品编号与申购金额，不一致则返回冲突而非返回旧订阅。
pub(crate) fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required for earn subscriptions".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for earn subscriptions".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

/// 裁剪可选图片地址并限制长度不超过 2048 字符，纯空白归一为空值而非报错。
/// 横幅图与小图标共用本函数，field 参数决定超长时错误消息指向哪一个。
/// 只校验长度，不校验协议、域名或可达性，非法地址会照常落库并在前端渲染时失效。
pub(crate) fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

/// 仅使用订阅快照计算赎回：到期按 `本金*APR*term_days/365`，提前赎回按实际秒数计毛收益。
/// 通用赎回费按本金+毛收益计；到期收益费只在到期后按毛收益计；提前费按配置对本金或毛收益计。
/// 各中间费用与收益统一保留 18 位，净到账最低为零；产品后续修改不影响结果，本函数不写钱包。
pub(crate) fn redemption_amounts_for_subscription(
    subscription: &EarnSubscriptionResponse,
    now: DateTime<Utc>,
) -> EarnRedemptionAmounts {
    calculate_earn_redemption_amounts(
        EarnRedemptionTerms {
            amount: &subscription.amount,
            apr_rate: &subscription.apr_rate,
            term_days: subscription.term_days,
            subscribed_at: subscription.subscribed_at,
            matures_at: subscription.matures_at,
            redemption_fee_rate: &subscription.redemption_fee_rate,
            maturity_profit_fee_rate: &subscription.maturity_profit_fee_rate,
            early_redeem_fee_basis: &subscription.early_redeem_fee_basis,
            early_redeem_fee_rate: &subscription.early_redeem_fee_rate,
        },
        now,
    )
}

/// 在幂等重放路径上核对旧订阅的产品与金额是否与本次请求完全一致，不一致即返回冲突。
/// 金额用 BigDecimal 的相等性比较，因此 scale 不同但数值相同的两个金额会被判为不等。
/// 该检查是理财与借贷幂等语义的关键差异：借贷直接返回旧订单，理财则要求同键必须同请求，
/// 从而防止客户端复用一个已成功的键去申购另一个产品或另一笔金额。
/// 只做比较，不修改订阅、不移动资金，也不判断订阅当前处于何种状态。
pub(crate) fn ensure_existing_subscription_matches_request(
    existing: &EarnSubscriptionResponse,
    product_id: u64,
    amount: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id || existing.amount != *amount {
        return Err(AppError::Conflict(
            "earn idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}
