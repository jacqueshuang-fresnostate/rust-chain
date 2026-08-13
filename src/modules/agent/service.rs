//! agent bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前集中处理代理分页、口令、邀请码、返佣产品归一化及响应聚合规则。
//! 这里的函数全部是无 I/O 的纯计算，负责在应用层触达数据库之前把外部输入收敛成受控取值，
//! 以及在查询返回后把数据库记录拼装成对外响应，因此既不开事务也不发布事件。

use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            presentation::{
                AgentCommissionResponse, AgentCommissionsResponse, AgentConvertStatsResponse,
                AgentDashboardAssetSummaryResponse, AgentDashboardResponse,
            },
            repository::{AgentConvertStatsRecord, AgentDashboardCountsRecord, AgentListPage},
        },
        auth::domain::{required_string, validate_reset_password},
    },
};
use bigdecimal::BigDecimal;
use uuid::Uuid;

// 以下五个常量是返佣规则表 product_type 列的全部合法取值，新增业务线必须同时扩充归一化函数。
pub(crate) const AGENT_COMMISSION_PRODUCT_CONVERT: &str = "convert";
pub(crate) const AGENT_COMMISSION_PRODUCT_MARGIN: &str = "margin";
pub(crate) const AGENT_COMMISSION_PRODUCT_PREDICTION: &str = "prediction";
pub(crate) const AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT: &str = "seconds_contract";
pub(crate) const AGENT_COMMISSION_PRODUCT_SPOT: &str = "spot";

/// 将返佣产品类型归一化为已实现的五类稳定存储值，未知类型直接拒绝。
/// 输入会去除空白并转小写；函数无持久化副作用，不得把未支持类型透传到规则 SQL。
pub(crate) fn normalize_agent_commission_product_type(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        AGENT_COMMISSION_PRODUCT_CONVERT => Ok(AGENT_COMMISSION_PRODUCT_CONVERT.to_owned()),
        AGENT_COMMISSION_PRODUCT_MARGIN => Ok(AGENT_COMMISSION_PRODUCT_MARGIN.to_owned()),
        AGENT_COMMISSION_PRODUCT_PREDICTION => Ok(AGENT_COMMISSION_PRODUCT_PREDICTION.to_owned()),
        AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT => {
            Ok(AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT.to_owned())
        }
        AGENT_COMMISSION_PRODUCT_SPOT => Ok(AGENT_COMMISSION_PRODUCT_SPOT.to_owned()),
        _ => Err(AppError::Validation(
            "unsupported agent commission product type".to_owned(),
        )),
    }
}

/// 构造代理查询分页：未传页大小时使用调用方上限，显式值限制在 `1..=default_limit`，偏移默认零。
/// 上限由各用例按数据量自行给定，团队用户与佣金列表取一百，子代理与团队树取五百，避免单次扫描整棵子树。
/// 偏移不设上限也不做总数校验，越界时数据库自然返回空集，不视为错误。
pub(crate) fn agent_list_page(
    limit: Option<u32>,
    offset: Option<u32>,
    default_limit: u32,
) -> AgentListPage {
    AgentListPage {
        limit: limit.unwrap_or(default_limit).clamp(1, default_limit),
        offset: offset.unwrap_or(0),
    }
}

/// 从 JWT 主体串中提取代理管理员主键，只接受 `agent:<数字>` 这一种形式。
/// 前缀不符说明令牌属于普通用户或后台管理员，数字段溢出 u64 同样视为伪造，两类都直接返回未授权而非校验错误，
/// 避免向调用方泄露主体格式细节。本函数不查库，返回的 ID 仍需由后续查询确认账号有效。
pub(crate) fn agent_admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("agent:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 校验代理自助改密的两个入参：旧口令只要求非空，新口令复用平台统一的重置口令强度策略。
/// 两者都缺省会先报出对应字段名的校验错误；新旧口令完全相同时拒绝，避免用户误以为已完成轮换。
/// 返回原文口令对，由调用方负责比对旧哈希并生成新哈希，本函数不做任何加密或落库动作。
pub(crate) fn validate_agent_password_change(
    current_password: Option<String>,
    new_password: Option<String>,
) -> AppResult<(String, String)> {
    let current_password = required_string(current_password, "current_password")?;
    let new_password = validate_reset_password(&required_string(new_password, "new_password")?)?;
    if current_password == new_password {
        return Err(AppError::Validation(
            "new_password must be different from current_password".to_owned(),
        ));
    }
    Ok((current_password, new_password))
}

/// 校验代理邀请码的可选使用上限：字段缺省表示不限领取次数，一旦显式给出则必须为正整数。
/// 零和负数会让邀请码创建出来即不可用，属于典型的运营误填，因此在入库前直接拒绝为校验错误。
/// 本函数不比较历史已用次数，也不校验上限是否小于既有用量，创建场景下用量恒为零。
pub(crate) fn validate_agent_invite_code_usage_limit(limit: Option<i32>) -> AppResult<()> {
    if limit.is_some_and(|limit| limit <= 0) {
        return Err(AppError::Validation(
            "usage_limit must be positive".to_owned(),
        ));
    }

    Ok(())
}

/// 把邀请码状态入参收敛成 active 与 disabled 两个稳定存储值，返回静态串以杜绝客户端原文透传进 SQL。
/// 仅去除首尾空白且区分大小写，不做小写归一，任何其他取值都按校验错误拒绝，防止写入前端自造的过渡态。
/// 状态切换本身是否允许由调用方按所有权判断，本函数只负责取值合法性。
pub(crate) fn validate_agent_invite_code_status(status: &str) -> AppResult<&'static str> {
    match status.trim() {
        "active" => Ok("active"),
        "disabled" => Ok("disabled"),
        _ => Err(AppError::Validation(
            "status must be active or disabled".to_owned(),
        )),
    }
}

/// 生成代理邀请码文本：AGT 前缀加 UUIDv7 的无分隔十六进制串，时间有序便于按生成顺序排查。
/// 本函数只保证极低碰撞概率，不查库预检，真正的唯一性由邀请码表的唯一索引在插入时兜底。
/// 冲突时不会在此重试，调用链会把数据库错误直接上抛，由运营重新发起创建。
pub(crate) fn generated_agent_invite_code() -> String {
    // 代理邀请码统一使用 AGT 前缀，便于和普通用户邀请码在运营侧快速区分。
    format!("AGT{}", Uuid::now_v7().simple())
}

/// 将 SQL 聚合记录映射为代理兑换统计，计数字段无法转为整数时返回内部错误。
/// 金额保持数据库精度原样返回；转换不修改记录，也不触发佣金或订单副作用。
/// 待处理与已完成两项计数来自条件求和因而是十进制，经字符串中转解析为整数，越界时报内部错误而非静默截断。
pub(crate) fn agent_convert_stats_response(
    row: AgentConvertStatsRecord,
) -> AppResult<AgentConvertStatsResponse> {
    Ok(AgentConvertStatsResponse {
        agent_id: row.agent_id,
        total_orders: row.total_orders,
        pending_orders: row.pending_orders.to_string().parse().map_err(|_| {
            AppError::Internal("failed to decode pending convert order count".to_owned())
        })?,
        completed_orders: row.completed_orders.to_string().parse().map_err(|_| {
            AppError::Internal("failed to decode completed convert order count".to_owned())
        })?,
        total_from_amount: row.total_from_amount,
        total_to_amount: row.total_to_amount,
    })
}

/// 组装代理看板汇总；仅单一发放资产时才在顶层展示可相加的佣金金额。
/// 多资产时顶层金额置零并保留分资产明细，本转换不读写数据库且不发布事件。
/// 佣金记录总数由各资产明细相加得出，与顶层金额是否归零无关，因此计数在任何情况下都真实可用。
pub(crate) fn agent_dashboard_response(
    agent_id: u64,
    counts: AgentDashboardCountsRecord,
    commission_assets: Vec<AgentDashboardAssetSummaryResponse>,
) -> AgentDashboardResponse {
    let commission_record_count = commission_assets
        .iter()
        .map(|asset| asset.commission_record_count)
        .sum();
    // 跨资产金额不可相加：顶层金额仅在单一发放资产时有意义，否则归零并以明细为准。
    let (pending, settled, total) = match commission_assets.as_slice() {
        [single] => (
            single.pending_commission_amount.clone(),
            single.settled_commission_amount.clone(),
            single.total_commission_amount.clone(),
        ),
        _ => (
            BigDecimal::from(0),
            BigDecimal::from(0),
            BigDecimal::from(0),
        ),
    };

    AgentDashboardResponse {
        agent_id,
        team_user_count: counts.team_user_count,
        active_invite_code_count: counts.active_invite_code_count,
        commission_record_count,
        pending_commission_amount: pending,
        settled_commission_amount: settled,
        total_commission_amount: total,
        commission_assets,
    }
}

/// 组装当前代理的佣金列表，总额仅对查询已返回的记录求和。
/// 调用方须保证记录已按代理权限过滤；函数无持久化、钱包或结算副作用。
/// 总记录数取本页长度而非全量计数，合计金额也只覆盖本页并对不同发放资产直接相加，不做汇率换算。
pub(crate) fn agent_commissions_response(
    agent_id: u64,
    commissions: Vec<AgentCommissionResponse>,
) -> AgentCommissionsResponse {
    let total_commission_amount: BigDecimal = commissions
        .iter()
        .map(|record| record.commission_amount.clone())
        .sum();

    AgentCommissionsResponse {
        agent_id,
        total_records: commissions.len() as u64,
        total_commission_amount,
        commissions,
    }
}
