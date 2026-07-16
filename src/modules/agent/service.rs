//! agent bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    architecture::ServiceLayer,
    error::{AppError, AppResult},
    modules::agent::{
        presentation::{
            AgentCommissionResponse, AgentCommissionsResponse, AgentConvertStatsResponse,
        },
        repository::AgentConvertStatsRecord,
    },
};
use bigdecimal::BigDecimal;
use uuid::Uuid;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

pub(crate) const AGENT_COMMISSION_PRODUCT_CONVERT: &str = "convert";
pub(crate) const AGENT_COMMISSION_PRODUCT_MARGIN: &str = "margin";
pub(crate) const AGENT_COMMISSION_PRODUCT_PREDICTION: &str = "prediction";
pub(crate) const AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT: &str = "seconds_contract";
pub(crate) const AGENT_COMMISSION_PRODUCT_SPOT: &str = "spot";

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

pub(crate) fn agent_admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("agent:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn validate_agent_invite_code_usage_limit(limit: Option<i32>) -> AppResult<()> {
    if limit.is_some_and(|limit| limit <= 0) {
        return Err(AppError::Validation(
            "usage_limit must be positive".to_owned(),
        ));
    }

    Ok(())
}

pub(crate) fn validate_agent_invite_code_status(status: &str) -> AppResult<&'static str> {
    match status.trim() {
        "active" => Ok("active"),
        "disabled" => Ok("disabled"),
        _ => Err(AppError::Validation(
            "status must be active or disabled".to_owned(),
        )),
    }
}

pub(crate) fn generated_agent_invite_code() -> String {
    // 代理邀请码统一使用 AGT 前缀，便于和普通用户邀请码在运营侧快速区分。
    format!("AGT{}", Uuid::now_v7().simple())
}

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
