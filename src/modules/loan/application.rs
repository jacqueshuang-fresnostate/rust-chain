//! loan bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//!
//! 借贷订单状态机在此收敛：pending 可迁移到 cancelled、rejected 或 disbursed，
//! disbursed 与 overdue 可迁移到 repaid，终态之间不再互相迁移。
//! 四个状态迁移用例都遵循同一套骨架：开启事务、FOR UPDATE 锁订单、
//! 命中终态则提交并以 `changed=false` 返回实现幂等、状态不符则返回冲突、
//! 其余情况在同一事务内完成资金移动与状态写入，最后提交并回读响应。
//! 锁序固定为订单在前、钱包在后；还款场景再按贷款资产、抵押资产的顺序依次锁钱包。
//! 产品配置类用例不使用事务，写入与回读分两次自动提交，回读失败不会撤销已生效的配置。

use super::{
    LOAN_TYPE_COLLATERALIZED, STATUS_ACTIVE, STATUS_CANCELLED, STATUS_DISBURSED, STATUS_OVERDUE,
    STATUS_PENDING, STATUS_REJECTED, STATUS_REPAID, ensure_amount_precision,
    ensure_amount_within_product_limits, ensure_non_negative_amount, ensure_positive_amount,
    normalized_product_name_json, optional_string, product_default_name, route_limit, route_offset,
    validate_idempotency_key, validate_interest_mode, validate_loan_type, validate_product_status,
};
use crate::{
    error::{AppError, AppResult},
    modules::{
        loan::{
            infrastructure::{
                AdminLoanOrdersFilter, LoanOrderCreate, LoanProductWrite, apply_loan_wallet_credit,
                apply_loan_wallet_debit, apply_loan_wallet_freeze, ensure_loan_user_kyc_level,
                insert_loan_order_in_tx, insert_loan_product, is_duplicate_key_error,
                list_admin_loan_orders, list_admin_loan_products, list_loan_products,
                list_user_loan_orders, load_active_asset_meta, load_active_asset_meta_in_tx,
                load_loan_order_by_idempotency, load_loan_order_response,
                load_loan_product_response, load_user_loan_order_response,
                lock_active_loan_product_terms, lock_loan_order, lock_user_loan_order,
                mark_loan_order_cancelled_in_tx, mark_loan_order_disbursed_in_tx,
                mark_loan_order_rejected_in_tx, mark_loan_order_repaid_in_tx,
                release_loan_collateral_if_needed, update_loan_product, update_loan_product_status,
            },
            presentation::{
                AdminLoanOrdersQuery, AdminLoanOrdersResponse, AdminLoanProductsQuery,
                AdminLoanProductsResponse, CreateLoanOrderRequest, CreateLoanProductRequest,
                ListQuery, LoanOrderResponse, LoanOrdersResponse, LoanProductResponse,
                LoanProductsResponse, UpdateLoanProductRequest, UserLoanOrdersQuery,
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

/// 按产品编号倒序列出可申请的借贷产品，状态过滤硬编码为 active，调用方无法查看已下架配置。
/// 数量默认 50、夹紧到 1..=200，不支持偏移翻页，因此产品较多时只能看到最新的一批。
/// 不读取调用者的 KYC 等级、既有订单或钱包余额，返回的额度区间是否可用需下单时才知道。
pub(crate) async fn list_active_products_use_case(
    pool: &Pool<MySql>,
    query: ListQuery,
) -> AppResult<LoanProductsResponse> {
    // 查询公开可见的进行中产品列表，并做统一分页限制。
    let products = list_loan_products(pool, Some(STATUS_ACTIVE), route_limit(query.limit)).await?;
    Ok(LoanProductsResponse { products })
}

/// 先把可选的借贷类型与状态裁剪归一再做枚举校验，然后查询后台产品分页及匹配总数。
/// 空白筛选值被归一为不过滤，非空值必须是合法枚举，非法取值在执行 SQL 前就返回参数错误。
/// 与用户端列表不同，这里不强制只看 active，已下架产品同样可见以便运营核对历史配置。
/// 行数据与 total 由同一组谓词构建，因此总数始终跟随当前筛选而非全表行数。
/// 该只读用例不锁产品，也不改写任何订单条款或钱包余额。
pub(crate) async fn list_admin_products_use_case(
    pool: &Pool<MySql>,
    query: AdminLoanProductsQuery,
) -> AppResult<AdminLoanProductsResponse> {
    let loan_type = optional_string(query.loan_type)
        .map(|loan_type| validate_loan_type(&loan_type))
        .transpose()?;
    let status = optional_string(query.status)
        .map(|status| validate_product_status(&status))
        .transpose()?;
    // 非空筛选先完成枚举校验，再交给基础设施层构造参数化查询。
    let (products, total) = list_admin_loan_products(
        pool,
        loan_type.as_deref(),
        status.as_deref(),
        route_limit(query.limit),
        route_offset(query.offset),
    )
    .await?;
    Ok(AdminLoanProductsResponse { products, total })
}

/// 读取单个借贷产品的当前配置与关联资产符号，编号不存在时返回 NotFound。
/// 不加行锁，返回值只是即时快照，不能当作并发下单时的条款依据。
/// 下单流程会在事务内用 FOR UPDATE 重新锁定产品，并以彼时的利率、期限和额度写入订单。
pub(crate) async fn get_admin_product_use_case(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<LoanProductResponse> {
    // 查询单个贷款产品详情，找不到时返回 NotFound。
    load_loan_product_response(pool, product_id).await
}

/// 按当前用户读取借贷订单列表，可选状态过滤，按订单编号倒序等价于按创建时间从新到旧。
/// user_id 由调用方从 JWT 解析后传入并固定拼进 SQL 条件，请求参数无法覆盖，不存在跨用户读取。
/// 不锁订单也不锁钱包，返回的 interest_amount 与 repayment_amount 只在还款成功后才有值，
/// 未结清订单的这两项不代表当前应计利息，本用例不做任何实时计息。
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

/// 按用户与订单编号读取详情，归属隔离落在 SQL 的 user_id 条件而非应用层判断。
/// 他人订单与不存在的订单都返回 NotFound，调用方无法据错误码探测订单是否真实存在。
/// 只读且不加行锁，不触发取消、还款或抵押释放中的任何一种状态迁移。
pub(crate) async fn get_user_order_use_case(
    pool: &Pool<MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    // 查询某用户的订单详情，确保订单归属校验在 SQL 层过滤中完成。
    load_user_loan_order_response(pool, user_id, order_id).await
}

/// 把后台的用户编号、邮箱、产品、借贷类型和状态五项筛选打包成过滤器并查询订单分页与总数。
/// 与产品列表不同，这里的筛选值不做枚举校验，只在基础设施层裁剪空白，未知状态自然查不到数据。
/// 邮箱走前后通配的 LIKE 匹配，因此支持按域名或片段检索，其余条件均为等值匹配。
/// 分页量与偏移分别夹紧到 1..=200 和不超过十万，防止深分页把订单大表拖成全表扫描。
/// 该只读用例不获取订单或钱包行锁，也不触发审核、还款或抵押释放。
pub(crate) async fn list_admin_orders_use_case(
    pool: &Pool<MySql>,
    query: AdminLoanOrdersQuery,
) -> AppResult<AdminLoanOrdersResponse> {
    // 在后台列表里组装筛选条件，复用统一分页和基础设施查询。
    let (orders, total) = list_admin_loan_orders(
        pool,
        AdminLoanOrdersFilter {
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
            user_id: query.user_id,
            email: query.email,
            product_id: query.product_id,
            loan_type: query.loan_type,
            status: query.status,
        },
    )
    .await?;
    Ok(AdminLoanOrdersResponse { orders, total })
}

/// 按编号读取任意用户的借贷订单详情，供后台审核与客服排查，不施加归属限制。
/// 相比用户端详情少了 user_id 条件，因此无需区分订单不存在与无权访问。
/// 响应含审批人、拒绝人、拒绝原因等审计字段与各阶段时间戳，缺失阶段对应字段为空。
/// 不加行锁，返回值不能作为并发审批的判断依据，审批用例会在事务内重新锁行复核状态。
pub(crate) async fn get_admin_order_use_case(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    // 查询后台订单详情。
    load_loan_order_response(pool, order_id).await
}

/// 校验活动资产、金额精度、额度、计息模式与多语言名称后以单条写入创建借贷产品。
/// 产品写入和随后回读不是同一事务：写入成功但回读失败时产品仍已存在；本用例不修改用户钱包或订单。
pub(crate) async fn create_loan_product_use_case(
    pool: &Pool<MySql>,
    request: CreateLoanProductRequest,
) -> AppResult<LoanProductResponse> {
    let request = validate_create_product_request(pool, request).await?;
    let product_id = insert_loan_product(pool, request.into_write()).await?;
    load_loan_product_response(pool, product_id).await
}

/// 校验完整产品配置与活动资产精度后覆盖指定借贷产品，再以独立查询回读响应。
/// 更新成功后的回读失败不会撤销配置；既有订单已保存的利率、期限、额度和抵押资金状态不被改写。
pub(crate) async fn update_loan_product_use_case(
    pool: &Pool<MySql>,
    product_id: u64,
    request: UpdateLoanProductRequest,
) -> AppResult<LoanProductResponse> {
    let request = validate_update_product_request(pool, request).await?;
    update_loan_product(pool, product_id, request.into_write()).await?;
    load_loan_product_response(pool, product_id).await
}

/// 校验 active/disabled 后自动提交产品状态更新，再独立回读完整响应。
/// 状态只影响后续创建订单；回读失败不回滚已提交状态，也不处理已有订单或钱包。
pub(crate) async fn update_loan_product_status_use_case(
    pool: &Pool<MySql>,
    product_id: u64,
    status: String,
) -> AppResult<LoanProductResponse> {
    let status = validate_product_status(&status)?;
    update_loan_product_status(pool, product_id, &status).await?;
    load_loan_product_response(pool, product_id).await
}

/// 按当前产品条款创建用户借贷订单，并在抵押贷场景同步冻结抵押资产。
/// 用户须满足 KYC、金额及资产精度要求；抵押贷必须提供正数且精度合法的抵押资产数量。
/// 事务先锁定启用产品，再校验条款、插入订单，随后锁定钱包并完成可用额到冻结额的双流水迁移。
/// 订单、抵押余额和账本必须原子提交，任何失败都不得留下未足额抵押的有效订单。
/// 用户级幂等键唯一；重复插入会回滚当前事务并返回该键既有订单，`created=false` 且不再次冻结抵押。
/// 当前实现不会核对重放请求的产品、金额或抵押参数是否与旧订单一致，调用方必须保证同一键只表示同一请求。
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

/// 把创建产品请求转交统一校验流程，唯一的差异是 status 可选并默认取 active。
/// 其余字段的校验口径与更新请求完全一致，避免同一份配置在两条入口上出现宽严不一。
/// 校验期间会按 asset_id 读取启用资产的精度，因此本函数需要连接池而非纯内存运算。
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

/// 把更新产品请求转交统一校验流程，status 在此为必填而非可选，其余口径与创建一致。
/// 更新是整体覆盖语义，请求体缺字段等同于置空，本函数不会从数据库读回旧值做合并。
/// 同样需要连接池以读取目标资产的精度并据此校验最小额与最大额的小数位。
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

/// 已通过全部校验与归一化的产品配置，创建与更新两条入口在此汇合成同一形态。
/// 其中 name 可能已被多语言结构里的默认标题覆盖，name_json 则必定是合法结构。
struct NormalizedLoanProductRequest {
    loan_type: String,
    asset_id: u64,
    /// 用于列表展示的纯文本名称，优先取自多语言结构的默认语言标题。
    name: String,
    /// 已校验的多语言名称结构，缺省时由回退名称生成中文兜底条目。
    name_json: Value,
    term_days: u32,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    min_kyc_level: i32,
    min_amount: BigDecimal,
    /// 可为空表示该产品不设借款上限。
    max_amount: Option<BigDecimal>,
    status: String,
}

impl NormalizedLoanProductRequest {
    /// 移交所有权并原样转成基础设施层的写入载荷，字段一一对应不做二次加工。
    /// 类型区分的意义在于让「已校验」这一事实体现在类型上，未经校验的请求无法构造出写入载荷。
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

/// 集中执行借贷产品配置的全部准入校验，创建与更新共用这一条路径以保证口径一致。
/// 先归一三个枚举字段，再要求 name 裁剪后非空，随后补齐并校验多语言名称结构；
/// 若多语言结构中能取到默认语言标题，就用它覆盖纯文本 name，使两者展示一致。
/// 数值约束依次是：term_days 必须为正，interest_rate 允许为零但不得为负，
/// min_kyc_level 不得为负，min_amount 必须为正，max_amount 若存在须为正且不小于 min_amount。
/// 最后按 asset_id 读取资产元数据，资产必须处于 active，并用其 precision_scale 校验两个额度的小数位。
/// 全程只读数据库，不写产品表也不触碰订单与钱包；任一校验失败都在写入前返回参数错误。
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

/// 锁定当前用户 pending 订单，抵押贷先把 collateral frozen 等额退回 available，再标记 cancelled。
/// 锁序为订单→抵押钱包；释放写 available 正额与 frozen 负额两条 `loan_collateral_release` 流水，locked 不变。
/// 应用层拥有事务；已取消重放返回 `changed=false`，余额、双流水、释放时间或状态任一步失败都回滚。
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

/// 锁定 pending 订单后把订单本金增加到贷款资产 available，并以审核时刻加 term_days 记录到期时间。
/// 锁序为订单→贷款资产钱包；只写一条正向 `loan_disbursement` available 流水，frozen/locked 保持原值。
/// 余额、流水与 disbursed 状态同事务提交；已放款或已还款重放返回 `changed=false`，不二次入账。
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

/// 锁定 pending 订单，抵押贷把 collateral frozen 退回 available 后记录拒绝管理员与可选原因。
/// 锁序为订单→抵押钱包，释放写 available 正额与 frozen 负额两条流水；无抵押或已释放时不移动资金。
/// 应用层事务原子提交余额、流水、释放时间和 rejected 状态；已拒绝重放返回 `changed=false`。
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

/// 为当前用户结清已放款或逾期借贷订单，计算应计利息并释放抵押资产。
/// 订单须归属当前用户且具有放款时间；已还款订单直接返回原结果，其他状态拒绝操作。
/// 事务锁定订单后，按贷款资产精度向零截断利息及本金加利息，再从贷款资产 available 扣除总还款额。
/// 随后抵押贷把 collateral frozen 退回 available；实际锁序为订单→贷款钱包→抵押钱包，代码不按资产编号重排。
/// 还款写一条 `loan_repayment` available 负流水；抵押释放另写 available 正/frozen 负两条流水，locked 始终不变。
/// 钱包、流水、抵押释放时间与 repaid 状态同事务提交，任一步失败回滚本次扣款和释放。
/// 已还款状态构成幂等边界并返回 `changed=false`；余额不足或任一步失败均整体回滚。
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
    // 逾期订单仍必须可还款，否则抵押资产会被永久锁死。
    if order.status != STATUS_DISBURSED && order.status != STATUS_OVERDUE {
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
