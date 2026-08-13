//! new_coin bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 本文件承载新币发行的全部用户侧用例：项目列表与详情、申购与买入下单、
//! 申购与分发与买入与解禁四类历史查询，以及解禁手续费缴纳和锁仓释放。
//! 每个写用例都拆成 `*_with_events` 外壳与 `*_with_internal` 内核两层：
//! 内核只负责校验、编排与调用仓储事务并回吐事件载荷，
//! 外壳负责在内核成功返回后才向事件广播中心发布用户私有事件。
//! 这样拆分保证事件绝不会在事务失败或幂等重放时被误发，
//! 也让路由层完全不感知事件消息体的结构。
//! 本层自身不开启数据库事务，原子性由 infrastructure 的仓储实现负责；
//! 本层承担的是事务之前的业务校验与事务之后的通知编排。
//! 所有用例的用户范围一律从会话 subject 解析，绝不接受请求参数传入的用户标识。

use crate::{
    error::{AppError, AppResult},
    modules::{
        events::{EventBroadcastHub, EventBroadcastMessage},
        new_coin::{
            LifecycleStatus,
            infrastructure::MySqlNewCoinReadRepository,
            presentation::{
                CreatePurchaseRequest, CreateSubscriptionRequest, ListQuery,
                NewCoinDistributionResponse, NewCoinDistributionsResponse,
                NewCoinOrderCreationResponse, NewCoinProjectResponse, NewCoinProjectsResponse,
                NewCoinPurchaseResponse, NewCoinPurchasesResponse, NewCoinSubscriptionResponse,
                NewCoinSubscriptionsResponse, NewCoinUnlockResponse, NewCoinUnlocksResponse,
                PayUnlockFeeRequest, PayUnlockFeeResponse, ReleaseUnlockResponse,
            },
            repository::{
                NewCoinOrderRepository, NewCoinPurchaseOrderWrite, NewCoinReadRepository,
                NewCoinSubscriptionOrderWrite, NewCoinUnlockFeeRepository,
                NewCoinUnlockReleaseRepository, UnlockFeePaymentWrite,
            },
            service::{
                ensure_idempotency_key, ensure_positive_amount,
                ensure_post_listing_purchase_enabled, ensure_unlock_fee_payment_matches,
                lifecycle_status, lock_positions_for_project, route_limit, user_id_from_subject,
            },
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde_json::json;
use sqlx::{MySql, Pool};

/// 只返回启用新币项目，列表上限由服务规则约束且不推断购买交易对。
/// 条数经 `route_limit` 夹取到 1 到 100，缺省为 50，因此超范围参数退化为边界值而不报错。
/// 这是公开用例，不解析会话主体也不做用户过滤，返回内容对所有访客一致。
pub(crate) async fn list_new_coin_projects(
    pool: Option<Pool<MySql>>,
    query: ListQuery,
) -> AppResult<NewCoinProjectsResponse> {
    let repository = new_coin_read_repository(pool)?;
    let projects = repository
        .list_active_projects(route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinProjectResponse::from)
        .collect();
    Ok(NewCoinProjectsResponse { projects })
}

/// 按符号读取启用项目；不存在或停用返回 NotFound，不回退到后台草稿。
/// 把仓储的 `Option` 显式折成 `NotFound`，使「项目不存在」与「查询失败」在响应上彻底分开。
/// 输出视图与列表用例完全一致，因此详情页与列表页看到的生命周期和解禁配置口径相同。
pub(crate) async fn get_new_coin_project(
    pool: Option<Pool<MySql>>,
    symbol: &str,
) -> AppResult<NewCoinProjectResponse> {
    let repository = new_coin_read_repository(pool)?;
    repository
        .find_active_project_by_symbol(symbol)
        .await?
        .map(NewCoinProjectResponse::from)
        .ok_or(AppError::NotFound)
}

/// 按认证用户读取新币申购单，用户编号从会话 subject 解析，查询条件不允许跨账户结果。
/// subject 格式不符直接返回未授权，此时不会建立仓储也不会触碰数据库。
/// 返回的每条记录同时包含申请数量与最终配额数量，本用例不做中签率换算或状态推进。
/// 条数沿用公共上限规则，结果按下单时间由新到旧排列。
pub(crate) async fn list_new_coin_subscriptions(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let subscriptions = repository
        .list_user_subscriptions(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinSubscriptionResponse::from)
        .collect();
    Ok(NewCoinSubscriptionsResponse { subscriptions })
}

/// 按认证用户读取新币分发记录，展示每次认购结果落到钱包的动作，不重新计算分配数量或推进状态。
/// 记录中关联申购单的字段可空，为空表示该笔分发并非来自申购流程；
/// 锁仓位置字段可空，为空表示当时按项目规则无需锁仓、资产已直接进入可用余额。
/// 本用例是纯读路径，不会补发遗漏的分发，也不校验引用的锁仓此刻是否仍然存在。
pub(crate) async fn list_new_coin_distributions(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let distributions = repository
        .list_user_distributions(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinDistributionResponse::from)
        .collect();
    Ok(NewCoinDistributionsResponse { distributions })
}

/// 按认证用户读取上市后二级市场买入记录，不以公开项目列表替代历史订单。
/// 返回的价格、数量与计价总额是下单当时固化的快照，不随行情或后台改配置而变化，可直接用于对账。
/// 因此当用户看到的历史成交价与当前项目发行价不一致时属于预期行为，本用例不做任何修正。
/// 用户范围来自会话 subject，条数沿用公共上限规则。
pub(crate) async fn list_new_coin_purchases(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinPurchasesResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let purchases = repository
        .list_user_purchases(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinPurchaseResponse::from)
        .collect();
    Ok(NewCoinPurchasesResponse { purchases })
}

/// 按认证用户读取最近的锁仓解锁记录并映射到账数量、到期时间和解锁费状态，条数按公共列表上限裁剪。
/// 查询只使用用户范围且不加行锁，不执行缴费、释放锁仓或钱包入账；存储失败直接返回而不伪造空历史。
pub(crate) async fn list_new_coin_unlocks(
    pool: Option<Pool<MySql>>,
    subject: &str,
    query: ListQuery,
) -> AppResult<NewCoinUnlocksResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let unlocks = repository
        .list_user_unlocks(user_id, route_limit(query.limit))
        .await?
        .into_iter()
        .map(NewCoinUnlockResponse::from)
        .collect();
    Ok(NewCoinUnlocksResponse { unlocks })
}

/// 校验解锁记录归属及配置的手续费资产与金额，再把费用状态从非 paid 更新为 paid。
/// 先按解禁幂等键与用户读取应收口径，读不到即返回 `NotFound`，据此同时挡住不存在的记录和越权访问。
/// 随后由 `ensure_unlock_fee_payment_matches` 拦截四类非法缴费：本不收费、支付资产不符、
/// 记录未配置应收金额、金额不等；金额比较按 normalized 进行，仅 scale 不同的等值支付会被接受。
/// 当前实现只更新解锁记录，不扣钱包也不写资金流水；状态守卫压在 UPDATE 的 WHERE 内，
/// 因此重复调用不报错而是返回 `paid=false`，调用方须据该字段而非 HTTP 状态判断本次是否生效。
/// 本用例不发布事件，缴费成功也不触发任何广播。
pub(crate) async fn pay_new_coin_unlock_fee(
    pool: Option<Pool<MySql>>,
    subject: &str,
    unlock_idempotency_key: String,
    request: PayUnlockFeeRequest,
) -> AppResult<PayUnlockFeeResponse> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let expectation = repository
        .find_unlock_fee_expectation(&unlock_idempotency_key, user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    ensure_unlock_fee_payment_matches(&expectation, request.payment_asset_id, &request.amount)?;
    let paid = repository
        .mark_unlock_fee_paid(UnlockFeePaymentWrite {
            unlock_idempotency_key: unlock_idempotency_key.clone(),
            user_id,
            payment_asset_id: request.payment_asset_id,
            amount: request.amount,
        })
        .await?;

    Ok(PayUnlockFeeResponse {
        unlock_idempotency_key,
        paid,
    })
}

/// 释放已缴费且到期的新币锁仓，并仅在钱包到账事务提交后发布用户私有解锁事件。
/// 本函数是事件外壳：先由内核完成校验与仓储事务，只有内核回吐了资产与数量才广播事件，
/// 因此事务回滚或幂等重放这两种情况下都不会有通知发出。
/// 事件类型为 `new_coin.unlock.released`，携带解禁幂等键、资产编号与释放数量，
/// 且固定以私有频道投递给解析出的用户，不进入任何公共频道。
/// 广播中心未配置时静默跳过通知，资金结果不受影响，因为释放在此之前已经提交。
/// 已释放记录重放时不二次入账、不广播事件，但兼容响应仍返回 `released=true`；
/// 未到期、未缴费或持久化失败一律返回错误且此前无任何资金变动。
pub(crate) async fn release_new_coin_unlock_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    unlock_idempotency_key: String,
) -> AppResult<ReleaseUnlockResponse> {
    // 应用层统一处理解锁放行后的私有事件广播，路由层不再感知解锁结果格式。
    let user_id = user_id_from_subject(subject)?;
    let (response, released_outcome) =
        release_new_coin_unlock_with_internal(pool, subject, unlock_idempotency_key.clone())
            .await?;
    if let Some((asset_id, unlock_quantity)) = released_outcome
        && let Some(hub) = event_broadcast_hub
    {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.unlock.released",
                "unlock_idempotency_key": unlock_idempotency_key,
                "asset_id": asset_id,
                "unlock_quantity": unlock_quantity,
                "released": true,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

/// 解锁释放的内核：解析用户、调用仓储的到期释放事务，并把结果拆成响应体与可选事件载荷。
/// 事件载荷只在仓储确实释放了资产时才为 `Some`，命中幂等重放时为 `None`，
/// 外壳正是依赖这一区分来决定是否广播，因此内核绝不能对重放也返回载荷。
/// 响应体的 `released` 恒为 `true`，与是否真正发生资金变动无关，
/// 这是为兼容既有客户端而保留的行为：调用方无法从响应区分首次释放与重放。
/// 本函数不广播事件也不开启事务，到期判定、缴费判定与钱包入账全在仓储的单个事务内完成。
async fn release_new_coin_unlock_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    unlock_idempotency_key: String,
) -> AppResult<(ReleaseUnlockResponse, Option<(u64, BigDecimal)>)> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let outcome = repository
        .release_due_paid_unlock(&unlock_idempotency_key, user_id)
        .await?;
    let event_payload = if outcome.released {
        Some((outcome.asset_id, outcome.unlock_quantity))
    } else {
        None
    };

    Ok((
        ReleaseUnlockResponse {
            unlock_idempotency_key,
            released: true,
        },
        event_payload,
    ))
}

/// 创建新币申购：先校验用户、subscription 生命周期、正金额与幂等键，再生成锁仓计划并交由仓储原子扣减计价钱包。
/// 本函数是事件外壳，只负责在内核成功返回后把申购结果广播给下单用户本人。
/// 事件类型为 `new_coin.subscription.created`，携带幂等键、项目与资产编号、计价资产与金额、
/// 申购数量、订单状态和锁仓位置编号，字段全部取自内核回吐的载荷而非重新查库。
/// 仓储提交订单、余额、流水和锁仓后才发布该私有事件；广播中心未配置时静默跳过，不影响资金结果。
/// 重复幂等键由仓储返回 Conflict，此时既不扣款也不广播，因此重放不会产生第二条通知。
pub(crate) async fn create_new_coin_subscription_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    symbol: String,
    request: CreateSubscriptionRequest,
) -> AppResult<NewCoinOrderCreationResponse> {
    // 应用层负责编排认购事件，仅在成功创建后发布统一事件体。
    let user_id = user_id_from_subject(subject)?;
    let (response, event_payload) =
        create_new_coin_subscription_with_internal(pool, subject, symbol, request).await?;
    if let Some(payload) = event_payload
        && let Some(hub) = event_broadcast_hub
    {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.subscription.created",
                "idempotency_key": payload.idempotency_key,
                "project_id": payload.project_id,
                "asset_id": payload.asset_id,
                "quote_asset_id": payload.quote_asset_id,
                "quote_amount": payload.quote_amount,
                "quantity": payload.quantity,
                "status": payload.status,
                "lock_position_id": payload.lock_position_id,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

struct NewCoinSubscriptionEventPayload {
    idempotency_key: String,
    project_id: u64,
    asset_id: u64,
    quote_asset_id: u64,
    quote_amount: BigDecimal,
    quantity: BigDecimal,
    status: String,
    lock_position_id: Option<u64>,
}

/// 申购下单的内核：按符号取项目规则、执行全部前置校验、算出锁仓计划，再交仓储在单事务中落地。
/// 校验顺序为项目存在、生命周期恰为 subscription、支付金额与申购数量均为正、幂等键去空后非空，
/// 任一失败都在建立事务之前返回，因此不会占用行锁也不会留下半张订单。
/// 项目规则在此处不加锁读取，仓储的申购事务也不会重新锁定项目行，
/// 所以本路径不防御「校验通过后后台改规则」的竞态，这一点与买入路径不同。
/// 锁仓计划以幂等键作为来源编号、以当前时刻作为来源时间生成，
/// 因此相对周期类解禁从下单时刻起算，且同一幂等键重跑会得到相同的来源标识。
/// 响应状态由仓储返回的锁仓编号推导：有编号说明资产被锁仓记为 allocated，
/// 无编号说明按规则直接到账记为 available。
/// 资金动作是扣减计价资产可用余额并分配新币，全部由仓储在单个事务内完成；
/// 本函数不开启事务也不广播事件，事件载荷仅回吐给外壳。
async fn create_new_coin_subscription_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    symbol: String,
    request: CreateSubscriptionRequest,
) -> AppResult<(
    NewCoinOrderCreationResponse,
    Option<NewCoinSubscriptionEventPayload>,
)> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let project = repository
        .find_project_rule_by_symbol(&symbol)
        .await?
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Subscription {
        return Err(AppError::Validation(
            "new coin subscription is not open for this project".to_owned(),
        ));
    }
    ensure_positive_amount(&request.quote_amount, "quote_amount")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    ensure_idempotency_key(&request.idempotency_key)?;

    let idempotency_key = request.idempotency_key.clone();
    let quantity = request.quantity.clone();
    let quote_amount = request.quote_amount.clone();
    let quote_asset_id = request.quote_asset_id;
    let lock_positions = lock_positions_for_project(
        &project,
        user_id,
        project.asset_id,
        &idempotency_key,
        quantity.clone(),
        Utc::now(),
        "new_coin_subscription",
    )?;
    let lock_position_id = repository
        .create_subscription_order(NewCoinSubscriptionOrderWrite {
            user_id,
            project: project.clone(),
            quote_asset_id,
            quote_amount: quote_amount.clone(),
            quantity: quantity.clone(),
            idempotency_key: idempotency_key.clone(),
            lock_positions,
        })
        .await?;
    let status = if lock_position_id.is_some() {
        "allocated".to_owned()
    } else {
        "available".to_owned()
    };
    let response = NewCoinOrderCreationResponse {
        idempotency_key,
        status,
        lock_position_id,
    };
    let event_payload = NewCoinSubscriptionEventPayload {
        idempotency_key: response.idempotency_key.clone(),
        project_id: project.id,
        asset_id: project.asset_id,
        quote_asset_id,
        quote_amount,
        quantity,
        status: response.status.clone(),
        lock_position_id,
    };
    Ok((response, Some(event_payload)))
}

/// 创建上市后购买：要求 listed、后台开关开启且 `pair_id` 精确匹配批准交易对，再按 `price × quantity` 原子扣款并分配新币。
/// 本函数是事件外壳，只负责在内核成功返回后把成交结果广播给下单用户本人。
/// 事件类型为 `new_coin.purchase.created`，比申购事件多出交易对编号与成交价格两个字段，
/// 使订阅方无需回查即可还原这笔二级市场买入的完整成交口径。
/// 仓储提交订单、余额、流水和锁仓后才发布该私有事件；广播中心未配置时静默跳过。
/// 重复幂等键返回 Conflict，不产生第二笔资金变更，也不会重复发出通知。
pub(crate) async fn create_new_coin_purchase_with_events(
    pool: Option<Pool<MySql>>,
    event_broadcast_hub: Option<&EventBroadcastHub>,
    subject: &str,
    symbol: String,
    request: CreatePurchaseRequest,
) -> AppResult<NewCoinOrderCreationResponse> {
    // 应用层统一认购后事件构建，避免路由层感知具体消息体。
    let user_id = user_id_from_subject(subject)?;
    let (response, event_payload) =
        create_new_coin_purchase_with_internal(pool, subject, symbol, request).await?;
    if let Some(payload) = event_payload
        && let Some(hub) = event_broadcast_hub
    {
        hub.publish(EventBroadcastMessage::private_user(
            user_id,
            json!({
                "type": "new_coin.purchase.created",
                "idempotency_key": payload.idempotency_key,
                "project_id": payload.project_id,
                "pair_id": payload.pair_id,
                "asset_id": payload.asset_id,
                "quote_asset_id": payload.quote_asset_id,
                "price": payload.price,
                "quantity": payload.quantity,
                "quote_amount": payload.quote_amount,
                "status": payload.status,
                "lock_position_id": payload.lock_position_id,
            })
            .to_string(),
        ));
    }
    Ok(response)
}

struct NewCoinPurchaseEventPayload {
    idempotency_key: String,
    project_id: u64,
    pair_id: u64,
    asset_id: u64,
    quote_asset_id: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    quote_amount: BigDecimal,
    status: String,
    lock_position_id: Option<u64>,
}

/// 买入下单的内核：校验项目已上市与购买开关、确认交易对、算出计价总额，再交仓储在单事务中落地。
/// 校验顺序为项目存在、生命周期恰为 listed、后台开关开启且请求交易对等于批准的那一个、
/// 价格与数量均为正、幂等键非空，之后才回查交易对确认其基础资产确为项目资产。
/// 计价总额由价格乘数量得出，`BigDecimal` 相乘不丢精度也不做舍入，落库精度由数据库列定义决定。
/// 与申购路径的关键差异在于仓储的买入事务会重新锁定项目行与交易对行并再校验一次，
/// 因此这里的不加锁预校验只用于快速失败，真正的成交口径以事务内重读的配置为准。
/// 锁仓计划同样在仓储事务内基于成交时刻现算，本函数不预先生成。
/// 响应状态由锁仓编号推导：有编号记为 locked，无编号说明直接到账记为 available。
/// 本函数不开启事务也不广播事件，事件载荷仅回吐给外壳，其中计价资产取自回查到的交易对。
async fn create_new_coin_purchase_with_internal(
    pool: Option<Pool<MySql>>,
    subject: &str,
    symbol: String,
    request: CreatePurchaseRequest,
) -> AppResult<(
    NewCoinOrderCreationResponse,
    Option<NewCoinPurchaseEventPayload>,
)> {
    let user_id = user_id_from_subject(subject)?;
    let repository = new_coin_read_repository(pool)?;
    let project = repository
        .find_project_rule_by_symbol(&symbol)
        .await?
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    // 上市后认购必须服从后台单独开关和绑定交易对，避免用户绕过后台配置直接下单。
    ensure_post_listing_purchase_enabled(&project, request.pair_id)?;
    ensure_positive_amount(&request.price, "price")?;
    ensure_positive_amount(&request.quantity, "quantity")?;
    ensure_idempotency_key(&request.idempotency_key)?;

    let pair = repository
        .find_pair_for_purchase(request.pair_id, project.asset_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let idempotency_key = request.idempotency_key.clone();
    let price = request.price.clone();
    let quantity = request.quantity.clone();
    let quote_amount = price.clone() * quantity.clone();
    let pair_id = request.pair_id;
    let lock_position_id = repository
        .create_purchase_order(NewCoinPurchaseOrderWrite {
            user_id,
            project: project.clone(),
            pair_id,
            price: price.clone(),
            quantity: quantity.clone(),
            quote_amount: quote_amount.clone(),
            idempotency_key: idempotency_key.clone(),
        })
        .await?;
    let status = if lock_position_id.is_some() {
        "locked".to_owned()
    } else {
        "available".to_owned()
    };
    let response = NewCoinOrderCreationResponse {
        idempotency_key,
        status,
        lock_position_id,
    };
    let event_payload = NewCoinPurchaseEventPayload {
        idempotency_key: response.idempotency_key.clone(),
        project_id: project.id,
        pair_id,
        asset_id: project.asset_id,
        quote_asset_id: pair.quote_asset_id,
        price,
        quantity,
        quote_amount,
        status: response.status.clone(),
        lock_position_id,
    };
    Ok((response, Some(event_payload)))
}

/// 从可选连接池装配新币仓储适配器，是本文件所有用例接触持久化的唯一入口。
/// 连接池缺失时透过 `new_coin_mysql_pool` 转成内部错误，因此各用例无需各自处理未配置的情况。
/// 构造本身不发起连接或查询，失败只可能来自配置缺失，不会因数据库不可达而在此报错。
fn new_coin_read_repository(pool: Option<Pool<MySql>>) -> AppResult<MySqlNewCoinReadRepository> {
    Ok(MySqlNewCoinReadRepository::new(new_coin_mysql_pool(pool)?))
}

/// 把可选连接池解包为必需连接池，缺失时返回带明确说明的 `Internal` 错误。
/// 归为内部错误而非参数校验错误，是因为连接池缺失属于部署配置问题而不是调用方的输入问题，
/// 这样既不会误导客户端重试，也能让该情况在监控中与业务拒绝区分开。
fn new_coin_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for new coin routes".to_owned())
    })
}
