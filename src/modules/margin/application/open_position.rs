//! 杠杆市价/限价开仓用例。
//!
//! 这是本上下文风险最高的资金入口，负责把一次开仓请求变成仓位行、抵押扣款、代理返佣和一条私有事件。
//! 关键顺序是：事务外校验并做只读幂等预检；全仓成交路径在事务内先锁账户，再锁产品、仓位与钱包。
//! 先占键后动钱是防重复扣款的核心，唯一键冲突会回滚并转入只读重放，逐字段核对后返回既有仓位。
//! 市价单立即以服务端 Redis 新鲜 ticker 成交；限价单的客户价格只用于判定是否触发，
//! 真正入场价同样只认服务端权威 ticker。未触发限价单会先冻结抵押、保留 `entry_price = NULL`，
//! 不建全仓账户、不登记佣金也不发布开仓事件；只有首次实际成交后才会完成这些副作用。

use super::product_config::validate_hourly_interest_rate;
use super::support::{
    decimal_matches_string, ensure_supported_user_margin_mode, is_duplicate_key_error,
    non_negative_amount, normalized_margin_mode, validate_positive_decimal,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        agent::{
            infrastructure::insert_agent_business_commission_in_tx,
            repository::AgentBusinessCommissionWrite, service::AGENT_COMMISSION_PRODUCT_MARGIN,
        },
        events::EventBroadcastHub,
        margin::{
            domain::{
                MarginOrderType, margin_limit_order_is_triggered, validate_margin_limit_price,
            },
            infrastructure::{
                MarginOpenProductRule, activate_cross_margin_account_for_open,
                bump_cross_margin_account_version, cached_margin_entry_price,
                debit_margin_position_open_collateral,
                discard_new_cross_margin_account_for_pending_order,
                ensure_and_lock_cross_margin_account_with_creation,
                existing_position_for_idempotency_key,
                existing_position_for_idempotency_key_readonly, insert_margin_position,
                load_margin_open_product_account_scope, load_position_by_id,
                lock_active_open_product, require_active_cross_margin_account,
                set_margin_position_wallet_scope,
            },
            presentation::{
                MarginPositionResponse, OpenMarginPositionRequest, OpenMarginPositionResponse,
            },
            service::publish_margin_position_opened_event_if_needed,
        },
    },
};
use bigdecimal::BigDecimal;
use redis::aio::ConnectionManager;
use sqlx::{MySql, Pool};

/// 在进入资金事务前冻结的订单语义；限价仅用于触发判定，不是成交价。
struct ValidatedMarginOpenOrder {
    order_type: MarginOrderType,
    limit_price: Option<BigDecimal>,
}

/// 幂等重放需要逐字段核对的不可变请求意图，集中持有借用以避免参数列表和调用点发生漂移。
struct MarginOpenIdempotencyIntent<'a> {
    product_id: u64,
    direction: &'a str,
    margin_mode: Option<&'a str>,
    margin_amount: &'a BigDecimal,
    leverage: &'a BigDecimal,
    order_type: &'a str,
    limit_price: Option<&'a BigDecimal>,
}

/// 校验市价/限价开仓语义和用户幂等键，以服务端新鲜行情作为唯一可能的入场价创建仓位或挂单。
/// 事务先占用幂等键，再锁钱包扣抵押并写流水；返佣与全仓账户只在入场价已确定的真实成交分支落库。
/// 同键同参重放不再次扣款，任一步失败整体回滚。
///
/// 进入事务前先做只读幂等预检：命中既有仓位就逐字段核对产品、方向、模式、保证金和杠杆，
/// 一致则直接返回原仓位且第二个返回值为假，不一致返回冲突，全程不消耗行锁也不读行情。
/// 事务内按 FOR UPDATE 锁定 active 产品固定规则快照；产品缺失时先回滚再尝试重放，
/// 这样即便管理员在两次请求之间停用了产品，已提交的同键请求仍能拿回原结果而不是报 404。
/// 保证金必须落在产品的最小最大区间内，杠杆必须精确命中某个配置档位，模式必须被产品和风控同时支持。
/// 抵押扣减由结算适配器按模式选钱包并返回实际资金域，随后写回仓位的 `wallet_scope`，
/// 平仓、撤销和强平都据此原路返还；全仓还会补建按用户和保证金币种唯一的全仓账户。
/// 同一事务内按开仓保证金额登记代理业务返佣，最后回读完整仓位快照并提交。
/// 本函数不发布任何事件，广播时机由调用方在提交成功且返回值为真时决定。
pub(crate) async fn open_margin_position(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    user_id: u64,
    request: OpenMarginPositionRequest,
) -> AppResult<(MarginPositionResponse, bool)> {
    let order = validate_open_order_semantics(&request)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    let requested_margin_mode = match request.margin_mode.as_deref() {
        Some(value) => Some(normalized_margin_mode(value)?),
        None => None,
    };
    validate_positive_decimal(&request.margin_amount, "margin amount")?;
    validate_positive_decimal(&request.leverage, "leverage")?;
    let idempotency_intent = MarginOpenIdempotencyIntent {
        product_id: request.product_id,
        direction: &direction,
        margin_mode: requested_margin_mode.as_deref(),
        margin_amount: &request.margin_amount,
        leverage: &request.leverage,
        order_type: order.order_type.as_str(),
        limit_price: order.limit_price.as_ref(),
    };

    if let Some(existing) =
        existing_position_for_idempotency_key_readonly(pool, user_id, &idempotency_key).await?
    {
        ensure_existing_position_matches_request(&existing, &idempotency_intent)?;
        return Ok((existing, false));
    }

    // 全仓写路径必须先确定账户键并锁账户，再锁产品/仓位/钱包；事务内会重新核对产品配置。
    let preflight_scope = load_margin_open_product_account_scope(pool, request.product_id).await?;
    let preflight_cross_asset = preflight_scope
        .as_ref()
        .filter(|scope| scope.status == "active")
        .and_then(|scope| {
            let mode = requested_margin_mode
                .as_deref()
                .unwrap_or(scope.margin_mode.as_str());
            (mode == "cross" && scope.margin_modes.0.iter().any(|item| item == "cross"))
                .then_some(scope.margin_asset)
        });

    let mut tx = pool.begin().await?;
    let mut cross_account = if let Some(margin_asset) = preflight_cross_asset {
        let (account, created) =
            ensure_and_lock_cross_margin_account_with_creation(&mut tx, user_id, margin_asset)
                .await?;
        if account.status == "liquidating" {
            return Err(AppError::Conflict(
                "cross margin account is liquidating".to_owned(),
            ));
        }
        Some((margin_asset, account, created))
    } else {
        None
    };
    let product = match lock_active_open_product(&mut tx, request.product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_position_if_present(
                pool,
                user_id,
                &idempotency_key,
                &idempotency_intent,
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
    if position_margin_mode == "cross" {
        let Some((locked_asset, _, _)) = cross_account.as_ref() else {
            return Err(AppError::Conflict(
                "margin product mode changed while acquiring cross account lock".to_owned(),
            ));
        };
        if *locked_asset != product.margin_asset {
            return Err(AppError::Conflict(
                "margin product asset changed while acquiring cross account lock".to_owned(),
            ));
        }
    } else if cross_account.is_some() {
        return Err(AppError::Conflict(
            "margin product mode changed while acquiring cross account lock".to_owned(),
        ));
    }
    validate_product_margin(&request.margin_amount, &request.leverage, &product)?;
    if let Some(limit_price) = order.limit_price.as_ref() {
        validate_margin_limit_price(limit_price, product.price_precision)
            .map_err(|message| AppError::Validation(message.to_owned()))?;
    }
    let notional_amount = request.margin_amount.clone() * request.leverage.clone();
    let borrowed_amount = margin_borrowed_amount(&notional_amount, &request.margin_amount);
    let market_price =
        cached_margin_entry_price(redis, product.pair_id, product.symbol.as_str()).await?;
    let entry_price = match order.order_type {
        MarginOrderType::Market => Some(market_price.clone()),
        MarginOrderType::Limit => {
            let limit_price = order
                .limit_price
                .as_ref()
                .expect("validated limit order must carry a limit price");
            if margin_limit_order_is_triggered(&direction, limit_price, &market_price)
                .map_err(|message| AppError::Validation(message.to_owned()))?
            {
                Some(market_price.clone())
            } else {
                None
            }
        }
    };
    let is_filled = entry_price.is_some();
    if let Some((margin_asset, account, _)) = cross_account.as_mut() {
        if is_filled {
            activate_cross_margin_account_for_open(&mut tx, user_id, *margin_asset, account)
                .await?;
        } else {
            // 未成交挂单不进入账户风险集合；已清算账户也不能挂一笔永远无法触发的 cross 委托。
            require_active_cross_margin_account(account)?;
        }
    }
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
        order.order_type.as_str(),
        order.limit_price.as_ref(),
        entry_price.as_ref(),
        &idempotency_key,
    )
    .await
    {
        Ok(position_id) => position_id,
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_position(pool, user_id, &idempotency_key, &idempotency_intent)
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
        &position_margin_mode,
    )
    .await?;
    set_margin_position_wallet_scope(&mut tx, position_id, &wallet_scope).await?;
    if is_filled {
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
    }
    if let Some((margin_asset, account, created)) = cross_account.take() {
        if is_filled {
            bump_cross_margin_account_version(&mut tx, user_id, margin_asset, account.version)
                .await?;
        } else if created {
            discard_new_cross_margin_account_for_pending_order(
                &mut tx,
                user_id,
                margin_asset,
                account.version,
            )
            .await?;
        }
    }
    let position = load_position_by_id(&mut tx, position_id).await?;
    tx.commit().await?;
    Ok((position, is_filled))
}

/// 调用保证金开仓事务，并仅在本请求首次提交且已真实成交时发布用户私有开仓事件。
/// 同键重放不再扣抵押或发事件；未触发限价单等后续 ticker 成交用例提交后再发事件。
/// 事件发布严格排在 `open_margin_position` 返回之后，因此一定发生在数据库提交成功之后，
/// 不会出现事务回滚但客户端已收到开仓通知的情况；广播失败也不会反向影响已提交的资金结果。
pub(crate) async fn open_margin_position_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    request: OpenMarginPositionRequest,
) -> AppResult<OpenMarginPositionResponse> {
    let (position, is_new_fill) = open_margin_position(pool, redis, user_id, request).await?;
    let response = OpenMarginPositionResponse { position };
    publish_margin_position_opened_event_if_needed(hub, user_id, &response.position, is_new_fill);
    Ok(response)
}

/// 在唯一键冲突之后强制取回既有仓位，用于「插入失败说明键已被占用」这条确定性分支。
/// 与可空版本的差别是这里查不到记录会返回冲突而不是 None，因为此时另一个并发事务尚未提交，
/// 客户端应当重试而不是被误判为产品不存在。异参冲突仍由逐字段核对给出。
async fn replay_existing_position(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
    intent: &MarginOpenIdempotencyIntent<'_>,
) -> AppResult<MarginPositionResponse> {
    replay_existing_position_if_present(pool, user_id, idempotency_key, intent)
        .await?
        .ok_or_else(|| AppError::Conflict("margin idempotency key is being committed".to_owned()))
}

/// 开新事务对幂等键加行锁读取既有仓位，命中则逐字段核对后返回，未命中返回 None。
/// 用 FOR UPDATE 而非只读查询，是为了等待并发事务落定，避免在对方提交瞬间读到空结果。
/// 未命中时直接返回而不提交事务，让它随作用域结束自动回滚，反正没有任何写入。
/// 核对不通过时同样不提交，冲突错误先于提交返回，因此这条路径永远不产生资金副作用。
async fn replay_existing_position_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
    intent: &MarginOpenIdempotencyIntent<'_>,
) -> AppResult<Option<MarginPositionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        existing_position_for_idempotency_key(&mut tx, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    ensure_existing_position_matches_request(&existing, intent)?;
    tx.commit().await?;
    Ok(Some(existing))
}

/// 核对幂等键命中的既有仓位与本次请求是否描述同一笔开仓，防止同键复用到不同下单意图。
/// 比对产品、方向、保证金额、杠杆、订单类型与限价；保证金模式只在本次请求显式指定时才参与比对，
/// 因为缺省模式会在服务端按产品默认值推导，未传时不应把它当成不匹配。
/// 任一项不同返回 Conflict，调用方据此拒绝请求，绝不能落到复用旧仓位或重新扣款。
fn ensure_existing_position_matches_request(
    existing: &MarginPositionResponse,
    intent: &MarginOpenIdempotencyIntent<'_>,
) -> AppResult<()> {
    if existing.product_id != intent.product_id
        || existing.direction != intent.direction
        || intent
            .margin_mode
            .is_some_and(|mode| existing.margin_mode != mode)
        || existing.margin_amount != *intent.margin_amount
        || existing.leverage != *intent.leverage
        || existing.order_type != intent.order_type
        || existing.limit_price.as_ref() != intent.limit_price
    {
        return Err(AppError::Conflict(
            "margin idempotency key belongs to a different request".to_owned(),
        ));
    }
    Ok(())
}

/// 用事务内锁定到的产品规则复核本次开仓：状态、保证金区间、杠杆档位和小时利率。
/// 产品已停用时返回 NotFound 而非参数错误，对外表现与产品不存在一致，不泄漏配置状态。
/// 保证金低于最小值或高于可选的最大值都判为参数非法；最大值未配置表示不设上限。
/// 最后复查产品的小时利率合法性，因为历史配置可能早于当前精度规则，越界必须在扣抵押前失败。
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

/// 要求请求的杠杆倍数精确命中产品配置的某一个档位，不接受区间内的任意取值。
/// 逐档把存储的字符串解析回十进制做精确相等比较，无法解析的档位视为不匹配并继续尝试下一档。
/// 一档都不命中返回参数错误，避免用户绕过档位配置直接开出非标准倍数的仓位。
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

/// 确定本次开仓最终采用的保证金模式：请求显式指定则归一化后使用，否则回落到产品默认模式。
/// 选定的模式必须同时出现在产品支持列表里，并且是后端风控真正实现的逐仓或全仓之一。
/// 该结果直接决定抵押从哪个钱包扣、是否补建全仓账户，以及平仓时走单仓返还还是账户级权益结算。
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

/// 计算开仓借款额，口径为名义价值减去自有保证金，并按十八位小数做非负截断。
/// 名义价值由保证金乘杠杆得出，因此一倍杠杆时借款额恰好为零，利息 worker 对它计提结果也是零。
/// 该值只记录在仓位行上用于计息和展示，不代表任何实际转账，开仓时只扣自有保证金部分。
fn margin_borrowed_amount(notional_amount: &BigDecimal, margin_amount: &BigDecimal) -> BigDecimal {
    non_negative_amount(&(notional_amount.clone() - margin_amount.clone()))
}

/// 把开仓方向裁剪空白并折叠为小写后限制为 long 或 short，其余取值判为参数非法。
/// 与保证金模式不同，这里接受大小写混写，方向是历史上客户端写法最不统一的字段。
/// 归一化后的值既写入仓位行，也参与幂等重放的逐字段比对和平仓时的盈亏方向判定。
fn normalize_direction(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "long" => Ok("long".to_owned()),
        "short" => Ok("short".to_owned()),
        _ => Err(AppError::Validation(
            "margin direction must be long or short".to_owned(),
        )),
    }
}

/// 裁剪并校验开仓幂等键：空白视为缺失并报必填，长度按字节数限制在两百五十五以内。
/// 杠杆开仓的幂等键是必填项而非划转那样可由服务端补 UUID，客户端必须自己保证同一笔下单复用同一个键。
/// 归一化后的文本与用户标识共同构成唯一约束，是先占键后扣款这套防重复扣款机制的基础。
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
/// 在进入任何资金事务前把请求归一化为市价或限价语义。
/// 历史客户端未传 `order_type` 时仍视为市价；市价单严禁携带 `price`，限价单则必须携带严格为正的价格。
/// `trigger_price` 不属于当前杠杆订单模型，两种订单都一律拒绝，防止客户端把未实现能力当成已提交。
/// 交易对小数精度依赖事务内锁定的产品快照，因此此处只验正数，精度在锁产品后再次验证。
fn validate_open_order_semantics(
    request: &OpenMarginPositionRequest,
) -> AppResult<ValidatedMarginOpenOrder> {
    if request.trigger_price.is_some() {
        return Err(AppError::Validation(
            "margin orders must not include trigger_price".to_owned(),
        ));
    }
    let order_type = MarginOrderType::parse(request.order_type.as_deref())
        .map_err(|message| AppError::Validation(message.to_owned()))?;
    let limit_price = match order_type {
        MarginOrderType::Market => {
            if request.price.is_some() {
                return Err(AppError::Validation(
                    "margin market orders must not include price".to_owned(),
                ));
            }
            None
        }
        MarginOrderType::Limit => {
            let price = request.price.as_ref().ok_or_else(|| {
                AppError::Validation("margin limit orders require price".to_owned())
            })?;
            if price <= &BigDecimal::from(0) {
                return Err(AppError::Validation(
                    "margin limit price must be positive".to_owned(),
                ));
            }
            Some(price.clone())
        }
    };
    Ok(ValidatedMarginOpenOrder {
        order_type,
        limit_price,
    })
}

#[cfg(test)]
#[path = "../../../../tests/unit_src/src_modules_margin_open_position_tests.rs"]
mod tests;
