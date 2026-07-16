//! earn bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    modules::{
        earn::{
            infrastructure,
            presentation::{
                AdminCategoriesQuery, AdminSubscriptionsQuery, CreateEarnCategoryRequest,
                CreateEarnProductRequest, EarnCategoriesResponse, EarnCategoryResponse,
                EarnProductResponse, EarnProductsResponse, EarnSubscriptionResponse,
                EarnSubscriptionsResponse, ListQuery, RedeemEarnResponse, SubscribeEarnRequest,
                SubscribeEarnResponse, UpdateEarnCategoryRequest, UpdateEarnCategoryStatusRequest,
                UpdateEarnProductRequest, UpdateEarnProductStatusRequest,
            },
            repository::{EarnCategoryWrite, EarnProductWrite},
            service::{
                admin_id_from_subject, category_audit_json, earn_matures_at,
                ensure_existing_subscription_matches_request, normalize_idempotency_key,
                normalized_category_name_json, normalized_category_status,
                normalized_introduction_json, normalized_product_category,
                normalized_product_status, normalized_required_category_code, optional_image_url,
                optional_string, product_audit_json, product_fee_config_from_create_request,
                product_fee_config_from_update_request, redemption_amounts_for_subscription,
                required_reason, route_limit, user_id_from_subject, validate_amount,
                validate_create_product_request, validate_product_amount,
                validate_update_product_request,
            },
        },
        events::{EventBroadcastHub, EventBroadcastMessage},
    },
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde_json::json;
use sqlx::{MySql, Pool};

#[derive(Debug)]
pub struct ApplicationLayerMarker;

impl ApplicationLayer for ApplicationLayerMarker {}

pub(crate) async fn list_active_earn_products(
    pool: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<EarnProductsResponse> {
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_products(&pool, Some("active"), route_limit(query.limit)).await
}

pub(crate) async fn list_admin_earn_products(
    pool: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<EarnProductsResponse> {
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_products(&pool, None, route_limit(query.limit)).await
}

pub(crate) async fn get_admin_earn_product(
    pool: Option<Pool<MySql>>,
    product_id: u64,
) -> AppResult<EarnProductResponse> {
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let product = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    tx.commit().await?;
    Ok(product)
}

pub(crate) async fn list_earn_subscriptions(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<EarnSubscriptionsResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_user_subscriptions(&pool, user_id, route_limit(query.limit)).await
}

pub(crate) async fn list_admin_earn_subscriptions(
    pool: Option<Pool<MySql>>,
    query: AdminSubscriptionsQuery,
) -> AppResult<EarnSubscriptionsResponse> {
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_admin_subscriptions(
        &pool,
        route_limit(query.limit),
        query.user_id,
        optional_string(query.email),
        optional_string(query.status),
    )
    .await
}

pub(crate) async fn get_admin_earn_subscription(
    pool: Option<Pool<MySql>>,
    subscription_id: u64,
) -> AppResult<EarnSubscriptionResponse> {
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let subscription = infrastructure::load_subscription_by_id(&mut tx, subscription_id).await?;
    tx.commit().await?;
    Ok(subscription)
}

pub(crate) async fn list_admin_earn_categories(
    pool: Option<Pool<MySql>>,
    query: AdminCategoriesQuery,
) -> AppResult<EarnCategoriesResponse> {
    let pool = earn_mysql_pool(pool)?;
    infrastructure::list_admin_categories(
        &pool,
        route_limit(query.limit),
        optional_string(query.status),
    )
    .await
}

pub(crate) async fn get_admin_earn_category(
    pool: Option<Pool<MySql>>,
    category_id: u64,
) -> AppResult<EarnCategoryResponse> {
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let category = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    tx.commit().await?;
    Ok(category)
}

pub(crate) async fn create_earn_category(
    pool: Option<Pool<MySql>>,
    subject: &str,
    request: CreateEarnCategoryRequest,
) -> AppResult<EarnCategoryResponse> {
    let code = normalized_required_category_code(&request.code)?;
    let status = normalized_category_status(request.status.as_deref().unwrap_or("active"))?;
    let name_json = normalized_category_name_json(request.name_json, &code)?;
    let sort_order = request.sort_order.unwrap_or(0);
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let category_id = infrastructure::insert_category_in_tx(
        &mut tx,
        &EarnCategoryWrite {
            code,
            name_json,
            sort_order,
            status,
        },
    )
    .await?;
    let category = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_category.create",
        "earn_category",
        category.id,
        None,
        Some(category_audit_json(&category)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(category)
}

pub(crate) async fn update_earn_category(
    pool: Option<Pool<MySql>>,
    subject: &str,
    category_id: u64,
    request: UpdateEarnCategoryRequest,
) -> AppResult<EarnCategoryResponse> {
    let status = normalized_category_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_category_by_id(&mut tx, category_id).await?;
    let name_json = normalized_category_name_json(request.name_json, &before.code)?;
    infrastructure::update_category_in_tx(
        &mut tx,
        category_id,
        &EarnCategoryWrite {
            code: before.code.clone(),
            name_json,
            sort_order: request.sort_order,
            status,
        },
    )
    .await?;
    let after = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_category.update",
        "earn_category",
        category_id,
        Some(category_audit_json(&before)),
        Some(category_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_earn_category_status(
    pool: Option<Pool<MySql>>,
    subject: &str,
    category_id: u64,
    request: UpdateEarnCategoryStatusRequest,
) -> AppResult<EarnCategoryResponse> {
    let status = normalized_category_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_category_by_id(&mut tx, category_id).await?;
    infrastructure::update_category_status_in_tx(&mut tx, category_id, &status).await?;
    let after = infrastructure::load_category_by_id(&mut tx, category_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_category.update_status",
        "earn_category",
        category_id,
        Some(category_audit_json(&before)),
        Some(category_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn create_earn_product(
    pool: Option<Pool<MySql>>,
    subject: &str,
    request: CreateEarnProductRequest,
) -> AppResult<EarnProductResponse> {
    validate_create_product_request(&request)?;
    let fee_config = product_fee_config_from_create_request(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let category = normalized_product_category(request.category.as_deref())?;
    let banner_url = optional_image_url(request.banner_url, "earn product banner_url")?;
    let small_logo_url = optional_image_url(request.small_logo_url, "earn product small_logo_url")?;
    let name = request.name.trim().to_owned();
    let introduction_json = normalized_introduction_json(request.introduction_json, &name)?;
    let write = EarnProductWrite {
        asset_id: request.asset_id,
        name,
        banner_url,
        small_logo_url,
        category,
        introduction_json,
        term_days: request.term_days,
        apr_rate: request.apr_rate,
        redemption_fee_rate: fee_config.redemption_fee_rate,
        maturity_profit_fee_rate: fee_config.maturity_profit_fee_rate,
        early_redeem_fee_basis: fee_config.early_redeem_fee_basis,
        early_redeem_fee_rate: fee_config.early_redeem_fee_rate,
        min_subscribe: request.min_subscribe,
        max_subscribe: request.max_subscribe,
        status,
    };
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    infrastructure::ensure_asset_exists(&mut tx, write.asset_id).await?;
    infrastructure::ensure_active_category_exists(&mut tx, &write.category).await?;
    let product_id = infrastructure::insert_product_in_tx(&mut tx, &write).await?;
    let product = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.create",
        "earn_product",
        product.id,
        None,
        Some(product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(product)
}

pub(crate) async fn update_earn_product(
    pool: Option<Pool<MySql>>,
    subject: &str,
    product_id: u64,
    request: UpdateEarnProductRequest,
) -> AppResult<EarnProductResponse> {
    validate_update_product_request(&request)?;
    let fee_config = product_fee_config_from_update_request(&request)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let status = normalized_product_status(&request.status)?;
    let category = normalized_product_category(request.category.as_deref())?;
    let banner_url = optional_image_url(request.banner_url, "earn product banner_url")?;
    let small_logo_url = optional_image_url(request.small_logo_url, "earn product small_logo_url")?;
    let name = request.name.trim().to_owned();
    let introduction_json = normalized_introduction_json(request.introduction_json, &name)?;
    let write = EarnProductWrite {
        asset_id: request.asset_id,
        name,
        banner_url,
        small_logo_url,
        category,
        introduction_json,
        term_days: request.term_days,
        apr_rate: request.apr_rate,
        redemption_fee_rate: fee_config.redemption_fee_rate,
        maturity_profit_fee_rate: fee_config.maturity_profit_fee_rate,
        early_redeem_fee_basis: fee_config.early_redeem_fee_basis,
        early_redeem_fee_rate: fee_config.early_redeem_fee_rate,
        min_subscribe: request.min_subscribe,
        max_subscribe: request.max_subscribe,
        status,
    };
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::ensure_asset_exists(&mut tx, write.asset_id).await?;
    infrastructure::ensure_active_category_exists(&mut tx, &write.category).await?;
    infrastructure::update_product_in_tx(&mut tx, product_id, &write).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.update",
        "earn_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_earn_product_status(
    pool: Option<Pool<MySql>>,
    subject: &str,
    product_id: u64,
    request: UpdateEarnProductStatusRequest,
) -> AppResult<EarnProductResponse> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::update_product_status_in_tx(&mut tx, product_id, &status).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "earn_product.update_status",
        "earn_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn subscribe_earn_product_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    request: SubscribeEarnRequest,
) -> AppResult<SubscribeEarnResponse> {
    // 应用层负责提交订阅后事件的统一编排，路由层只负责请求参数与鉴权。
    let user_id = user_id_from_subject(subject)?;
    let (response, is_new_subscription) =
        subscribe_earn_product_with_internal(pool, subject, request).await?;
    if is_new_subscription && let Some(hub) = event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "earn.subscription.created",
                "subscription_id": response.subscription.id,
                "product_id": response.subscription.product_id,
                "asset_id": response.subscription.asset_id,
                "amount": response.subscription.amount,
                "status": response.subscription.status,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

async fn subscribe_earn_product_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    request: SubscribeEarnRequest,
) -> AppResult<(SubscribeEarnResponse, bool)> {
    let user_id = user_id_from_subject(subject)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    validate_amount(&request.amount)?;
    let pool = earn_mysql_pool(pool)?;
    let (subscription, is_new_subscription) = subscribe_in_tx(
        &pool,
        user_id,
        request.product_id,
        request.amount,
        idempotency_key,
    )
    .await?;
    Ok((SubscribeEarnResponse { subscription }, is_new_subscription))
}

pub(crate) async fn redeem_earn_subscription_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    subscription_id: u64,
) -> AppResult<RedeemEarnResponse> {
    // 应用层负责赎回成功后的事件推送，路由层只透传上下文与 idempotency 结果。
    let user_id = user_id_from_subject(subject)?;
    let (response, is_new_redemption) =
        redeem_earn_subscription_with_internal(pool, subject, subscription_id).await?;
    if is_new_redemption && let Some(hub) = event_broadcast_hub {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "earn.subscription.redeemed",
                "subscription_id": response.subscription.id,
                "product_id": response.subscription.product_id,
                "asset_id": response.subscription.asset_id,
                "principal_amount": response.principal_amount,
                "gross_yield_amount": response.gross_yield_amount,
                "yield_amount": response.yield_amount,
                "redemption_fee_amount": response.redemption_fee_amount,
                "maturity_profit_fee_amount": response.maturity_profit_fee_amount,
                "early_redeem_fee_amount": response.early_redeem_fee_amount,
                "fee_amount": response.fee_amount,
                "redeem_amount": response.redeem_amount,
                "status": response.subscription.status,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

async fn redeem_earn_subscription_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    subscription_id: u64,
) -> AppResult<(RedeemEarnResponse, bool)> {
    let user_id = user_id_from_subject(subject)?;
    let pool = earn_mysql_pool(pool)?;
    let (response, is_new_redemption) =
        redeem_subscription_in_tx(&pool, user_id, subscription_id).await?;
    Ok((response, is_new_redemption))
}

async fn subscribe_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: BigDecimal,
    idempotency_key: String,
) -> AppResult<(EarnSubscriptionResponse, bool)> {
    if let Some(existing) = infrastructure::existing_subscription_for_idempotency_key_readonly(
        pool,
        user_id,
        &idempotency_key,
    )
    .await?
    {
        ensure_existing_subscription_matches_request(&existing, product_id, &amount)?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match infrastructure::lock_active_product(&mut tx, product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_subscription_if_present(
                pool,
                user_id,
                product_id,
                &amount,
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
    validate_product_amount(&amount, &product)?;
    let matures_at = earn_matures_at(product.term_days)?;
    let Some(subscription_id) = infrastructure::insert_subscription_in_tx(
        &mut tx,
        user_id,
        &product,
        &amount,
        &idempotency_key,
        matures_at,
    )
    .await?
    else {
        tx.rollback().await?;
        return replay_existing_subscription(pool, user_id, product_id, &amount, &idempotency_key)
            .await
            .map(|subscription| (subscription, false));
    };

    let wallet = infrastructure::lock_wallet_row(&mut tx, user_id, product.asset_id).await?;
    if wallet.available < amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for earn subscription: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    infrastructure::debit_wallet_for_subscription_in_tx(
        &mut tx,
        user_id,
        product.asset_id,
        &amount,
        &wallet,
        subscription_id,
    )
    .await?;

    let subscription = infrastructure::load_subscription_by_id(&mut tx, subscription_id).await?;
    tx.commit().await?;
    Ok((subscription, true))
}

async fn redeem_subscription_in_tx(
    pool: &Pool<MySql>,
    user_id: u64,
    subscription_id: u64,
) -> AppResult<(RedeemEarnResponse, bool)> {
    let mut tx = pool.begin().await?;
    let subscription =
        infrastructure::lock_subscription_by_id(&mut tx, user_id, subscription_id).await?;

    if subscription.status == "redeemed" {
        let response = redeemed_response_from_existing_subscription(&mut tx, subscription).await?;
        tx.commit().await?;
        return Ok((response, false));
    }
    if subscription.status != "subscribed" {
        return Err(AppError::Conflict(
            "earn subscription is not redeemable".to_owned(),
        ));
    }

    let now = Utc::now();
    let amounts = redemption_amounts_for_subscription(&subscription, now);
    let wallet =
        infrastructure::lock_wallet_row(&mut tx, subscription.user_id, subscription.asset_id)
            .await?;
    infrastructure::credit_wallet_for_redemption_in_tx(
        &mut tx,
        &subscription,
        &wallet,
        &amounts.redeem_amount,
    )
    .await?;
    infrastructure::mark_subscription_redeemed_in_tx(&mut tx, subscription.id).await?;
    let redeemed_subscription =
        infrastructure::load_subscription_by_id(&mut tx, subscription.id).await?;
    tx.commit().await?;
    Ok((
        RedeemEarnResponse {
            subscription: redeemed_subscription,
            principal_amount: amounts.principal_amount,
            gross_yield_amount: amounts.gross_yield_amount,
            yield_amount: amounts.yield_amount,
            redemption_fee_amount: amounts.redemption_fee_amount,
            maturity_profit_fee_amount: amounts.maturity_profit_fee_amount,
            early_redeem_fee_amount: amounts.early_redeem_fee_amount,
            fee_amount: amounts.fee_amount,
            redeem_amount: amounts.redeem_amount,
        },
        true,
    ))
}

async fn replay_existing_subscription(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<EarnSubscriptionResponse> {
    replay_existing_subscription_if_present(pool, user_id, product_id, amount, idempotency_key)
        .await?
        .ok_or_else(|| AppError::Conflict("earn idempotency key is being committed".to_owned()))
}

async fn replay_existing_subscription_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<EarnSubscriptionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) = infrastructure::existing_subscription_for_idempotency_key(
        &mut tx,
        user_id,
        idempotency_key,
    )
    .await?
    else {
        return Ok(None);
    };
    ensure_existing_subscription_matches_request(&existing, product_id, amount)?;
    tx.commit().await?;
    Ok(Some(existing))
}

async fn redeemed_response_from_existing_subscription(
    tx: &mut sqlx::Transaction<'_, MySql>,
    subscription: EarnSubscriptionResponse,
) -> AppResult<RedeemEarnResponse> {
    let (principal_amount, yield_amount, redeem_amount) =
        infrastructure::load_redeemed_amounts_from_ledger(tx, &subscription).await?;
    let redeemed_at = subscription.redeemed_at.unwrap_or_else(Utc::now);
    let amounts = redemption_amounts_for_subscription(&subscription, redeemed_at);
    Ok(RedeemEarnResponse {
        subscription,
        principal_amount,
        gross_yield_amount: amounts.gross_yield_amount,
        yield_amount,
        redemption_fee_amount: amounts.redemption_fee_amount,
        maturity_profit_fee_amount: amounts.maturity_profit_fee_amount,
        early_redeem_fee_amount: amounts.early_redeem_fee_amount,
        fee_amount: amounts.fee_amount,
        redeem_amount,
    })
}

fn earn_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for earn routes".to_owned())
    })
}
