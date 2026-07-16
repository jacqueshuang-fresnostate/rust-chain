//! earn bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。

use crate::{
    architecture::InfrastructureLayer,
    error::{AppError, AppResult},
    modules::earn::{
        presentation::{
            EarnCategoriesResponse, EarnCategoryResponse, EarnProductResponse,
            EarnProductsResponse, EarnSubscriptionResponse, EarnSubscriptionsResponse,
        },
        repository::{EarnCategoryWrite, EarnProductRuleRow, EarnProductWrite, EarnWalletRow},
    },
};
use bigdecimal::BigDecimal;
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug)]
pub struct InfrastructureLayerMarker;

impl InfrastructureLayer for InfrastructureLayerMarker {}

pub(crate) async fn list_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<EarnProductsResponse> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.banner_url, products.small_logo_url,
                  products.category,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(categories.name_json, '$.items[0].title')), products.category) AS category_name,
                  categories.name_json AS category_name_json,
                  products.introduction_json,
                  products.term_days, products.apr_rate, products.redemption_fee_rate,
                  products.maturity_profit_fee_rate, products.early_redeem_fee_basis,
                  products.early_redeem_fee_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category"#,
    );

    if let Some(status) = status {
        builder.push(" WHERE products.status = ");
        builder.push_bind(status);
    }

    builder.push(" ORDER BY products.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let products = builder
        .build_query_as::<EarnProductResponse>()
        .fetch_all(pool)
        .await?;
    Ok(EarnProductsResponse { products })
}

pub(crate) async fn list_user_subscriptions(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<EarnSubscriptionsResponse> {
    let subscriptions = sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE user_id = ?
           ORDER BY created_at DESC, id DESC
           LIMIT ?"#,
    )
    .bind(user_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;
    Ok(EarnSubscriptionsResponse { subscriptions })
}

pub(crate) async fn list_admin_subscriptions(
    pool: &Pool<MySql>,
    limit: u32,
    user_id: Option<u64>,
    email: Option<String>,
    status: Option<String>,
) -> AppResult<EarnSubscriptionsResponse> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions"#,
    );
    let mut has_filter = false;
    if let Some(user_id) = user_id {
        builder.push(" WHERE user_id = ");
        builder.push_bind(user_id);
        has_filter = true;
    }
    if let Some(email) = email {
        builder.push(if has_filter { " AND " } else { " WHERE " });
        builder.push("EXISTS (SELECT 1 FROM users WHERE users.id = user_id AND users.email = ");
        builder.push_bind(email);
        builder.push(")");
        has_filter = true;
    }
    if let Some(status) = status {
        builder.push(if has_filter {
            " AND status = "
        } else {
            " WHERE status = "
        });
        builder.push_bind(status);
    }
    builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    builder.push_bind(limit as i64);

    let subscriptions = builder
        .build_query_as::<EarnSubscriptionResponse>()
        .fetch_all(pool)
        .await?;
    Ok(EarnSubscriptionsResponse { subscriptions })
}

pub(crate) async fn list_admin_categories(
    pool: &Pool<MySql>,
    limit: u32,
    status: Option<String>,
) -> AppResult<EarnCategoriesResponse> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT id, code, name_json,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(name_json, '$.items[0].title')), code) AS default_name,
                  sort_order, status
           FROM earn_product_categories"#,
    );
    if let Some(status) = status {
        builder.push(" WHERE status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY sort_order ASC, id ASC LIMIT ");
    builder.push_bind(limit as i64);

    let categories = builder
        .build_query_as::<EarnCategoryResponse>()
        .fetch_all(pool)
        .await?;
    Ok(EarnCategoriesResponse { categories })
}

pub(crate) async fn insert_category_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: &EarnCategoryWrite,
) -> AppResult<u64> {
    match sqlx::query(
        r#"INSERT INTO earn_product_categories
           (code, name_json, sort_order, status)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(&input.code)
    .bind(SqlxJson(input.name_json.clone()))
    .bind(input.sort_order)
    .bind(&input.status)
    .execute(&mut **tx)
    .await
    {
        Ok(result) => Ok(result.last_insert_id()),
        Err(error) if is_duplicate_key_error(&error) => Err(AppError::Conflict(
            "earn product category code already exists".to_owned(),
        )),
        Err(error) => Err(AppError::Database(error)),
    }
}

pub(crate) async fn update_category_in_tx(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
    input: &EarnCategoryWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE earn_product_categories
           SET name_json = ?, sort_order = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(SqlxJson(input.name_json.clone()))
    .bind(input.sort_order)
    .bind(&input.status)
    .bind(category_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_category_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE earn_product_categories SET status = ? WHERE id = ?")
        .bind(status)
        .bind(category_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_category_by_id(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
) -> AppResult<EarnCategoryResponse> {
    sqlx::query_as::<_, EarnCategoryResponse>(
        r#"SELECT id, code, name_json,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(name_json, '$.items[0].title')), code) AS default_name,
                  sort_order, status
           FROM earn_product_categories
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(category_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn lock_category_by_id(
    tx: &mut Transaction<'_, MySql>,
    category_id: u64,
) -> AppResult<EarnCategoryResponse> {
    sqlx::query_as::<_, EarnCategoryResponse>(
        r#"SELECT id, code, name_json,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(name_json, '$.items[0].title')), code) AS default_name,
                  sort_order, status
           FROM earn_product_categories
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(category_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn ensure_asset_exists(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub(crate) async fn ensure_active_category_exists(
    tx: &mut Transaction<'_, MySql>,
    code: &str,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM earn_product_categories WHERE code = ? AND status = 'active' LIMIT 1",
    )
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?;
    if exists.is_none() {
        return Err(AppError::Validation(
            "earn product category must reference an active category".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) async fn insert_product_in_tx(
    tx: &mut Transaction<'_, MySql>,
    input: &EarnProductWrite,
) -> AppResult<u64> {
    let product_id = sqlx::query(
        r#"INSERT INTO earn_products
           (asset_id, name, banner_url, small_logo_url, category, introduction_json, term_days,
            apr_rate, redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
            early_redeem_fee_rate, min_subscribe, max_subscribe, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(input.asset_id)
    .bind(&input.name)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(SqlxJson(input.introduction_json.clone()))
    .bind(input.term_days)
    .bind(&input.apr_rate)
    .bind(&input.redemption_fee_rate)
    .bind(&input.maturity_profit_fee_rate)
    .bind(&input.early_redeem_fee_basis)
    .bind(&input.early_redeem_fee_rate)
    .bind(&input.min_subscribe)
    .bind(&input.max_subscribe)
    .bind(&input.status)
    .execute(&mut **tx)
    .await?
    .last_insert_id();
    Ok(product_id)
}

pub(crate) async fn update_product_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    input: &EarnProductWrite,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE earn_products
           SET asset_id = ?, name = ?, banner_url = ?, small_logo_url = ?, category = ?,
               introduction_json = ?, term_days = ?, apr_rate = ?, redemption_fee_rate = ?,
               maturity_profit_fee_rate = ?, early_redeem_fee_basis = ?,
               early_redeem_fee_rate = ?, min_subscribe = ?, max_subscribe = ?, status = ?
           WHERE id = ?"#,
    )
    .bind(input.asset_id)
    .bind(&input.name)
    .bind(&input.banner_url)
    .bind(&input.small_logo_url)
    .bind(&input.category)
    .bind(SqlxJson(input.introduction_json.clone()))
    .bind(input.term_days)
    .bind(&input.apr_rate)
    .bind(&input.redemption_fee_rate)
    .bind(&input.maturity_profit_fee_rate)
    .bind(&input.early_redeem_fee_basis)
    .bind(&input.early_redeem_fee_rate)
    .bind(&input.min_subscribe)
    .bind(&input.max_subscribe)
    .bind(&input.status)
    .bind(product_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn update_product_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    status: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE earn_products SET status = ? WHERE id = ?")
        .bind(status)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn load_product_by_id(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    sqlx::query_as::<_, EarnProductResponse>(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.banner_url, products.small_logo_url,
                  products.category,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(categories.name_json, '$.items[0].title')), products.category) AS category_name,
                  categories.name_json AS category_name_json,
                  products.introduction_json,
                  products.term_days, products.apr_rate, products.redemption_fee_rate,
                  products.maturity_profit_fee_rate, products.early_redeem_fee_basis,
                  products.early_redeem_fee_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category
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
) -> AppResult<EarnProductResponse> {
    sqlx::query_as::<_, EarnProductResponse>(
        r#"SELECT products.id, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.banner_url, products.small_logo_url,
                  products.category,
                  COALESCE(JSON_UNQUOTE(JSON_EXTRACT(categories.name_json, '$.items[0].title')), products.category) AS category_name,
                  categories.name_json AS category_name_json,
                  products.introduction_json,
                  products.term_days, products.apr_rate, products.redemption_fee_rate,
                  products.maturity_profit_fee_rate, products.early_redeem_fee_basis,
                  products.early_redeem_fee_rate,
                  products.min_subscribe, products.max_subscribe, products.status
           FROM earn_products products
           INNER JOIN assets ON assets.id = products.asset_id
           LEFT JOIN earn_product_categories categories ON categories.code = products.category
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn load_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(subscription_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn existing_subscription_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
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

pub(crate) async fn existing_subscription_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub(crate) async fn lock_active_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<EarnProductRuleRow> {
    let product = sqlx::query_as::<_, EarnProductRuleRow>(
        r#"SELECT id, asset_id, term_days, apr_rate, redemption_fee_rate,
                  maturity_profit_fee_rate, early_redeem_fee_basis, early_redeem_fee_rate,
                  min_subscribe, max_subscribe, status
           FROM earn_products
           WHERE id = ?
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

pub(crate) async fn insert_subscription_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product: &EarnProductRuleRow,
    amount: &BigDecimal,
    idempotency_key: &str,
    matures_at: chrono::DateTime<chrono::Utc>,
) -> AppResult<Option<u64>> {
    match sqlx::query(
        r#"INSERT INTO earn_subscriptions
           (user_id, product_id, asset_id, amount, apr_rate, redemption_fee_rate,
            maturity_profit_fee_rate, early_redeem_fee_basis, early_redeem_fee_rate,
            term_days, status, idempotency_key, matures_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'subscribed', ?, ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.asset_id)
    .bind(amount)
    .bind(&product.apr_rate)
    .bind(&product.redemption_fee_rate)
    .bind(&product.maturity_profit_fee_rate)
    .bind(&product.early_redeem_fee_basis)
    .bind(&product.early_redeem_fee_rate)
    .bind(product.term_days)
    .bind(idempotency_key)
    .bind(matures_at)
    .execute(&mut **tx)
    .await
    {
        Ok(result) => Ok(Some(result.last_insert_id())),
        Err(error) if is_duplicate_key_error(&error) => Ok(None),
        Err(error) => Err(AppError::Database(error)),
    }
}

pub(crate) async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<EarnWalletRow> {
    sqlx::query_as::<_, EarnWalletRow>(
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
    .ok_or_else(|| AppError::Validation("wallet account is required for earn".to_owned()))
}

pub(crate) async fn debit_wallet_for_subscription_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    wallet: &EarnWalletRow,
    subscription_id: u64,
) -> AppResult<()> {
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_subscribe', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(-amount.clone())
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn lock_subscription_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    sqlx::query_as::<_, EarnSubscriptionResponse>(
        r#"SELECT id, user_id, product_id, asset_id, amount, apr_rate,
                  redemption_fee_rate, maturity_profit_fee_rate, early_redeem_fee_basis,
                  early_redeem_fee_rate, term_days, status, idempotency_key,
                  subscribed_at, matures_at, redeemed_at
           FROM earn_subscriptions
           WHERE id = ? AND user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(subscription_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn credit_wallet_for_redemption_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription: &EarnSubscriptionResponse,
    wallet: &EarnWalletRow,
    redeem_amount: &BigDecimal,
) -> AppResult<()> {
    let available_after = wallet.available.clone() + redeem_amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(subscription.user_id)
        .bind(subscription.asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, 'earn_redeem', ?, 'available', ?, ?, ?, ?, 'earn_subscription', ?)"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(redeem_amount)
    .bind(&available_after)
    .bind(&available_after)
    .bind(&wallet.frozen)
    .bind(&wallet.locked)
    .bind(subscription.id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn mark_subscription_redeemed_in_tx(
    tx: &mut Transaction<'_, MySql>,
    subscription_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE earn_subscriptions SET status = 'redeemed', redeemed_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(subscription_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn load_redeemed_amounts_from_ledger(
    tx: &mut Transaction<'_, MySql>,
    subscription: &EarnSubscriptionResponse,
) -> AppResult<(BigDecimal, BigDecimal, BigDecimal)> {
    let ref_id = subscription.id.to_string();
    let principal_amount = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT -amount
           FROM wallet_ledger
           WHERE user_id = ?
             AND asset_id = ?
             AND change_type = 'earn_subscribe'
             AND ref_type = 'earn_subscription'
             AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&ref_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("earn subscribe ledger is missing".to_owned()))?;

    let redeem_amount = sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT amount
           FROM wallet_ledger
           WHERE user_id = ?
             AND asset_id = ?
             AND change_type = 'earn_redeem'
             AND ref_type = 'earn_subscription'
             AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(subscription.user_id)
    .bind(subscription.asset_id)
    .bind(&ref_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Internal("earn redeem ledger is missing".to_owned()))?;

    let yield_amount = redeem_amount.clone() - principal_amount.clone();
    Ok((principal_amount, yield_amount, redeem_amount))
}

pub(crate) async fn insert_admin_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    let Some(database_error) = error.as_database_error() else {
        return false;
    };
    matches!(database_error.code().as_deref(), Some("1062"))
        || database_error.message().contains("Duplicate entry")
}
