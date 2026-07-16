//! loan bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    error::{AppError, AppResult},
    modules::loan::presentation::{LoanOrderResponse, LoanProductResponse},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

const STATUS_ACTIVE: &str = "active";
const REF_TYPE_LOAN_ORDER: &str = "loan_order";

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct LoanProductTermsRow {
    pub(crate) id: u64,
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct LoanOrderLockRow {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) term_days: u32,
    pub(crate) collateral_asset_id: Option<u64>,
    pub(crate) collateral_amount: Option<BigDecimal>,
    pub(crate) status: String,
    pub(crate) disbursed_at: Option<DateTime<Utc>>,
    pub(crate) collateral_released_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AssetMetaRow {
    pub(crate) precision_scale: i32,
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct UserKycRow {
    kyc_level: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct WalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

pub(crate) struct AdminLoanOrdersFilter {
    pub(crate) limit: u32,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) product_id: Option<u64>,
    pub(crate) loan_type: Option<String>,
    pub(crate) status: Option<String>,
}

pub(crate) struct LoanProductWrite {
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) name_json: Value,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) status: String,
}

pub(crate) struct LoanOrderCreate {
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) term_days: u32,
    pub(crate) min_kyc_level: i32,
    pub(crate) collateral_asset_id: Option<u64>,
    pub(crate) collateral_amount: Option<BigDecimal>,
    pub(crate) idempotency_key: String,
}

pub(crate) async fn insert_loan_product(
    pool: &Pool<MySql>,
    product: LoanProductWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO loan_products
           (loan_type, asset_id, name, name_json, term_days, interest_rate, interest_calculation_mode,
            min_kyc_level, min_amount, max_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&product.loan_type)
    .bind(product.asset_id)
    .bind(&product.name)
    .bind(SqlxJson(product.name_json))
    .bind(product.term_days)
    .bind(&product.interest_rate)
    .bind(&product.interest_calculation_mode)
    .bind(product.min_kyc_level)
    .bind(&product.min_amount)
    .bind(&product.max_amount)
    .bind(&product.status)
    .execute(pool)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn update_loan_product(
    pool: &Pool<MySql>,
    product_id: u64,
    product: LoanProductWrite,
) -> AppResult<()> {
    let updated = sqlx::query(
        r#"UPDATE loan_products
           SET loan_type = ?, asset_id = ?, name_json = ?, name = ?, term_days = ?, interest_rate = ?,
               interest_calculation_mode = ?, min_kyc_level = ?, min_amount = ?,
               max_amount = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(&product.loan_type)
    .bind(product.asset_id)
    .bind(SqlxJson(product.name_json))
    .bind(&product.name)
    .bind(product.term_days)
    .bind(&product.interest_rate)
    .bind(&product.interest_calculation_mode)
    .bind(product.min_kyc_level)
    .bind(&product.min_amount)
    .bind(&product.max_amount)
    .bind(&product.status)
    .bind(product_id)
    .execute(pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(crate) async fn update_loan_product_status(
    pool: &Pool<MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    let updated = sqlx::query("UPDATE loan_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(pool)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(crate) async fn list_loan_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<LoanProductResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.loan_type, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.name_json, products.term_days, products.interest_rate,
                  products.interest_calculation_mode, products.min_kyc_level,
                  products.min_amount, products.max_amount, products.status,
                  products.created_at, products.updated_at
           FROM loan_products products
           INNER JOIN assets ON assets.id = products.asset_id"#,
    );
    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    Ok(builder
        .build_query_as::<LoanProductResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_loan_product_response(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<LoanProductResponse> {
    sqlx::query_as::<_, LoanProductResponse>(
        r#"SELECT products.id, products.loan_type, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.name_json, products.term_days, products.interest_rate,
                  products.interest_calculation_mode, products.min_kyc_level,
                  products.min_amount, products.max_amount, products.status,
                  products.created_at, products.updated_at
           FROM loan_products products
           INNER JOIN assets ON assets.id = products.asset_id
           WHERE products.id = ?
           LIMIT 1"#,
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn list_user_loan_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<String>,
    limit: u32,
) -> AppResult<Vec<LoanOrderResponse>> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    if let Some(status) = optional_string(status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY orders.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    Ok(builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn list_admin_loan_orders(
    pool: &Pool<MySql>,
    filter: AdminLoanOrdersFilter,
) -> AppResult<Vec<LoanOrderResponse>> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = filter.user_id {
        builder.push(" AND orders.user_id = ");
        builder.push_bind(user_id);
    }
    if let Some(email) = optional_string(filter.email) {
        builder.push(" AND users.email LIKE ");
        builder.push_bind(format!("%{email}%"));
    }
    if let Some(product_id) = filter.product_id {
        builder.push(" AND orders.product_id = ");
        builder.push_bind(product_id);
    }
    if let Some(loan_type) = optional_string(filter.loan_type) {
        builder.push(" AND orders.loan_type = ");
        builder.push_bind(loan_type);
    }
    if let Some(status) = optional_string(filter.status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY orders.id DESC LIMIT ");
    builder.push_bind(filter.limit as i64);

    Ok(builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_all(pool)
        .await?)
}

pub(crate) async fn load_loan_order_response(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_user_loan_order_response(
    pool: &Pool<MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder.push(" AND orders.user_id = ");
    builder.push_bind(user_id);
    builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn load_loan_order_by_idempotency(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<LoanOrderResponse> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND orders.idempotency_key = ");
    builder.push_bind(idempotency_key.to_owned());
    builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_active_loan_product_terms(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<LoanProductTermsRow> {
    let product = sqlx::query_as::<_, LoanProductTermsRow>(
        r#"SELECT id, loan_type, asset_id, term_days, interest_rate,
                  interest_calculation_mode, min_kyc_level, min_amount, max_amount, status
           FROM loan_products
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != STATUS_ACTIVE {
        return Err(AppError::Validation(
            "loan product is not active".to_owned(),
        ));
    }
    Ok(product)
}

pub(crate) async fn lock_loan_order(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<LoanOrderLockRow> {
    sqlx::query_as::<_, LoanOrderLockRow>(
        r#"SELECT id, user_id, asset_id, amount, interest_rate,
                  interest_calculation_mode, term_days, collateral_asset_id,
                  collateral_amount, status, disbursed_at, collateral_released_at
           FROM loan_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_user_loan_order(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<Option<LoanOrderLockRow>> {
    sqlx::query_as::<_, LoanOrderLockRow>(
        r#"SELECT id, user_id, asset_id, amount, interest_rate,
                  interest_calculation_mode, term_days, collateral_asset_id,
                  collateral_amount, status, disbursed_at, collateral_released_at
           FROM loan_orders
           WHERE id = ? AND user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Database)
}

pub(crate) async fn insert_loan_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: LoanOrderCreate,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"INSERT INTO loan_orders
           (user_id, product_id, loan_type, asset_id, amount, interest_rate,
            interest_calculation_mode, term_days, min_kyc_level, collateral_asset_id,
            collateral_amount, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(order.user_id)
    .bind(order.product_id)
    .bind(&order.loan_type)
    .bind(order.asset_id)
    .bind(&order.amount)
    .bind(&order.interest_rate)
    .bind(&order.interest_calculation_mode)
    .bind(order.term_days)
    .bind(order.min_kyc_level)
    .bind(order.collateral_asset_id)
    .bind(&order.collateral_amount)
    .bind(&order.idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

pub(crate) async fn mark_loan_order_cancelled_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE loan_orders SET status = 'cancelled', cancelled_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn mark_loan_order_disbursed_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    admin_id: u64,
    due_at: NaiveDateTime,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'disbursed',
               approved_by = ?,
               approved_at = CURRENT_TIMESTAMP(6),
               disbursed_at = CURRENT_TIMESTAMP(6),
               due_at = ?
           WHERE id = ?"#,
    )
    .bind(admin_id)
    .bind(due_at)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn mark_loan_order_rejected_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    admin_id: u64,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'rejected',
               rejected_by = ?,
               rejected_reason = ?,
               rejected_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(admin_id)
    .bind(reason)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn mark_loan_order_repaid_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    interest_amount: &BigDecimal,
    repayment_amount: &BigDecimal,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'repaid',
               interest_amount = ?,
               repayment_amount = ?,
               repaid_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(interest_amount)
    .bind(repayment_amount)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_active_asset_meta_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AssetMetaRow> {
    let asset = sqlx::query_as::<_, AssetMetaRow>(
        "SELECT precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

pub(crate) async fn load_active_asset_meta(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<AssetMetaRow> {
    let asset = sqlx::query_as::<_, AssetMetaRow>(
        "SELECT precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

pub(crate) async fn ensure_loan_user_kyc_level(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    min_kyc_level: i32,
) -> AppResult<()> {
    let user = sqlx::query_as::<_, UserKycRow>("SELECT kyc_level FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if user.kyc_level < min_kyc_level {
        return Err(AppError::Validation(format!(
            "loan product requires KYC level {min_kyc_level}"
        )));
    }
    Ok(())
}

pub(crate) async fn release_loan_collateral_if_needed(
    tx: &mut Transaction<'_, MySql>,
    order: &LoanOrderLockRow,
) -> AppResult<()> {
    let Some(collateral_asset_id) = order.collateral_asset_id else {
        return Ok(());
    };
    let Some(collateral_amount) = order.collateral_amount.as_ref() else {
        return Ok(());
    };
    if order.collateral_released_at.is_some() {
        return Ok(());
    }
    apply_loan_wallet_unfreeze(
        tx,
        order.user_id,
        collateral_asset_id,
        collateral_amount,
        "loan_collateral_release",
        order.id,
    )
    .await?;
    sqlx::query(
        "UPDATE loan_orders SET collateral_released_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(order.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn apply_loan_wallet_freeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for loan collateral: requested {}, available {}",
            amount, wallet.available
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    let frozen_after = wallet.frozen.clone() + amount.clone();
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
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

pub(crate) async fn apply_loan_wallet_credit(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

pub(crate) async fn apply_loan_wallet_debit(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for loan repayment: requested {}, available {}",
            amount, wallet.available
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

async fn apply_loan_wallet_unfreeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for loan collateral: requested {}, frozen {}",
            amount, wallet.frozen
        )));
    }
    let available_after = wallet.available.clone() + amount.clone();
    let frozen_after = wallet.frozen.clone() - amount.clone();
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
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletRow> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, WalletRow>(
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
async fn insert_wallet_ledger(
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
    .bind(REF_TYPE_LOAN_ORDER)
    .bind(order_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn loan_order_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.user_id, users.email AS user_email,
                  orders.product_id, products.name AS product_name,
                  products.name_json AS product_name_json,
                  orders.loan_type, orders.asset_id, assets.symbol AS asset_symbol,
                  orders.amount, orders.interest_rate, orders.interest_calculation_mode,
                  orders.term_days, orders.min_kyc_level,
                  orders.collateral_asset_id, collateral_assets.symbol AS collateral_asset_symbol,
                  orders.collateral_amount, orders.status, orders.interest_amount,
                  orders.repayment_amount, orders.approved_by, orders.rejected_by,
                  orders.rejected_reason, orders.approved_at, orders.rejected_at,
                  orders.disbursed_at, orders.due_at, orders.cancelled_at, orders.repaid_at,
                  orders.collateral_released_at, orders.created_at, orders.updated_at
           FROM loan_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN loan_products products ON products.id = orders.product_id
           INNER JOIN assets ON assets.id = orders.asset_id
           LEFT JOIN assets collateral_assets ON collateral_assets.id = orders.collateral_asset_id"#,
    )
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|db_error| db_error.code())
        .is_some_and(|code| code == "1062" || code == "23000")
}
