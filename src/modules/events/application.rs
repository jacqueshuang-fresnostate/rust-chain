//! events bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::{
        admin::service::admin_id_from_subject,
        auth::AdminAuth,
        events::{
            infrastructure::{
                EventRecordListFilter, InboxRecordRow, OutboxRecordRow,
                list_inbox_records as list_inbox_record_rows,
                list_outbox_records as list_outbox_record_rows, requeue_outbox_dead_letter_in_tx,
            },
            presentation::{
                EventRecordsQuery, EventRecordsResponse, InboxRecordResponse, OutboxRecordResponse,
                PrivateWsQuery, RequeueOutboxRequest,
            },
            service::PrivateWsAuth,
        },
    },
    state::AppState,
};
use sqlx::{MySql, Pool};

/// 由路由层构建完成的查询参数触发私有 WebSocket 鉴权，应用层统一对外部 token 进行消费。
pub(crate) async fn authorize_private_ws(
    state: &AppState,
    query: PrivateWsQuery,
) -> AppResult<PrivateWsAuth> {
    // 保持“路由只透传参数，身份解析集中到应用层”的边界。
    PrivateWsAuth::from_token_query(query.token.as_deref(), state).await
}

/// 查询 outbox 运维记录；调用者必须先通过管理员鉴权，应用层统一获取数据库并保留分页合同。
pub(crate) async fn list_outbox_records(
    state: &AppState,
    query: EventRecordsQuery,
) -> AppResult<EventRecordsResponse<OutboxRecordResponse>> {
    let params = query.normalize();
    let (records, total) = list_outbox_record_rows(
        &events_pool(state)?,
        EventRecordListFilter {
            status: params.status.as_deref(),
            limit: params.limit,
            offset: params.offset,
        },
    )
    .await?;

    Ok(EventRecordsResponse::new(
        records.into_iter().map(outbox_response).collect(),
        total,
    ))
}

/// 查询 inbox 运维记录；调用者必须先通过管理员鉴权，应用层统一获取数据库并保留分页合同。
pub(crate) async fn list_inbox_records(
    state: &AppState,
    query: EventRecordsQuery,
) -> AppResult<EventRecordsResponse<InboxRecordResponse>> {
    let params = query.normalize();
    let (records, total) = list_inbox_record_rows(
        &events_pool(state)?,
        EventRecordListFilter {
            status: params.status.as_deref(),
            limit: params.limit,
            offset: params.offset,
        },
    )
    .await?;

    Ok(EventRecordsResponse::new(
        records.into_iter().map(inbox_response).collect(),
        total,
    ))
}

/// 管理员重排 outbox 死信并写入同事务审计。
///
/// 仅接受 `admin:<id>` 身份和非空原因；只有 `dead_letter` 状态可转回 `pending`。
/// 已重排记录再次调用会返回冲突，因此不会重复清零重试次数或追加第二条审计记录。
pub(crate) async fn requeue_outbox_dead_letter(
    state: &AppState,
    auth: AdminAuth,
    id: u64,
    request: RequeueOutboxRequest,
) -> AppResult<OutboxRecordResponse> {
    let reason = request.require_reason()?;
    let admin_id = admin_id_from_subject(&auth.0.sub)?;
    let pool = events_pool(state)?;
    let mut tx = pool.begin().await?;
    let record = requeue_outbox_dead_letter_in_tx(&mut tx, admin_id, id, &reason).await?;
    tx.commit().await?;

    Ok(outbox_response(record))
}

/// 从应用状态取得 events 数据库连接池，统一保持未配置 MySQL 时的错误语义。
fn events_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for event routes".to_owned())
    })
}

/// 将 outbox 持久化行映射为稳定的运维接口响应，不暴露 SQLx 行类型。
fn outbox_response(row: OutboxRecordRow) -> OutboxRecordResponse {
    OutboxRecordResponse {
        id: row.id,
        aggregate_type: row.aggregate_type,
        aggregate_id: row.aggregate_id,
        event_type: row.event_type,
        routing_key: row.routing_key,
        status: row.status,
        retry_count: row.retry_count,
        next_retry_at: row.next_retry_at,
        published_at: row.published_at,
        created_at: row.created_at,
    }
}

/// 将 inbox 持久化行映射为稳定的运维接口响应，保留时间字段的毫秒序列化合同。
fn inbox_response(row: InboxRecordRow) -> InboxRecordResponse {
    InboxRecordResponse {
        id: row.id,
        consumer_name: row.consumer_name,
        message_id: row.message_id,
        status: row.status,
        retry_count: row.retry_count,
        error_message: row.error_message,
        consumed_at: row.consumed_at,
        created_at: row.created_at,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_events_application_tests.rs"]
mod tests;
