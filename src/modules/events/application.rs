//! events bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    error::{AppError, AppResult},
    modules::{
        admin::service::admin_id_from_subject,
        auth::{AdminAuth, TokenScope, claims_from_bearer_token},
        events::{
            infrastructure::{
                EventRecordListFilter, InboxRecordRow, MySqlEventInboxRepository,
                MySqlEventOutboxRepository, MySqlUserWalletInitializer, OutboxRecordRow,
                list_inbox_records as list_inbox_record_rows,
                list_outbox_records as list_outbox_record_rows, requeue_outbox_dead_letter_in_tx,
            },
            presentation::{
                EventRecordsQuery, EventRecordsResponse, InboxRecordResponse, OutboxRecordResponse,
                PrivateWsQuery, RequeueOutboxRequest,
            },
            service::{
                EventInboxConsumerService, EventInboxProductionHandler, EventOutboxService,
                InboxRetryPolicy, PrivateWsAuth, PublishedOutboxBatch, RabbitMqOutboxPublisher,
            },
        },
    },
    state::AppState,
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{MySql, Pool};
use std::sync::Arc;

/// 校验私有 WebSocket 查询中的用户 token，并把会话 subject 收敛为单一 `user_id` 路由身份。
/// 仅接受 user scope；缺失、过期或后端已撤销的会话直接失败，鉴权成功前不会升级连接，也不创建订阅、持久化游标或广播消息。
pub(crate) async fn authorize_private_ws(
    state: &AppState,
    query: PrivateWsQuery,
) -> AppResult<PrivateWsAuth> {
    let token = query
        .token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or(AppError::Unauthorized)?;
    let claims = claims_from_bearer_token(state, token, TokenScope::User).await?;
    PrivateWsAuth::from_user_subject(&claims.sub)
}

impl PrivateWsAuth {
    /// 从兼容查询串中提取首个非空 `token` 参数，并通过运行时会话存储校验 user scope。
    /// 缺失、失效或已撤销 token 返回鉴权错误；本入口只解析身份，不升级 WebSocket、不订阅私有频道，也不修改登录态。
    pub async fn from_query_state(query: Option<&str>, state: &AppState) -> AppResult<Self> {
        let token = query
            .and_then(|query| {
                query.split('&').find_map(|pair| {
                    let (key, value) = pair.split_once('=')?;
                    (key == "token" && !value.is_empty()).then_some(value)
                })
            })
            .ok_or(AppError::Unauthorized)?;
        Self::from_token_query(Some(token), state).await
    }

    /// 校验调用方已提取的 token，并把严格的 `user:<u64>` subject 转为私有频道用户 ID。
    /// 空 token、非 user scope、会话失效或 subject 畸形均拒绝；认证存储错误原样上抛，不降级为未校验连接。
    pub async fn from_token_query(query_token: Option<&str>, state: &AppState) -> AppResult<Self> {
        let token = query_token
            .filter(|value| !value.is_empty())
            .ok_or(AppError::Unauthorized)?;
        let claims = claims_from_bearer_token(state, token, TokenScope::User).await?;
        Self::from_user_subject(&claims.sub)
    }
}

/// 从运行时状态组装生产 outbox 服务；保持既有 exchange、5 次/30 秒重试与默认批量 100。
/// 缺少 MySQL/RabbitMQ 时返回原有配置错误；仅装配依赖，不查询、发布或修改 outbox。
pub(crate) fn outbox_service_from_state(
    state: &AppState,
) -> AppResult<EventOutboxService<MySqlEventOutboxRepository, RabbitMqOutboxPublisher>> {
    outbox_service_from_state_with_batch_size(state, 100)
}

/// 以调用方给定的单轮扫描上限装配生产 outbox：读取 MySQL，向 durable topic exchange `exchange.events` 发布，并采用最多 5 次、固定 30 秒退避。
/// `batch_size` 原样交给仓储，零值产生空批；构造阶段不查询数据库、不创建 RabbitMQ channel，也不推进发布、重试或死信状态。
pub(crate) fn outbox_service_from_state_with_batch_size(
    state: &AppState,
    batch_size: u32,
) -> AppResult<EventOutboxService<MySqlEventOutboxRepository, RabbitMqOutboxPublisher>> {
    let pool = state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for event outbox persistence".to_owned())
    })?;
    let rabbitmq = state.rabbitmq.clone().ok_or_else(|| {
        AppError::Internal(
            "rabbitmq connection is not configured for event outbox publisher".to_owned(),
        )
    })?;
    let retry_policy = InboxRetryPolicy::new(5, TimeDelta::seconds(30)).map_err(|error| {
        AppError::Internal(format!("invalid event outbox retry policy: {error}"))
    })?;

    Ok(EventOutboxService::new(
        MySqlEventOutboxRepository::new(pool),
        RabbitMqOutboxPublisher::new(rabbitmq, "exchange.events"),
        retry_policy,
        batch_size,
    ))
}

/// 为一个稳定 `consumer_name` 装配 MySQL inbox 仓储、生产 dispatch 与用户钱包初始化适配器，并采用最多 5 次、固定 30 秒退避。
/// consumer 名称决定去重和补偿重放范围；构造不领取租约、不执行 handler，缺少 MySQL 或策略非法时在启动消费前失败。
pub(crate) fn inbox_service_from_state(
    state: &AppState,
    consumer_name: impl Into<String>,
) -> AppResult<EventInboxConsumerService<MySqlEventInboxRepository, EventInboxProductionHandler>> {
    let pool = state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for event inbox persistence".to_owned())
    })?;
    let retry_policy = InboxRetryPolicy::new(5, TimeDelta::seconds(30)).map_err(|error| {
        AppError::Internal(format!("invalid event inbox retry policy: {error}"))
    })?;
    let initializer = Arc::new(MySqlUserWalletInitializer::new(pool.clone()));

    Ok(EventInboxConsumerService::new(
        consumer_name,
        MySqlEventInboxRepository::new(pool),
        EventInboxProductionHandler::new(Some(initializer)),
        retry_policy,
    ))
}

impl EventOutboxService<MySqlEventOutboxRepository, RabbitMqOutboxPublisher> {
    /// 从 `AppState` 装配单轮最多读取 100 条的生产 outbox 服务，固定发布到 `exchange.events`。
    /// 缺 MySQL 或 RabbitMQ 即返回配置错误；实际网络发布和持久状态推进只在 `publish_once` 发生。
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        outbox_service_from_state(state)
    }

    /// 从 `AppState` 装配调用方指定扫描上限的生产 outbox 服务，并保留 5 次/30 秒重试与 `exchange.events` 路由合同。
    /// 这里只绑定仓储和 publisher；不持锁、不打开业务事务，`basic_publish` 完成后的状态推进由后续 `publish_once` 负责。
    pub fn from_state_with_batch_size(state: &AppState, batch_size: u32) -> AppResult<Self> {
        outbox_service_from_state_with_batch_size(state, batch_size)
    }
}

impl EventInboxConsumerService<MySqlEventInboxRepository, EventInboxProductionHandler> {
    /// 从 `AppState` 为指定消费者装配持久化 inbox 与生产 handler；`consumer_name` 同时限定去重键和数据库补偿扫描范围。
    /// 构造不接收 RabbitMQ delivery、不领取处理租约；业务错误在消费时按 5 次/30 秒策略落 retry 或 dead-letter。
    pub fn from_state(state: &AppState, consumer_name: impl Into<String>) -> AppResult<Self> {
        inbox_service_from_state(state, consumer_name)
    }
}

/// 执行默认上限 100 条的 outbox 发布周期：只扫描 `pending` 与已到期 `retry`，逐条等待 RabbitMQ `basic_publish` future 完成后再标记 `published`。
/// 当前 channel 未启用 publisher-confirm 模式，因此该完成不等同 broker ACK；崩溃窗口可能造成丢失或重复，下游仍须按 message_id 幂等。
/// broker 失败按 5 次/30 秒策略持久化为 retry/dead-letter；消息间没有总事务，前项成功不会因后项失败回滚，调用方时间只限定扫描到期边界。
pub(crate) async fn publish_outbox_once(
    state: &AppState,
    now: DateTime<Utc>,
) -> AppResult<PublishedOutboxBatch> {
    outbox_service_from_state(state)?.publish_once(now).await
}

/// 查询 outbox 运维记录；调用者必须先通过管理员鉴权，应用层统一获取数据库并保留分页合同。
/// 应用层校验运维筛选和分页后读取 outbox，不发布、重试或修改消息。
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
/// 应用层校验消费者、状态与分页后读取 inbox，不领取租约或执行 handler。
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
