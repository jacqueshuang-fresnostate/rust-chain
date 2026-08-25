//! seconds_contract bounded context application layer.
//!
//! 应用层：秒合约全部用例的事务边界所在，路由层只做鉴权与参数搬运，真正的编排都收敛在本文件。
//! 用例分两类。管理类用例（产品增删改查、启停、人工结算）统一按「锁定旧快照、校验、写入、
//! 写 before/after 审计、提交」的顺序编排，保证配置或资金变更与审计留痕原子生效。
//! 交易类用例中，`open_order` 先做无事务的幂等只读探测，命中即回放原单；未命中才开启事务，
//! 按锁产品规则、插订单占用幂等键、锁钱包扣本金、写流水、记代理佣金的固定顺序推进，
//! 幂等键的占位刻意排在钱包扣款之前，使并发同键请求最多只有一路能进入扣款；
//! `settle_order` 则按锁订单、读资产精度、按需锁钱包入账、置终态、写审计的顺序推进。
//! 事件发布一律不在事务内进行：`open_order` 与 `settle_order` 各自返回一个是否为首次执行的布尔量，
//! 由 `*_with_events` 包装层在提交成功之后才广播，重放与失败路径都不产生任何外部副作用。
//! 秒合约不使用独立余额账户，本金扣减与派奖入账都直接作用于共享现货钱包的可用余额。

use super::{
    infrastructure,
    presentation::{
        AdminOrdersQuery, AdminProductsQuery, AdminSecondsContractOrdersResponse,
        AdminSecondsContractProductsResponse, CreateSecondsContractProductRequest,
        DeleteSecondsContractProductRequest, OpenSecondsContractOrderRequest,
        OpenSecondsContractOrderResponse, SecondsContractOrderResponse,
        SecondsContractOrdersResponse, SecondsContractProductResponse,
        SecondsContractProductsResponse, SettleSecondsContractOrderRequest,
        SettleSecondsContractOrderResponse, UpdateSecondsContractProductRequest,
        UpdateSecondsContractProductStatusRequest,
    },
    repository::{
        SecondsContractAdminOrderFilter, SecondsContractOrderInsert, SecondsContractProductWrite,
        SecondsContractWalletLedgerWrite,
    },
    service::{
        NormalizedSecondsContractProductCycle, SETTLEMENT_PRICE_WINDOW_SECONDS,
        ensure_existing_order_matches_request, ensure_existing_settlement_matches,
        normalize_direction, normalize_idempotency_key, normalize_settlement_result,
        normalized_product_status, optional_image_url, optional_string, order_audit_json,
        product_audit_json, publish_seconds_contract_order_opened_event_if_needed,
        publish_seconds_contract_order_settled_event_if_needed, required_reason, route_limit,
        route_offset, settlement_payout_amount, settlement_result_from_prices,
        validate_create_product_request, validate_product_stake, validate_stake_amount,
        validate_update_product_request,
    },
};
use crate::{
    error::{AppError, AppResult},
    modules::agent::{
        infrastructure::insert_agent_business_commission_in_tx,
        repository::AgentBusinessCommissionWrite,
        service::AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT,
    },
    state::AppState,
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

/// 从应用状态取出 MySQL 连接池的克隆句柄，供只读路由直接使用。
/// 连接池未配置时返回 `AppError::Internal` 而非校验错误，因为那属于部署配置缺失而不是请求问题。
/// 克隆的是内部共享句柄，不会新建连接，成本可忽略。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for seconds contract routes".to_owned())
    })
}

/// 组装面向用户的秒合约产品目录，状态在用例内硬编码为 `active`，客户端无法通过参数放宽过滤。
/// 底层查询会连带要求交易对与相关资产同为启用状态，因此返回的都是当下真实可下单的标的。
/// 只读不加锁，返回的赔率与限额仅供展示，实际下单时会在事务内重新锁定核对。
pub(crate) async fn list_active_products(
    pool: &Pool<MySql>,
    limit: u32,
) -> AppResult<SecondsContractProductsResponse> {
    let products = infrastructure::list_products(pool, Some("active"), limit).await?;
    Ok(SecondsContractProductsResponse { products })
}

/// 组装后台产品分页列表，状态参数固定传 `None`，因此结果包含启用与已禁用的全部产品。
/// 条数与偏移分别经 `route_limit` 与 `route_offset` 归一，防止超大分页拖垮查询。
/// 返回的总数与行集来自同一组过滤条件，读取过程不加锁也不改动任何配置。
pub(crate) async fn list_admin_products(
    pool: &Pool<MySql>,
    query: AdminProductsQuery,
) -> AppResult<AdminSecondsContractProductsResponse> {
    let (products, total) = infrastructure::list_admin_products(
        pool,
        None,
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminSecondsContractProductsResponse { products, total })
}

/// 读取后台单个产品详情，含全部周期档位，供管理页展示与编辑表单回填。
/// 不限定产品状态，已禁用产品同样可查；记录缺失时透传底层的 `AppError::NotFound`。
/// 走连接池只读查询，不开启事务也不加行锁。
pub(crate) async fn get_admin_product(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<SecondsContractProductResponse> {
    infrastructure::load_product_by_id_from_pool(pool, product_id).await
}

/// 校验交易对、投注资产、周期、赔率、限额、状态和原因后，在同一管理事务写产品、周期及创建审计。
/// 所有纯参数校验都排在开启事务之前完成，非法请求连数据库连接都不会占用。
/// 未显式给出状态时默认按 `active` 建产品，即创建后立即可下单；周期集合的第一条会被写进产品主记录，
/// 作为不带周期参数的旧版客户端的默认档位。
/// 事务内按交易对存在性、资产存在性、插产品、插周期、回读快照、写审计的顺序推进，
/// 任一步失败回滚全部配置，不会留下缺周期的孤立产品或无审计的配置。
/// 该用例不创建订单也不移动任何用户资金。
pub(crate) async fn create_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    request: CreateSecondsContractProductRequest,
) -> AppResult<SecondsContractProductResponse> {
    let cycles = validate_create_product_request(&request)?;
    let reason = required_reason(request.reason.clone())?;
    let status = normalized_product_status(request.status.as_deref().unwrap_or("active"))?;
    let logo_url = optional_image_url(
        request.logo_url.clone(),
        "seconds contract product logo_url",
    )?;
    let default_cycle = default_product_cycle(&cycles)?;
    let write = product_write_from_cycle(
        request.pair_id,
        request.stake_asset,
        logo_url,
        status,
        default_cycle,
    );

    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 产品主表、周期配置和后台审计必须同事务提交，避免配置生效后缺少可追溯记录。
    infrastructure::ensure_pair_exists(&mut tx, write.pair_id).await?;
    infrastructure::ensure_asset_exists(&mut tx, write.stake_asset).await?;
    let product_id = infrastructure::insert_product(&mut tx, &write).await?;
    infrastructure::insert_product_cycles(&mut tx, product_id, &cycles).await?;
    let product = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.create",
        "seconds_contract_product",
        product.id,
        None,
        Some(product_audit_json(&product)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(product)
}

/// 事务内先锁产品旧快照，再校验交易对/资产并原子替换主记录、完整周期集合和 before/after 审计。
/// 与创建不同，这里的状态是必填项，且周期集合按整体覆盖处理：请求中未出现的旧周期会被删除。
/// 先加锁再读 before 快照，使审计的前后镜像必定对应同一次变更，不会被并发管理操作插入其他改动。
/// 更新失败保留原配置；已开仓订单在下单时已固化自己的周期、赔率和限额，因此改配置不影响存量订单结算。
pub(crate) async fn update_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateSecondsContractProductRequest,
) -> AppResult<SecondsContractProductResponse> {
    let cycles = validate_update_product_request(&request)?;
    let reason = required_reason(request.reason.clone())?;
    let status = normalized_product_status(&request.status)?;
    let logo_url = optional_image_url(
        request.logo_url.clone(),
        "seconds contract product logo_url",
    )?;
    let default_cycle = default_product_cycle(&cycles)?;
    let write = product_write_from_cycle(
        request.pair_id,
        request.stake_asset,
        logo_url,
        status,
        default_cycle,
    );

    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 编辑产品时先锁定旧快照，再写入新快照和审计，确保审计 before/after 对应同一次变更。
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::ensure_pair_exists(&mut tx, write.pair_id).await?;
    infrastructure::ensure_asset_exists(&mut tx, write.stake_asset).await?;
    infrastructure::update_product(&mut tx, product_id, &write).await?;
    infrastructure::replace_product_cycles(&mut tx, product_id, &cycles).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.update",
        "seconds_contract_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 锁定产品后原子更新 active/disabled 状态并写 before/after 管理审计，供运营快速上下架。
/// 只改状态字段，交易对、质押资产、周期集合和赔率一概保持原样，因此无需回填完整配置。
/// 下架仅阻止新订单开仓，既有持仓订单仍按各自快照到期结算；本用例不结算、不派奖、不动任何钱包。
/// 状态与原因校验在开事务前完成，写入失败整体回滚，产品状态保持变更前取值。
pub(crate) async fn update_product_status(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: UpdateSecondsContractProductStatusRequest,
) -> AppResult<SecondsContractProductResponse> {
    let status = normalized_product_status(&request.status)?;
    let reason = required_reason(request.reason.clone())?;
    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 状态变更同样保留 before/after 审计，便于追踪产品下架或恢复的责任人和原因。
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    infrastructure::update_product_status(&mut tx, product_id, &status).await?;
    let after = infrastructure::load_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.update_status",
        "seconds_contract_product",
        product_id,
        Some(product_audit_json(&before)),
        Some(product_audit_json(&after)),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

/// 物理删除秒合约产品，成功时不返回实体，前置条件是产品已禁用且从未产生过任何订单。
/// 两道前置检查都在持有产品行锁之后进行：状态非 `disabled` 返回校验错误，提示必须先下架；
/// 存在历史订单同样拒绝，保护订单外键与资金对账的可追溯性，此类产品只能长期保持禁用。
/// 产品锁、约束检查、删除与仅含 before 镜像的审计在同一事务提交，任一步失败产品原样保留。
/// 本用例不处理订单，也不退还任何资金。
pub(crate) async fn delete_product(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    product_id: u64,
    request: DeleteSecondsContractProductRequest,
) -> AppResult<()> {
    let reason = required_reason(request.reason.clone())?;
    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    // 删除前锁定产品并确认已禁用、无订单，避免仍可交易的秒合约配置被物理删除。
    let before = infrastructure::lock_product_by_id(&mut tx, product_id).await?;
    if before.status != "disabled" {
        return Err(AppError::Validation(
            "seconds contract product must be disabled before deletion".to_owned(),
        ));
    }
    infrastructure::ensure_product_has_no_orders(&mut tx, product_id).await?;
    infrastructure::delete_product_by_id(&mut tx, product_id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_product.delete",
        "seconds_contract_product",
        product_id,
        Some(product_audit_json(&before)),
        None,
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 开立秒合约订单；方向、本金精度、产品周期/限额及活动资产必须有效，并取得服务端新鲜正入场价。
/// 新请求在事务中先锁产品并插入订单占用幂等键，再锁共享现货钱包扣本金、写流水及代理佣金。
/// 订单快照、可用余额扣减和 `seconds_contract_open` 流水必须同事务提交，禁止独立秒合约余额。
/// 同用户同键同请求重放原订单且不再取行情或扣款；提交后仅事件包装层对新订单发布通知。
pub(crate) async fn open_order(
    pool: Option<&Pool<MySql>>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    request: OpenSecondsContractOrderRequest,
) -> AppResult<(OpenSecondsContractOrderResponse, bool)> {
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    validate_stake_amount(&request.stake_amount)?;
    let pool = require_mysql_pool(pool)?;

    if let Some(existing) =
        infrastructure::existing_order_for_idempotency_key_readonly(pool, user_id, &idempotency_key)
            .await?
    {
        ensure_existing_order_matches_request(
            &existing,
            request.product_id,
            request.duration_seconds,
            &direction,
            &request.stake_amount,
        )?;
        return Ok((OpenSecondsContractOrderResponse { order: existing }, false));
    }

    let mut tx = pool.begin().await?;
    let product = match infrastructure::lock_active_product(
        &mut tx,
        request.product_id,
        request.duration_seconds,
    )
    .await
    {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_order_if_present(
                pool,
                user_id,
                request.product_id,
                request.duration_seconds,
                &direction,
                &request.stake_amount,
                &idempotency_key,
            )
            .await?
            {
                return Ok((OpenSecondsContractOrderResponse { order: existing }, false));
            }
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error),
    };
    validate_product_stake(&request.stake_amount, &product)?;
    let entry_price =
        infrastructure::cached_entry_price(redis, product.pair_id, product.symbol.as_str()).await?;
    let expires_at = infrastructure::database_now(&mut tx).await?
        + chrono::TimeDelta::seconds(product.duration_seconds as i64);
    let order = SecondsContractOrderInsert {
        user_id,
        product_id: product.id,
        pair_id: product.pair_id,
        stake_asset: product.stake_asset,
        direction,
        stake_amount: request.stake_amount.clone(),
        duration_seconds: product.duration_seconds,
        payout_rate: product.payout_rate.clone(),
        entry_price,
        idempotency_key,
        expires_at,
    };

    // 先占用用户幂等键，再锁钱包扣款；并发同 key 请求只会有一个进入扣款路径。
    let order_id = match infrastructure::insert_open_order(&mut tx, &order).await {
        Ok(order_id) => order_id,
        Err(error) if infrastructure::is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_order(
                pool,
                user_id,
                request.product_id,
                request.duration_seconds,
                &order.direction,
                &request.stake_amount,
                &order.idempotency_key,
            )
            .await
            .map(|order| (OpenSecondsContractOrderResponse { order }, false));
        }
        Err(error) => return Err(AppError::Database(error)),
    };

    let wallet = infrastructure::lock_wallet_row(&mut tx, user_id, product.stake_asset).await?;
    if wallet.available < request.stake_amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for seconds contract: requested {}, available {}, locked {}",
            request.stake_amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - request.stake_amount.clone();
    // 订单、钱包扣款和流水必须同事务提交，避免出现已开仓但余额/流水不一致。
    infrastructure::update_wallet_available(
        &mut tx,
        user_id,
        product.stake_asset,
        &available_after,
    )
    .await?;
    infrastructure::insert_wallet_ledger(
        &mut tx,
        SecondsContractWalletLedgerWrite {
            user_id,
            asset_id: product.stake_asset,
            change_type: "seconds_contract_open",
            amount: -request.stake_amount.clone(),
            available_after: available_after.clone(),
            frozen_after: wallet.frozen,
            locked_after: wallet.locked,
            ref_id: order_id.to_string(),
        },
    )
    .await?;

    let commission_source_id = order_id.to_string();
    insert_agent_business_commission_in_tx(
        &mut tx,
        AgentBusinessCommissionWrite {
            user_id,
            product_type: AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT,
            source_type: "seconds_contract_order",
            source_id: &commission_source_id,
            source_amount: &request.stake_amount,
            payout_asset_id: product.stake_asset,
        },
    )
    .await?;

    let order = infrastructure::load_order_by_id(&mut tx, order_id).await?;
    tx.commit().await?;
    Ok((OpenSecondsContractOrderResponse { order }, true))
}

/// 执行幂等开仓，并只在新订单资金事务提交后发布用户私有开仓事件；重放和失败均不广播。
/// 事件发布刻意放在 `open_order` 返回之后，此时资金事务已提交，不存在推送了事件却回滚扣款的窗口。
/// 是否首次开仓由 `open_order` 返回的布尔量决定，包装层不重新判断，避免两处口径不一致。
pub(crate) async fn open_order_with_events(
    pool: Option<&Pool<MySql>>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    request: OpenSecondsContractOrderRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<OpenSecondsContractOrderResponse> {
    // 秒合约开仓保持原有幂等和钱包结算逻辑不变，在应用层统一触发开仓事件。
    let (response, is_new_order) = open_order(pool, redis, user_id, request).await?;
    publish_seconds_contract_order_opened_event_if_needed(hub, user_id, &response, is_new_order);
    Ok(response)
}

/// 管理员请求结算秒合约订单；实际结果必须由事件时间窗口中的 MySQL 历史价格推导并与请求一致。
/// 事务先锁订单，再以数据库时间确认窗口已关闭并选择不可变快照；胜单随后锁共享现货钱包，
/// 入账与流水、价格证据、订单终态及管理员审计原子提交。
/// 负单不入账；已 settled 且结果一致时返回原结算并不重复派奖，结果冲突或非 opened 状态拒绝处理。
/// 成功提交后仅事件包装层对首次结算发布通知，重放与失败路径均不得产生外部副作用。
pub(crate) async fn settle_order(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    order_id: u64,
    request: SettleSecondsContractOrderRequest,
) -> AppResult<(SettleSecondsContractOrderResponse, bool)> {
    let requested_result = normalize_settlement_result(&request.result)?;
    let reason = required_reason(request.reason.clone())?;
    let pool = require_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let order = infrastructure::lock_order_by_id(&mut tx, order_id).await?;
    let stake_asset_precision =
        infrastructure::load_asset_precision_scale(&mut tx, order.stake_asset).await?;
    if order.status == "settled" {
        ensure_existing_settlement_matches(&order, &requested_result)?;
        let payout_amount =
            settlement_payout_amount(&order, &requested_result, stake_asset_precision);
        tx.commit().await?;
        return Ok((
            SettleSecondsContractOrderResponse {
                order,
                payout_amount,
            },
            false,
        ));
    }
    if order.status != "opened" {
        return Err(AppError::Conflict(
            "seconds contract order is not open for settlement".to_owned(),
        ));
    }

    let database_now = infrastructure::database_now(&mut tx).await?;
    let settlement_window_closes_at =
        order.expires_at + chrono::TimeDelta::seconds(SETTLEMENT_PRICE_WINDOW_SECONDS);
    if database_now < settlement_window_closes_at {
        return Err(AppError::Conflict(
            "seconds contract settlement event window has not closed".to_owned(),
        ));
    }
    let snapshot =
        infrastructure::select_settlement_price_snapshot(&mut tx, &order.symbol, order.expires_at)
            .await?
            .ok_or_else(|| {
                AppError::Conflict(
                    "seconds contract settlement price history is pending for the event window"
                        .to_owned(),
                )
            })?;
    let entry_price = order.entry_price.as_ref().ok_or_else(|| {
        AppError::Validation("seconds contract entry price is required for settlement".to_owned())
    })?;
    let result = settlement_result_from_prices(&order.direction, entry_price, &snapshot.price)?;
    if result != requested_result {
        return Err(AppError::Conflict(
            "requested seconds contract result does not match the event-time price".to_owned(),
        ));
    }

    let before_json = Some(order_audit_json(&order, BigDecimal::from(0)));
    let payout_amount = settlement_payout_amount(&order, result, stake_asset_precision);

    if payout_amount > 0 {
        let wallet =
            infrastructure::lock_wallet_row(&mut tx, order.user_id, order.stake_asset).await?;
        let available_after = wallet.available.clone() + payout_amount.clone();
        // 派奖入账和流水写入必须与订单结算状态同事务完成，避免重复派奖或遗漏审计。
        infrastructure::update_wallet_available(
            &mut tx,
            order.user_id,
            order.stake_asset,
            &available_after,
        )
        .await?;
        infrastructure::insert_wallet_ledger(
            &mut tx,
            SecondsContractWalletLedgerWrite {
                user_id: order.user_id,
                asset_id: order.stake_asset,
                change_type: "seconds_contract_settle_win",
                amount: payout_amount.clone(),
                available_after: available_after.clone(),
                frozen_after: wallet.frozen,
                locked_after: wallet.locked,
                ref_id: order.id.to_string(),
            },
        )
        .await?;
    }

    infrastructure::mark_order_settled(&mut tx, order.id, result, &snapshot).await?;
    let settled_order = infrastructure::load_order_by_id(&mut tx, order.id).await?;
    infrastructure::insert_admin_audit_log_in_tx(
        &mut tx,
        admin_id,
        "seconds_contract_order.settle",
        "seconds_contract_order",
        order.id,
        before_json,
        Some(order_audit_json(&settled_order, payout_amount.clone())),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok((
        SettleSecondsContractOrderResponse {
            order: settled_order,
            payout_amount,
        },
        true,
    ))
}

/// 执行人工结算，并只在首次结算资金事务提交后发布用户私有结算事件；同结果重放不重复广播。
/// 事件的收件人取自订单归属用户而非发起结算的管理员，因此后台代操作也能正确推给持仓用户。
/// 派奖入账已在 `settle_order` 的事务中完成，这里只做投递，广播失败不会回滚已入账资金。
pub(crate) async fn settle_order_with_events(
    pool: Option<&Pool<MySql>>,
    admin_id: u64,
    order_id: u64,
    request: SettleSecondsContractOrderRequest,
    hub: Option<&crate::modules::events::EventBroadcastHub>,
) -> AppResult<SettleSecondsContractOrderResponse> {
    // 秒合约结算统一返回响应对象，同时在应用层根据是否为新结算推送事件。
    let (response, is_new_settlement) = settle_order(pool, admin_id, order_id, request).await?;
    publish_seconds_contract_order_settled_event_if_needed(
        hub,
        response.order.user_id,
        &response,
        is_new_settlement,
    );
    Ok(response)
}

/// 按认证用户读取秒合约订单历史，用户编号来自令牌解析，绝不通过订单标识跨用户返回记录。
/// 返回结果同时包含持仓中与已结算订单，按创建时间倒序，条数由调用方归一后传入。
/// 纯读取，不触发到期结算，也不改动任何订单状态。
pub(crate) async fn list_user_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    limit: u32,
) -> AppResult<SecondsContractOrdersResponse> {
    let orders = infrastructure::list_user_orders(pool, user_id, limit).await?;
    Ok(SecondsContractOrdersResponse { orders })
}

/// 组装后台订单分页查询条件并返回订单列表与匹配总数，供客服核单与风控排查。
/// 邮箱与状态经 `optional_string` 裁剪，空白串会降级为不筛选而不是当作空值精确匹配；
/// 用户编号为可选数值，三个筛选项同时给出时按 AND 叠加。
/// 条数与偏移经统一归一后传入，查询过程不加锁，也不触发任何自动结算。
pub(crate) async fn list_admin_orders(
    pool: &Pool<MySql>,
    query: AdminOrdersQuery,
) -> AppResult<AdminSecondsContractOrdersResponse> {
    let filter = SecondsContractAdminOrderFilter {
        user_id: query.user_id,
        email: optional_string(query.email),
        status: optional_string(query.status),
        limit: route_limit(query.limit),
        offset: route_offset(query.offset),
    };
    let (orders, total) = infrastructure::list_admin_orders(pool, filter).await?;
    Ok(AdminSecondsContractOrdersResponse { orders, total })
}

/// 读取后台单笔订单详情，返回开仓价、结算价、结果与状态，供人工结算前核对价格与胜负判定。
/// 按订单主键定位而不限定归属用户，越权控制依赖路由层的管理员鉴权。
/// 记录缺失返回 `AppError::NotFound`；纯读取，不改动订单状态也不产生任何赔付。
pub(crate) async fn get_admin_order(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SecondsContractOrderResponse> {
    infrastructure::load_order_by_id_from_pool(pool, order_id).await
}

/// 在插入订单撞上唯一键冲突后强制回读原单，把「键已存在」这一事实转成可返回给客户端的既有订单。
/// 与可空版本的差别在于此处必须读到记录：读不到说明并发的同键事务尚未提交，
/// 此时返回 `AppError::Conflict` 提示稍后重试，而不是误判为无冲突继续走扣款路径造成重复下单。
/// 读到记录仍要逐字段核对产品、方向和金额，不一致同样返回冲突。本函数不扣款也不新建订单。
async fn replay_existing_order(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    duration_seconds: Option<u32>,
    direction: &str,
    stake_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<SecondsContractOrderResponse> {
    replay_existing_order_if_present(
        pool,
        user_id,
        product_id,
        duration_seconds,
        direction,
        stake_amount,
        idempotency_key,
    )
    .await?
    .ok_or_else(|| {
        AppError::Conflict("seconds contract idempotency key is being committed".to_owned())
    })
}

/// 在独立事务中加锁查找同一幂等键的既有订单，命中且请求一致时回放原单，未命中返回 `None`。
/// 之所以另开事务而不是复用调用方事务，是因为两个调用点都发生在原事务已回滚之后：
/// 一是锁定产品得到 NotFound、二是插入订单撞唯一键，回滚后必须用新事务重新读取。
/// 加锁读取可等待并发同键事务落定，避免在对方提交前误判为不存在。
/// 一致性校验失败时直接向上返回冲突，此时事务未提交而是随函数返回被丢弃回滚，
/// 由于全程只有读取，回滚不会撤销任何业务数据。本函数不扣款、不建单、不发事件。
async fn replay_existing_order_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    duration_seconds: Option<u32>,
    direction: &str,
    stake_amount: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<SecondsContractOrderResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        infrastructure::existing_order_for_idempotency_key(&mut tx, user_id, idempotency_key)
            .await?
    else {
        return Ok(None);
    };
    ensure_existing_order_matches_request(
        &existing,
        product_id,
        duration_seconds,
        direction,
        stake_amount,
    )?;
    tx.commit().await?;
    Ok(Some(existing))
}

/// 取周期集合的首条作为产品默认档位，其取值会被写进产品主记录供旧版单周期客户端使用。
/// 集合已由服务层按时长升序排好，因此默认档位就是最短周期；集合为空返回校验错误而非静默兜底。
fn default_product_cycle(
    cycles: &[NormalizedSecondsContractProductCycle],
) -> AppResult<&NormalizedSecondsContractProductCycle> {
    cycles
        .first()
        .ok_or_else(|| AppError::Validation("seconds contract cycles must not be empty".to_owned()))
}

/// 把产品级字段与选定的默认周期合成产品主表写入结构，创建与更新共用同一套拼装逻辑。
/// 主记录上的时长、赔率、投注上下限全部取自传入的默认周期，因此主记录始终是周期集合首条的冗余副本，
/// 二者由同一次写入保持同步，不会出现主记录与周期子表互相矛盾的配置。
/// 本函数只做字段搬运与克隆，不做任何校验，取值合法性由服务层在更早的阶段保证。
fn product_write_from_cycle(
    pair_id: u64,
    stake_asset: u64,
    logo_url: Option<String>,
    status: String,
    cycle: &NormalizedSecondsContractProductCycle,
) -> SecondsContractProductWrite {
    SecondsContractProductWrite {
        pair_id,
        stake_asset,
        logo_url,
        duration_seconds: cycle.duration_seconds,
        payout_rate: cycle.payout_rate.clone(),
        min_stake: cycle.min_stake.clone(),
        max_stake: cycle.max_stake.clone(),
        status,
    }
}

/// 把可选连接池收敛为必备引用，供需要自行开启事务的写用例在最前面做一次前置断言。
/// 缺失时返回 `AppError::Internal`，因为连接池未配置属于部署问题而非调用方参数问题。
/// 与 `mysql_pool` 的区别是这里借用而不克隆，用于已持有句柄引用的用例入口。
fn require_mysql_pool(pool: Option<&Pool<MySql>>) -> AppResult<&Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for seconds contract routes".to_owned())
    })
}
