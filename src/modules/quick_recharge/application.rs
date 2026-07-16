//! quick_recharge bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use super::{
    infrastructure,
    presentation::{
        CreateQuickRechargeOrderRequest, DeleteQuickRechargeOrderRequest,
        QuickRechargeConfigResponse, QuickRechargeOrderResponse, QuickRechargeOrdersQuery,
        QuickRechargeOrdersResponse, SaveQuickRechargeConfigRequest,
        TestQuickRechargeConfigRequest, TestQuickRechargeConfigResponse,
        UserQuickRechargeConfigResponse,
    },
    repository::{
        QuickRechargeAdminOrderFilter, QuickRechargeConfigWrite, QuickRechargeOrderCreateWrite,
        QuickRechargeOrderPaidUpdate, QuickRechargeOrderProviderUpdate, QuickRechargeOrderRow,
        QuickRechargeUserOrderFilter,
    },
    service::{
        admin_id_from_subject, config_audit_json, decimal_to_gmpay_string, optional_json_string,
        optional_str, optional_string, prepare_secret_field, redirect_url_for_target,
        required_json_decimal, required_json_string, required_reason, route_limit,
        runtime_config_from_row, test_config_audit_json, user_id_from_subject,
        validate_enabled_config_secrets, validate_order_status, validate_recharge_amount,
        validate_save_config_request, verify_gmpay_notify_signature,
    },
};
use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    infra::secrets::mask_secret,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{MySql, Pool};
use uuid::Uuid;

#[derive(Debug)]
pub struct ApplicationLayerMarker;

impl ApplicationLayer for ApplicationLayerMarker {}

pub(crate) async fn get_user_quick_recharge_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<UserQuickRechargeConfigResponse> {
    let config = quick_recharge_config_response(
        infrastructure::load_config_row(&quick_recharge_mysql_pool(pool)?).await?,
    );
    Ok(UserQuickRechargeConfigResponse {
        enabled: config.enabled,
        currency: config.currency,
        token: config.token.to_ascii_uppercase(),
        network: config.network,
        min_amount: config.min_amount,
        max_amount: config.max_amount,
    })
}

pub(crate) async fn list_user_quick_recharge_orders(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: QuickRechargeOrdersQuery,
) -> AppResult<QuickRechargeOrdersResponse> {
    let user_id = user_id_from_subject(subject)?;
    let status = optional_string(query.status)
        .map(|status| validate_order_status(&status))
        .transpose()?;
    let filter = QuickRechargeUserOrderFilter {
        user_id,
        status,
        limit: route_limit(query.limit),
    };
    let orders =
        infrastructure::list_user_orders(&quick_recharge_mysql_pool(pool)?, filter).await?;
    Ok(QuickRechargeOrdersResponse {
        orders: quick_recharge_order_responses(orders),
    })
}

pub(crate) async fn get_admin_quick_recharge_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<QuickRechargeConfigResponse> {
    Ok(quick_recharge_config_response(
        infrastructure::load_config_row(&quick_recharge_mysql_pool(pool)?).await?,
    ))
}

pub(crate) async fn list_admin_quick_recharge_orders(
    pool: Option<Pool<MySql>>,
    query: QuickRechargeOrdersQuery,
) -> AppResult<QuickRechargeOrdersResponse> {
    let filter = QuickRechargeAdminOrderFilter {
        user_id: query.user_id,
        email: optional_string(query.email),
        status: optional_string(query.status)
            .map(|status| validate_order_status(&status))
            .transpose()?,
        order_id: optional_string(query.order_id),
        provider_trade_id: optional_string(query.provider_trade_id),
        limit: route_limit(query.limit),
    };
    let orders =
        infrastructure::list_admin_orders(&quick_recharge_mysql_pool(pool)?, filter).await?;
    Ok(QuickRechargeOrdersResponse {
        orders: quick_recharge_order_responses(orders),
    })
}

pub(crate) async fn create_user_quick_recharge_order(
    pool: Option<Pool<MySql>>,
    key: Option<&str>,
    subject: &str,
    request: CreateQuickRechargeOrderRequest,
) -> AppResult<QuickRechargeOrderResponse> {
    let user_id = user_id_from_subject(subject)?;
    let pool = quick_recharge_mysql_pool(pool)?;
    let runtime = load_runtime_config(&pool, key, true).await?;
    validate_recharge_amount(&request.amount, &runtime)?;
    let asset =
        infrastructure::load_active_asset_by_symbol(&pool, &runtime.token.to_ascii_uppercase())
            .await?;
    let user_email = infrastructure::load_user_email(&pool, user_id).await?;
    let order_id = Uuid::now_v7().simple().to_string();
    let return_target = request.return_target;
    let return_target_value = return_target.map(|target| target.as_str().to_owned());
    let redirect_url = redirect_url_for_target(&runtime, return_target);

    infrastructure::insert_created_order(
        &pool,
        &QuickRechargeOrderCreateWrite {
            order_id: order_id.clone(),
            user_id,
            user_email,
            asset_id: asset.id,
            asset_symbol: asset.symbol,
            currency: runtime.currency.clone(),
            token: runtime.token.clone(),
            network: runtime.network.clone(),
            fiat_amount: request.amount.clone(),
            return_target: return_target_value,
            redirect_url: redirect_url.clone(),
        },
    )
    .await?;

    let provider_result = infrastructure::create_gmpay_order(
        &runtime,
        &order_id,
        &request.amount,
        redirect_url.as_deref(),
    )
    .await;
    match provider_result {
        Ok(provider_order) => {
            if provider_order.order_id != order_id {
                infrastructure::mark_order_failed(&pool, &order_id).await?;
                return Err(AppError::Internal(
                    "gmpay returned an unexpected order_id".to_owned(),
                ));
            }
            if provider_order.amount != request.amount {
                infrastructure::mark_order_failed(&pool, &order_id).await?;
                return Err(AppError::Internal(
                    "gmpay returned an unexpected amount".to_owned(),
                ));
            }
            infrastructure::mark_order_pending_with_provider(
                &pool,
                &QuickRechargeOrderProviderUpdate {
                    order_id: order_id.clone(),
                    provider_trade_id: provider_order.trade_id,
                    actual_amount: provider_order.actual_amount,
                    receive_address: provider_order.receive_address,
                    payment_url: provider_order.payment_url,
                    expiration_time: provider_order.expiration_time,
                    currency: provider_order.currency,
                    token: provider_order.token,
                },
            )
            .await?;
        }
        Err(error) => {
            infrastructure::mark_order_failed(&pool, &order_id).await?;
            return Err(error);
        }
    }

    Ok(infrastructure::load_order_by_order_id(&pool, &order_id)
        .await?
        .into())
}

pub(crate) async fn save_admin_quick_recharge_config(
    pool: Option<Pool<MySql>>,
    key: Option<&str>,
    subject: &str,
    request: SaveQuickRechargeConfigRequest,
) -> AppResult<QuickRechargeConfigResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    let pool = quick_recharge_mysql_pool(pool)?;
    let reason = required_reason(request.reason.clone())?;
    let validated = validate_save_config_request(&request)?;
    let mut tx = pool.begin().await?;
    let before = infrastructure::lock_config_in_tx(&mut tx).await?;
    let secret_ciphertext = prepare_secret_field(
        request.merchant_secret.as_deref(),
        before
            .as_ref()
            .and_then(|row| row.merchant_secret_ciphertext.clone()),
        key,
    )?;
    let secret_mask = request
        .merchant_secret
        .as_deref()
        .and_then(optional_str)
        .map(mask_secret)
        .or_else(|| {
            before
                .as_ref()
                .and_then(|row| row.merchant_secret_mask.clone())
        });
    validate_enabled_config_secrets(&validated, &secret_ciphertext)?;

    infrastructure::upsert_config(
        &mut tx,
        &QuickRechargeConfigWrite {
            enabled: validated.enabled,
            api_base_url: validated.api_base_url.clone(),
            merchant_pid: validated.merchant_pid.clone(),
            merchant_secret_ciphertext: secret_ciphertext.clone(),
            merchant_secret_mask: secret_mask.clone(),
            currency: validated.currency.clone(),
            token: validated.token.clone(),
            network: validated.network.clone(),
            notify_url: validated.notify_url.clone(),
            redirect_url: validated.redirect_url.clone(),
            pc_app_redirect_url: validated.pc_app_redirect_url.clone(),
            mac_app_redirect_url: validated.mac_app_redirect_url.clone(),
            ios_app_redirect_url: validated.ios_app_redirect_url.clone(),
            android_app_redirect_url: validated.android_app_redirect_url.clone(),
            mobile_web_redirect_url: validated.mobile_web_redirect_url.clone(),
            desktop_web_redirect_url: validated.desktop_web_redirect_url.clone(),
            min_amount: validated.min_amount.clone(),
            max_amount: validated.max_amount.clone(),
            updated_by: admin_id,
        },
    )
    .await?;

    let after = infrastructure::load_config_row_in_tx(&mut tx).await?;
    // 配置修改与后台审计同事务提交，避免支付参数生效但缺少操作追踪。
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "quick_recharge_config.save",
        "quick_recharge_config",
        after.id,
        before.as_ref().map(config_audit_json),
        Some(config_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(quick_recharge_config_response(after))
}

pub(crate) async fn test_admin_quick_recharge_config(
    pool: Option<Pool<MySql>>,
    key: Option<&str>,
    subject: &str,
    request: TestQuickRechargeConfigRequest,
) -> AppResult<TestQuickRechargeConfigResponse> {
    let admin_id = admin_id_from_subject(subject)?;
    let pool = quick_recharge_mysql_pool(pool)?;
    let reason = required_reason(request.reason.clone())?;
    let row = infrastructure::load_config_row(&pool).await?;
    let runtime = runtime_config_from_row(row.clone(), key, false)?;
    validate_recharge_amount(&request.amount, &runtime)?;

    let order_id = Uuid::now_v7().simple().to_string();
    let provider_order = infrastructure::create_gmpay_order_with_name(
        &runtime,
        &order_id,
        &request.amount,
        "Admin Quick Recharge Test",
        None,
    )
    .await?;
    if provider_order.order_id != order_id {
        return Err(AppError::Internal(
            "gmpay returned an unexpected order_id for quick recharge test".to_owned(),
        ));
    }
    if provider_order.amount != request.amount {
        return Err(AppError::Internal(
            "gmpay returned an unexpected amount for quick recharge test".to_owned(),
        ));
    }

    let response = TestQuickRechargeConfigResponse {
        order_id,
        provider_trade_id: provider_order.trade_id,
        currency: provider_order.currency.to_ascii_lowercase(),
        token: provider_order.token.to_ascii_lowercase(),
        network: runtime.network,
        fiat_amount: provider_order.amount,
        actual_amount: provider_order.actual_amount,
        receive_address: provider_order.receive_address,
        payment_url: provider_order.payment_url,
        expiration_time: provider_order.expiration_time,
        tested_at: Utc::now().timestamp_millis(),
    };

    let mut tx = pool.begin().await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "quick_recharge_config.test",
        "quick_recharge_config",
        row.id,
        Some(config_audit_json(&row)),
        Some(test_config_audit_json(&response)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;

    Ok(response)
}

pub(crate) async fn delete_admin_quick_recharge_order(
    pool: Option<Pool<MySql>>,
    subject: &str,
    order_id: &str,
    request: DeleteQuickRechargeOrderRequest,
) -> AppResult<()> {
    let reason = required_reason(request.reason)?;
    let admin_id = admin_id_from_subject(subject)?;
    let pool = quick_recharge_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let order = infrastructure::lock_order_by_order_id(&mut tx, order_id).await?;
    if order.status == "paid"
        || infrastructure::has_wallet_ledger_for_order(&mut tx, &order.order_id).await?
    {
        return Err(AppError::Conflict(
            "paid quick recharge order cannot be deleted".to_owned(),
        ));
    }

    // 删除订单前写审计，且与删除动作同事务提交，便于追溯后台人工清理原因。
    let before_json = json!(QuickRechargeOrderResponse::from(order.clone()));
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "quick_recharge_order.delete",
        "quick_recharge_order",
        order.id,
        Some(before_json),
        None,
        Some(reason),
    )
    .await?;
    infrastructure::delete_order_by_id(&mut tx, order.id).await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn handle_gmpay_notify(
    pool: Option<Pool<MySql>>,
    key: Option<&str>,
    payload: Value,
) -> AppResult<()> {
    tracing::info!(payload = %payload, "收到 GMPay 快速充值异步回调");
    let object = payload
        .as_object()
        .ok_or_else(|| AppError::Validation("gmpay notify payload must be an object".to_owned()))?;
    let pool = quick_recharge_mysql_pool(pool)?;
    let runtime = match load_runtime_config(&pool, key, false).await {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::warn!(%error, payload = %payload, "GMPay 快速充值回调读取配置失败");
            return Err(error);
        }
    };
    if let Err(error) = verify_gmpay_notify_signature(object, &runtime.merchant_secret) {
        tracing::warn!(%error, payload = %payload, "GMPay 快速充值回调验签失败");
        return Err(error);
    }

    let pid = required_json_string(object, "pid")?;
    if pid != runtime.merchant_pid {
        tracing::warn!(
            pid = %pid,
            expected_pid = %runtime.merchant_pid,
            payload = %payload,
            "GMPay 快速充值回调商户 PID 不匹配"
        );
        return Err(AppError::Validation(
            "gmpay notify pid is invalid".to_owned(),
        ));
    }
    let status = required_json_string(object, "status")?;
    if status != "2" {
        tracing::warn!(
            pid = %pid,
            status = %status,
            payload = %payload,
            "GMPay 快速充值回调状态不是已支付"
        );
        return Err(AppError::Validation(
            "gmpay notify status is not paid".to_owned(),
        ));
    }
    let order_id = required_json_string(object, "order_id")?;
    let trade_id = required_json_string(object, "trade_id")?;
    let amount = required_json_decimal(object, "amount")?;
    let actual_amount = required_json_decimal(object, "actual_amount")?;
    if actual_amount <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "gmpay notify actual_amount must be positive".to_owned(),
        ));
    }
    let token = required_json_string(object, "token")?;
    let receive_address = optional_json_string(object, "receive_address");
    let block_transaction_id = optional_json_string(object, "block_transaction_id");
    tracing::info!(
        order_id = %order_id,
        trade_id = %trade_id,
        pid = %pid,
        status = %status,
        amount = %decimal_to_gmpay_string(&amount),
        actual_amount = %decimal_to_gmpay_string(&actual_amount),
        token = %token,
        receive_address = ?receive_address,
        block_transaction_id = ?block_transaction_id,
        "GMPay 快速充值回调验签通过"
    );

    let mut tx = pool.begin().await?;
    let order = infrastructure::lock_order_by_order_id(&mut tx, &order_id).await?;
    if order.status == "paid" {
        tracing::info!(
            order_id = %order_id,
            trade_id = %trade_id,
            user_id = order.user_id,
            asset_id = order.asset_id,
            "GMPay 快速充值回调重复通知，订单已入账"
        );
        tx.commit().await?;
        return Ok(());
    }
    if let Some(existing_trade_id) = order.provider_trade_id.as_deref() {
        if existing_trade_id != trade_id {
            tracing::warn!(
                order_id = %order_id,
                trade_id = %trade_id,
                existing_trade_id = %existing_trade_id,
                "GMPay 快速充值回调交易号不匹配"
            );
            return Err(AppError::Validation(
                "gmpay notify trade_id does not match order".to_owned(),
            ));
        }
    }
    if order.fiat_amount != amount {
        tracing::warn!(
            order_id = %order_id,
            trade_id = %trade_id,
            notify_amount = %decimal_to_gmpay_string(&amount),
            order_amount = %decimal_to_gmpay_string(&order.fiat_amount),
            "GMPay 快速充值回调金额不匹配"
        );
        return Err(AppError::Validation(
            "gmpay notify amount does not match order".to_owned(),
        ));
    }
    if !order.token.eq_ignore_ascii_case(&token) {
        tracing::warn!(
            order_id = %order_id,
            trade_id = %trade_id,
            notify_token = %token,
            order_token = %order.token,
            "GMPay 快速充值回调到账币种不匹配"
        );
        return Err(AppError::Validation(
            "gmpay notify token does not match order".to_owned(),
        ));
    }

    infrastructure::mark_order_paid_from_notify(
        &mut tx,
        &QuickRechargeOrderPaidUpdate {
            order_id: order_id.clone(),
            provider_trade_id: trade_id.clone(),
            actual_amount: actual_amount.clone(),
            receive_address: receive_address.clone(),
            block_transaction_id: block_transaction_id.clone(),
            callback_payload_json: payload,
        },
    )
    .await?;
    // 支付回调确认后，订单更新、钱包入账和流水写入必须同事务提交，避免重复回调造成多入账。
    infrastructure::credit_wallet_available(
        &mut tx,
        order.user_id,
        order.asset_id,
        &actual_amount,
        &order_id,
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        order_id = %order_id,
        trade_id = %trade_id,
        user_id = order.user_id,
        asset_id = order.asset_id,
        actual_amount = %decimal_to_gmpay_string(&actual_amount),
        "GMPay 快速充值回调处理完成，订单已入账"
    );
    Ok(())
}

fn quick_recharge_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for quick recharge routes".to_owned())
    })
}

async fn load_runtime_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
    require_enabled: bool,
) -> AppResult<super::service::QuickRechargeRuntimeConfig> {
    let row = infrastructure::load_config_row(pool).await?;
    runtime_config_from_row(row, key, require_enabled)
}

fn quick_recharge_config_response(
    row: super::repository::QuickRechargeConfigRow,
) -> QuickRechargeConfigResponse {
    row.into()
}

fn quick_recharge_order_responses(
    rows: Vec<QuickRechargeOrderRow>,
) -> Vec<QuickRechargeOrderResponse> {
    rows.into_iter().map(Into::into).collect()
}
