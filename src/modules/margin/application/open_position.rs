//! 杠杆市价开仓用例。
//!
//! 这是本上下文风险最高的资金入口，负责把一次开仓请求变成仓位行、抵押扣款、代理返佣和一条私有事件。
//! 关键顺序是：事务外校验并做只读幂等预检，事务内锁产品、写仓位占用幂等键、再锁钱包扣抵押。
//! 先占键后动钱是防重复扣款的核心，唯一键冲突会回滚并转入只读重放，逐字段核对后返回既有仓位。
//! 入场价一律取自服务端 Redis 行情缓存，请求体不接受任何客户端价格；名义价值等于保证金乘杠杆，
//! 借款额为名义价值减保证金并非负截断，两者都按十八位小数落库。
//! 事件只在事务提交且确实新建仓位时由包装函数发布，重放和失败路径都不广播。

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
            infrastructure::{
                MarginOpenProductRule, cached_margin_entry_price,
                debit_margin_position_open_collateral, ensure_cross_margin_account,
                existing_position_for_idempotency_key,
                existing_position_for_idempotency_key_readonly, insert_margin_position,
                load_position_by_id, lock_active_open_product, set_margin_position_wallet_scope,
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
/// 校验市价开仓语义和用户幂等键，以服务端新鲜行情作为唯一入场价后创建保证金仓位。
/// 事务先占用幂等键，再锁钱包扣抵押、写流水及返佣；同键同参重放不再次扣款，任一步失败整体回滚。
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
    validate_market_open_order_semantics(&request)?;
    let idempotency_key = normalize_idempotency_key(&request.idempotency_key)?;
    let direction = normalize_direction(&request.direction)?;
    let requested_margin_mode = match request.margin_mode.as_deref() {
        Some(value) => Some(normalized_margin_mode(value)?),
        None => None,
    };
    validate_positive_decimal(&request.margin_amount, "margin amount")?;
    validate_positive_decimal(&request.leverage, "leverage")?;

    if let Some(existing) =
        existing_position_for_idempotency_key_readonly(pool, user_id, &idempotency_key).await?
    {
        ensure_existing_position_matches_request(
            &existing,
            request.product_id,
            &direction,
            requested_margin_mode.as_deref(),
            &request.margin_amount,
            &request.leverage,
        )?;
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    let product = match lock_active_open_product(&mut tx, request.product_id).await {
        Ok(product) => product,
        Err(AppError::NotFound) => {
            tx.rollback().await?;
            if let Some(existing) = replay_existing_position_if_present(
                pool,
                user_id,
                request.product_id,
                &direction,
                requested_margin_mode.as_deref(),
                &request.margin_amount,
                &request.leverage,
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
    let position_margin_mode =
        selected_open_margin_mode(&product, requested_margin_mode.as_deref())?;
    validate_product_margin(&request.margin_amount, &request.leverage, &product)?;
    let notional_amount = request.margin_amount.clone() * request.leverage.clone();
    let borrowed_amount = margin_borrowed_amount(&notional_amount, &request.margin_amount);
    let entry_price =
        cached_margin_entry_price(redis, product.pair_id, product.symbol.as_str()).await?;
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
        &entry_price,
        &idempotency_key,
    )
    .await
    {
        Ok(position_id) => position_id,
        Err(error) if is_duplicate_key_error(&error) => {
            tx.rollback().await?;
            return replay_existing_position(
                pool,
                user_id,
                request.product_id,
                &direction,
                requested_margin_mode.as_deref(),
                &request.margin_amount,
                &request.leverage,
                &idempotency_key,
            )
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
    if position_margin_mode == "cross" {
        ensure_cross_margin_account(&mut tx, user_id, product.margin_asset).await?;
    }
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
    let position = load_position_by_id(&mut tx, position_id).await?;
    tx.commit().await?;
    Ok((position, true))
}

/// 调用保证金开仓事务，并仅在首次提交新仓位后发布用户私有开仓事件。
/// 同键重放返回原仓位且不再扣抵押或发事件；行情及事务失败不产生广播。
/// 事件发布严格排在 `open_margin_position` 返回之后，因此一定发生在数据库提交成功之后，
/// 不会出现事务回滚但客户端已收到开仓通知的情况；广播失败也不会反向影响已提交的资金结果。
pub(crate) async fn open_margin_position_with_events(
    pool: &Pool<MySql>,
    redis: Option<&ConnectionManager>,
    hub: Option<&EventBroadcastHub>,
    user_id: u64,
    request: OpenMarginPositionRequest,
) -> AppResult<OpenMarginPositionResponse> {
    let (position, is_new_position) = open_margin_position(pool, redis, user_id, request).await?;
    let response = OpenMarginPositionResponse { position };
    publish_margin_position_opened_event_if_needed(
        hub,
        user_id,
        &response.position,
        is_new_position,
    );
    Ok(response)
}

/// 在唯一键冲突之后强制取回既有仓位，用于「插入失败说明键已被占用」这条确定性分支。
/// 与可空版本的差别是这里查不到记录会返回冲突而不是 None，因为此时另一个并发事务尚未提交，
/// 客户端应当重试而不是被误判为产品不存在。异参冲突仍由逐字段核对给出。
#[allow(clippy::too_many_arguments)] // 幂等核对必须逐字段比对原始下单语义，保持参数显式。
async fn replay_existing_position(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    margin_mode: Option<&str>,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<MarginPositionResponse> {
    replay_existing_position_if_present(
        pool,
        user_id,
        product_id,
        direction,
        margin_mode,
        margin_amount,
        leverage,
        idempotency_key,
    )
    .await?
    .ok_or_else(|| AppError::Conflict("margin idempotency key is being committed".to_owned()))
}

/// 开新事务对幂等键加行锁读取既有仓位，命中则逐字段核对后返回，未命中返回 None。
/// 用 FOR UPDATE 而非只读查询，是为了等待并发事务落定，避免在对方提交瞬间读到空结果。
/// 未命中时直接返回而不提交事务，让它随作用域结束自动回滚，反正没有任何写入。
/// 核对不通过时同样不提交，冲突错误先于提交返回，因此这条路径永远不产生资金副作用。
#[allow(clippy::too_many_arguments)] // 幂等核对必须逐字段比对原始下单语义，保持参数显式。
async fn replay_existing_position_if_present(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: u64,
    direction: &str,
    margin_mode: Option<&str>,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    let mut tx = pool.begin().await?;
    let Some(existing) =
        existing_position_for_idempotency_key(&mut tx, user_id, idempotency_key).await?
    else {
        return Ok(None);
    };
    ensure_existing_position_matches_request(
        &existing,
        product_id,
        direction,
        margin_mode,
        margin_amount,
        leverage,
    )?;
    tx.commit().await?;
    Ok(Some(existing))
}

/// 核对幂等键命中的既有仓位与本次请求是否描述同一笔开仓，防止同键复用到不同下单意图。
/// 比对产品、方向、保证金额和杠杆四项必比字段；保证金模式只在本次请求显式指定时才参与比对，
/// 因为缺省模式会在服务端按产品默认值推导，未传时不应把它当成不匹配。
/// 任一项不同返回 Conflict，调用方据此拒绝请求，绝不能落到复用旧仓位或重新扣款。
fn ensure_existing_position_matches_request(
    existing: &MarginPositionResponse,
    product_id: u64,
    direction: &str,
    margin_mode: Option<&str>,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
) -> AppResult<()> {
    if existing.product_id != product_id
        || existing.direction != direction
        || margin_mode.is_some_and(|mode| existing.margin_mode != mode)
        || existing.margin_amount != *margin_amount
        || existing.leverage != *leverage
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
/// 拒绝一切非市价的开仓语义：`order_type` 传了就必须忽略大小写等于 market，`price` 与
/// `trigger_price` 出现任意一个都直接判为参数非法，不做静默丢弃。
/// 因为杠杆入场价只认服务端行情缓存，容忍客户端价格字段会让人误以为存在限价或触发单能力。
/// 这道校验排在所有校验最前面，保证旧版客户端的限价请求在触碰幂等键之前就被挡住。
fn validate_market_open_order_semantics(request: &OpenMarginPositionRequest) -> AppResult<()> {
    if let Some(order_type) = request.order_type.as_deref()
        && !order_type.trim().eq_ignore_ascii_case("market")
    {
        return Err(AppError::Validation(
            "margin only supports market orders".to_owned(),
        ));
    }
    if request.price.is_some() || request.trigger_price.is_some() {
        return Err(AppError::Validation(
            "margin market orders must not include price or trigger_price".to_owned(),
        ));
    }
    Ok(())
}
