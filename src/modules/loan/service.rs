//! loan bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//!
//! 这里集中了借贷的全部纯判定与计算口径，所有函数都不做 I/O，可脱离数据库单测。
//! 核心是利息计算：全期模式按本金乘利率一次计足，实际天数模式再按计费天数与产品期限的比例折算，
//! 天数向上取整、下限一天、上限为产品期限，结果一律按贷款资产 precision_scale 向零截断。
//! 其余部分是产品配置与订单参数的准入规则：枚举归一、金额正负与区间、小数位不得超过资产精度、
//! 多语言名称结构校验，以及分页与鉴权主体解析。任何函数都不移动资金、不改订单状态。

use crate::{
    error::{AppError, AppResult},
    modules::{
        loan::domain::{
            INTEREST_MODE_ACTUAL_DAYS, INTEREST_MODE_FULL_TERM, LOAN_PRODUCT_NAME_TITLE_MAX_LEN,
            LOAN_TYPE_COLLATERALIZED, LOAN_TYPE_CREDIT,
        },
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

/// 构造借贷金额比较专用的零值基准，固定带 18 位小数以对齐账本列的最大精度。
/// 显式指定 scale 是为了让 `0` 与 `0.000000000000000000` 在比较时行为一致，
/// 避免不同 scale 的 BigDecimal 在边界判定上出现歧义。
fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}

/// 计算还款利息：full_term 为 `本金*利率`，actual_days 为该值再乘计费天数/产品天数。
/// 实际天数按秒数向上取整，放款后至少计一天且最多 term_days；当前时刻早于放款时也按一天计算。
/// 结果按贷款资产 precision_scale 向零截断；未知计息模式返回校验错误，本函数不扣钱包或写流水。
pub(crate) fn calculate_interest_amount(
    principal: &BigDecimal,
    interest_rate: &BigDecimal,
    mode: &str,
    term_days: u32,
    disbursed_at: DateTime<Utc>,
    now: DateTime<Utc>,
    precision_scale: i32,
) -> AppResult<BigDecimal> {
    let raw_interest = match mode {
        INTEREST_MODE_FULL_TERM => principal.clone() * interest_rate.clone(),
        INTEREST_MODE_ACTUAL_DAYS => {
            let elapsed_seconds = (now - disbursed_at).num_seconds().max(0);
            let elapsed_days = ((elapsed_seconds + 86_399) / 86_400).max(1);
            let charged_days = elapsed_days.min(i64::from(term_days));
            principal.clone() * interest_rate.clone() * BigDecimal::from(charged_days)
                / BigDecimal::from(term_days)
        }
        _ => {
            return Err(AppError::Validation(
                "unsupported interest_calculation_mode".to_owned(),
            ));
        }
    };
    Ok(truncate_amount_to_asset_precision(
        &raw_interest,
        precision_scale,
    ))
}

/// 校验借款金额落在产品额度区间内，下界为闭区间，上界在配置了最大额时同样为闭区间。
/// 最大额为空表示该产品不设上限，此时只检查下界；两条判定都返回参数错误而非静默夹紧。
/// 本函数只比较数值，不裁剪金额也不看小数位，精度校验由调用方按贷款资产另行执行。
/// 校验通过不代表用户有资格借款，KYC 等级和产品启用状态在下单事务内另行判定。
pub(crate) fn ensure_amount_within_product_limits(
    amount: &BigDecimal,
    min_amount: &BigDecimal,
    max_amount: &Option<BigDecimal>,
) -> AppResult<()> {
    if amount < min_amount {
        return Err(AppError::Validation(
            "amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_amount) = max_amount.as_ref()
        && amount > max_amount
    {
        return Err(AppError::Validation(
            "amount exceeds product maximum".to_owned(),
        ));
    }
    Ok(())
}

/// 归一并校验借贷类型，先裁剪首尾空白，纯空白按缺失必填项拒绝。
/// 只接受 credit 与 collateralized 两种取值，其中后者会在下单时强制要求抵押资产和抵押金额。
/// 返回裁剪后的字符串供落库使用，调用方不应再使用原始未裁剪值。
pub(crate) fn validate_loan_type(value: &str) -> AppResult<String> {
    let value = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation("loan_type is required".to_owned()))?;
    match value.as_str() {
        LOAN_TYPE_CREDIT | LOAN_TYPE_COLLATERALIZED => Ok(value),
        _ => Err(AppError::Validation("unsupported loan_type".to_owned())),
    }
}

/// 归一并校验计息模式，决定还款时利息按哪种口径折算。
/// full_term 表示无论提前多久还款都按整期收取本金乘利率，actual_days 则按实际占用天数比例收取。
/// 空白值按必填缺失拒绝，未知模式在产品落库前就被挡下，避免还款阶段才发现无法计息。
pub(crate) fn validate_interest_mode(value: &str) -> AppResult<String> {
    let value = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation("interest_calculation_mode is required".to_owned()))?;
    match value.as_str() {
        INTEREST_MODE_FULL_TERM | INTEREST_MODE_ACTUAL_DAYS => Ok(value),
        _ => Err(AppError::Validation(
            "unsupported interest_calculation_mode".to_owned(),
        )),
    }
}

/// 归一并校验借贷产品的上下架状态，只接受 active 与 disabled 两种取值。
/// 该状态属于产品配置维度，与订单状态机的 pending、disbursed、repaid 等取值互不相干。
/// disabled 只阻断后续下单，不影响已存在订单的审批、计息与还款流程。
pub(crate) fn validate_product_status(value: &str) -> AppResult<String> {
    let value = optional_string(Some(value.to_owned()))
        .ok_or_else(|| AppError::Validation("status is required".to_owned()))?;
    if value == "active" || value == "disabled" {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "unsupported loan product status".to_owned(),
        ))
    }
}

/// 裁剪并校验用户借贷幂等键，空值或超过 255 字节时拒绝创建订单。
/// 该函数不把键与产品/金额绑定；重复请求内容一致性当前由调用方保证。
pub(crate) fn validate_idempotency_key(value: String) -> AppResult<String> {
    let value = optional_string(Some(value))
        .ok_or_else(|| AppError::Validation("idempotency_key is required".to_owned()))?;
    if value.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long".to_owned(),
        ));
    }
    Ok(value)
}

/// 要求金额严格大于零，用于借款本金、抵押数量和产品额度这类不允许为零的字段。
/// 比较基准取 18 位精度的零值，因此 `0`、`0.0` 与 `-0` 都会被判为不合法。
/// 错误消息带上调用方给出的字段名，便于前端定位是哪一项非法。
pub(crate) fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &zero_amount() {
        return Err(AppError::Validation(format!("{field} must be positive")));
    }
    Ok(())
}

/// 要求金额不小于零，与严格为正的版本区分开：利率允许配置为零表示免息产品。
/// 只拒绝负值，零值放行；同样以 18 位精度零值作为比较基准。
/// 本函数不设上限，超高利率是否合理由运营流程把关而非代码判定。
pub(crate) fn ensure_non_negative_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount < &zero_amount() {
        return Err(AppError::Validation(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(())
}

/// 确认金额的有效小数位不超过对应资产的 precision_scale，超出即拒绝而不做隐式舍入。
/// 判定前先归一化，因此尾随零不计入有效精度，`1.500` 在精度为 1 的资产上仍然合法。
/// 选择拒绝而非截断，是为了让用户提交的金额与最终落库、入账的金额始终一致，避免对账出现无法解释的尾差。
pub(crate) fn ensure_amount_precision(
    amount: &BigDecimal,
    precision_scale: i32,
    field: &str,
) -> AppResult<()> {
    if amount_scale_within_precision(amount, precision_scale) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "{field} exceeds asset precision_scale {precision_scale}"
    )))
}

/// 判断归一化后的小数位是否落在资产精度以内，是精度校验的底层判定。
/// 先 normalized 去掉尾随零再取 exponent，负 scale 表示整数带尾零，用 max(0) 折算为零位小数。
/// 只回答布尔结果，不产生错误信息，也不修改传入的金额。
fn amount_scale_within_precision(amount: &BigDecimal, precision_scale: i32) -> bool {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    scale.max(0) <= precision_scale.into()
}

/// 裁剪可选文本并把纯空白归一为空值，使「未传该字段」与「传了空串」得到一致语义。
/// 借贷模块的筛选条件、拒绝原因和名称字段都经由本函数归一，避免空串被当作有效过滤条件写进 SQL。
/// 只做裁剪与判空，不校验长度、编码或业务枚举。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 在管理端未提供多语言名称时，用纯文本产品名兜底生成一份合法的名称结构。
/// 生成结果固定为 version 1、默认语言 zh-CN，并只含一条 locale 为 zh-CN、country 为 CN 的条目。
/// 该结构刻意与校验函数的要求对齐，因此兜底值必然能通过后续格式校验。
/// 本函数不裁剪也不校验传入名称，空白名称会原样进入标题字段。
pub(crate) fn default_product_name_json(name: &str) -> Value {
    json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            {
                "locale": "zh-CN",
                "country": "CN",
                "title": name
            }
        ]
    })
}

/// 把可选的多语言名称归一为一份必定通过校验的结构：缺省时按回退名称生成中文兜底。
/// 调用方显式传入的结构不会被补齐或修正，只做整体格式校验，任何一项不合规都直接拒绝。
/// 校验通过后返回的值可直接落库；本函数不写数据库，也不决定最终展示用的纯文本名称。
pub(crate) fn normalized_product_name_json(
    value: Option<Value>,
    fallback_name: &str,
) -> AppResult<Value> {
    let name_json = value.unwrap_or_else(|| default_product_name_json(fallback_name));
    validate_product_name_json(&name_json)?;
    Ok(name_json)
}

/// 逐层校验借贷产品多语言名称结构，任一项不合规都返回参数错误并阻止产品配置落库。
/// 顶层必须是对象，version 必须恰为 1，default_locale 裁剪后不得为空，items 必须是非空数组。
/// 每个条目必须是对象且 locale、country、title 三项裁剪后均非空，title 按字符数不超过 128。
/// 长度用 chars 计数而非字节，保证中文标题与英文标题使用同一口径。
/// 遍历过程中还要确认 default_locale 至少对应一个条目，否则展示时会取不到默认标题。
/// 本函数只做校验，不补齐缺省字段、不改写传入结构，也不接触数据库。
pub(crate) fn validate_product_name_json(value: &Value) -> AppResult<()> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("loan product name_json must be an object".to_owned())
    })?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::Validation(
            "loan product name_json version must be 1".to_owned(),
        ));
    }
    let default_locale = object
        .get("default_locale")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("loan product name_json default_locale is required".to_owned())
        })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            AppError::Validation("loan product name_json items are required".to_owned())
        })?;
    let mut has_default_locale = false;
    for item in items {
        let item_object = item.as_object().ok_or_else(|| {
            AppError::Validation("loan product name_json item must be an object".to_owned())
        })?;
        let locale = required_product_name_string(item_object.get("locale"), "locale")?;
        if locale == default_locale {
            has_default_locale = true;
        }
        required_product_name_string(item_object.get("country"), "country")?;
        let title = required_product_name_string(item_object.get("title"), "title")?;
        if title.chars().count() > LOAN_PRODUCT_NAME_TITLE_MAX_LEN {
            return Err(AppError::Validation(
                "loan product name_json title is too long".to_owned(),
            ));
        }
    }
    if !has_default_locale {
        return Err(AppError::Validation(
            "loan product name_json default_locale must exist in items".to_owned(),
        ));
    }
    Ok(())
}

/// 从名称条目里取出一个必填字符串字段，缺失、类型不符或裁剪后为空都归为同一种参数错误。
/// 返回的是裁剪后的借用切片，生命周期绑定原始 JSON，因此调用方不需要额外分配。
/// 错误消息带上字段名以区分是 locale、country 还是 title 不合规。
fn required_product_name_string<'a>(value: Option<&'a Value>, field: &str) -> AppResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("loan product name_json {field} is required")))
}

/// 从多语言名称结构中挑出一个用于列表展示的纯文本标题，优先取默认语言对应的条目。
/// 默认语言条目缺失或其标题为空白时，退而取 items 中第一个标题非空的条目。
/// 匹配 locale 时两侧都做裁剪，避免配置里的多余空格导致默认语言匹配不上。
/// 结构不完整、字段类型不符或所有标题均为空白时返回 `None`，由调用方决定用原始名称兜底。
/// 全程只读，不修改原始配置，也不写回数据库。
pub(crate) fn product_default_name(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let default_locale = object.get("default_locale")?.as_str()?.trim();
    let items = object.get("items")?.as_array()?;
    let default_title = items.iter().find_map(|item| {
        let item_object = item.as_object()?;
        if item_object.get("locale")?.as_str()?.trim() != default_locale {
            return None;
        }
        item_object
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    });
    default_title
        .or_else(|| {
            items.iter().find_map(|item| {
                item.as_object()?
                    .get("title")?
                    .as_str()
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .map(ToOwned::to_owned)
            })
        })
        .filter(|title| !title.is_empty())
}

/// 归一借贷各列表接口的分页量，缺省 50，越界夹紧到 1..=200 而不是返回参数错误。
/// 上限比闪兑的 100 更宽，因为后台产品与订单列表常需一次拉取较多行核对。
/// 用户端和管理端共用同一口径，调用方不得绕过本函数直接使用原始 limit。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// 归一后台分页偏移，缺省 0 并硬性截断到十万。
/// 设上限是因为 MySQL 的 LIMIT OFFSET 需要先扫描并丢弃前 offset 行，
/// 超大偏移会让订单这类大表退化为全表扫描加文件排序，拖垮整个后台查询。
/// 超限时静默截断而非报错，代价是极深分页会重复返回同一页数据。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 从鉴权主体中解出用户编号，只接受 `user:{u64}` 形式，其余一律按未授权拒绝。
/// 前缀不符与数字解析失败走同一分支，不向调用方泄露主体的具体格式问题。
/// 借贷所有用户侧资金入口都必须经此取得 user_id，禁止从请求体读取用户维度。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从鉴权主体中解出管理员编号，只接受 `admin:{u64}` 形式，其余按未授权拒绝。
/// 前缀与用户版本不同，因此用户令牌无法冒充管理员通过审批或拒绝接口。
/// 解析结果会落入订单的 approved_by 或 rejected_by 字段，构成审核操作的审计线索。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}
