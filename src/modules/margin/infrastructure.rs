//! margin bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 钱包划转和用户设置持久化放在这里，应用层只编排事务边界。

use crate::{
    error::{AppError, AppResult},
    modules::{
        margin::presentation::{
            AdminInterestSummaryItem, AdminMarginPositionResponse, MarginPositionResponse,
            MarginProductResponse, MarginUserSettingResponse, MarginWalletAccountResponse,
            MarginWalletAccountSnapshot,
        },
        market::market_ticker_redis_key,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MarginProductSettingRule {
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MarginOpenProductRule {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
    pub(crate) min_margin: BigDecimal,
    pub(crate) max_margin: Option<BigDecimal>,
    pub(crate) hourly_interest_rate: BigDecimal,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MarginWalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MarginTransferAssetRule {
    pub(crate) id: u64,
    pub(crate) precision_scale: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MarginTransferRecord {
    pub(crate) transfer_id: String,
    pub(crate) asset_id: u64,
    pub(crate) from_account: String,
    pub(crate) to_account: String,
    pub(crate) amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "crate::time::unix_millis")]
    observed_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct LockedMarginPositionRow {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) wallet_scope: String,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MarginRiskPositionRow {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) margin_asset: u64,
    pub(crate) direction: String,
    pub(crate) margin_amount: BigDecimal,
    pub(crate) notional_amount: BigDecimal,
    pub(crate) interest_amount: BigDecimal,
    pub(crate) entry_price: Option<BigDecimal>,
    pub(crate) maintenance_margin_rate: BigDecimal,
    pub(crate) status: String,
}

pub(crate) struct MarginRiskTicker {
    pub(crate) last_price: BigDecimal,
    pub(crate) observed_at: DateTime<Utc>,
}

pub(crate) async fn transfer_spot_to_margin_wallets(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 双向划转统一先锁现货、再锁杠杆钱包，避免反向请求形成交叉等待。
    let spot_wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    if spot_wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin transfer: requested {}, available {}, locked {}",
            amount, spot_wallet.available, spot_wallet.locked
        )));
    }
    let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let spot_available_after = spot_wallet.available.clone() - amount.clone();
    let margin_available_after = margin_wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&spot_available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&margin_available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_out",
        &(-amount.clone()),
        &spot_available_after,
        &spot_available_after,
        &spot_wallet.frozen,
        &spot_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_in",
        amount,
        &margin_available_after,
        &margin_available_after,
        &margin_wallet.frozen,
        &margin_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    Ok((
        MarginWalletAccountSnapshot {
            asset_id,
            available: spot_available_after,
            frozen: spot_wallet.frozen,
            locked: spot_wallet.locked,
        },
        MarginWalletAccountSnapshot {
            asset_id,
            available: margin_available_after,
            frozen: margin_wallet.frozen,
            locked: margin_wallet.locked,
        },
    ))
}

pub(crate) async fn transfer_margin_to_spot_wallets(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 与 spot -> margin 保持相同锁序。
    let spot_wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    if margin_wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient margin available balance for transfer: requested {}, available {}, locked {}",
            amount, margin_wallet.available, margin_wallet.locked
        )));
    }
    let margin_available_after = margin_wallet.available.clone() - amount.clone();
    let spot_available_after = spot_wallet.available.clone() + amount.clone();
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&margin_available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&spot_available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_out",
        &(-amount.clone()),
        &margin_available_after,
        &margin_available_after,
        &margin_wallet.frozen,
        &margin_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_in",
        amount,
        &spot_available_after,
        &spot_available_after,
        &spot_wallet.frozen,
        &spot_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    Ok((
        MarginWalletAccountSnapshot {
            asset_id,
            available: spot_available_after,
            frozen: spot_wallet.frozen,
            locked: spot_wallet.locked,
        },
        MarginWalletAccountSnapshot {
            asset_id,
            available: margin_available_after,
            frozen: margin_wallet.frozen,
            locked: margin_wallet.locked,
        },
    ))
}

pub(crate) async fn resolve_active_transfer_asset(
    tx: &mut Transaction<'_, MySql>,
    asset_id: Option<u64>,
    asset_symbol: Option<&str>,
) -> AppResult<MarginTransferAssetRule> {
    if let Some(asset_id) = asset_id {
        return sqlx::query_as::<_, MarginTransferAssetRule>(
            "SELECT id, precision_scale FROM assets WHERE id = ? AND status = 'active' LIMIT 1",
        )
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound);
    }
    let Some(symbol) = asset_symbol
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
    else {
        return Err(AppError::Validation(
            "margin transfer asset_id or asset_symbol is required".to_owned(),
        ));
    };
    sqlx::query_as::<_, MarginTransferAssetRule>(
        r#"SELECT id, precision_scale
           FROM assets
           WHERE UPPER(symbol) = UPPER(?) AND status = 'active'
           LIMIT 1"#,
    )
    .bind(symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn resolve_transfer_asset_id_for_replay(
    pool: &Pool<MySql>,
    asset_id: Option<u64>,
    asset_symbol: Option<&str>,
) -> AppResult<u64> {
    if let Some(asset_id) = asset_id {
        return sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
            .bind(asset_id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound);
    }
    let Some(symbol) = asset_symbol
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
    else {
        return Err(AppError::Validation(
            "margin transfer asset_id or asset_symbol is required".to_owned(),
        ));
    };
    sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE UPPER(symbol) = UPPER(?) LIMIT 1")
        .bind(symbol)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_margin_transfer(
    tx: &mut Transaction<'_, MySql>,
    transfer_id: &str,
    user_id: u64,
    asset_id: u64,
    from_account: &str,
    to_account: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_transfers
           (transfer_id, user_id, asset_id, from_account, to_account, amount, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(transfer_id)
    .bind(user_id)
    .bind(asset_id)
    .bind(from_account)
    .bind(to_account)
    .bind(amount)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_margin_transfer_by_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginTransferRecord>> {
    sqlx::query_as::<_, MarginTransferRecord>(
        r#"SELECT transfer_id, asset_id, from_account, to_account, amount
           FROM margin_transfers
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn load_margin_transfer_wallet_snapshots(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 幂等重放必须使用原划转流水的 after 快照，不能泄漏后续交易形成的当前余额。
    let spot_wallet = sqlx::query_as::<_, MarginWalletAccountSnapshot>(
        r#"SELECT asset_id, available_after AS available, frozen_after AS frozen,
                  locked_after AS locked
           FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ?
             AND ref_type = 'margin_transfer' AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(transfer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal(format!(
            "margin transfer {transfer_id} is missing its spot wallet ledger snapshot"
        ))
    })?;
    let margin_wallet = sqlx::query_as::<_, MarginWalletAccountSnapshot>(
        r#"SELECT asset_id, available_after AS available, frozen_after AS frozen,
                  locked_after AS locked
           FROM margin_wallet_ledger
           WHERE user_id = ? AND asset_id = ?
             AND ref_type = 'margin_transfer' AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(transfer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal(format!(
            "margin transfer {transfer_id} is missing its margin wallet ledger snapshot"
        ))
    })?;
    Ok((spot_wallet, margin_wallet))
}

pub(crate) async fn ensure_pair_exists(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM trading_pairs WHERE id = ? LIMIT 1")
        .bind(pair_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(crate) async fn lock_active_product_setting_rule(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductSettingRule> {
    let product = sqlx::query_as::<_, MarginProductSettingRule>(
        r#"SELECT margin_mode, margin_modes, leverage_levels
           FROM margin_products
           WHERE id = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(product)
}

pub(crate) async fn upsert_user_margin_setting(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product_id: u64,
    margin_mode: Option<&str>,
    leverage: Option<&BigDecimal>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_user_settings (user_id, product_id, margin_mode, leverage)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             margin_mode = COALESCE(VALUES(margin_mode), margin_mode),
             leverage = COALESCE(VALUES(leverage), leverage)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(margin_mode)
    .bind(leverage)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_user_margin_setting(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product_id: u64,
) -> AppResult<MarginUserSettingResponse> {
    sqlx::query_as::<_, (Option<String>, Option<BigDecimal>)>(
        "SELECT margin_mode, leverage FROM margin_user_settings WHERE user_id = ? AND product_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|(margin_mode, leverage)| MarginUserSettingResponse {
        product_id,
        margin_mode,
        leverage,
    })
    .ok_or(AppError::NotFound)
}

pub(crate) async fn load_user_margin_setting_from_pool(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
) -> AppResult<MarginUserSettingResponse> {
    sqlx::query_as::<_, (Option<String>, Option<BigDecimal>)>(
        "SELECT margin_mode, leverage FROM margin_user_settings WHERE user_id = ? AND product_id = ? LIMIT 1",
    )
    .bind(user_id)
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .map(|(margin_mode, leverage)| MarginUserSettingResponse {
        product_id,
        margin_mode,
        leverage,
    })
    .ok_or(AppError::NotFound)
}

pub(crate) async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    sqlx::query_as::<_, MarginProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.logo_url,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginProductResponse> {
    sqlx::query_as::<_, MarginProductResponse>(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.logo_url,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

#[derive(Debug)]
pub(crate) struct MarginProductUpsertValues<'a> {
    pub(crate) pair_id: u64,
    pub(crate) margin_asset: u64,
    pub(crate) logo_url: Option<String>,
    pub(crate) margin_mode: String,
    pub(crate) margin_modes: Vec<String>,
    pub(crate) leverage_levels: Vec<String>,
    pub(crate) max_leverage: &'a BigDecimal,
    pub(crate) min_margin: &'a BigDecimal,
    pub(crate) max_margin: Option<&'a BigDecimal>,
    pub(crate) maintenance_margin_rate: &'a BigDecimal,
    pub(crate) hourly_interest_rate: BigDecimal,
    pub(crate) status: &'a str,
}

pub(crate) async fn insert_margin_product(
    tx: &mut Transaction<'_, MySql>,
    values: &MarginProductUpsertValues<'_>,
) -> AppResult<u64> {
    sqlx::query(
        r#"INSERT INTO margin_products
           (pair_id, margin_asset, logo_url, margin_mode, margin_modes, leverage_levels, max_leverage, min_margin, max_margin,
            maintenance_margin_rate, hourly_interest_rate, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(values.pair_id)
    .bind(values.margin_asset)
    .bind(&values.logo_url)
    .bind(&values.margin_mode)
    .bind(SqlxJson(values.margin_modes.clone()))
    .bind(SqlxJson(values.leverage_levels.clone()))
    .bind(values.max_leverage)
    .bind(values.min_margin)
    .bind(values.max_margin)
    .bind(values.maintenance_margin_rate)
    .bind(&values.hourly_interest_rate)
    .bind(values.status)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
    .map_err(AppError::from)
}

pub(crate) async fn update_margin_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    values: &MarginProductUpsertValues<'_>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE margin_products
           SET pair_id = ?, margin_asset = ?, logo_url = ?, margin_mode = ?, margin_modes = ?,
               leverage_levels = ?, max_leverage = ?, min_margin = ?, max_margin = ?,
               maintenance_margin_rate = ?, hourly_interest_rate = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(values.pair_id)
    .bind(values.margin_asset)
    .bind(&values.logo_url)
    .bind(&values.margin_mode)
    .bind(SqlxJson(values.margin_modes.clone()))
    .bind(SqlxJson(values.leverage_levels.clone()))
    .bind(values.max_leverage)
    .bind(values.min_margin)
    .bind(values.max_margin)
    .bind(values.maintenance_margin_rate)
    .bind(&values.hourly_interest_rate)
    .bind(values.status)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_margin_product_status(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE margin_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn insert_admin_audit_log(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, 'margin_product', ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(optional_string(reason))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn list_margin_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<MarginProductResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.pair_id, pairs.symbol,
                  products.margin_asset, assets.symbol AS margin_asset_symbol,
                  products.logo_url,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.max_leverage,
                  products.min_margin, products.max_margin, products.maintenance_margin_rate,
                  products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           INNER JOIN assets ON assets.id = products.margin_asset"#,
    );
    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<MarginProductResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn list_user_margin_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<MarginPositionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT positions.id, positions.user_id, positions.product_id, positions.pair_id,
                  positions.margin_asset, positions.wallet_scope, positions.margin_mode,
                  positions.direction, positions.margin_amount, positions.leverage,
                  positions.notional_amount, positions.borrowed_amount, positions.interest_amount,
                  positions.entry_price, positions.exit_price, positions.realized_pnl,
                  positions.closed_at, positions.status, positions.idempotency_key
           FROM margin_positions positions
           WHERE positions.user_id = "#,
    );
    builder.push_bind(user_id);
    if let Some(status) = status {
        builder.push(" AND positions.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY positions.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<MarginPositionResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn list_margin_wallet_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MarginWalletAccountResponse>> {
    sqlx::query_as::<_, MarginWalletAccountResponse>(
        r#"SELECT wallets.asset_id, assets.symbol AS asset_symbol,
                  wallets.available, wallets.frozen, wallets.locked
           FROM margin_wallet_accounts wallets
           INNER JOIN assets ON assets.id = wallets.asset_id
           WHERE wallets.user_id = ?
           ORDER BY wallets.asset_id ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn load_user_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ? AND user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn list_admin_margin_positions(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<AdminMarginPositionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = user_id {
        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
    }
    push_user_email_filter(&mut builder, "user_id", email);
    if let Some(pair_id) = pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<AdminMarginPositionResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn load_admin_margin_position_by_id(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<Option<AdminMarginPositionResponse>> {
    sqlx::query_as::<_, AdminMarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn list_admin_interest_summary(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<AdminInterestSummaryItem>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT margin_asset, status, COUNT(*) AS position_count,
                  COALESCE(SUM(borrowed_amount), 0) AS borrowed_amount,
                  COALESCE(SUM(interest_amount), 0) AS interest_amount
           FROM margin_positions
           WHERE 1 = 1"#,
    );
    if let Some(user_id) = user_id {
        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
    }
    push_user_email_filter(&mut builder, "user_id", email);
    if let Some(pair_id) = pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status);
    }
    builder.push(" GROUP BY margin_asset, status ORDER BY margin_asset ASC, status ASC LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<AdminInterestSummaryItem>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn load_user_risk_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginRiskPositionRow>> {
    sqlx::query_as::<_, MarginRiskPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol, positions.margin_asset,
                  positions.direction, positions.margin_amount, positions.notional_amount,
                  positions.interest_amount, positions.entry_price,
                  products.maintenance_margin_rate, positions.status
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn cached_margin_mark_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "cached ticker is required to close margin position",
        "margin close ticker",
    )
    .await?;
    Ok(ticker.last_price)
}

pub(crate) async fn cached_margin_risk_ticker(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<MarginRiskTicker> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "cached ticker is required for margin risk snapshot",
        "margin risk ticker",
    )
    .await?;
    Ok(MarginRiskTicker {
        last_price: ticker.last_price,
        observed_at: ticker.observed_at,
    })
}

pub(crate) async fn cached_margin_entry_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "fresh cached ticker is required to open margin position",
        "margin entry ticker",
    )
    .await?;
    Ok(ticker.last_price)
}

async fn cached_valid_margin_ticker(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
    missing_message: &str,
    label: &str,
) -> AppResult<CachedTickerPayload> {
    let Some(redis) = redis else {
        return Err(AppError::Validation(format!(
            "{missing_message} for pair {pair_id}"
        )));
    };
    let ticker = cached_ticker_price(redis, symbol)
        .await?
        .ok_or_else(|| AppError::Validation(format!("{missing_message} for pair {pair_id}")))?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(format!(
            "{label} price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "{label} is stale for pair {pair_id}"
        )));
    }
    Ok(ticker)
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
) -> AppResult<Option<CachedTickerPayload>> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    payload
        .map(|payload| {
            serde_json::from_str::<CachedTickerPayload>(&payload).map_err(|error| {
                AppError::Internal(format!("invalid cached margin ticker payload: {error}"))
            })
        })
        .transpose()
}

pub(crate) async fn load_open_position_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<Vec<u64>> {
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT id FROM margin_positions WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND status = 'opened' AND entry_price IS NOT NULL");
    if let Some(product_id) = product_id {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
    }
    builder.push(" ORDER BY id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn load_cancelable_position_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<Vec<u64>> {
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT id FROM margin_positions WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND status = 'opened' AND entry_price IS NULL");
    if let Some(product_id) = product_id {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
    }
    builder.push(" ORDER BY id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

pub(crate) async fn existing_position_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn existing_position_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn lock_active_open_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginOpenProductRule> {
    let product = sqlx::query_as::<_, MarginOpenProductRule>(
        r#"SELECT products.id, products.pair_id, pairs.symbol, products.margin_asset,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.min_margin,
                  products.max_margin, products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    Ok(product)
}

pub(crate) async fn insert_margin_position(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product: &MarginOpenProductRule,
    margin_mode: &str,
    direction: &str,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    notional_amount: &BigDecimal,
    borrowed_amount: &BigDecimal,
    entry_price: &BigDecimal,
    idempotency_key: &str,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, interest_accrued_at,
            entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP(6), ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.pair_id)
    .bind(product.margin_asset)
    .bind(margin_mode)
    .bind(direction)
    .bind(margin_amount)
    .bind(leverage)
    .bind(notional_amount)
    .bind(borrowed_amount)
    .bind(zero_amount())
    .bind(entry_price)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
}

pub(crate) async fn set_margin_position_wallet_scope(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
    wallet_scope: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE margin_positions SET wallet_scope = ? WHERE id = ?")
        .bind(wallet_scope)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn lock_user_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<LockedMarginPositionRow>> {
    sqlx::query_as::<_, LockedMarginPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol,
                  positions.margin_asset, positions.wallet_scope, positions.direction, positions.margin_amount,
                  positions.notional_amount, positions.interest_amount, positions.entry_price,
                  positions.status
           FROM margin_positions positions
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn debit_margin_position_open_collateral(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    position_id: u64,
) -> AppResult<String> {
    if let Some(margin_wallet) = lock_existing_margin_wallet_row(tx, user_id, asset_id).await?
        && margin_wallet.available >= *amount
    {
        let available_after = margin_wallet.available.clone() - amount.clone();
        sqlx::query(
            "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
        insert_margin_wallet_ledger(
            tx,
            user_id,
            asset_id,
            "margin_position_open",
            &(-amount.clone()),
            &available_after,
            &available_after,
            &margin_wallet.frozen,
            &margin_wallet.locked,
            "margin_position",
            &position_id.to_string(),
        )
        .await?;
        return Ok("margin".to_owned());
    }

    let wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin position: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_position_open",
        &(-amount.clone()),
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        "margin_position",
        &position_id.to_string(),
    )
    .await?;
    Ok("spot".to_owned())
}

pub(crate) async fn load_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
) -> AppResult<MarginPositionResponse> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn credit_margin_position_amount(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    wallet_scope: &str,
    amount: &BigDecimal,
    change_type: &str,
    position_id: u64,
) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Ok(());
    }
    match wallet_scope {
        "margin" => {
            let wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
            let available_after = wallet.available.clone() + amount.clone();
            sqlx::query(
                "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
            )
            .bind(&available_after)
            .bind(user_id)
            .bind(asset_id)
            .execute(&mut **tx)
            .await?;
            insert_margin_wallet_ledger(
                tx,
                user_id,
                asset_id,
                change_type,
                amount,
                &available_after,
                &available_after,
                &wallet.frozen,
                &wallet.locked,
                "margin_position",
                &position_id.to_string(),
            )
            .await
        }
        "spot" => {
            let wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
            let available_after = wallet.available.clone() + amount.clone();
            sqlx::query(
                "UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
            )
            .bind(&available_after)
            .bind(user_id)
            .bind(asset_id)
            .execute(&mut **tx)
            .await?;
            insert_spot_wallet_ledger(
                tx,
                user_id,
                asset_id,
                change_type,
                amount,
                &available_after,
                &available_after,
                &wallet.frozen,
                &wallet.locked,
                "margin_position",
                &position_id.to_string(),
            )
            .await
        }
        _ => Err(AppError::Validation(
            "margin position wallet_scope must be spot or margin".to_owned(),
        )),
    }
}

pub(crate) async fn mark_position_closed(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
    closed_at: DateTime<Utc>,
    exit_price: &BigDecimal,
    realized_pnl: &BigDecimal,
) -> AppResult<()> {
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'closed', closed_at = ?, exit_price = ?, realized_pnl = ?,
               next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened'"#,
    )
    .bind(closed_at.naive_utc())
    .bind(exit_price)
    .bind(realized_pnl)
    .bind(position_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if update_position.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin position close status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn mark_position_canceled(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
    closed_at: DateTime<Utc>,
) -> AppResult<()> {
    let update_position = sqlx::query(
        r#"UPDATE margin_positions
           SET status = 'canceled', closed_at = ?, next_liquidation_attempt_at = NULL
           WHERE id = ? AND user_id = ? AND status = 'opened' AND entry_price IS NULL"#,
    )
    .bind(closed_at.naive_utc())
    .bind(position_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if update_position.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "margin position cancel status changed concurrently".to_owned(),
        ));
    }
    Ok(())
}

async fn lock_spot_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    sqlx::query_as::<_, MarginWalletRow>(
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
    .ok_or_else(|| AppError::Validation("wallet account is required for margin".to_owned()))
}

async fn ensure_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO margin_wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn lock_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    ensure_margin_wallet_row(tx, user_id, asset_id).await?;
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn lock_existing_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<Option<MarginWalletRow>> {
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn ensure_asset_exists(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&mut **tx)
        .await?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn insert_spot_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_margin_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn push_user_email_filter(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id_column: &'static str,
    email: Option<String>,
) {
    if let Some(email) = optional_string(email) {
        builder.push(" AND EXISTS (SELECT 1 FROM users WHERE users.id = ");
        builder.push(user_id_column);
        builder.push(" AND users.email = ");
        builder.push_bind(email);
        builder.push(")");
    }
}

fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}
