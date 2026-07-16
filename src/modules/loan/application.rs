//! loan bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。

use super::{
    LOAN_TYPE_COLLATERALIZED, STATUS_ACTIVE, STATUS_CANCELLED, STATUS_DISBURSED, STATUS_PENDING,
    STATUS_REJECTED, STATUS_REPAID, ensure_amount_precision, ensure_amount_within_product_limits,
    ensure_non_negative_amount, ensure_positive_amount, normalized_product_name_json,
    optional_string, product_default_name, route_limit, validate_idempotency_key,
    validate_interest_mode, validate_loan_type, validate_product_status,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        loan::{
            infrastructure::{
                AdminLoanOrdersFilter, LoanOrderCreate, LoanProductWrite, apply_loan_wallet_credit,
                apply_loan_wallet_debit, apply_loan_wallet_freeze, ensure_loan_user_kyc_level,
                insert_loan_order_in_tx, insert_loan_product, is_duplicate_key_error,
                list_admin_loan_orders, list_loan_products, list_user_loan_orders,
                load_active_asset_meta, load_active_asset_meta_in_tx,
                load_loan_order_by_idempotency, load_loan_order_response,
                load_loan_product_response, load_user_loan_order_response,
                lock_active_loan_product_terms, lock_loan_order, lock_user_loan_order,
                mark_loan_order_cancelled_in_tx, mark_loan_order_disbursed_in_tx,
                mark_loan_order_rejected_in_tx, mark_loan_order_repaid_in_tx,
                release_loan_collateral_if_needed, update_loan_product, update_loan_product_status,
            },
            presentation::{
                AdminLoanOrdersQuery, CreateLoanOrderRequest, CreateLoanProductRequest, ListQuery,
                LoanOrderResponse, LoanOrdersResponse, LoanProductResponse, LoanProductsResponse,
                UpdateLoanProductRequest, UserLoanOrdersQuery,
            },
            service::calculate_interest_amount,
        },
        wallet::truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool};

pub(crate) async fn list_active_products_use_case(
    pool: &Pool<MySql>,
    query: ListQuery,
) -> AppResult<LoanProductsResponse> {
    // 查询公开可见的进行中产品列表，并做统一分页限制。
    let products = list_loan_products(pool, Some(STATUS_ACTIVE), route_limit(query.limit)).await?;
    Ok(LoanProductsResponse { products })
}

pub(crate) async fn list_admin_products_use_case(
    pool: &Pool<MySql>,
    query: ListQuery,
) -> AppResult<LoanProductsResponse> {
    // 查询后台可见的全部产品清单，使用统一分页策略。
    let products = list_loan_products(pool, None, route_limit(query.limit)).await?;
    Ok(LoanProductsResponse { products })
}

pub(crate) async fn get_admin_product_use_case(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<LoanProductResponse> {
    // 查询单个贷款产品详情，找不到时返回 NotFound。
    load_loan_product_response(pool, product_id).await
}

pub(crate) async fn list_user_orders_use_case(
    pool: &Pool<MySql>,
    user_id: u64,
    query: UserLoanOrdersQuery,
) -> AppResult<LoanOrdersResponse> {
    // 按用户聚合查询订单，支持状态过滤和分页限制。
    let orders =
        list_user_loan_orders(pool, user_id, query.status, route_limit(query.limit)).await?;
    Ok(LoanOrdersResponse { orders })
}

pub(crate) async fn get_user_order_use_case(
    pool: &Pool<MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    // 查询某用户的订单详情，确保订单归属校验在 SQL 层过滤中完成。
    load_user_loan_order_response(pool, user_id, order_id).await
}

pub(crate) async fn list_admin_orders_use_case(
    pool: &Pool<MySql>,
    query: AdminLoanOrdersQuery,
) -> AppResult<LoanOrdersResponse> {
    // 在后台列表里组装筛选条件，复用统一分页和基础设施查询。
    let orders = list_admin_loan_orders(
        pool,
        AdminLoanOrdersFilter {
            limit: route_limit(query.limit),
            user_id: query.user_id,
            email: query.email,
            product_id: query.product_id,
            loan_type: query.loan_type,
            status: query.status,
        },
    )
    .await?;
    Ok(LoanOrdersResponse { orders })
}

pub(crate) async fn get_admin_order_use_case(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    // 查询后台订单详情。
    load_loan_order_response(pool, order_id).await
}

pub(crate) async fn create_loan_product_use_case(
    pool: &Pool<MySql>,
    request: CreateLoanProductRequest,
) -> AppResult<LoanProductResponse> {
    let request = validate_create_product_request(pool, request).await?;
    let product_id = insert_loan_product(pool, request.into_write()).await?;
    load_loan_product_response(pool, product_id).await
}

pub(crate) async fn update_loan_product_use_case(
    pool: &Pool<MySql>,
    product_id: u64,
    request: UpdateLoanProductRequest,
) -> AppResult<LoanProductResponse> {
    let request = validate_update_product_request(pool, request).await?;
    update_loan_product(pool, product_id, request.into_write()).await?;
    load_loan_product_response(pool, product_id).await
}

pub(crate) async fn update_loan_product_status_use_case(
    pool: &Pool<MySql>,
    product_id: u64,
    status: String,
) -> AppResult<LoanProductResponse> {
    let status = validate_product_status(&status)?;
    update_loan_product_status(pool, product_id, &status).await?;
    load_loan_product_response(pool, product_id).await
}

pub(crate) async fn create_loan_order_use_case(
    pool: &Pool<MySql>,
    user_id: u64,
    request: CreateLoanOrderRequest,
) -> AppResult<(LoanOrderResponse, bool)> {
    let idempotency_key = validate_idempotency_key(request.idempotency_key)?;
    let amount = request.amount;
    ensure_positive_amount(&amount, "amount")?;

    let mut tx = pool.begin().await?;
    let product = lock_active_loan_product_terms(&mut tx, request.product_id).await?;
    let asset = load_active_asset_meta_in_tx(&mut tx, product.asset_id).await?;
    ensure_amount_precision(&amount, asset.precision_scale, "amount")?;
    ensure_amount_within_product_limits(&amount, &product.min_amount, &product.max_amount)?;
    ensure_loan_user_kyc_level(&mut tx, user_id, product.min_kyc_level).await?;

    let (collateral_asset_id, collateral_amount) = if product.loan_type == LOAN_TYPE_COLLATERALIZED
    {
        let collateral_asset_id = request.collateral_asset_id.ok_or_else(|| {
            AppError::Validation(
                "collateral_asset_id is required for collateralized loan".to_owned(),
            )
        })?;
        let collateral_amount = request.collateral_amount.ok_or_else(|| {
            AppError::Validation("collateral_amount is required for collateralized loan".to_owned())
        })?;
        ensure_positive_amount(&collateral_amount, "collateral_amount")?;
        let collateral_asset = load_active_asset_meta_in_tx(&mut tx, collateral_asset_id).await?;
        ensure_amount_precision(
            &collateral_amount,
            collateral_asset.precision_scale,
            "collateral_amount",
        )?;
        (Some(collateral_asset_id), Some(collateral_amount))
    } else {
        (None, None)
    };

    let insert = insert_loan_order_in_tx(
        &mut tx,
        LoanOrderCreate {
            user_id,
            product_id: product.id,
            loan_type: product.loan_type,
            asset_id: product.asset_id,
            amount,
            interest_rate: product.interest_rate,
            interest_calculation_mode: product.interest_calculation_mode,
            term_days: product.term_days,
            min_kyc_level: product.min_kyc_level,
            collateral_asset_id,
            collateral_amount: collateral_amount.clone(),
            idempotency_key: idempotency_key.clone(),
        },
    )
    .await;

    let order_id = match insert {
        Ok(order_id) => order_id,
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            let order = load_loan_order_by_idempotency(pool, user_id, &idempotency_key).await?;
            return Ok((order, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    if let (Some(collateral_asset_id), Some(collateral_amount)) =
        (collateral_asset_id, collateral_amount.as_ref())
    {
        // 抵押冻结必须和订单创建在同一事务中完成，避免出现订单已创建但抵押资产未锁定的风险。
        apply_loan_wallet_freeze(
            &mut tx,
            user_id,
            collateral_asset_id,
            collateral_amount,
            "loan_collateral_freeze",
            order_id,
        )
        .await?;
    }

    tx.commit().await?;
    Ok((load_loan_order_response(pool, order_id).await?, true))
}

async fn validate_create_product_request(
    pool: &Pool<MySql>,
    request: CreateLoanProductRequest,
) -> AppResult<NormalizedLoanProductRequest> {
    normalize_product_request(
        pool,
        request.loan_type,
        request.asset_id,
        request.name,
        request.name_json,
        request.term_days,
        request.interest_rate,
        request.interest_calculation_mode,
        request.min_kyc_level,
        request.min_amount,
        request.max_amount,
        request
            .status
            .unwrap_or_else(|| super::STATUS_ACTIVE.to_owned()),
    )
    .await
}

async fn validate_update_product_request(
    pool: &Pool<MySql>,
    request: UpdateLoanProductRequest,
) -> AppResult<NormalizedLoanProductRequest> {
    normalize_product_request(
        pool,
        request.loan_type,
        request.asset_id,
        request.name,
        request.name_json,
        request.term_days,
        request.interest_rate,
        request.interest_calculation_mode,
        request.min_kyc_level,
        request.min_amount,
        request.max_amount,
        request.status,
    )
    .await
}

struct NormalizedLoanProductRequest {
    loan_type: String,
    asset_id: u64,
    name: String,
    name_json: Value,
    term_days: u32,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    min_kyc_level: i32,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    status: String,
}

impl NormalizedLoanProductRequest {
    fn into_write(self) -> LoanProductWrite {
        LoanProductWrite {
            loan_type: self.loan_type,
            asset_id: self.asset_id,
            name: self.name,
            name_json: self.name_json,
            term_days: self.term_days,
            interest_rate: self.interest_rate,
            interest_calculation_mode: self.interest_calculation_mode,
            min_kyc_level: self.min_kyc_level,
            min_amount: self.min_amount,
            max_amount: self.max_amount,
            status: self.status,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn normalize_product_request(
    pool: &Pool<MySql>,
    loan_type: String,
    asset_id: u64,
    name: String,
    name_json: Option<Value>,
    term_days: u32,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    min_kyc_level: i32,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    status: String,
) -> AppResult<NormalizedLoanProductRequest> {
    let loan_type = validate_loan_type(&loan_type)?;
    let interest_calculation_mode = validate_interest_mode(&interest_calculation_mode)?;
    let status = validate_product_status(&status)?;
    let name = optional_string(Some(name))
        .ok_or_else(|| AppError::Validation("name is required".to_owned()))?;
    let name_json = normalized_product_name_json(name_json, &name)?;
    let name = product_default_name(&name_json).unwrap_or(name);
    if term_days == 0 {
        return Err(AppError::Validation(
            "term_days must be positive".to_owned(),
        ));
    }
    ensure_non_negative_amount(&interest_rate, "interest_rate")?;
    if min_kyc_level < 0 {
        return Err(AppError::Validation(
            "min_kyc_level must be non-negative".to_owned(),
        ));
    }
    ensure_positive_amount(&min_amount, "min_amount")?;
    if let Some(max_amount) = max_amount.as_ref() {
        ensure_positive_amount(max_amount, "max_amount")?;
        if max_amount < &min_amount {
            return Err(AppError::Validation(
                "max_amount must be greater than or equal to min_amount".to_owned(),
            ));
        }
    }
    let asset = load_active_asset_meta(pool, asset_id).await?;
    ensure_amount_precision(&min_amount, asset.precision_scale, "min_amount")?;
    if let Some(max_amount) = max_amount.as_ref() {
        ensure_amount_precision(max_amount, asset.precision_scale, "max_amount")?;
    }

    Ok(NormalizedLoanProductRequest {
        loan_type,
        asset_id,
        name,
        name_json,
        term_days,
        interest_rate,
        interest_calculation_mode,
        min_kyc_level,
        min_amount,
        max_amount,
        status,
    })
}

pub(crate) async fn cancel_loan_order_use_case(
    pool: &Pool<MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<(LoanOrderResponse, bool)> {
    let mut tx = pool.begin().await?;
    let Some(order) = lock_user_loan_order(&mut tx, user_id, order_id).await? else {
        return Err(AppError::NotFound);
    };
    if order.status == STATUS_CANCELLED {
        tx.commit().await?;
        return Ok((load_loan_order_response(pool, order_id).await?, false));
    }
    if order.status != STATUS_PENDING {
        return Err(AppError::Conflict(
            "loan order can only be cancelled while pending".to_owned(),
        ));
    }

    // 取消待审核订单时先释放抵押，再写订单状态，二者必须共享事务。
    release_loan_collateral_if_needed(&mut tx, &order).await?;
    mark_loan_order_cancelled_in_tx(&mut tx, order.id).await?;
    tx.commit().await?;
    Ok((load_loan_order_response(pool, order_id).await?, true))
}

pub(crate) async fn approve_loan_order_use_case(
    pool: &Pool<MySql>,
    admin_id: u64,
    order_id: u64,
) -> AppResult<(LoanOrderResponse, bool)> {
    let mut tx = pool.begin().await?;
    let order = lock_loan_order(&mut tx, order_id).await?;
    if order.status == STATUS_DISBURSED || order.status == STATUS_REPAID {
        tx.commit().await?;
        return Ok((load_loan_order_response(pool, order_id).await?, false));
    }
    if order.status != STATUS_PENDING {
        return Err(AppError::Conflict(
            "loan order is not pending review".to_owned(),
        ));
    }

    let due_at = Utc::now()
        .checked_add_signed(TimeDelta::days(i64::from(order.term_days)))
        .ok_or_else(|| AppError::Validation("loan due_at is outside valid range".to_owned()))?;
    // 放款入账和订单审核状态必须原子提交，避免余额入账后订单仍处于待审核。
    apply_loan_wallet_credit(
        &mut tx,
        order.user_id,
        order.asset_id,
        &order.amount,
        "loan_disbursement",
        order.id,
    )
    .await?;
    mark_loan_order_disbursed_in_tx(&mut tx, order.id, admin_id, due_at.naive_utc()).await?;

    tx.commit().await?;
    Ok((load_loan_order_response(pool, order_id).await?, true))
}

pub(crate) async fn reject_loan_order_use_case(
    pool: &Pool<MySql>,
    admin_id: u64,
    order_id: u64,
    reason: Option<String>,
) -> AppResult<(LoanOrderResponse, bool)> {
    let mut tx = pool.begin().await?;
    let order = lock_loan_order(&mut tx, order_id).await?;
    if order.status == STATUS_REJECTED {
        tx.commit().await?;
        return Ok((load_loan_order_response(pool, order_id).await?, false));
    }
    if order.status != STATUS_PENDING {
        return Err(AppError::Conflict(
            "loan order is not pending review".to_owned(),
        ));
    }

    // 拒绝审核会释放抵押资产，状态更新与钱包解冻必须保持同一事务边界。
    release_loan_collateral_if_needed(&mut tx, &order).await?;
    mark_loan_order_rejected_in_tx(&mut tx, order.id, admin_id, optional_string(reason)).await?;
    tx.commit().await?;
    Ok((load_loan_order_response(pool, order_id).await?, true))
}

pub(crate) async fn repay_loan_order_use_case(
    pool: &Pool<MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<(LoanOrderResponse, bool)> {
    let mut tx = pool.begin().await?;
    let Some(order) = lock_user_loan_order(&mut tx, user_id, order_id).await? else {
        return Err(AppError::NotFound);
    };
    if order.status == STATUS_REPAID {
        tx.commit().await?;
        return Ok((load_loan_order_response(pool, order_id).await?, false));
    }
    if order.status != STATUS_DISBURSED {
        return Err(AppError::Conflict(
            "loan order is not disbursed for repayment".to_owned(),
        ));
    }
    let disbursed_at = order.disbursed_at.ok_or_else(|| {
        AppError::Validation("loan order disbursed_at is required for repayment".to_owned())
    })?;
    let asset = load_active_asset_meta_in_tx(&mut tx, order.asset_id).await?;
    let interest_amount = calculate_interest_amount(
        &order.amount,
        &order.interest_rate,
        &order.interest_calculation_mode,
        order.term_days,
        disbursed_at,
        Utc::now(),
        asset.precision_scale,
    )?;
    let repayment_amount = truncate_amount_to_asset_precision(
        &(order.amount.clone() + interest_amount.clone()),
        asset.precision_scale,
    );

    // 还款扣款、抵押释放、订单结清金额必须原子提交，保证账务和订单状态一致。
    apply_loan_wallet_debit(
        &mut tx,
        order.user_id,
        order.asset_id,
        &repayment_amount,
        "loan_repayment",
        order.id,
    )
    .await?;
    release_loan_collateral_if_needed(&mut tx, &order).await?;
    mark_loan_order_repaid_in_tx(&mut tx, order.id, &interest_amount, &repayment_amount).await?;

    tx.commit().await?;
    Ok((load_loan_order_response(pool, order_id).await?, true))
}
