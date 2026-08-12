//! quick_recharge bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use super::{
    infrastructure,
    presentation::{
        AdminQuickRechargeOrdersResponse, CreateQuickRechargeOrderRequest,
        DeleteQuickRechargeOrderRequest, QuickRechargeConfigResponse, QuickRechargeOrderResponse,
        QuickRechargeOrdersQuery, QuickRechargeOrdersResponse, SaveQuickRechargeConfigRequest,
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
        required_json_decimal, required_json_string, required_reason, route_limit, route_offset,
        runtime_config_from_row, test_config_audit_json, user_id_from_subject,
        validate_enabled_config_secrets, validate_order_status, validate_recharge_amount,
        validate_save_config_request, verify_gmpay_notify_signature,
    },
};
use crate::{
    error::{AppError, AppResult},
    infra::secrets::mask_secret,
};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{MySql, Pool};
use uuid::Uuid;

/// 读取用户可见的快充启停、币种、网络和金额范围，不暴露商户密钥。
/// 该用例只读单例配置，不调用支付方，也不创建订单或修改钱包余额。
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

/// 按鉴权用户和可选状态读取其快充订单，限制单次返回数量。
/// 状态在查询前按本地状态机校验，结果不包含其他用户订单或资金流水。
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

/// 读取后台快充完整配置响应，密钥仅以掩码形式展示。
pub(crate) async fn get_admin_quick_recharge_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<QuickRechargeConfigResponse> {
    Ok(quick_recharge_config_response(
        infrastructure::load_config_row(&quick_recharge_mysql_pool(pool)?).await?,
    ))
}

/// 规范用户、邮箱、状态和支付方编号筛选后查询后台订单分页及总数。
/// 该只读用例不锁订单，不触发支付方请求，也不改变钱包或支付状态。
pub(crate) async fn list_admin_quick_recharge_orders(
    pool: Option<Pool<MySql>>,
    query: QuickRechargeOrdersQuery,
) -> AppResult<AdminQuickRechargeOrdersResponse> {
    let filter = QuickRechargeAdminOrderFilter {
        user_id: query.user_id,
        email: optional_string(query.email),
        status: optional_string(query.status)
            .map(|status| validate_order_status(&status))
            .transpose()?,
        order_id: optional_string(query.order_id),
        provider_trade_id: optional_string(query.provider_trade_id),
        limit: route_limit(query.limit),
        offset: route_offset(query.offset),
    };
    let (orders, total) =
        infrastructure::list_admin_orders(&quick_recharge_mysql_pool(pool)?, filter).await?;
    Ok(AdminQuickRechargeOrdersResponse {
        orders: quick_recharge_order_responses(orders),
        total,
    })
}

/// 为当前用户创建 GMPay 快充订单；需有可用配置和密钥，充值金额合规且配置币种对应活动资产。
/// 本地先持久化 created 订单，再调用支付方；响应订单号和金额必须匹配，成功转 pending，失败标记 failed。
/// 此阶段不修改钱包或流水，真正入账仅由已验签回调完成；支付方 HTTP 调用是数据库事务外副作用。
/// 每次请求生成新订单号，不提供请求级幂等重放；重试会创建另一条本地记录和另一笔外部支付请求。
/// 外部订单成功后，本地 pending 更新失败不会撤销支付方订单；外部结果异常时标记 failed 若再失败，本地记录可能保留 created。
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

/// 保存管理员快充配置；管理员身份、修改原因、字段格式以及启用时必需密钥必须全部有效。
/// 事务先锁定单例配置，空白密钥沿用既有密文与掩码，再更新配置并写入前后快照审计。
/// 配置与审计日志必须原子提交，任何校验、加密或持久化失败都不得留下半生效配置。
/// 本操作无幂等键，重复保存会形成新的审计记录；提交后不发起支付方请求或其他外部副作用。
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

/// 使用当前保存配置发起管理员 GMPay 连通性测试；需验证管理员身份、原因、密钥和测试金额。
/// 支付方调用发生在审计事务之前，返回订单号和金额必须与本次随机测试单一致，且不创建本地充值订单。
/// 测试结果与配置快照仅写管理员审计，不修改用户钱包或流水；审计提交失败也不会撤销支付方测试单。
/// 每次调用都会生成新订单号并产生新的外部请求，不具备幂等重放语义。
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

/// 锁定未支付快充订单并确认不存在钱包流水后，写管理员审计再删除订单。
/// 应用层拥有事务；paid 或已有入账流水的订单禁止删除，审计与删除失败时整体回滚。
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

/// 处理 GMPay 已支付异步通知；首次入账须验签并确认 PID、状态、订单号、交易号、法币金额和到账币种与本地订单一致。
/// 事务先锁定快充订单：已为 `paid` 时按幂等重放直接成功，未支付时将订单状态、支付方原始回调、钱包可用余额及对应 `quick_recharge` 流水在同一事务内提交，任何一步失败都不得留下半入账状态。
/// `paid` 重放在验签、PID/status 和字段解析后短路，不再核对本次 trade_id、法币金额或 token；它不产生第二次余额变化或流水。
/// 首次实际到账数量只校验为正，当前路径不按资产 precision_scale 截断；available 与流水直接使用已验签 `actual_amount`，frozen/locked 不变。
/// 日志边界为：收到原始回调、配置/验签失败、关键字段不匹配、幂等命中及事务提交后的入账成功；成功日志只能在提交完成后发出，本用例不调用支付方 HTTP，也不在日志之外发布不可回滚事件。
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
    if actual_amount <= 0 {
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
    if let Some(existing_trade_id) = order.provider_trade_id.as_deref()
        && existing_trade_id != trade_id
    {
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
