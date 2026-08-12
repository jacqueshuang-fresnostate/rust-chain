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

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 归一化列表页大小到 `1..=100`，缺省为 50，避免一次读取过多仓位或产品。
/// 该纯函数不访问存储、不持有事务，也不改变幂等、资金或事件语义。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 识别 MySQL 唯一键冲突，供幂等键并发写入分支区分重放与真实数据库故障。
pub(super) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
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
/// 校验保证金金额、杠杆或划转量严格为正；失败后调用方须在开启资金事务前停止。
pub(super) fn validate_positive_decimal(amount: &BigDecimal, label: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "margin {label} must be positive"
        )));
    }
    Ok(())
}
/// 将保证金模式规范为 isolated 或 cross，未知值按参数错误处理且不触发持久化。
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

/// 将仓位筛选状态限制为 opened、closed、liquidated 或 canceled，避免拼入非法查询条件。
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

/// 把配置中的杠杆文本解析后与十进制请求精确比较，解析失败视为不匹配而非采用近似值。
pub(super) fn decimal_matches_string(value: &BigDecimal, expected: &str) -> bool {
    BigDecimal::from_str(expected)
        .map(|level| &level == value)
        .unwrap_or(false)
}

/// 裁剪可选文本并把空白值归一为空，供保证金筛选和配置校验共享。
pub(super) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 校验用户可见的保证金模式；逐仓与账户级全仓是当前唯一已实现的风险语义。
pub(super) fn ensure_supported_user_margin_mode(mode: &str) -> AppResult<()> {
    if !matches!(mode, "isolated" | "cross") {
        return Err(AppError::Validation("unsupported margin mode".to_owned()));
    }
    Ok(())
}

/// 将计算结果截断为非负 18 位金额，用于逐仓借款额和返还额边界。
pub(super) fn non_negative_amount(amount: &BigDecimal) -> BigDecimal {
    if amount > &BigDecimal::from(0) {
        amount.clone().with_scale(18)
    } else {
        BigDecimal::from(0).with_scale(18)
    }
}
