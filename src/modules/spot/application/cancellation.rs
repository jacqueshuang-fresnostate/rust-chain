//! 现货撤单应用用例：收口用户/管理员撤单、批量部分失败和提交后事件。

use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        infrastructure::{SqlxSpotOrderCancelRepository, list_user_cancellable_spot_order_ids},
        presentation::{
            AdminCancelSpotOrderRequest, CancelAllSpotOrdersQuery, SpotBatchActionFailure,
            SpotCancelAllResponse, SpotCancelResponse,
        },
        repository::{SpotAdminCancelCommand, SpotOrderCancelRepository, SpotUserCancelCommand},
        service::{
            publish_spot_cancel_private_event_by_order_if_needed,
            publish_spot_cancel_private_event_if_needed,
        },
    },
};
use sqlx::{MySql, Pool};

use super::queries::optional_query_string;

/// 取消认证用户自己的单笔现货订单；`user_id` 必须来自鉴权主体，非本人订单按未找到处理以避免越权泄露。
/// 仓储拥有事务：先锁订单，再计算剩余预留额并锁对应钱包，将剩余 frozen 解冻到 available、写镜像流水，最后更新订单状态后提交。
/// 已非可撤状态视为幂等成功且 `cancelled=false`，不会二次解冻；任一步失败回滚订单、余额和流水，本函数不发布事件。
pub(crate) async fn cancel_user_spot_order(
    pool: &Pool<MySql>,
    order_id: u64,
    user_id: u64,
) -> AppResult<SpotCancelResponse> {
    let repository = SqlxSpotOrderCancelRepository::new(pool.clone());
    let result = repository
        .cancel_user_order(SpotUserCancelCommand { order_id, user_id })
        .await?;
    Ok(SpotCancelResponse {
        order: result.order.into(),
        cancelled: result.cancelled,
    })
}

/// 执行用户单笔撤单并在事务成功后发布私有撤单事件；前置条件及锁序与 [`cancel_user_spot_order`] 相同。
/// 只有首次实际撤单才广播，因此重放不会重复解冻或重复事件；事件位于提交之后，不参与数据库事务。
/// 仓储失败时无资金或事件副作用，事件发送通道缺失按既有策略跳过，不改变已提交撤单结果。
pub(crate) async fn cancel_user_spot_order_with_events(
    pool: &Pool<MySql>,
    order_id: u64,
    user_id: u64,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotCancelResponse> {
    // 取消订单和事件发布作为一个用例返回，路由层只做参数透传。
    let response = cancel_user_spot_order(pool, order_id, user_id).await?;
    publish_spot_cancel_private_event_if_needed(hub, user_id, &response.order, response.cancelled);
    Ok(response)
}

/// 逐笔取消认证用户当前可撤订单，可选交易对只缩小本人订单集合；调用方必须提供可信 `user_id`。
/// 每笔复用独立的单撤事务和“订单后钱包”锁序，避免一个遗留坏单回滚已成功项；仅解冻各订单尚未消耗的预留额并同步写流水。
/// 已撤/已成交订单不会进入候选集，重放不二次解冻或发事件；单项失败记录到 `failures` 后继续，候选集查询失败则整体返回错误。
pub(crate) async fn cancel_all_user_spot_orders_with_events(
    pool: &Pool<MySql>,
    user_id: u64,
    query: CancelAllSpotOrdersQuery,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotCancelAllResponse> {
    let order_ids =
        list_user_cancellable_spot_order_ids(pool, user_id, optional_query_string(query.pair_id))
            .await?;
    let mut orders = Vec::with_capacity(order_ids.len());
    let mut failures = Vec::new();
    for order_id in order_ids {
        // 批量撤单逐笔复用单单事务，重试时已撤订单不会再次解冻或重复发事件。
        match cancel_user_spot_order_with_events(pool, order_id, user_id, hub).await {
            Ok(response) if response.cancelled => orders.push(response.order),
            Ok(_) => {}
            Err(error) => failures.push(spot_batch_action_failure(order_id, error)),
        }
    }
    Ok(SpotCancelAllResponse { orders, failures })
}

fn spot_batch_action_failure(id: u64, error: AppError) -> SpotBatchActionFailure {
    let code = match &error {
        AppError::Config(_) => "CONFIG_ERROR",
        AppError::Database(_) => "DATABASE_ERROR",
        AppError::Mongo(_) => "MONGO_ERROR",
        AppError::Redis(_) => "REDIS_ERROR",
        AppError::RabbitMq(_) => "RABBITMQ_ERROR",
        AppError::Unauthorized => "UNAUTHORIZED",
        AppError::Forbidden => "FORBIDDEN",
        AppError::Validation(_) => "VALIDATION_ERROR",
        AppError::NotFound => "NOT_FOUND",
        AppError::Conflict(_) => "CONFLICT",
        AppError::Internal(_) => "INTERNAL_ERROR",
        AppError::Api { code, .. } => *code,
    };
    SpotBatchActionFailure {
        id: id.to_string(),
        code,
        message: error.to_string(),
    }
}

/// 以管理员身份取消任意现货订单；`admin_id` 与非空原因必须由上层鉴权和请求校验提供。
/// 仓储事务先锁订单，再按订单所有者计算剩余预留、锁钱包并解冻，随后更新订单并在首次撤单时写管理员审计记录。
/// 非可撤订单幂等返回 `cancelled=false`，不重复解冻/记账/审计；任一步失败整体回滚，本函数本身不发布私有事件。
pub(crate) async fn cancel_admin_spot_order(
    pool: &Pool<MySql>,
    order_id: u64,
    admin_id: u64,
    reason: String,
) -> AppResult<SpotCancelResponse> {
    let repository = SqlxSpotOrderCancelRepository::new(pool.clone());
    let result = repository
        .cancel_admin_order(SpotAdminCancelCommand {
            order_id,
            admin_id,
            reason,
        })
        .await?;
    Ok(SpotCancelResponse {
        order: result.order.into(),
        cancelled: result.cancelled,
    })
}

/// 执行管理员撤单并在事务提交后向订单所有者发布私有事件；调用前须完成管理员鉴权和非空原因校验。
/// 订单锁、剩余预留解冻、钱包锁、流水与审计均由单一仓储事务承担；仅 `cancelled=true` 的首次状态迁移才发布事件。
/// 数据库失败不产生事件，幂等重放不重复资金或事件副作用；提交后事件失败沿用现有语义但不撤销已提交撤单。
pub(crate) async fn cancel_admin_spot_order_with_events(
    pool: &Pool<MySql>,
    order_id: u64,
    admin_id: u64,
    reason: String,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SpotCancelResponse> {
    // 管理员撤单需要 admin 审计上下文，事件发布与交易结果同一事务边界外执行。
    let response = cancel_admin_spot_order(pool, order_id, admin_id, reason).await?;
    publish_spot_cancel_private_event_by_order_if_needed(hub, &response.order, response.cancelled)?;
    Ok(response)
}
/// 标准化管理员撤单原因；调用方仍须独立完成管理员鉴权，空值或纯空白值返回既有校验错误。
/// 该纯校验不启动事务、不锁订单/钱包，不改变冻结额、流水或幂等状态，也不发布事件。
pub(crate) fn validate_admin_cancel_spot_order_request(
    request: AdminCancelSpotOrderRequest,
) -> AppResult<String> {
    request
        .reason
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("reason is required".to_owned()))
}
