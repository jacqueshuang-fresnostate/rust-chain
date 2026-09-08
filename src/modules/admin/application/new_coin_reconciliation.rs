//! 新币派发/退款批次对账应用用例。

use super::*;
use crate::modules::admin::{
    infrastructure::load_admin_new_coin_reconciliation,
    presentation::NewCoinReconciliationResponse,
};

fn zero() -> BigDecimal {
    BigDecimal::from(0)
}

fn is_zero(value: &BigDecimal) -> bool {
    value.normalized() == zero().normalized()
}

/// 把基础聚合转换成后台可直接展示的差额与异常列表。
/// 该函数不访问外部依赖，所有异常判定都基于同一份数据库快照，便于单元测试和重放。
pub(crate) fn build_new_coin_reconciliation_response(
    record: crate::modules::admin::infrastructure::AdminNewCoinReconciliationRecord,
    checked_at: DateTime<Utc>,
) -> NewCoinReconciliationResponse {
    let supply_delta = &record.total_supply
        - &record.reserved_supply
        - &record.allocated_supply
        - &record.remaining_supply;
    let subscription_distribution_delta =
        &record.subscription_allocated_quantity - &record.linked_distribution_quantity;
    let distribution_ledger_delta =
        &record.distribution_quantity - &record.distribution_ledger_quantity;
    let manual_quote_delta = &record.manual_quote_amount
        - &record.manual_frozen_quote_amount
        - &record.manual_settled_quote_amount
        - &record.manual_refunded_quote_amount;

    let mut anomalies = Vec::new();
    if !is_zero(&supply_delta) {
        anomalies.push("发行供给未守恒".to_owned());
    }
    if record.subscription_allocated_quantity > record.requested_quantity {
        anomalies.push("申购配售量超过申购量".to_owned());
    }
    if !is_zero(&subscription_distribution_delta) {
        anomalies.push("申购配售量与关联派发量不一致".to_owned());
    }
    if !is_zero(&distribution_ledger_delta) {
        anomalies.push("派发数量与钱包入账流水不一致".to_owned());
    }
    if record.invalid_subscription_link_count > 0 {
        anomalies.push("存在无效申购关联".to_owned());
    }
    if !is_zero(&manual_quote_delta) {
        anomalies.push("人工申购资金未守恒".to_owned());
    }
    if record.pending_manual_count > 0 && record.lifecycle_status == "listed" {
        anomalies.push("项目已上市但仍有待处理人工申购".to_owned());
    }

    NewCoinReconciliationResponse {
        project_id: record.project_id,
        symbol: record.symbol,
        lifecycle_status: record.lifecycle_status,
        total_supply: record.total_supply,
        reserved_supply: record.reserved_supply,
        allocated_supply: record.allocated_supply,
        remaining_supply: record.remaining_supply,
        supply_delta,
        subscription_count: record.subscription_count,
        pending_manual_count: record.pending_manual_count,
        requested_quantity: record.requested_quantity,
        subscription_allocated_quantity: record.subscription_allocated_quantity,
        distribution_quantity: record.distribution_quantity,
        linked_distribution_quantity: record.linked_distribution_quantity,
        unlinked_distribution_quantity: record.unlinked_distribution_quantity,
        distribution_ledger_quantity: record.distribution_ledger_quantity,
        invalid_subscription_link_count: record.invalid_subscription_link_count,
        subscription_distribution_delta,
        distribution_ledger_delta,
        manual_quote_amount: record.manual_quote_amount,
        manual_frozen_quote_amount: record.manual_frozen_quote_amount,
        manual_settled_quote_amount: record.manual_settled_quote_amount,
        manual_refunded_quote_amount: record.manual_refunded_quote_amount,
        manual_quote_delta,
        anomaly_count: anomalies.len() as i64,
        status: if anomalies.is_empty() {
            "balanced".to_owned()
        } else {
            "attention".to_owned()
        },
        anomalies,
        checked_at,
    }
}

/// 读取单个项目的派发/退款对账快照；查询只读，不创建审计记录或改变业务状态。
pub(crate) async fn get_admin_new_coin_reconciliation(
    pool: Option<Pool<MySql>>,
    project_id: u64,
) -> AppResult<NewCoinReconciliationResponse> {
    let pool = admin_mysql_pool(pool)?;
    let record = load_admin_new_coin_reconciliation(&pool, project_id).await?;
    Ok(build_new_coin_reconciliation_response(record, Utc::now()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> crate::modules::admin::infrastructure::AdminNewCoinReconciliationRecord {
        crate::modules::admin::infrastructure::AdminNewCoinReconciliationRecord {
            project_id: 7,
            symbol: "HIP".to_owned(),
            lifecycle_status: "distribution".to_owned(),
            total_supply: BigDecimal::from(100),
            reserved_supply: BigDecimal::from(10),
            allocated_supply: BigDecimal::from(20),
            remaining_supply: BigDecimal::from(70),
            subscription_count: 2,
            pending_manual_count: 0,
            requested_quantity: BigDecimal::from(30),
            subscription_allocated_quantity: BigDecimal::from(20),
            distribution_quantity: BigDecimal::from(20),
            linked_distribution_quantity: BigDecimal::from(20),
            unlinked_distribution_quantity: BigDecimal::from(0),
            distribution_ledger_quantity: BigDecimal::from(20),
            invalid_subscription_link_count: 0,
            manual_quote_amount: BigDecimal::from(50),
            manual_frozen_quote_amount: BigDecimal::from(0),
            manual_settled_quote_amount: BigDecimal::from(40),
            manual_refunded_quote_amount: BigDecimal::from(10),
        }
    }

    #[test]
    fn balanced_snapshot_has_zero_deltas() {
        let response = build_new_coin_reconciliation_response(record(), Utc::now());
        assert_eq!(response.status, "balanced");
        assert_eq!(response.anomaly_count, 0);
        assert!(response.anomalies.is_empty());
        assert!(is_zero(&response.supply_delta));
        assert!(is_zero(&response.manual_quote_delta));
    }

    #[test]
    fn inconsistent_snapshot_lists_actionable_anomalies() {
        let mut input = record();
        input.distribution_ledger_quantity = BigDecimal::from(19);
        input.pending_manual_count = 1;
        input.lifecycle_status = "listed".to_owned();
        let response = build_new_coin_reconciliation_response(input, Utc::now());
        assert_eq!(response.status, "attention");
        assert!(response.anomalies.iter().any(|value| value.contains("钱包")));
        assert!(response.anomalies.iter().any(|value| value.contains("已上市")));
    }
}
