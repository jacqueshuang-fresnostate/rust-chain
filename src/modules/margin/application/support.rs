//! 杠杆用例层的共享校验与归一化工具。
//!
//! 这里集中三类无状态能力：从应用状态取 MySQL 连接池、把分页参数夹到安全区间，
//! 以及对金额、费率、保证金模式、仓位状态等入参做统一的枚举与精度校验。
//! 所有校验都发生在开启事务之前，目的是让非法请求在触碰任何钱包行锁前就失败。
//! 精度上限常量与资金列的 `DECIMAL` 定义对齐：费率八位小数、金额十八位小数，超出一律拒绝。

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool};
use std::str::FromStr;
/// 统一从应用状态中取得 Margin 用例所需的 MySQL 连接池。
/// 路由只负责传入状态；缺少连接池时在开启事务或产生任何资金副作用前失败。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for margin routes".to_owned())
    })
}

/// 归一化后台分页偏移，缺省为零并封顶十万条，防止超大 offset 把仓位和产品大表拖成全表扫描加文件排序。
/// 与 `route_limit` 配合决定最终 SQL 的 LIMIT 与 OFFSET；纯函数不访问存储也不影响资金语义。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 归一化列表页大小到 `1..=100`，缺省为 50，避免一次读取过多仓位或产品。
/// 该纯函数不访问存储、不持有事务，也不改变幂等、资金或事件语义。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 识别 MySQL 唯一键冲突，同时接受错误码 1062 与 SQLSTATE 23000 两种驱动上报形式。
/// 开仓与划转在插入幂等键记录时靠它区分并发重放和真实数据库故障：判定为冲突才回滚去走只读重放，
/// 否则原样上抛为数据库错误，避免把连接中断之类的问题误当成重复请求而返回旧结果。
pub(super) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

/// 为钱包划转生成用户作用域的稳定 SHA-256 指纹，覆盖解析后资产、归一化方向与精确金额。
pub(crate) fn margin_transfer_request_fingerprint(
    user_id: u64,
    asset_id: u64,
    from_account: &str,
    to_account: &str,
    amount: &BigDecimal,
) -> String {
    let fields = [
        "margin_wallet_transfer_v1".to_owned(),
        user_id.to_string(),
        asset_id.to_string(),
        from_account.trim().to_owned(),
        to_account.trim().to_owned(),
        amount.normalized().to_plain_string(),
    ];
    let mut digest = sha2::Sha256::default();
    for field in fields {
        sha2::Digest::update(&mut digest, (field.len() as u64).to_be_bytes());
        sha2::Digest::update(&mut digest, field.as_bytes());
    }
    hex::encode(sha2::Digest::finalize(digest))
}

/// 保证金费率最多保留八位小数，超过该精度的产品配置必须拒绝。
pub(super) const MARGIN_RATE_MAX_SCALE: i64 = 8;
/// 保证金费率整数部分最多十位，约束后台配置与数据库列容量一致。
pub(super) const MARGIN_RATE_MAX_INTEGER_DIGITS: usize = 10;
/// 后台保证金操作原因最多五百一十二字符，避免审计字段被无界输入占满。
pub(super) const MARGIN_AUDIT_REASON_MAX_LEN: usize = 512;
/// 保证金金额最多保留十八位小数，与钱包和仓位金额列精度一致。
pub(super) const MARGIN_AMOUNT_MAX_SCALE: i64 = 18;
/// 保证金金额整数部分最多二十位，防止超出资金字段容量。
pub(super) const MARGIN_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;
/// 校验保证金额、杠杆倍数、划转金额或行情价格严格大于零，零和负数一律判为参数非法。
/// `label` 只用于拼接错误文案以便定位是哪个字段越界，不参与判定逻辑。
/// 调用方须在开启资金事务、获取任何行锁之前先过这一关，避免非法输入进入结算计算。
pub(super) fn validate_positive_decimal(amount: &BigDecimal, label: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "margin {label} must be positive"
        )));
    }
    Ok(())
}
/// 把保证金模式文本裁剪空白后规范为 isolated 或 cross，仅接受这两个字面量。
/// 空串与纯空白按缺失处理并报必填，其余未知取值报枚举非法，两种情况都不会落库或触发持久化。
/// 注意这里不做大小写折叠，客户端必须传小写；产品配置和用户设置共用同一套判定口径。
pub(super) fn normalized_margin_mode(value: &str) -> AppResult<String> {
    let Some(mode) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin product margin_mode is required".to_owned(),
        ));
    };
    match mode.as_str() {
        "isolated" | "cross" => Ok(mode),
        _ => Err(AppError::Validation(
            "margin product margin_mode must be isolated or cross".to_owned(),
        )),
    }
}

/// 把仓位状态筛选值限制为 opened、closed、liquidated、canceled 四个终态或活跃态之一。
/// 这四个值就是 `margin_positions.status` 的全部合法取值，用户列表、后台历史和利息汇总共用。
/// 值虽然通过 `push_bind` 参数化绑定，仍在这里先做白名单校验，避免无效条件白跑一次全表筛选。
pub(super) fn normalized_position_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin position status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "opened" | "closed" | "liquidated" | "canceled" => Ok(status),
        _ => Err(AppError::Validation(
            "margin position status must be opened, closed, canceled, or liquidated".to_owned(),
        )),
    }
}

/// 把产品配置里以字符串保存的某个杠杆档位解析成十进制，再与请求传入的倍数做精确相等比较。
/// 用 `BigDecimal` 相等而非浮点近似，所以 "10" 与 "10.0" 视为同一档位，而 10.01 不会被放行。
/// 解析失败时返回不匹配而不是报错，让调用方继续尝试后续档位，最终由「无任何档位命中」统一报错。
pub(super) fn decimal_matches_string(value: &BigDecimal, expected: &str) -> bool {
    BigDecimal::from_str(expected)
        .map(|level| &level == value)
        .unwrap_or(false)
}

/// 裁剪可选文本两端空白，并把裁剪后为空的值一并折叠成 None，消除「传了但等于没传」的中间态。
/// 后台筛选条件、变更原因和状态字段都先过这一步，保证空串不会被当成有效筛选值拼进查询。
pub(super) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 确认某个保证金模式已被后端风控真正实现，目前只有逐仓和账户级全仓两种。
/// 与 `normalized_margin_mode` 的分工是：那个负责文本合法性，这个负责能力可用性。
/// 后台配置产品时也要过这道关，避免管理员配出用户点了就报错、或风控无法处置的模式。
pub(super) fn ensure_supported_user_margin_mode(mode: &str) -> AppResult<()> {
    if !matches!(mode, "isolated" | "cross") {
        return Err(AppError::Validation("unsupported margin mode".to_owned()));
    }
    Ok(())
}

/// 把中间计算结果截断为非负数并统一到十八位小数，与资金列 `DECIMAL(38,18)` 精度对齐。
/// 用于两个边界：一倍杠杆时名义价值减保证金得零的借款额，以及亏损吃穿保证金后归零的返还额。
/// 截断掉的负数部分不会在这里被记录，穿仓缺口须由全仓账户结算的坏账字段单独承担。
pub(super) fn non_negative_amount(amount: &BigDecimal) -> BigDecimal {
    if amount > &BigDecimal::from(0) {
        amount.clone().with_scale(18)
    } else {
        BigDecimal::from(0).with_scale(18)
    }
}
