//! 现货建单幂等应用规则：按用户与完整订单参数安全重放既有订单。

use crate::{
    error::AppResult,
    modules::spot::{
        OrderType,
        infrastructure::load_spot_order_by_idempotency_key,
        presentation::{CreateSpotOrderRequest, SpotOrderResponse},
        service::{
            SpotOrderIdempotencyCheck, ensure_spot_order_request_fingerprint_matches,
            spot_order_idempotency_response,
        },
        spot_reservation_amount,
    },
};
use sqlx::{MySql, Pool};

/// 在建单前按用户与幂等键查找可安全重放的现货订单。
/// 新记录直接核对稳定指纹，历史空指纹记录才逐字段比对；命中后返回首次响应快照。
/// 该路径只读且不拥有事务或锁，不重新读取 Redis 执行价、不冻结钱包、不追加流水或事件；并发唯一键竞态仍由插入事务兜底。
pub(crate) async fn replay_spot_order_for_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    request: &CreateSpotOrderRequest,
    request_fingerprint: &str,
) -> AppResult<Option<SpotOrderResponse>> {
    let Some(order) =
        load_spot_order_by_idempotency_key(pool, user_id, &request.idempotency_key).await?
    else {
        return Ok(None);
    };
    let expected = spot_order_idempotency_check_for_request(request);
    ensure_spot_order_request_fingerprint_matches(&order, request_fingerprint, &expected)?;
    Ok(Some(spot_order_idempotency_response(order)?))
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
