//! margin bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 用户侧划转和交易设置在这里完成校验与事务编排。

use crate::modules::wallet::amount_fits_asset_precision;
use crate::{
    error::{AppError, AppResult},
    modules::agent::{
        infrastructure::insert_agent_business_commission_in_tx,
        repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_MARGIN,
    },
    modules::events::EventBroadcastHub,
    modules::margin::{
        infrastructure::{
            LockedMarginPositionRow, MarginOpenProductRule, MarginProductSettingRule,
            MarginProductUpsertValues, cached_margin_entry_price, cached_margin_mark_price,
            cached_margin_risk_ticker, credit_margin_position_amount,
            debit_margin_position_open_collateral, ensure_asset_exists, ensure_pair_exists,
            existing_position_for_idempotency_key, existing_position_for_idempotency_key_readonly,
            insert_admin_audit_log, insert_margin_position, insert_margin_product,
            insert_margin_transfer, list_admin_interest_summary, list_admin_margin_positions,
            list_margin_products, list_margin_wallet_accounts,
            list_user_margin_positions as list_user_margin_positions_rows,
            load_admin_margin_position_by_id, load_cancelable_position_ids,
            load_margin_transfer_by_idempotency_key, load_margin_transfer_wallet_snapshots,
            load_open_position_ids, load_position_by_id, load_product_by_id,
            load_user_margin_setting, load_user_margin_setting_from_pool, load_user_position_by_id,
            load_user_risk_position_by_id, lock_active_open_product,
            lock_active_product_setting_rule, lock_product_by_id, lock_user_position_by_id,
            mark_position_canceled, mark_position_closed, resolve_active_transfer_asset,
            resolve_transfer_asset_id_for_replay, set_margin_position_wallet_scope,
            transfer_margin_to_spot_wallets, transfer_spot_to_margin_wallets,
            update_margin_product,
            update_margin_product_status as update_margin_product_status_row,
            upsert_user_margin_setting,
        },
        presentation::{
            AdminInterestSummaryResponse, AdminMarginPositionResponse,
            AdminMarginPositionsResponse, CancelAllMarginPositionsResponse,
            CancelMarginPositionResponse, CloseAllMarginPositionsResponse,
            CloseMarginPositionResponse, CreateMarginProductRequest, MarginBatchActionFailure,
            MarginPositionDetailResponse, MarginPositionResponse, MarginPositionsResponse,
            MarginProductResponse, MarginProductsResponse, MarginRiskSnapshot,
            MarginRiskSnapshotResponse, MarginTradingCapabilitiesResponse,
            MarginUserSettingResponse, MarginWalletsResponse, OpenMarginPositionRequest,
            OpenMarginPositionResponse, TransferMarginFundsRequest, TransferMarginFundsResponse,
            UpdateMarginProductRequest, UpdateMarginProductStatusRequest,
            UpdateUserLeverageRequest, UpdateUserMarginModeRequest,
        },
        service::{
            margin_product_audit_json, publish_margin_position_canceled_event_if_needed,
            publish_margin_position_closed_event_if_needed,
            publish_margin_position_opened_event_if_needed,
        },
    },
    state::AppState,
    workers::margin_liquidation::margin_liquidation_risk_state,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};
use std::{collections::BTreeSet, str::FromStr};
use uuid::Uuid;

/// 统一从应用状态中获取数据库连接池。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for margin routes".to_owned())
    })
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

pub(crate) fn margin_position_payout_amount(
    margin_amount: &BigDecimal,
    realized_pnl: Option<&BigDecimal>,
    interest_amount: &BigDecimal,
) -> BigDecimal {
    realized_pnl
        .map(|pnl| non_negative_amount(&(margin_amount + pnl - interest_amount)))
        .unwrap_or_else(|| BigDecimal::from(0).with_scale(18))
}

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
    )
    .await?;
    set_margin_position_wallet_scope(&mut tx, position_id, &wallet_scope).await?;
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

pub(crate) async fn list_active_margin_products(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<MarginProductsResponse> {
    let products = list_margin_products(pool, Some("active"), limit).await?;
    Ok(MarginProductsResponse {
        products,
        capabilities: margin_trading_capabilities(),
    })
}

pub(crate) async fn list_admin_margin_products(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<MarginProductsResponse> {
    let products = list_margin_products(pool, None, limit).await?;
    Ok(MarginProductsResponse {
        products,
        capabilities: margin_trading_capabilities(),
    })
}

pub(crate) async fn get_admin_margin_product(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    let mut tx = pool.begin().await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(product)
}

pub(crate) async fn create_margin_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    request: CreateMarginProductRequest,
) -> AppResult<MarginProductResponse> {
    validate_create_product_request(&request)?;
    let reason = required_reason(request.reason)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let values = margin_product_upsert_values(
        request.pair_id,
        request.margin_asset,
        request.logo_url,
        request.margin_mode.as_deref(),
        request.margin_modes.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate,
        &status,
    )?;
    let pool = required_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    ensure_pair_exists(&mut tx, request.pair_id).await?;
    ensure_asset_exists(&mut tx, request.margin_asset).await?;
    // 产品配置和后台审计必须同事务提交，避免配置已生效但没有审计原因。
    let product_id = insert_margin_product(&mut tx, &values).await?;
    let product = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log(
        &mut tx,
        admin_id,
        "margin_product.create",
        product.id,
        None,
        Some(margin_product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(product)
}

pub(crate) async fn update_margin_product_config(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateMarginProductRequest,
) -> AppResult<MarginProductResponse> {
    validate_update_product_request(&request)?;
    let reason = required_reason(request.reason)?;
    let status = normalized_product_status(&request.status)?;
    let values = margin_product_upsert_values(
        request.pair_id,
        request.margin_asset,
        request.logo_url,
        request.margin_mode.as_deref(),
        request.margin_modes.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate,
        &status,
    )?;
    let pool = required_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    ensure_pair_exists(&mut tx, request.pair_id).await?;
    ensure_asset_exists(&mut tx, request.margin_asset).await?;
    update_margin_product(&mut tx, product_id, &values).await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log(
        &mut tx,
        admin_id,
        "margin_product.update",
        product_id,
        Some(margin_product_audit_json(&before)),
        Some(margin_product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_margin_product_status(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateMarginProductStatusRequest,
) -> AppResult<MarginProductResponse> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let pool = required_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_product_by_id(&mut tx, product_id).await?;
    update_margin_product_status_row(&mut tx, product_id, &status).await?;
    let after = load_product_by_id(&mut tx, product_id).await?;
    insert_admin_audit_log(
        &mut tx,
        admin_id,
        "margin_product.update_status",
        product_id,
        Some(margin_product_audit_json(&before)),
        Some(margin_product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_user_margin_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<String>,
    limit: u32,
) -> AppResult<MarginPositionsResponse> {
    let status = optional_string(status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let positions =
        list_user_margin_positions_rows(pool, user_id, status.as_deref(), limit).await?;
    Ok(MarginPositionsResponse { positions })
}

pub(crate) async fn list_user_margin_wallets(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<MarginWalletsResponse> {
    let wallets = list_margin_wallet_accounts(pool, user_id).await?;
    let positions = list_user_margin_positions_rows(pool, user_id, Some("opened"), limit).await?;
    Ok(MarginWalletsResponse { wallets, positions })
}

pub(crate) async fn get_user_margin_position(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<MarginPositionDetailResponse> {
    let position = load_user_position_by_id(pool, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(MarginPositionDetailResponse { position })
}

pub(crate) async fn list_admin_margin_position_history(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<String>,
    limit: u32,
) -> AppResult<AdminMarginPositionsResponse> {
    let status = optional_string(status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let positions =
        list_admin_margin_positions(pool, user_id, email, pair_id, status.as_deref(), limit)
            .await?;
    Ok(AdminMarginPositionsResponse { positions })
}

pub(crate) async fn get_admin_margin_position(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<AdminMarginPositionResponse> {
    load_admin_margin_position_by_id(pool, position_id)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn list_admin_margin_interest_summary(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<String>,
    limit: u32,
) -> AppResult<AdminInterestSummaryResponse> {
    let status = optional_string(status)
        .map(|status| normalized_position_status(&status))
        .transpose()?;
    let summaries =
        list_admin_interest_summary(pool, user_id, email, pair_id, status.as_deref(), limit)
            .await?;
    Ok(AdminInterestSummaryResponse { summaries })
}

pub(crate) async fn get_margin_position_risk_snapshot(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    position_id: u64,
) -> AppResult<MarginRiskSnapshotResponse> {
    let position = load_user_risk_position_by_id(pool, user_id, position_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if position.status != "opened" {
        return Err(AppError::Validation(
            "margin risk snapshot requires an opened position".to_owned(),
        ));
    }
    let Some(entry_price) = position.entry_price.clone() else {
        return Err(AppError::Validation(
            "margin entry price is required for risk snapshot".to_owned(),
        ));
    };
    let ticker = cached_margin_risk_ticker(redis, position.pair_id, &position.symbol).await?;
    let risk_state = margin_liquidation_risk_state(
        &position.direction,
        &position.margin_amount,
        &position.notional_amount,
        &position.interest_amount,
        &entry_price,
        &ticker.last_price,
        &position.maintenance_margin_rate,
    )?;
    Ok(MarginRiskSnapshotResponse {
        risk: MarginRiskSnapshot {
            position_id: position.id,
            pair_id: position.pair_id,
            symbol: position.symbol,
            margin_asset: position.margin_asset,
            direction: position.direction,
            margin_amount: position.margin_amount,
            notional_amount: position.notional_amount,
            interest_amount: position.interest_amount,
            entry_price,
            mark_price: ticker.last_price,
            maintenance_margin_rate: position.maintenance_margin_rate,
            realized_pnl: risk_state.realized_pnl,
            equity: risk_state.equity,
            maintenance_margin: risk_state.maintenance_margin,
            should_liquidate: risk_state.should_liquidate,
            observed_at: ticker.observed_at,
        },
    })
}

pub(crate) async fn transfer_margin_funds(
    pool: &Pool<MySql>,
    user_id: u64,
    request: TransferMarginFundsRequest,
) -> AppResult<TransferMarginFundsResponse> {
    let TransferMarginFundsRequest {
        asset_id,
        asset_symbol,
        from,
        to,
        amount,
        idempotency_key,
    } = request;
    validate_positive_decimal(&amount, "transfer amount")?;
    let from = normalized_margin_account(&from)?;
    let to = normalized_margin_account(&to)?;
    if from == to {
        return Err(AppError::Validation(
            "margin transfer source and target must be different".to_owned(),
        ));
    }
    let idempotency_key = normalize_transfer_idempotency_key(idempotency_key)?;
    if let Some(response) = replay_margin_transfer_if_present(
        pool,
        user_id,
        asset_id,
        asset_symbol.as_deref(),
        &from,
        &to,
        &amount,
        &idempotency_key,
    )
    .await?
    {
        return Ok(response);
    }
    let transfer_id = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;
    let asset = resolve_active_transfer_asset(&mut tx, asset_id, asset_symbol.as_deref()).await?;
    if !amount_fits_asset_precision(&amount, asset.precision_scale) {
        return Err(AppError::Validation(format!(
            "margin transfer amount supports at most {} decimal places for asset {}",
            asset.precision_scale, asset.id
        )));
    }
    // 先占用用户幂等键，再触碰两侧钱包；任一后续步骤失败时同事务整体回滚。
    match insert_margin_transfer(
        &mut tx,
        &transfer_id,
        user_id,
        asset.id,
        &from,
        &to,
        &amount,
        &idempotency_key,
    )
    .await
    {
        Ok(()) => {}
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            if let Some(response) = replay_margin_transfer_if_present(
                pool,
                user_id,
                asset_id,
                asset_symbol.as_deref(),
                &from,
                &to,
                &amount,
                &idempotency_key,
            )
            .await?
            {
                return Ok(response);
            }
            return Err(AppError::Database(error));
        }
        Err(error) => return Err(AppError::Database(error)),
    }
    // 现货账户和杠杆账户的余额变化、两边流水必须同事务提交，避免出现单边扣款或审计缺口。
    let (spot_wallet, margin_wallet) = match (from.as_str(), to.as_str()) {
        ("spot", "margin") => {
            transfer_spot_to_margin_wallets(&mut tx, user_id, asset.id, &amount, &transfer_id)
                .await?
        }
        ("margin", "spot") => {
            transfer_margin_to_spot_wallets(&mut tx, user_id, asset.id, &amount, &transfer_id)
                .await?
        }
        _ => {
            return Err(AppError::Validation(
                "margin transfer only supports spot and margin accounts".to_owned(),
            ));
        }
    };
    tx.commit().await?;
    Ok(TransferMarginFundsResponse {
        transfer_id,
        spot_wallet,
        margin_wallet,
    })
}

#[allow(clippy::too_many_arguments)]
async fn replay_margin_transfer_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    request_asset_id: Option<u64>,
    request_asset_symbol: Option<&str>,
    from: &str,
    to: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<TransferMarginFundsResponse>> {
    let Some(existing) =
        load_margin_transfer_by_idempotency_key(pool, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    let requested_asset_id =
        resolve_transfer_asset_id_for_replay(pool, request_asset_id, request_asset_symbol).await?;
    if existing.asset_id != requested_asset_id
        || existing.from_account != from
        || existing.to_account != to
        || existing.amount != *amount
    {
        return Err(AppError::Conflict(
            "margin transfer idempotency_key was already used with different parameters".to_owned(),
        ));
    }
    let mut tx = pool.begin().await?;
    let (spot_wallet, margin_wallet) = load_margin_transfer_wallet_snapshots(
        &mut tx,
        user_id,
        existing.asset_id,
        &existing.transfer_id,
    )
    .await?;
    tx.commit().await?;
    Ok(Some(TransferMarginFundsResponse {
        transfer_id: existing.transfer_id,
        spot_wallet,
        margin_wallet,
    }))
}

pub(crate) async fn update_user_leverage(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    request: UpdateUserLeverageRequest,
) -> AppResult<MarginUserSettingResponse> {
    validate_positive_decimal(&request.leverage, "leverage")?;
    let mut tx = pool.begin().await?;
    let product = lock_active_product_setting_rule(&mut tx, product_id).await?;
    validate_product_leverage(&request.leverage, &product)?;
    upsert_user_margin_setting(&mut tx, user_id, product_id, None, Some(&request.leverage)).await?;
    let setting = load_user_margin_setting(&mut tx, user_id, product_id).await?;
    tx.commit().await?;
    Ok(setting)
}

pub(crate) async fn get_user_margin_setting(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
) -> AppResult<MarginUserSettingResponse> {
    load_user_margin_setting_from_pool(pool, user_id, product_id).await
}

pub(crate) async fn update_user_margin_mode(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    request: UpdateUserMarginModeRequest,
) -> AppResult<MarginUserSettingResponse> {
    let mut tx = pool.begin().await?;
    let product = lock_active_product_setting_rule(&mut tx, product_id).await?;
    let mode = selected_margin_mode(&product, Some(&request.margin_mode))?;
    upsert_user_margin_setting(&mut tx, user_id, product_id, Some(&mode), None).await?;
    let setting = load_user_margin_setting(&mut tx, user_id, product_id).await?;
    tx.commit().await?;
    Ok(setting)
}

pub(crate) async fn close_margin_position(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    position_id: u64,
) -> AppResult<(MarginPositionResponse, bool)> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_user_position_by_id(&mut tx, user_id, position_id).await? else {
        return Err(AppError::NotFound);
    };
    if position.status != "opened" {
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((position, false));
    }
    let Some(entry_price) = position.entry_price.as_ref() else {
        return Err(AppError::Validation(
            "margin entry price is required to close position".to_owned(),
        ));
    };
    let mark_price = cached_margin_mark_price(redis, position.pair_id, &position.symbol).await?;
    let realized_pnl = margin_realized_pnl(
        &position.direction,
        &position.notional_amount,
        entry_price,
        &mark_price,
    )?;
    let payout_amount = margin_payout_amount(
        &position.margin_amount,
        &realized_pnl,
        &position.interest_amount,
    );
    // 平仓返还金额、流水和仓位状态必须同事务提交，避免用户收到余额但仓位仍显示 opened。
    credit_margin_position_amount(
        &mut tx,
        user_id,
        position.margin_asset,
        &position.wallet_scope,
        &payout_amount,
        "margin_position_close",
        position.id,
    )
    .await?;
    mark_position_closed(
        &mut tx,
        user_id,
        position.id,
        Utc::now(),
        &mark_price,
        &realized_pnl,
    )
    .await?;
    let position = load_position_by_id(&mut tx, position.id).await?;
    tx.commit().await?;
    Ok((position, true))
}

pub(crate) async fn close_margin_position_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position_id: u64,
) -> AppResult<CloseMarginPositionResponse> {
    let (position, is_new_close) = close_margin_position(pool, redis, user_id, position_id).await?;
    let response = CloseMarginPositionResponse { position };
    publish_margin_position_closed_event_if_needed(hub, user_id, &response.position, is_new_close);
    Ok(response)
}

pub(crate) async fn close_all_margin_positions_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<CloseAllMarginPositionsResponse> {
    let position_ids = load_open_position_ids(pool, user_id, product_id).await?;
    let mut positions = Vec::with_capacity(position_ids.len());
    let mut failures = Vec::new();
    for position_id in position_ids {
        // 每笔平仓独立提交后立刻发事件；后续失败不能吞掉前面已成功交易的通知。
        match close_margin_position_with_events(pool, redis, hub, user_id, position_id).await {
            Ok(response) => positions.push(response.position),
            Err(error) => failures.push(margin_batch_action_failure(position_id, error)),
        }
    }

    Ok(CloseAllMarginPositionsResponse {
        positions,
        failures,
    })
}

pub(crate) async fn cancel_margin_position(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<(MarginPositionResponse, bool)> {
    let mut tx = pool.begin().await?;
    let Some(position) = lock_user_position_by_id(&mut tx, user_id, position_id).await? else {
        return Err(AppError::NotFound);
    };
    if position.status == "canceled" {
        let position = load_position_by_id(&mut tx, position.id).await?;
        tx.commit().await?;
        return Ok((position, false));
    }
    validate_cancelable_position(&position)?;
    // 撤单只允许未成交仓位，保证金原路返还并与状态更新保持事务一致。
    credit_margin_position_amount(
        &mut tx,
        user_id,
        position.margin_asset,
        &position.wallet_scope,
        &position.margin_amount,
        "margin_position_cancel",
        position.id,
    )
    .await?;
    mark_position_canceled(&mut tx, user_id, position.id, Utc::now()).await?;
    let position = load_position_by_id(&mut tx, position.id).await?;
    tx.commit().await?;
    Ok((position, true))
}

pub(crate) async fn cancel_margin_position_with_events(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    position_id: u64,
) -> AppResult<CancelMarginPositionResponse> {
    let (position, is_new_cancel) = cancel_margin_position(pool, user_id, position_id).await?;
    let response = CancelMarginPositionResponse { position };
    publish_margin_position_canceled_event_if_needed(
        hub,
        user_id,
        &response.position,
        is_new_cancel,
    );
    Ok(response)
}

pub(crate) async fn cancel_all_margin_positions_with_events(
    pool: &Pool<MySql>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<CancelAllMarginPositionsResponse> {
    let position_ids = load_cancelable_position_ids(pool, user_id, product_id).await?;
    let mut positions = Vec::with_capacity(position_ids.len());
    let mut failures = Vec::new();
    for position_id in position_ids {
        // 撤单与事件同样逐笔收口，保留前序成功结果及其私有事件。
        match cancel_margin_position_with_events(pool, hub, user_id, position_id).await {
            Ok(response) => positions.push(response.position),
            Err(error) => failures.push(margin_batch_action_failure(position_id, error)),
        }
    }

    Ok(CancelAllMarginPositionsResponse {
        positions,
        failures,
    })
}

fn margin_batch_action_failure(id: u64, error: AppError) -> MarginBatchActionFailure {
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
    MarginBatchActionFailure {
        id,
        code,
        message: error.to_string(),
    }
}

fn validate_cancelable_position(position: &LockedMarginPositionRow) -> AppResult<()> {
    if position.status != "opened" {
        return Err(AppError::Validation(
            "only opened margin positions can be canceled".to_owned(),
        ));
    }
    if position.entry_price.is_some() {
        return Err(AppError::Validation(
            "filled margin positions cannot be canceled; close the position instead".to_owned(),
        ));
    }
    Ok(())
}

fn margin_realized_pnl(
    direction: &str,
    notional_amount: &BigDecimal,
    entry_price: &BigDecimal,
    mark_price: &BigDecimal,
) -> AppResult<BigDecimal> {
    validate_positive_decimal(entry_price, "entry price")?;
    validate_positive_decimal(mark_price, "mark price")?;
    let price_delta = match direction {
        "long" => mark_price.clone() - entry_price.clone(),
        "short" => entry_price.clone() - mark_price.clone(),
        _ => {
            return Err(AppError::Validation(
                "margin direction must be long or short".to_owned(),
            ));
        }
    };
    Ok((notional_amount.clone() * price_delta / entry_price.clone()).with_scale(18))
}

fn margin_payout_amount(
    margin_amount: &BigDecimal,
    realized_pnl: &BigDecimal,
    interest_amount: &BigDecimal,
) -> BigDecimal {
    non_negative_amount(&(margin_amount.clone() + realized_pnl.clone() - interest_amount.clone()))
}

fn non_negative_amount(amount: &BigDecimal) -> BigDecimal {
    if amount > &BigDecimal::from(0) {
        amount.clone().with_scale(18)
    } else {
        BigDecimal::from(0).with_scale(18)
    }
}

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

fn validate_create_product_request(request: &CreateMarginProductRequest) -> AppResult<()> {
    validate_product_fields(
        request.pair_id,
        request.margin_asset,
        request.margin_modes.as_deref(),
        request.margin_mode.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate.as_ref(),
        request.status.as_deref(),
        request.reason.as_deref(),
    )
}

fn validate_update_product_request(request: &UpdateMarginProductRequest) -> AppResult<()> {
    validate_product_fields(
        request.pair_id,
        request.margin_asset,
        request.margin_modes.as_deref(),
        request.margin_mode.as_deref(),
        request.leverage_levels.as_deref(),
        &request.max_leverage,
        &request.min_margin,
        request.max_margin.as_ref(),
        &request.maintenance_margin_rate,
        request.hourly_interest_rate.as_ref(),
        Some(request.status.as_str()),
        request.reason.as_deref(),
    )
}

fn margin_product_upsert_values<'a>(
    pair_id: u64,
    margin_asset: u64,
    logo_url: Option<String>,
    margin_mode: Option<&str>,
    margin_modes: Option<&[String]>,
    leverage_levels: Option<&[BigDecimal]>,
    max_leverage: &'a BigDecimal,
    min_margin: &'a BigDecimal,
    max_margin: Option<&'a BigDecimal>,
    maintenance_margin_rate: &'a BigDecimal,
    hourly_interest_rate: Option<BigDecimal>,
    status: &'a str,
) -> AppResult<MarginProductUpsertValues<'a>> {
    let margin_modes = validated_margin_modes(margin_modes, margin_mode)?;
    let margin_mode = margin_modes
        .first()
        .cloned()
        .unwrap_or_else(|| "isolated".to_owned());
    let leverage_levels = validated_leverage_levels(max_leverage, leverage_levels)?;
    Ok(MarginProductUpsertValues {
        pair_id,
        margin_asset,
        logo_url: optional_image_url(logo_url, "margin product logo_url")?,
        margin_mode,
        margin_modes,
        leverage_levels,
        max_leverage,
        min_margin,
        max_margin,
        maintenance_margin_rate,
        hourly_interest_rate: hourly_interest_rate.unwrap_or_else(zero_rate),
        status,
    })
}

fn validate_product_fields(
    pair_id: u64,
    margin_asset: u64,
    margin_modes: Option<&[String]>,
    margin_mode: Option<&str>,
    leverage_levels: Option<&[BigDecimal]>,
    max_leverage: &BigDecimal,
    min_margin: &BigDecimal,
    max_margin: Option<&BigDecimal>,
    maintenance_margin_rate: &BigDecimal,
    hourly_interest_rate: Option<&BigDecimal>,
    status: Option<&str>,
    reason: Option<&str>,
) -> AppResult<()> {
    validated_margin_modes(margin_modes, margin_mode)?;
    validated_leverage_levels(max_leverage, leverage_levels)?;
    if pair_id == 0 {
        return Err(AppError::Validation("pair_id is required".to_owned()));
    }
    if margin_asset == 0 {
        return Err(AppError::Validation("margin_asset is required".to_owned()));
    }
    validate_max_leverage(max_leverage)?;
    validate_margin_amount(min_margin)?;
    if let Some(max_margin) = max_margin {
        validate_margin_amount(max_margin)?;
        if max_margin < min_margin {
            return Err(AppError::Validation(
                "margin product max_margin must be greater than or equal to min_margin".to_owned(),
            ));
        }
    }
    validate_maintenance_margin_rate(maintenance_margin_rate)?;
    if let Some(hourly_interest_rate) = hourly_interest_rate {
        validate_hourly_interest_rate(hourly_interest_rate)?;
    }
    if let Some(status) = status {
        normalized_product_status(status)?;
    }
    validate_reason_len(reason)?;
    Ok(())
}

fn normalized_product_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin product status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "margin product status must be active or disabled".to_owned(),
        )),
    }
}

fn validated_margin_modes(
    margin_modes: Option<&[String]>,
    legacy_margin_mode: Option<&str>,
) -> AppResult<Vec<String>> {
    let raw_modes: Vec<String> = match margin_modes {
        Some(modes) => modes.to_vec(),
        None => vec![legacy_margin_mode.unwrap_or("isolated").to_owned()],
    };
    if raw_modes.is_empty() {
        return Err(AppError::Validation(
            "margin product margin_modes must not be empty".to_owned(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut modes = Vec::with_capacity(raw_modes.len());
    for raw_mode in raw_modes {
        let mode = normalized_margin_mode(&raw_mode)?;
        // 杠杆产品配置必须与实际风控能力同步，不能让后台配置出用户无法使用的 cross。
        ensure_supported_user_margin_mode(&mode)?;
        if !seen.insert(mode.clone()) {
            return Err(AppError::Validation(
                "margin product margin_modes must not contain duplicates".to_owned(),
            ));
        }
        modes.push(mode);
    }

    Ok(modes)
}

fn validated_leverage_levels(
    max_leverage: &BigDecimal,
    leverage_levels: Option<&[BigDecimal]>,
) -> AppResult<Vec<String>> {
    validate_max_leverage(max_leverage)?;
    let Some(levels) = leverage_levels else {
        return Ok(vec![decimal_config_string(max_leverage)]);
    };
    if levels.is_empty() {
        return Err(AppError::Validation(
            "margin product leverage_levels must not be empty".to_owned(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(levels.len());
    for level in levels {
        validate_max_leverage(level)?;
        let level_text = decimal_config_string(level);
        if !seen.insert(level_text.clone()) {
            return Err(AppError::Validation(
                "margin product leverage_levels must not contain duplicates".to_owned(),
            ));
        }
        normalized.push(level_text);
    }

    let max_level = levels
        .iter()
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        .ok_or_else(|| {
            AppError::Validation("margin product leverage_levels must not be empty".to_owned())
        })?;
    if max_level != max_leverage {
        return Err(AppError::Validation(
            "margin product max_leverage must match maximum leverage level".to_owned(),
        ));
    }

    Ok(normalized)
}

fn decimal_config_string(value: &BigDecimal) -> String {
    let normalized = value.normalized().to_string();
    normalized
        .strip_suffix(".0")
        .unwrap_or(&normalized)
        .to_owned()
}

fn required_reason(reason: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(reason) else {
        return Err(AppError::Validation(
            "margin product reason is required".to_owned(),
        ));
    };
    validate_reason_len(Some(reason.as_str()))?;
    Ok(reason)
}

fn validate_reason_len(reason: Option<&str>) -> AppResult<()> {
    if let Some(reason) = reason
        && reason.trim().chars().count() > MARGIN_AUDIT_REASON_MAX_LEN
    {
        return Err(AppError::Validation(
            "margin product reason is too long".to_owned(),
        ));
    }
    Ok(())
}

fn validate_max_leverage(leverage: &BigDecimal) -> AppResult<()> {
    if leverage <= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "margin product max_leverage must be greater than 1".to_owned(),
        ));
    }
    validate_decimal_storage(
        leverage,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product max_leverage",
    )
}

fn validate_maintenance_margin_rate(rate: &BigDecimal) -> AppResult<()> {
    if rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product maintenance_margin_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        rate,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product maintenance_margin_rate",
    )
}

fn validate_margin_amount(amount: &BigDecimal) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product margin amount must be positive".to_owned(),
        ));
    }
    validate_decimal_storage(
        amount,
        MARGIN_AMOUNT_MAX_SCALE,
        MARGIN_AMOUNT_MAX_INTEGER_DIGITS,
        "margin product margin amount",
    )
}

fn zero_rate() -> BigDecimal {
    BigDecimal::from(0).with_scale(8)
}

fn optional_image_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    if url.chars().count() > 2048 {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(url))
}

fn required_mysql_pool(pool: Option<&Pool<MySql>>) -> AppResult<&Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for margin routes".to_owned())
    })
}

fn validate_hourly_interest_rate(rate: &BigDecimal) -> AppResult<()> {
    if rate < &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "margin product hourly_interest_rate must be non-negative".to_owned(),
        ));
    }
    validate_decimal_storage(
        rate,
        MARGIN_RATE_MAX_SCALE,
        MARGIN_RATE_MAX_INTEGER_DIGITS,
        "margin product hourly_interest_rate",
    )
}

fn validate_decimal_storage(
    value: &BigDecimal,
    max_scale: i64,
    max_integer_digits: usize,
    label: &str,
) -> AppResult<()> {
    let (digits, scale) = value.as_bigint_and_exponent();
    if scale > max_scale {
        return Err(AppError::Validation(format!(
            "{label} supports at most {max_scale} decimal places"
        )));
    }

    let significant_digits = digits
        .to_str_radix(10)
        .trim_start_matches('-')
        .trim_start_matches('0')
        .len();
    let integer_digits = if scale >= 0 {
        significant_digits.saturating_sub(scale as usize)
    } else {
        significant_digits.saturating_add(scale.unsigned_abs() as usize)
    };
    if integer_digits > max_integer_digits {
        return Err(AppError::Validation(format!(
            "{label} exceeds decimal storage precision"
        )));
    }
    Ok(())
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("1062")
            || database_error.code().as_deref() == Some("23000")
    })
}

const MARGIN_RATE_MAX_SCALE: i64 = 8;
const MARGIN_RATE_MAX_INTEGER_DIGITS: usize = 10;
const MARGIN_AUDIT_REASON_MAX_LEN: usize = 512;
const MARGIN_AMOUNT_MAX_SCALE: i64 = 18;
const MARGIN_AMOUNT_MAX_INTEGER_DIGITS: usize = 20;

fn validate_product_leverage(
    leverage: &BigDecimal,
    product: &MarginProductSettingRule,
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

fn selected_margin_mode(
    product: &MarginProductSettingRule,
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

fn ensure_supported_user_margin_mode(mode: &str) -> AppResult<()> {
    // 当前保证金、盈亏和强平仍按单仓结算；没有账户级风险池时不能把 cross 伪装成已支持。
    if mode == "cross" {
        return Err(AppError::Validation(
            "cross margin mode is unavailable until account-level risk management is implemented"
                .to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn margin_trading_capabilities() -> MarginTradingCapabilitiesResponse {
    // 订单、钱包和清算均按逐仓市价仓位实现，客户端必须以此能力集渲染交互。
    MarginTradingCapabilitiesResponse {
        order_types: vec!["market".to_owned()],
        margin_modes: vec!["isolated".to_owned()],
    }
}

fn validate_market_open_order_semantics(request: &OpenMarginPositionRequest) -> AppResult<()> {
    if let Some(order_type) = request.order_type.as_deref() {
        if !order_type.trim().eq_ignore_ascii_case("market") {
            return Err(AppError::Validation(
                "margin only supports market orders".to_owned(),
            ));
        }
    }
    if request.price.is_some() || request.trigger_price.is_some() {
        return Err(AppError::Validation(
            "margin market orders must not include price or trigger_price".to_owned(),
        ));
    }
    Ok(())
}

fn normalized_margin_account(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "spot" => Ok("spot".to_owned()),
        "swap" | "margin" => Ok("margin".to_owned()),
        _ => Err(AppError::Validation(
            "margin transfer account must be spot or margin".to_owned(),
        )),
    }
}

fn normalize_transfer_idempotency_key(value: Option<String>) -> AppResult<String> {
    let Some(value) = value else {
        return Ok(Uuid::now_v7().to_string());
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(
            "margin transfer idempotency_key must not be empty".to_owned(),
        ));
    }
    if value.chars().count() > 128 {
        return Err(AppError::Validation(
            "margin transfer idempotency_key must not exceed 128 characters".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_positive_decimal(amount: &BigDecimal, label: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "margin {label} must be positive"
        )));
    }
    Ok(())
}

fn normalized_margin_mode(value: &str) -> AppResult<String> {
    let Some(mode) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin product margin_mode is required".to_owned(),
        ));
    };
    match mode.as_str() {
        "isolated" | "cross" => Ok(mode),
        _ => Err(AppError::Validation(
            "margin product margin_mode must be isolated or cross".to_owned(),
        )),
    }
}

fn normalized_position_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "margin position status is required".to_owned(),
        ));
    };
    match status.as_str() {
        "opened" | "closed" | "liquidated" | "canceled" => Ok(status),
        _ => Err(AppError::Validation(
            "margin position status must be opened, closed, canceled, or liquidated".to_owned(),
        )),
    }
}

fn decimal_matches_string(value: &BigDecimal, expected: &str) -> bool {
    BigDecimal::from_str(expected)
        .map(|level| &level == value)
        .unwrap_or(false)
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
