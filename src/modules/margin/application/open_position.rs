use super::product_config::validate_hourly_interest_rate;
use super::support::{
    decimal_matches_string, ensure_supported_user_margin_mode, is_duplicate_key_error,
    non_negative_amount, normalized_margin_mode, validate_positive_decimal,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_MARGIN,
        },
        events::EventBroadcastHub,
        margin::{
            infrastructure::{
                MarginOpenProductRule, cached_margin_entry_price,
                debit_margin_position_open_collateral, ensure_cross_margin_account,
                existing_position_for_idempotency_key,
                existing_position_for_idempotency_key_readonly, insert_margin_position,
                load_position_by_id, lock_active_open_product, set_margin_position_wallet_scope,
            },
            presentation::{
                MarginPositionResponse, OpenMarginPositionRequest, OpenMarginPositionResponse,
            },
            service::publish_margin_position_opened_event_if_needed,
        },
    },
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
/// 校验市价开仓语义和用户幂等键，以服务端新鲜行情作为唯一入场价后创建保证金仓位。
/// 事务先占用幂等键，再锁钱包扣抵押、写流水及返佣；同键同参重放不再次扣款，任一步失败整体回滚。
pub(crate) async fn open_margin_position(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    request: OpenMarginPositionRequest,
) -> AppResult<(MarginPositionResponse, bool)> {
    validate_market_open_order_semantics(&request)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    let requested_margin_mode = match request.margin_mode.as_deref() {
        Some(value) => Some(normalized_margin_mode(value)?),
        None => None,
    };
    validate_positive_decimal(&request.margin_amount, "margin amount")?;
    validate_positive_decimal(&request.leverage, "leverage")?;

    if let Some(existing) =
        existing_position_for_idempotency_key_readonly(pool, user_id, &idempotency_key).await?
    {
        ensure_existing_position_matches_request(
            &existing,
            request.product_id,
            &direction,
            requested_margin_mode.as_deref(),
            &request.margin_amount,
            &request.leverage,
        )?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match lock_active_open_product(&mut tx, request.product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_position_if_present(
                pool,
                user_id,
                request.product_id,
                &direction,
                requested_margin_mode.as_deref(),
                &request.margin_amount,
                &request.leverage,
                &idempotency_key,
            )
            .await?
            {
                return Ok((existing, false));
            }
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error),
    };
    let position_margin_mode =
        selected_open_margin_mode(&product, requested_margin_mode.as_deref())?;
    validate_product_margin(&request.margin_amount, &request.leverage, &product)?;
    let notional_amount = request.margin_amount.clone() * request.leverage.clone();
    let borrowed_amount = margin_borrowed_amount(&notional_amount, &request.margin_amount);
    let entry_price =
        cached_margin_entry_price(redis, product.pair_id, product.symbol.as_str()).await?;
    // 先写入仓位占用用户幂等键，再锁定钱包扣保证金，避免同 key 并发重复扣款。
    let position_id = match insert_margin_position(
        &mut tx,
        user_id,
        &product,
        &position_margin_mode,
        &direction,
        &request.margin_amount,
        &request.leverage,
        &notional_amount,
        &borrowed_amount,
        &entry_price,
        &idempotency_key,
    )
    .await
    {
        Ok(position_id) => position_id,
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_position(
                pool,
                user_id,
                request.product_id,
                &direction,
                requested_margin_mode.as_deref(),
                &request.margin_amount,
                &request.leverage,
                &idempotency_key,
            )
            .await
            .map(|position| (position, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    let wallet_scope = debit_margin_position_open_collateral(
        &mut tx,
        user_id,
        product.margin_asset,
        &request.margin_amount,
        position_id,
        &position_margin_mode,
    )
    .await?;
    set_margin_position_wallet_scope(&mut tx, position_id, &wallet_scope).await?;
    if position_margin_mode == "cross" {
        ensure_cross_margin_account(&mut tx, user_id, product.margin_asset).await?;
    }
    let commission_source_id = position_id.to_string();
    insert_agent_business_commission_in_tx(
        &mut tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_MARGIN,
            source_type: "margin_position",
            source_id: &commission_source_id,
            source_amount: &request.margin_amount,
            payout_asset_id: product.margin_asset,
        },
    )
    .await?;
    let position = load_position_by_id(&mut tx, position_id).await?;
    tx.commit().await?;
    Ok((position, true))
}

/// 调用保证金开仓事务，并仅在首次提交新仓位后发布用户私有开仓事件。
/// 同键重放返回原仓位且不再扣抵押或发事件；行情及事务失败不产生广播。
pub(crate) async fn open_margin_position_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    request: OpenMarginPositionRequest,
) -> AppResult<OpenMarginPositionResponse> {
    let (position, is_new_position) = open_margin_position(pool, redis, user_id, request).await?;
    let response = OpenMarginPositionResponse { position };
    publish_margin_position_opened_event_if_needed(
        hub,
        user_id,
        &response.position,
        is_new_position,
    );
    Ok(response)
}

#[allow(clippy::too_many_arguments)] // 幂等核对必须逐字段比对原始下单语义，保持参数显式。
async fn replay_existing_position(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    margin_mode: Option<&str>,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<MarginPositionResponse> {
    replay_existing_position_if_present(
        pool,
        user_id,
        product_id,
        direction,
        margin_mode,
        margin_amount,
        leverage,
        idempotency_key,
    )
    .await?
    .ok_or_else(|| AppError::Conflict("margin idempotency key is being committed".to_owned()))
}

#[allow(clippy::too_many_arguments)] // 幂等核对必须逐字段比对原始下单语义，保持参数显式。
async fn replay_existing_position_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    margin_mode: Option<&str>,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        existing_position_for_idempotency_key(&mut tx, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    ensure_existing_position_matches_request(
        &existing,
        product_id,
        direction,
        margin_mode,
        margin_amount,
        leverage,
    )?;
    tx.commit().await?;
    Ok(Some(existing))
}

fn ensure_existing_position_matches_request(
    existing: &MarginPositionResponse,
    product_id: u64,
    direction: &str,
    margin_mode: Option<&str>,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id
        || existing.direction != direction
        || margin_mode.is_some_and(|mode| existing.margin_mode != mode)
        || existing.margin_amount != *margin_amount
        || existing.leverage != *leverage
    {
        return Err(AppError::Conflict(
            "margin idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

fn validate_product_margin(
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    product: &MarginOpenProductRule,
) -> AppResult<()> {
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    if margin_amount < &product.min_margin {
        return Err(AppError::Validation(
            "margin amount is below product minimum".to_owned(),
        ));
    }
    if let Some(max_margin) = &product.max_margin
        && margin_amount > max_margin
    {
        return Err(AppError::Validation(
            "margin amount exceeds product maximum".to_owned(),
        ));
    }
    validate_open_product_leverage(leverage, product)?;
    validate_hourly_interest_rate(&product.hourly_interest_rate)?;
    Ok(())
}

fn validate_open_product_leverage(
    leverage: &BigDecimal,
    product: &MarginOpenProductRule,
) -> AppResult<()> {
    if !product
        .leverage_levels
        .0
        .iter()
        .any(|level| decimal_matches_string(leverage, level))
    {
        return Err(AppError::Validation(
            "margin leverage must match a configured product level".to_owned(),
        ));
    }
    Ok(())
}

fn selected_open_margin_mode(
    product: &MarginOpenProductRule,
    requested_mode: Option<&str>,
) -> AppResult<String> {
    let mode = match requested_mode {
        Some(value) => normalized_margin_mode(value)?,
        None => product.margin_mode.clone(),
    };
    if !product
        .margin_modes
        .0
        .iter()
        .any(|supported| supported == &mode)
    {
        return Err(AppError::Validation(
            "margin_mode is not supported by this margin product".to_owned(),
        ));
    }
    ensure_supported_user_margin_mode(&mode)?;
    Ok(mode)
}

fn margin_borrowed_amount(notional_amount: &BigDecimal, margin_amount: &BigDecimal) -> BigDecimal {
    non_negative_amount(&(notional_amount.clone() - margin_amount.clone()))
}

fn normalize_direction(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "long" => Ok("long".to_owned()),
        "short" => Ok("short".to_owned()),
        _ => Err(AppError::Validation(
            "margin direction must be long or short".to_owned(),
        )),
    }
}

fn normalize_idempotency_key(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "idempotency_key is required for margin positions".to_owned(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::Validation(
            "idempotency_key is too long for margin positions".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}
fn validate_market_open_order_semantics(request: &OpenMarginPositionRequest) -> AppResult<()> {
    if let Some(order_type) = request.order_type.as_deref()
        && !order_type.trim().eq_ignore_ascii_case("market")
    {
        return Err(AppError::Validation(
            "margin only supports market orders".to_owned(),
        ));
    }
    if request.price.is_some() || request.trigger_price.is_some() {
        return Err(AppError::Validation(
            "margin market orders must not include price or trigger_price".to_owned(),
        ));
    }
    Ok(())
}
