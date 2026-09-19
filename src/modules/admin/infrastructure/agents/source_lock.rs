//! 秒合约佣金和来源退款共用来源订单优先锁序，不改变其他产品的既有生命周期。

use super::*;

/// 在事务开始前预读来源定位，避免占用资金连接后再等待连接池，也不提前建立 RR 旧快照。
pub(crate) async fn commission_source_hint(
    pool: &Pool<MySql>,
    commission_id: u64,
) -> AppResult<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT source_type, source_id FROM agent_commission_records WHERE id = ?",
    )
    .bind(commission_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// 预读只定位不可变来源，先锁秒合约主行再锁佣金并逐字段重验来源。
/// 后续资格必须锁定读来源状态，不能使用预读建立的一致性快照；来源变化失败关闭。
/// 手动、批量、worker 和冲正共用此入口，避免退款持来源锁等待佣金时产生反向锁。
pub(crate) async fn lock_agent_commission_with_source_in_tx(
    tx: &mut Transaction<'_, MySql>,
    commission_id: u64,
    hint: (String, String),
) -> AppResult<AdminAgentCommissionResponse> {
    if hint.0 == "seconds_contract_order" {
        crate::modules::seconds_contract::infrastructure::refund::lock_source_order(
            tx,
            parse_agent_commission_source_id(&hint.1)?,
        )
        .await?;
    }
    let locked = lock_agent_commission_in_tx(tx, commission_id).await?;
    if locked.source_type != hint.0 || locked.source_id != hint.1 {
        return Err(AppError::Conflict(
            "commission source changed before source lock".into(),
        ));
    }
    Ok(locked)
}
