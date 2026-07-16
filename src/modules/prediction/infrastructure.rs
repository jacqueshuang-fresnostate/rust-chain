//! prediction 有限上下文的基础设施层。
//!
//! 负责外部依赖交互、数据库持久化查询、HTTP 调用和订单/市场结算数据组装。

use super::{
    presentation::{
        CreatePredictionOrderRequest, CreatePredictionQuoteRequest, PredictionAssetConfigResponse,
        PredictionMarketResponse, PredictionOrderResponse, PredictionQuoteResponse,
        PredictionSyncResponse,
    },
    repository::{
        PredictionAssetConfigRow, PredictionAssetMetaRow, PredictionOrderSettlementRow,
        PredictionQuoteLockRow, PredictionSettingsRow, PredictionStakeAssetRow,
        PredictionSyncLogRow, PredictionWalletRow,
    },
    service,
};
use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_PREDICTION,
        },
        wallet::truncate_amount_to_asset_precision,
    },
    state::AppState,
};
use axum::http::StatusCode;
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use reqwest::Url;
use serde_json::{Value, json};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};
use std::{collections::HashSet, time::Duration};
use uuid::Uuid;

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

// 预测模块通用 SQL 片段，供管理端资产配置列表复用。
pub(crate) const ADMIN_ASSET_CONFIGS_SQL: &str = r#"SELECT assets.id AS asset_id, assets.symbol AS asset_symbol,
                  COALESCE(configs.enabled, FALSE) AS enabled,
                  COALESCE(configs.max_payout_amount, 0) AS max_payout_amount,
                  COALESCE(configs.created_at, assets.created_at) AS created_at,
                  COALESCE(configs.updated_at, assets.created_at) AS updated_at
           FROM assets
           LEFT JOIN prediction_asset_configs configs ON configs.asset_id = assets.id
           WHERE assets.status = 'active'
           ORDER BY assets.symbol ASC"#;

type SyncCounts = service::SyncCounts;
type EffectiveMarketConfig = service::EffectiveMarketConfig;

pub(crate) async fn create_quote_in_db(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreatePredictionQuoteRequest,
) -> AppResult<PredictionQuoteResponse> {
    let outcome = service::normalize_binary_outcome(&request.outcome)?;
    service::ensure_positive_amount(&request.stake_amount, "stake_amount")?;
    let settings = load_settings(pool).await?;
    let market = load_market_response(pool, request.market_id).await?;
    if market.display_status != service::STATUS_ACTIVE
        || market.settlement_status != service::SETTLEMENT_OPEN
    {
        return Err(AppError::Validation(
            "prediction market is not open for quotes".to_owned(),
        ));
    }
    let asset = load_active_asset(pool, request.asset_id).await?;
    service::ensure_amount_precision(&request.stake_amount, asset.precision_scale, "stake_amount")?;
    let effective = effective_market_config(&settings, &market);
    if !effective.allowed_asset_ids.contains(&request.asset_id) {
        return Err(AppError::Validation(
            "asset is not allowed for this prediction market".to_owned(),
        ));
    }
    ensure_prediction_asset_enabled(pool, request.asset_id).await?;

    let accepted_price = if outcome == service::OUTCOME_YES {
        market.yes_price.clone()
    } else {
        market.no_price.clone()
    };
    service::ensure_probability_price(&accepted_price)?;
    let raw_shares = request.stake_amount.clone() / accepted_price.clone();
    let shares = truncate_amount_to_asset_precision(&raw_shares, asset.precision_scale);
    let theoretical_payout = shares.clone();
    let fee_amount = truncate_amount_to_asset_precision(
        &(request.stake_amount.clone() * effective.fee_rate.clone()),
        asset.precision_scale,
    );
    let effective_payout_cap =
        effective_payout_cap(pool, request.asset_id, &effective.payout_cap_overrides).await?;
    if effective_payout_cap > BigDecimal::from(0) && theoretical_payout > effective_payout_cap {
        return Err(AppError::Validation(
            "prediction quote exceeds configured payout cap".to_owned(),
        ));
    }
    let quote_id = format!("pq_{}", Uuid::now_v7().simple());
    let ttl_seconds = i64::from(settings.quote_ttl_seconds.max(1));
    let expires_at = Utc::now()
        .checked_add_signed(TimeDelta::seconds(ttl_seconds))
        .ok_or_else(|| AppError::Validation("quote expiry is outside valid range".to_owned()))?;

    sqlx::query(
        r#"INSERT INTO prediction_quotes
           (quote_id, user_id, market_id, outcome, asset_id, stake_amount, fee_amount,
            accepted_price, shares, theoretical_payout, effective_payout_cap, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&quote_id)
    .bind(user_id)
    .bind(request.market_id)
    .bind(&outcome)
    .bind(request.asset_id)
    .bind(&request.stake_amount)
    .bind(&fee_amount)
    .bind(&accepted_price)
    .bind(&shares)
    .bind(&theoretical_payout)
    .bind(&effective_payout_cap)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(PredictionQuoteResponse {
        quote_id,
        market_id: request.market_id,
        outcome,
        asset_id: request.asset_id,
        asset_symbol: asset.symbol,
        stake_amount: request.stake_amount,
        fee_amount,
        accepted_price,
        shares,
        theoretical_payout,
        effective_payout_cap,
        expires_at,
    })
}

pub(crate) async fn save_admin_settings(
    pool: &Pool<MySql>,
    sync_enabled: bool,
    sync_interval_seconds: u32,
    sync_tags: &[String],
    allowed_asset_ids: &[u64],
    default_fee_rate: BigDecimal,
    default_settlement_mode: String,
    default_invalid_refund_policy: String,
    quote_ttl_seconds: u32,
) -> AppResult<()> {
    // 更新预测设置为后台配置页面提供的全局参数。
    sqlx::query(
        r#"UPDATE prediction_settings
           SET sync_enabled = ?, sync_interval_seconds = ?, sync_tags_json = ?,
               allowed_asset_ids_json = ?, default_fee_rate = ?,
               default_settlement_mode = ?, default_invalid_refund_policy = ?,
               quote_ttl_seconds = ?
           WHERE id = 1"#,
    )
    .bind(sync_enabled)
    .bind(sync_interval_seconds)
    .bind(SqlxJson(json!(sync_tags)))
    .bind(SqlxJson(json!(allowed_asset_ids)))
    .bind(default_fee_rate)
    .bind(default_settlement_mode)
    .bind(default_invalid_refund_policy)
    .bind(quote_ttl_seconds)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn list_admin_asset_configs(
    pool: &Pool<MySql>,
) -> AppResult<Vec<PredictionAssetConfigRow>> {
    // 列出所有激活资产的预测配置，缺失配置的资产会回退到资产创建时间作为时间字段。
    let rows = sqlx::query_as::<_, PredictionAssetConfigRow>(ADMIN_ASSET_CONFIGS_SQL)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub(crate) async fn list_stake_assets(
    pool: &Pool<MySql>,
) -> AppResult<Vec<PredictionStakeAssetRow>> {
    // 列出可下注资产，用于前端用户配置下拉与展示。
    let rows = sqlx::query_as::<_, PredictionStakeAssetRow>(
        r#"SELECT configs.asset_id, assets.symbol AS asset_symbol, configs.max_payout_amount
           FROM prediction_asset_configs configs
           INNER JOIN assets ON assets.id = configs.asset_id
           WHERE configs.enabled = TRUE AND assets.status = 'active'
           ORDER BY assets.symbol ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub(crate) async fn create_order_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreatePredictionOrderRequest,
) -> AppResult<(PredictionOrderResponse, bool)> {
    let quote_id = service::required_text(request.quote_id, "quote_id", 64)?;
    let idempotency_key = service::required_text(request.idempotency_key, "idempotency_key", 128)?;
    if let Some(existing) = load_order_by_idempotency(pool, user_id, &idempotency_key).await? {
        if existing.status.is_empty() {
            return Err(AppError::Conflict(
                "prediction order idempotency key is invalid".to_owned(),
            ));
        }
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let quote = lock_quote(&mut tx, &quote_id).await?;
    if quote.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if quote.consumed_at.is_some() {
        return Err(AppError::Conflict(
            "prediction quote was already used".to_owned(),
        ));
    }
    if quote.expires_at <= Utc::now() {
        return Err(AppError::Validation("prediction quote expired".to_owned()));
    }
    let market = lock_market(&mut tx, quote.market_id).await?;
    if market.display_status != service::STATUS_ACTIVE
        || market.settlement_status != service::SETTLEMENT_OPEN
    {
        return Err(AppError::Validation(
            "prediction market is not open for orders".to_owned(),
        ));
    }
    let asset = load_active_asset_in_tx(&mut tx, quote.asset_id).await?;
    service::ensure_amount_precision(&quote.stake_amount, asset.precision_scale, "stake_amount")?;
    service::ensure_amount_precision(&quote.fee_amount, asset.precision_scale, "fee_amount")?;

    let insert = sqlx::query(
        r#"INSERT INTO prediction_orders
           (user_id, market_id, quote_id, idempotency_key, outcome, asset_id,
            stake_amount, fee_amount, accepted_price, shares, theoretical_payout,
            effective_payout_cap, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open')"#,
    )
    .bind(user_id)
    .bind(quote.market_id)
    .bind(&quote.quote_id)
    .bind(&idempotency_key)
    .bind(&quote.outcome)
    .bind(quote.asset_id)
    .bind(&quote.stake_amount)
    .bind(&quote.fee_amount)
    .bind(&quote.accepted_price)
    .bind(&quote.shares)
    .bind(&quote.theoretical_payout)
    .bind(&quote.effective_payout_cap)
    .execute(&mut *tx)
    .await;

    let order_id = match insert {
        Ok(result) => result.last_insert_id(),
        Err(error) if service::is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            let order = load_order_by_idempotency(pool, user_id, &idempotency_key)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict("prediction idempotency key is being committed".to_owned())
                })?;
            return Ok((order, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };
    let order_no = service::prediction_order_no(order_id);
    sqlx::query("UPDATE prediction_orders SET order_no = ? WHERE id = ?")
        .bind(&order_no)
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE prediction_quotes SET consumed_at = CURRENT_TIMESTAMP(6) WHERE quote_id = ?",
    )
    .bind(&quote.quote_id)
    .execute(&mut *tx)
    .await?;
    apply_wallet_prediction_open(
        &mut tx,
        user_id,
        quote.asset_id,
        &quote.stake_amount,
        &quote.fee_amount,
        order_id,
    )
    .await?;
    let commission_source_id = order_id.to_string();
    insert_agent_business_commission_in_tx(
        &mut tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_PREDICTION,
            source_type: "prediction_order",
            source_id: &commission_source_id,
            source_amount: &quote.stake_amount,
            payout_asset_id: quote.asset_id,
        },
    )
    .await?;
    tx.commit().await?;
    Ok((load_order_response(pool, order_id).await?, true))
}

pub(crate) async fn settle_market_in_tx(
    pool: &Pool<MySql>,
    market_id: u64,
    result: String,
    requested_refund_policy: Option<String>,
) -> AppResult<(PredictionMarketResponse, u32, bool)> {
    let mut tx = pool.begin().await?;
    let market = lock_market(&mut tx, market_id).await?;
    if market.settlement_status == service::SETTLEMENT_SETTLED
        || market.settlement_status == service::SETTLEMENT_REFUNDED
    {
        tx.commit().await?;
        return Ok((load_market_response(pool, market_id).await?, 0, false));
    }
    let settings = load_settings_in_tx(&mut tx).await?;
    let refund_policy = if result == service::OUTCOME_INVALID {
        match requested_refund_policy {
            Some(policy) => policy,
            None => settings.default_invalid_refund_policy.clone(),
        }
    } else {
        settings.default_invalid_refund_policy.clone()
    };
    if result == service::OUTCOME_INVALID && refund_policy == service::REFUND_MANUAL {
        return Err(AppError::Validation(
            "manual invalid refund policy requires an explicit concrete refund policy".to_owned(),
        ));
    }
    let orders = sqlx::query_as::<_, PredictionOrderSettlementRow>(
        r#"SELECT id, user_id, asset_id, outcome, stake_amount, fee_amount,
                  theoretical_payout, effective_payout_cap, status
           FROM prediction_orders
           WHERE market_id = ? AND status = 'open'
           ORDER BY id ASC
           FOR UPDATE"#,
    )
    .bind(market_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut settled_orders = 0u32;
    for order in orders {
        if order.status != service::ORDER_STATUS_OPEN {
            continue;
        }
        if result == service::OUTCOME_INVALID {
            let fee_refund_amount = if refund_policy == service::REFUND_STAKE_AND_FEE {
                order.fee_amount.clone()
            } else {
                BigDecimal::from(0)
            };
            apply_wallet_prediction_refund(
                &mut tx,
                order.user_id,
                order.asset_id,
                &order.stake_amount,
                &fee_refund_amount,
                order.id,
            )
            .await?;
            sqlx::query(
                r#"UPDATE prediction_orders
                   SET status = 'refunded', result = ?, refund_amount = ?,
                       fee_refund_amount = ?, invalid_refund_policy_used = ?,
                       settled_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
            )
            .bind(&result)
            .bind(&order.stake_amount)
            .bind(&fee_refund_amount)
            .bind(&refund_policy)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        } else {
            let payout_amount = if order.outcome == result {
                service::capped_payout(&order.theoretical_payout, &order.effective_payout_cap)
            } else {
                BigDecimal::from(0)
            };
            apply_wallet_prediction_settlement(
                &mut tx,
                order.user_id,
                order.asset_id,
                &order.stake_amount,
                &payout_amount,
                order.id,
                order.outcome == result,
            )
            .await?;
            sqlx::query(
                r#"UPDATE prediction_orders
                   SET status = 'settled', result = ?, payout_amount = ?,
                       settled_at = CURRENT_TIMESTAMP(6)
                   WHERE id = ?"#,
            )
            .bind(&result)
            .bind(&payout_amount)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        }
        settled_orders += 1;
    }

    let settlement_status = if result == service::OUTCOME_INVALID {
        service::SETTLEMENT_REFUNDED
    } else {
        service::SETTLEMENT_SETTLED
    };
    let invalid_policy_used = if result == service::OUTCOME_INVALID {
        Some(refund_policy.clone())
    } else {
        None
    };
    sqlx::query(
        r#"UPDATE prediction_markets
           SET local_resolution = ?, settlement_status = ?,
               invalid_refund_policy_used = ?
           WHERE id = ?"#,
    )
    .bind(&result)
    .bind(settlement_status)
    .bind(invalid_policy_used)
    .bind(market_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        load_market_response(pool, market_id).await?,
        settled_orders,
        true,
    ))
}

pub(crate) async fn sync_polymarket_markets(
    pool: &Pool<MySql>,
    trigger_type: &str,
) -> AppResult<PredictionSyncResponse> {
    let started_at = Utc::now();
    let log_id = sqlx::query(
        r#"INSERT INTO prediction_sync_logs (trigger_type, status, started_at)
           VALUES (?, 'running', ?)"#,
    )
    .bind(trigger_type)
    .bind(started_at)
    .execute(pool)
    .await?
    .last_insert_id();
    sqlx::query(
        r#"UPDATE prediction_settings
           SET last_sync_status = 'running',
               last_sync_error = NULL,
               last_sync_started_at = ?
           WHERE id = 1"#,
    )
    .bind(started_at)
    .execute(pool)
    .await?;

    let result = sync_polymarket_markets_inner(pool).await;
    let finished_at = Utc::now();
    match result {
        Ok(counts) => {
            sqlx::query(
                r#"UPDATE prediction_sync_logs
                   SET status = 'success', imported_count = ?, updated_count = ?,
                       finished_at = ?
                   WHERE id = ?"#,
            )
            .bind(counts.imported_count)
            .bind(counts.updated_count)
            .bind(finished_at)
            .bind(log_id)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"UPDATE prediction_settings
                   SET last_sync_status = 'success', last_sync_error = NULL,
                       last_sync_finished_at = ?, last_successful_sync_at = ?,
                       last_sync_imported_count = ?, last_sync_updated_count = ?
                   WHERE id = 1"#,
            )
            .bind(finished_at)
            .bind(finished_at)
            .bind(counts.imported_count)
            .bind(counts.updated_count)
            .execute(pool)
            .await?;
            Ok(PredictionSyncResponse {
                imported_count: counts.imported_count,
                updated_count: counts.updated_count,
                status: "success".to_owned(),
                error_message: None,
            })
        }
        Err(error) => {
            let message = service::compact_error_message(&error.to_string());
            sqlx::query(
                r#"UPDATE prediction_sync_logs
                   SET status = 'failed', error_message = ?, finished_at = ?
                   WHERE id = ?"#,
            )
            .bind(&message)
            .bind(finished_at)
            .bind(log_id)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"UPDATE prediction_settings
                   SET last_sync_status = 'failed', last_sync_error = ?,
                       last_sync_finished_at = ?
                   WHERE id = 1"#,
            )
            .bind(&message)
            .bind(finished_at)
            .execute(pool)
            .await?;
            Err(error)
        }
    }
}

pub(crate) async fn sync_polymarket_markets_inner(pool: &Pool<MySql>) -> AppResult<SyncCounts> {
    let settings = load_settings(pool).await?;
    let tags = service::json_string_array(&settings.sync_tags_json);
    let remote_markets = fetch_polymarket_markets(&tags).await?;
    let mut seen_market_ids = HashSet::new();
    let parsed_markets = remote_markets
        .iter()
        .filter_map(|value| service::parse_polymarket_market(value).ok())
        .filter(|market| seen_market_ids.insert(market.external_market_id.clone()))
        .collect::<Vec<_>>();
    let mut counts = SyncCounts::default();
    for market in parsed_markets {
        let result = sqlx::query(
            r#"INSERT INTO prediction_markets
               (source, external_event_id, external_market_id, slug, title, description,
                image_url, category, tags_json, outcome_yes_label, outcome_no_label,
                yes_price, no_price, volume, liquidity, end_at, source_status,
                display_status, external_resolution, settlement_status, sync_payload_json,
                last_synced_at)
               VALUES ('polymarket', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open', ?, CURRENT_TIMESTAMP(6))
               ON DUPLICATE KEY UPDATE
                   external_event_id = VALUES(external_event_id),
                   slug = VALUES(slug),
                   title = VALUES(title),
                   description = VALUES(description),
                   image_url = VALUES(image_url),
                   category = VALUES(category),
                   tags_json = VALUES(tags_json),
                   outcome_yes_label = VALUES(outcome_yes_label),
                   outcome_no_label = VALUES(outcome_no_label),
                   yes_price = VALUES(yes_price),
                   no_price = VALUES(no_price),
                   volume = VALUES(volume),
                   liquidity = VALUES(liquidity),
                   end_at = VALUES(end_at),
                   source_status = VALUES(source_status),
                   display_status = VALUES(display_status),
                   external_resolution = VALUES(external_resolution),
                   sync_payload_json = VALUES(sync_payload_json),
                   last_synced_at = CURRENT_TIMESTAMP(6)"#,
        )
        .bind(&market.external_event_id)
        .bind(&market.external_market_id)
        .bind(&market.slug)
        .bind(&market.title)
        .bind(&market.description)
        .bind(&market.image_url)
        .bind(&market.category)
        .bind(SqlxJson(market.tags_json))
        .bind(&market.outcome_yes_label)
        .bind(&market.outcome_no_label)
        .bind(&market.yes_price)
        .bind(&market.no_price)
        .bind(&market.volume)
        .bind(&market.liquidity)
        .bind(market.end_at)
        .bind(&market.source_status)
        .bind(&market.source_status)
        .bind(&market.external_resolution)
        .bind(SqlxJson(market.payload))
        .execute(pool)
        .await?;
        let is_insert = result.last_insert_id() > 0;
        if is_insert {
            counts.imported_count += 1;
        } else {
            counts.updated_count += 1;
        }
        reconcile_synced_resolution(
            pool,
            &settings,
            &market.external_market_id,
            &market.source_status,
            &market.external_resolution,
        )
        .await?;
    }
    Ok(counts)
}

pub(crate) async fn reconcile_synced_resolution(
    pool: &Pool<MySql>,
    settings: &PredictionSettingsRow,
    external_market_id: &str,
    source_status: &str,
    external_resolution: &Option<String>,
) -> AppResult<()> {
    let market = load_market_by_source_external(pool, "polymarket", external_market_id).await?;
    if market.local_resolution.is_some()
        || market.settlement_status == service::SETTLEMENT_SETTLED
        || market.settlement_status == service::SETTLEMENT_REFUNDED
    {
        return Ok(());
    }

    let Some(result) = external_resolution.as_ref() else {
        // 上游明确关闭但还未给出结果时，停止继续对用户开放并交给后台确认，避免订单永久停在 open。
        if source_status == service::STATUS_HIDDEN
            && market.settlement_status == service::SETTLEMENT_OPEN
        {
            sqlx::query(
                "UPDATE prediction_markets SET settlement_status = ? WHERE id = ? AND settlement_status = ?",
            )
            .bind(service::SETTLEMENT_PENDING_CONFIRMATION)
            .bind(market.id)
            .bind(service::SETTLEMENT_OPEN)
            .execute(pool)
            .await?;
        }
        return Ok(());
    };

    let settlement_mode = market
        .settlement_mode_override
        .clone()
        .unwrap_or_else(|| settings.default_settlement_mode.clone());
    let invalid_requires_manual_policy = result == service::OUTCOME_INVALID
        && settings.default_invalid_refund_policy == service::REFUND_MANUAL;
    if settlement_mode == service::SETTLEMENT_MODE_AUTO && !invalid_requires_manual_policy {
        settle_market_in_tx(pool, market.id, result.clone(), None).await?;
        return Ok(());
    }

    if market.settlement_status == service::SETTLEMENT_OPEN {
        sqlx::query(
            "UPDATE prediction_markets SET settlement_status = ? WHERE id = ? AND settlement_status = ?",
        )
        .bind(service::SETTLEMENT_PENDING_CONFIRMATION)
        .bind(market.id)
        .bind(service::SETTLEMENT_OPEN)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub(crate) async fn fetch_polymarket_markets(tags: &[String]) -> AppResult<Vec<Value>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("rust-chain-prediction-sync/1.0")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let tags_to_fetch = if tags.is_empty() {
        vec![String::new()]
    } else {
        tags.to_vec()
    };
    let mut values = Vec::new();
    for tag in tags_to_fetch {
        for closed in [false, true] {
            let mut params = vec![
                ("closed".to_owned(), closed.to_string()),
                ("limit".to_owned(), service::DEFAULT_SYNC_LIMIT.to_owned()),
            ];
            if !closed {
                params.push(("active".to_owned(), "true".to_owned()));
            }
            if !tag.is_empty() {
                if tag.chars().all(|ch| ch.is_ascii_digit()) {
                    params.push(("tag_id".to_owned(), tag.clone()));
                } else {
                    params.push(("tag_slug".to_owned(), tag.clone()));
                }
            }
            let url = Url::parse_with_params(service::POLYMARKET_GAMMA_EVENTS_URL, &params)
                .map_err(|error| AppError::Internal(error.to_string()))?;
            let response = client
                .get(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(|error| upstream_sync_error(error.to_string()))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|error| upstream_sync_error(error.to_string()))?;
            if !status.is_success() {
                return Err(upstream_sync_error(format!(
                    "polymarket returned status {status}: {}",
                    service::compact_error_message(&body)
                )));
            }
            let payload: Value = serde_json::from_str(&body).map_err(|error| {
                upstream_sync_error(format!("polymarket returned invalid json: {error}"))
            })?;
            values.extend(service::extract_market_values(payload));
        }
    }
    Ok(values)
}

pub(crate) fn prediction_market_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT markets.id, markets.source, markets.external_event_id, markets.external_market_id,
                  markets.slug, markets.title, markets.description, markets.image_url,
                  markets.category, markets.tags_json, markets.outcome_yes_label,
                  markets.outcome_no_label, markets.yes_price, markets.no_price,
                  markets.volume, markets.liquidity, markets.end_at, markets.source_status,
                  markets.display_status, markets.external_resolution, markets.local_resolution,
                  markets.settlement_status, markets.settlement_mode_override,
                  markets.allowed_asset_ids_override_json, markets.payout_cap_overrides_json,
                  markets.fee_rate_override, markets.last_synced_at,
                  markets.created_at, markets.updated_at
           FROM prediction_markets markets"#,
    )
}

pub(crate) fn prediction_order_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.order_no, orders.user_id, users.email AS user_email,
                  orders.market_id, markets.title AS market_title, orders.outcome,
                  orders.asset_id, assets.symbol AS asset_symbol, orders.stake_amount,
                  orders.fee_amount, orders.accepted_price, orders.shares,
                  orders.theoretical_payout, orders.effective_payout_cap,
                  orders.status, orders.result, orders.payout_amount, orders.refund_amount,
                  orders.fee_refund_amount, orders.invalid_refund_policy_used,
                  orders.settled_at, orders.created_at
           FROM prediction_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN prediction_markets markets ON markets.id = orders.market_id
           INNER JOIN assets ON assets.id = orders.asset_id"#,
    )
}

pub(crate) async fn load_settings(pool: &Pool<MySql>) -> AppResult<PredictionSettingsRow> {
    sqlx::query_as::<_, PredictionSettingsRow>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json, allowed_asset_ids_json,
                  default_fee_rate, default_settlement_mode, default_invalid_refund_policy,
                  quote_ttl_seconds, last_sync_status, last_sync_error,
                  last_sync_started_at, last_sync_finished_at, last_successful_sync_at,
                  last_sync_imported_count, last_sync_updated_count
           FROM prediction_settings
           WHERE id = 1"#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal("prediction settings are missing".to_owned()))
}

pub(crate) async fn load_settings_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<PredictionSettingsRow> {
    sqlx::query_as::<_, PredictionSettingsRow>(
        r#"SELECT sync_enabled, sync_interval_seconds, sync_tags_json, allowed_asset_ids_json,
                  default_fee_rate, default_settlement_mode, default_invalid_refund_policy,
                  quote_ttl_seconds, last_sync_status, last_sync_error,
                  last_sync_started_at, last_sync_finished_at, last_successful_sync_at,
                  last_sync_imported_count, last_sync_updated_count
           FROM prediction_settings
           WHERE id = 1
           FOR UPDATE"#,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("prediction settings are missing".to_owned()))
}

pub(crate) async fn upsert_asset_config(
    pool: &Pool<MySql>,
    asset_id: u64,
    enabled: bool,
    max_payout_amount: BigDecimal,
) -> AppResult<PredictionAssetConfigResponse> {
    service::ensure_non_negative_decimal(&max_payout_amount, "max_payout_amount")?;
    load_active_asset(pool, asset_id).await?;
    sqlx::query(
        r#"INSERT INTO prediction_asset_configs (asset_id, enabled, max_payout_amount)
           VALUES (?, ?, ?)
           ON DUPLICATE KEY UPDATE enabled = VALUES(enabled),
                                   max_payout_amount = VALUES(max_payout_amount)"#,
    )
    .bind(asset_id)
    .bind(enabled)
    .bind(&max_payout_amount)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, PredictionAssetConfigResponse>(
        r#"SELECT configs.asset_id, assets.symbol AS asset_symbol, configs.enabled,
                  configs.max_payout_amount, configs.created_at, configs.updated_at
           FROM prediction_asset_configs configs
           INNER JOIN assets ON assets.id = configs.asset_id
           WHERE configs.asset_id = ?"#,
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn update_admin_market(
    pool: &Pool<MySql>,
    market_id: u64,
    display_status: &str,
    settlement_mode_override: Option<&str>,
    allowed_asset_ids_override: Option<&[u64]>,
    payout_cap_overrides: Option<&Value>,
    fee_rate_override: Option<&BigDecimal>,
) -> AppResult<bool> {
    // 管理端更新市场展示和结算策略，返回是否成功命中到对应市场。
    let result = sqlx::query(
        r#"UPDATE prediction_markets
           SET display_status = ?, settlement_mode_override = ?,
               allowed_asset_ids_override_json = ?, payout_cap_overrides_json = ?,
               fee_rate_override = ?
           WHERE id = ?"#,
    )
    .bind(display_status)
    .bind(settlement_mode_override)
    .bind(allowed_asset_ids_override.map(|ids| SqlxJson(json!(ids))))
    .bind(payout_cap_overrides.cloned().map(SqlxJson))
    .bind(fee_rate_override)
    .bind(market_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(crate) async fn list_admin_sync_logs(
    pool: &Pool<MySql>,
    limit: i64,
) -> AppResult<Vec<PredictionSyncLogRow>> {
    // 后台查询同步日志，按 ID 倒序分页返回。
    let rows = sqlx::query_as::<_, PredictionSyncLogRow>(
        r#"SELECT id, trigger_type, status, imported_count, updated_count,
                  error_message, started_at, finished_at
           FROM prediction_sync_logs
           ORDER BY id DESC
           LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub(crate) async fn load_market_response(
    pool: &Pool<MySql>,
    market_id: u64,
) -> AppResult<PredictionMarketResponse> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.id = ");
    builder.push_bind(market_id);
    builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_market_by_source_external(
    pool: &Pool<MySql>,
    source: &str,
    external_market_id: &str,
) -> AppResult<PredictionMarketResponse> {
    let mut builder = prediction_market_query_builder();
    builder.push(" WHERE markets.source = ");
    builder.push_bind(source.to_owned());
    builder.push(" AND markets.external_market_id = ");
    builder.push_bind(external_market_id.to_owned());
    builder
        .build_query_as::<PredictionMarketResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_order_response(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<PredictionOrderResponse> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_order_by_idempotency(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<PredictionOrderResponse>> {
    let mut builder = prediction_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND orders.idempotency_key = ");
    builder.push_bind(idempotency_key.to_owned());
    Ok(builder
        .build_query_as::<PredictionOrderResponse>()
        .fetch_optional(pool)
        .await?)
}

pub(crate) async fn lock_quote(
    tx: &mut Transaction<'_, MySql>,
    quote_id: &str,
) -> AppResult<PredictionQuoteLockRow> {
    sqlx::query_as::<_, PredictionQuoteLockRow>(
        r#"SELECT quote_id, user_id, market_id, outcome, asset_id, stake_amount,
                  fee_amount, accepted_price, shares, theoretical_payout,
                  effective_payout_cap, expires_at, consumed_at
           FROM prediction_quotes
           WHERE quote_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(quote_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_market(
    tx: &mut Transaction<'_, MySql>,
    market_id: u64,
) -> AppResult<PredictionMarketResponse> {
    let market = sqlx::query_as::<_, PredictionMarketResponse>(
        r#"SELECT id, source, external_event_id, external_market_id, slug, title, description,
                  image_url, category, tags_json, outcome_yes_label, outcome_no_label,
                  yes_price, no_price, volume, liquidity, end_at, source_status,
                  display_status, external_resolution, local_resolution, settlement_status,
                  settlement_mode_override, allowed_asset_ids_override_json,
                  payout_cap_overrides_json, fee_rate_override, last_synced_at,
                  created_at, updated_at
           FROM prediction_markets
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(market_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(market)
}

pub(crate) async fn load_active_asset(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<PredictionAssetMetaRow> {
    let asset = sqlx::query_as::<_, PredictionAssetMetaRow>(
        "SELECT symbol, precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != service::STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

pub(crate) async fn load_active_asset_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<PredictionAssetMetaRow> {
    let asset = sqlx::query_as::<_, PredictionAssetMetaRow>(
        "SELECT symbol, precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != service::STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

pub(crate) async fn ensure_prediction_asset_enabled(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let enabled = sqlx::query_as::<_, (bool,)>(
        "SELECT enabled FROM prediction_asset_configs WHERE asset_id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.0)
    .unwrap_or(false);
    if !enabled {
        return Err(AppError::Validation(
            "asset is not enabled for prediction betting".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn effective_market_config(
    settings: &PredictionSettingsRow,
    market: &PredictionMarketResponse,
) -> EffectiveMarketConfig {
    let allowed_asset_ids = market
        .allowed_asset_ids_override_json
        .as_ref()
        .map(|value| service::json_u64_array(&value.0))
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| service::json_u64_array(&settings.allowed_asset_ids_json));
    let fee_rate = market
        .fee_rate_override
        .clone()
        .unwrap_or_else(|| settings.default_fee_rate.clone());
    let payout_cap_overrides = market
        .payout_cap_overrides_json
        .as_ref()
        .map(|value| value.0.clone());
    EffectiveMarketConfig {
        allowed_asset_ids,
        fee_rate,
        payout_cap_overrides,
    }
}

pub(crate) async fn effective_payout_cap(
    pool: &Pool<MySql>,
    asset_id: u64,
    overrides: &Option<Value>,
) -> AppResult<BigDecimal> {
    let asset_key = asset_id.to_string();
    if let Some(value) = overrides
        && let Some(cap) = value
            .get(asset_key.as_str())
            .and_then(service::decimal_from_json)
    {
        return Ok(cap);
    }
    let cap = sqlx::query_as::<_, (BigDecimal,)>(
        "SELECT max_payout_amount FROM prediction_asset_configs WHERE asset_id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.0)
    .unwrap_or_else(|| BigDecimal::from(0));
    Ok(cap)
}

pub(crate) async fn validate_asset_ids_exist(pool: &Pool<MySql>, ids: &[u64]) -> AppResult<()> {
    for id in service::unique_u64_list(ids.to_vec()) {
        load_active_asset(pool, id).await?;
    }
    Ok(())
}

pub(crate) async fn apply_wallet_prediction_open(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    fee_amount: &BigDecimal,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let total_required = stake_amount.clone() + fee_amount.clone();
    if wallet.available < total_required {
        return Err(AppError::Validation(format!(
            "insufficient available balance for prediction order: requested {}, available {}",
            stake_amount.clone() + fee_amount.clone(),
            wallet.available
        )));
    }
    let available_after_stake = wallet.available.clone() - stake_amount.clone();
    let frozen_after = wallet.frozen.clone() + stake_amount.clone();
    let available_after_fee = available_after_stake.clone() - fee_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after_fee)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "available",
        &available_after_stake,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_freeze",
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_freeze",
        order_id,
    )
    .await?;
    if fee_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            -fee_amount.clone(),
            "available",
            &available_after_fee,
            &available_after_fee,
            &frozen_after,
            &wallet.locked,
            "prediction_fee",
            order_id,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn apply_wallet_prediction_settlement(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    payout_amount: &BigDecimal,
    order_id: u64,
    won: bool,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for prediction settlement: requested {}, frozen {}",
            stake_amount, wallet.frozen
        )));
    }
    let frozen_after = wallet.frozen.clone() - stake_amount.clone();
    let available_after = wallet.available.clone() + payout_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        if won {
            "prediction_settle_win"
        } else {
            "prediction_settle_loss"
        },
        order_id,
    )
    .await?;
    if payout_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            payout_amount.clone(),
            "available",
            &available_after,
            &available_after,
            &frozen_after,
            &wallet.locked,
            "prediction_payout",
            order_id,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn apply_wallet_prediction_refund(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    stake_amount: &BigDecimal,
    fee_refund_amount: &BigDecimal,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for prediction refund: requested {}, frozen {}",
            stake_amount, wallet.frozen
        )));
    }
    let available_after_stake = wallet.available.clone() + stake_amount.clone();
    let frozen_after = wallet.frozen.clone() - stake_amount.clone();
    let available_after_fee = available_after_stake.clone() + fee_refund_amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after_fee)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        stake_amount.clone(),
        "available",
        &available_after_stake,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_refund",
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -stake_amount.clone(),
        "frozen",
        &frozen_after,
        &available_after_stake,
        &frozen_after,
        &wallet.locked,
        "prediction_stake_refund",
        order_id,
    )
    .await?;
    if fee_refund_amount > &BigDecimal::from(0) {
        insert_wallet_ledger(
            tx,
            user_id,
            asset_id,
            fee_refund_amount.clone(),
            "available",
            &available_after_fee,
            &available_after_fee,
            &frozen_after,
            &wallet.locked,
            "prediction_fee_refund",
            order_id,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<PredictionWalletRow> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query_as::<_, PredictionWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(service::REF_TYPE_PREDICTION_ORDER)
    .bind(order_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state
        .mysql
        .clone()
        .ok_or_else(|| AppError::Internal("mysql pool is not configured".to_owned()))
}

pub(crate) fn upstream_sync_error(message: String) -> AppError {
    AppError::Api {
        status: StatusCode::BAD_GATEWAY,
        code: "POLYMARKET_SYNC_FAILED",
        message: service::compact_error_message(&message),
    }
}
