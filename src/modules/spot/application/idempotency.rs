//! 现货建单幂等应用规则：按用户与完整订单参数安全重放既有订单。

use crate::{
    error::AppResult,
    modules::spot::{
        OrderType,
        infrastructure::load_spot_order_by_idempotency_key,
        presentation::{CreateSpotOrderRequest, SpotOrderResponse},
        service::{
            SpotOrderIdempotencyCheck, ensure_spot_order_idempotency_matches,
            normalize_idempotency_key,
        },
        spot_reservation_amount,
    },
};
use sqlx::{MySql, Pool};

/// 在建单前按幂等键查找可安全重放的现货订单；键缺失或空白时返回 `None` 并允许继续创建。
/// 命中记录必须属于同一认证用户，且交易对、方向、类型、价格/触发价、数量、请求参考价与预期预留额完全兼容，否则返回冲突。
/// 该路径只读且不拥有事务或锁，不重新读取 Redis 执行价、不冻结钱包、不追加流水或事件；并发唯一键竞态仍由插入事务兜底。
pub(crate) async fn replay_spot_order_for_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    request: &CreateSpotOrderRequest,
) -> AppResult<Option<SpotOrderResponse>> {
    let Some(idempotency_key) = normalize_idempotency_key(request.idempotency_key.as_deref())
    else {
        return Ok(None);
    };
    let existing = load_spot_order_by_idempotency_key(pool, idempotency_key).await?;

    match existing {
        Some(order) if order.user_id == user_id => {
            let expected = spot_order_idempotency_check_for_request(request);
            ensure_spot_order_idempotency_matches(&order, &expected)?;
            Ok(Some(order.into()))
        }
        Some(_) => Err(crate::error::AppError::Conflict(
            "spot order idempotency key belongs to another user".to_owned(),
        )),
        None => Ok(None),
    }
}
fn spot_order_idempotency_check_for_request(
    request: &CreateSpotOrderRequest,
) -> SpotOrderIdempotencyCheck {
    let expected_reservation_price = match request.order_type {
        OrderType::Limit => request.price.as_ref(),
        OrderType::Market => request.reference_price.as_ref(),
        OrderType::StopLimit => request.price.as_ref(),
    };
    SpotOrderIdempotencyCheck {
        pair_id: request.pair_id.clone(),
        side: request.side,
        order_type: request.order_type,
        price: match request.order_type {
            OrderType::Limit | OrderType::StopLimit => request.price.clone(),
            OrderType::Market => None,
        },
        trigger_price: request.trigger_price.clone(),
        quantity: request.quantity.clone(),
        reserved_amount: expected_reservation_price
            .map(|price| spot_reservation_amount(request.side, price, &request.quantity)),
        request_reference_price: match request.order_type {
            OrderType::Limit | OrderType::StopLimit => None,
            OrderType::Market => request.reference_price.clone(),
        },
        request_price: request.price.clone(),
    }
}
