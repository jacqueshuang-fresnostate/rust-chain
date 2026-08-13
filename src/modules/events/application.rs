//! events bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//!
//! 本层承担三件事：把 `AppState` 中的 MySQL 与 RabbitMQ 句柄装配成 outbox 发布服务和 inbox 消费服务、
//! 为私有 WebSocket 做连接前鉴权、以及提供事件记录的运维查询与死信重排用例。
//!
//! 装配函数集中定义了全局一致的投递参数：发布目标固定为 `exchange.events`，
//! outbox 与 inbox 共用最多 5 次、固定 30 秒的退避策略，即退避曲线是常量间隔而非指数增长，
//! 单轮扫描默认上限 100 条。任何一处调整都应在此统一修改，避免两侧策略漂移。
//!
//! 所有装配函数都只绑定依赖，不建立连接、不查询数据库、不领取租约、不推进任何状态；
//! 真正的 I/O 只发生在发布轮次与消费循环中。缺少 MySQL 或 RabbitMQ 时在装配阶段即返回配置错误，
//! 从而把部署缺失暴露在启动或首次调用时，而不是在投递中途。

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
/// 交换机名与重试参数在此写死，是全局唯一的定义处，其余装配入口都经由本函数取得同一份配置。
/// 退避为固定 30 秒的等距间隔而非指数增长，最多 5 次后转入死信。
/// MySQL 或 RabbitMQ 任一缺失都返回内部错误，把部署缺失暴露在装配阶段而不是投递中途；
/// 重试参数非法同样在此失败，不会带着坏策略进入运行期。
/// `batch_size` 原样交给仓储，零值会得到空批次；构造阶段不查询数据库、不建立 channel，也不推进任何状态。
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
/// 消费者名同时界定去重范围与补偿扫描范围，改名等同于换一个消费者身份，历史去重记录随之失效，因此必须保持稳定。
/// 退避参数与 outbox 侧完全一致，两端由此保持同一条重试曲线。
/// 钱包初始化适配器与 inbox 仓储共用同一个连接池，但各自开事务，二者不共享事务边界。
/// 不需要 RabbitMQ，因为本服务只负责消费编排，传输适配由调用方另行提供。
/// 构造不领取租约、不执行任何业务处理；缺少 MySQL 或策略非法时在开始消费前就失败。
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

/// 查询 outbox 运维记录，调用方必须先通过管理员鉴权，本层不重复校验身份。
/// 查询串先经表现层归一：状态裁剪空白后空串降级为不筛选，条数夹到 1 至 100，偏移截断到 100000。
/// 归一结果转成持久化层筛选结构后查询，再把 SQLx 行映射成稳定的响应类型，使运维接口不受行结构变动影响。
/// 返回体固定为记录数组加总数两个字段；纯读取，不发布消息、不推进重试、不修改任何事件状态。
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

/// 查询 inbox 运维记录，与 outbox 查询共用同一份查询串结构和归一规则。
/// 实际可用的筛选维度只有状态一项，消费者名虽然出现在返回记录里但不参与过滤。
/// 返回项带错误摘要与失败次数，便于直接判断某条消息是偶发失败还是已进入死信。
/// 纯读取：不领取处理租约、不执行任何消费 handler、不改动消费状态。
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
/// 校验顺序为先查原因非空、再解析管理员编号，两者都在开启事务之前完成，非法请求不占用事务。
/// 状态变更与审计写入在同一事务提交，不会出现事件已重排却没有操作记录的情况。
/// 重排把失败次数清零并清空下次重试时间，等于给该事件重新发放一整轮重试预算。
/// 已重排记录再次调用会返回冲突，因此不会重复清零重试次数或追加第二条审计记录。
/// 本用例只改数据库状态，不直接向 broker 投递，实际发送由后续发布轮次完成。
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

/// 从应用状态取出 MySQL 连接池的克隆句柄，统一未配置时的错误语义。
/// 归为 `AppError::Internal` 而非校验错误，因为连接池缺失属于部署配置问题而非请求问题。
/// 克隆的是内部共享引用，不会新建物理连接。
fn events_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for event routes".to_owned())
    })
}

/// 把 outbox 持久化行映射为对外响应，隔离 SQLx 行类型使数据库结构变动不外溢到接口合同。
/// 列表查询与死信重排共用本转换，因此两个接口返回的记录形状完全一致。
/// 纯字段搬运，时间字段的毫秒序列化由响应类型上的属性负责，此处不做格式转换。
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

/// 把 inbox 持久化行映射为对外响应，同样隔离 SQLx 行类型。
/// 逐字段搬运且不做脱敏，因为该行本身不含消息载荷与处理令牌，前者体量大、后者是并发控制凭据，
/// 两者在查询阶段就未被选出。时间字段的毫秒序列化由响应类型上的属性负责。
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
