//! loan bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//!
//! 本文件的函数分成三类。产品配置写入与后台审计共享应用层事务，读取列表仍直接使用连接池。
//! 以 `_in_tx` 结尾或接收 `Transaction` 的函数不自行开启也不提交事务，
//! 全部由应用层持有同一事务并统一提交或回滚，它们各自都不具备独立幂等性。
//! 资金原语共四个方向：freeze 把 available 挪到 frozen，unfreeze 反向，
//! credit 增加 available，debit 减少 available，locked 桶在借贷全流程中始终不变。
//! 每次余额变动都同步写入 `wallet_ledger`，双桶迁移写两条、单桶变动写一条，
//! 流水记录变动后的三桶完整快照并统一以 `loan_order` 加订单编号作为引用。

use crate::{
    error::{AppError, AppResult},
    modules::loan::presentation::{LoanOrderResponse, LoanProductResponse},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

/// 资产与产品共用的启用状态字面量，本层用它判定资产可用性与产品可下单性。
const STATUS_ACTIVE: &str = "active";
/// 钱包流水的引用类型，借贷产生的所有流水都用该值加订单编号回溯来源。
const REF_TYPE_LOAN_ORDER: &str = "loan_order";

/// 下单事务内锁定产品后取回的条款集合，这些值随即被复制进订单成为不可变快照。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct LoanProductTermsRow {
    pub(crate) id: u64,
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) status: String,
}

/// 状态迁移事务内以 FOR UPDATE 锁定订单后取回的最小字段集，只含判定与算账真正需要的列。
/// 计息三要素、抵押两要素和两个时间戳共同决定后续能否放款、还款或释放抵押。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct LoanOrderLockRow {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    /// 放款与还款所用的贷款资产。
    pub(crate) asset_id: u64,
    /// 借款本金，也是计息基数。
    pub(crate) amount: BigDecimal,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) term_days: u32,
    pub(crate) collateral_asset_id: Option<u64>,
    pub(crate) collateral_amount: Option<BigDecimal>,
    pub(crate) status: String,
    /// 放款时刻，为空则无法按实际天数计息，还款会被拒绝。
    pub(crate) disbursed_at: Option<DateTime<Utc>>,
    /// 抵押释放时刻，非空即视为已释放，可防止重复退回 frozen。
    pub(crate) collateral_released_at: Option<DateTime<Utc>>,
}

/// 资产元数据，精度决定金额校验与截断口径，状态决定该资产能否参与借贷资金流。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AssetMetaRow {
    /// 小数位上限，取值范围与钱包账本列一致。
    pub(crate) precision_scale: i32,
    /// 资产状态，非 active 时中止相关资金流程。
    pub(crate) status: String,
}

/// 下单事务内读取的用户实名等级，用于与产品门槛比对。
#[derive(Debug, sqlx::FromRow)]
struct UserKycRow {
    kyc_level: i32,
}

/// 资金原语锁定钱包行后取回的三桶余额，写流水时需要完整快照因此三项都要读。
#[derive(Debug, sqlx::FromRow)]
struct WalletRow {
    available: BigDecimal,
    frozen: BigDecimal,
    /// 借贷不使用该桶，读出来只为原样写进流水的账后快照。
    locked: BigDecimal,
}

/// 后台订单查询的筛选条件集合，各项之间为「与」关系，为空即不参与过滤。
pub(crate) struct AdminLoanOrdersFilter {
    pub(crate) limit: u32,
    pub(crate) offset: u32,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) product_id: Option<u64>,
    pub(crate) loan_type: Option<String>,
    pub(crate) status: Option<String>,
}

/// 产品配置的写入载荷，创建与整体更新共用同一形态，字段均已通过应用层校验。
/// 该结构只能由 `NormalizedLoanProductRequest` 转换而来，未经校验的请求无法直达数据库。
pub(crate) struct LoanProductWrite {
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) name_json: Value,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) status: String,
}

/// 订单插入载荷，其中的计息与额度字段全部来自锁定产品时的条款快照而非请求体。
/// 状态不在此结构中，插入语句固定写死为 pending。
pub(crate) struct LoanOrderCreate {
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) amount: BigDecimal,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) term_days: u32,
    pub(crate) min_kyc_level: i32,
    pub(crate) collateral_asset_id: Option<u64>,
    pub(crate) collateral_amount: Option<BigDecimal>,
    pub(crate) idempotency_key: String,
}

/// 在调用方事务内插入借贷产品配置，revision 使用数据库缺省值一，并返回自增主键供同事务回读。
/// 多语言名称以 SQLx 的 Json 包装绑定，由驱动负责序列化，本函数不再校验其结构。
/// 应用层必须在提交前回读后快照并写入管理员审计；任一步失败都让产品插入一并回滚。
/// 该入口只写产品表，不触达用户钱包、订单或抵押资金，也不自行提交事务。
pub(crate) async fn insert_loan_product_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product: &LoanProductWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO loan_products
           (loan_type, asset_id, name, name_json, term_days, interest_rate, interest_calculation_mode,
            min_kyc_level, min_amount, max_amount, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&product.loan_type)
    .bind(product.asset_id)
    .bind(&product.name)
    .bind(SqlxJson(product.name_json.clone()))
    .bind(product.term_days)
    .bind(&product.interest_rate)
    .bind(&product.interest_calculation_mode)
    .bind(product.min_kyc_level)
    .bind(&product.min_amount)
    .bind(&product.max_amount)
    .bind(&product.status)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

/// 在持有产品行锁的调用方事务内整体覆盖配置，并以客户端 revision 作为条件把版本原子加一。
/// `WHERE revision = ?` 是行锁之外的第二道并发保护；受影响行数为零按旧版本冲突处理，
/// 调用方不得重试成无条件更新。既有订单的利率、期限、额度和抵押资金快照不会被改写。
pub(crate) async fn update_loan_product_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    expected_revision: u64,
    product: &LoanProductWrite,
) -> AppResult<()> {
    let updated = sqlx::query(
        r#"UPDATE loan_products
           SET loan_type = ?, asset_id = ?, name_json = ?, name = ?, term_days = ?, interest_rate = ?,
               interest_calculation_mode = ?, min_kyc_level = ?, min_amount = ?,
               max_amount = ?, status = ?, revision = revision + 1
           WHERE id = ? AND revision = ?"#,
    )
    .bind(&product.loan_type)
    .bind(product.asset_id)
    .bind(SqlxJson(product.name_json.clone()))
    .bind(&product.name)
    .bind(product.term_days)
    .bind(&product.interest_rate)
    .bind(&product.interest_calculation_mode)
    .bind(product.min_kyc_level)
    .bind(&product.min_amount)
    .bind(&product.max_amount)
    .bind(&product.status)
    .bind(product_id)
    .bind(expected_revision)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "loan product revision is stale; reload before retrying".to_owned(),
        ));
    }
    Ok(())
}

/// 在持有产品行锁的事务内只改写上下架状态，并以客户端 revision 为条件把版本原子加一。
/// 状态合法性由应用层先行校验；条件未命中按旧版本冲突处理，禁止无条件覆盖另一管理员的新结果。
/// disabled 只阻断后续下单，已 pending 的订单仍可审批，已放款订单照常计息和还款。
pub(crate) async fn update_loan_product_status_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
    expected_revision: u64,
    status: &str,
) -> AppResult<()> {
    let updated = sqlx::query(
        "UPDATE loan_products SET status = ?, revision = revision + 1 WHERE id = ? AND revision = ?",
    )
        .bind(status)
        .bind(product_id)
        .bind(expected_revision)
        .execute(&mut **tx)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "loan product revision is stale; reload before retrying".to_owned(),
        ));
    }
    Ok(())
}

/// 读取借贷产品列表并联表补上资产符号，状态为空时返回全部产品，非空时按等值过滤。
/// 借贷类型固定不参与过滤，用户端列表由调用方传入 active 实现只看在售产品。
/// 排序取产品编号倒序，编号唯一因而不会出现同排序值行在页间重复或丢失的问题。
/// 只支持限制条数不支持偏移，产品较多时只能看到编号最大的一批。
/// 该查询只读产品配置，不加行锁，也不读取用户借贷或资金状态。
pub(crate) async fn list_loan_products(
    pool: &Pool<MySql>,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<LoanProductResponse>> {
    let mut builder = loan_product_query_builder();
    push_loan_product_filters(&mut builder, None, status);
    builder.push(LOAN_PRODUCT_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);
    Ok(builder
        .build_query_as::<LoanProductResponse>()
        .fetch_all(pool)
        .await?)
}

/// 后台产品分页查询，同时返回当前页数据与命中筛选的总行数。
/// 关键约束是行查询与 COUNT 查询由同一个循环推入同一组谓词，
/// 一旦两者分开维护就会出现总数与列表口径不一致、前端页码错乱的问题。
/// 借贷类型与状态两项筛选均为可选，为空即整体省略该条件而非匹配空串。
/// 分页在 `fetch_admin_page` 内统一追加排序、LIMIT 与 OFFSET，本函数不重复处理。
/// 该只读入口不锁产品，也不修改已有订单条款或钱包余额。
pub(crate) async fn list_admin_loan_products(
    pool: &Pool<MySql>,
    loan_type: Option<&str>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<LoanProductResponse>, i64)> {
    let mut rows = loan_product_query_builder();
    let mut total = QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM loan_products products
           INNER JOIN assets ON assets.id = products.asset_id"#,
    );
    for builder in [&mut rows, &mut total] {
        push_loan_product_filters(builder, loan_type, status);
    }

    fetch_admin_page(pool, rows, total, LOAN_PRODUCT_ORDER_BY, limit, offset).await
}

/// 构造产品查询的 SELECT 与 JOIN 前缀，字段顺序和别名必须与 `LoanProductResponse` 保持一致。
/// 用 INNER JOIN 关联资产表取符号，因此资产行缺失会让对应产品整行从结果中消失。
/// 只返回未附加任何 WHERE 的构建器，筛选与分页由调用方后续推入。
fn loan_product_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT products.id, products.loan_type, products.asset_id, assets.symbol AS asset_symbol,
                  products.name, products.name_json, products.term_days, products.interest_rate,
                  products.interest_calculation_mode, products.min_kyc_level,
                  products.min_amount, products.max_amount, products.status, products.revision,
                  products.created_at, products.updated_at
           FROM loan_products products
           INNER JOIN assets ON assets.id = products.asset_id"#,
    )
}

/// 基于统一产品投影构造主键详情查询，按调用方需要在末尾追加 `FOR UPDATE`。
/// 详情、事务回读与锁行共享同一字段清单，revision 因而不会在某条路径遗漏；
/// 所有主键均以绑定参数传入，锁行查询只允许在应用层已开启的配置事务中执行。
fn loan_product_by_id_query(product_id: u64, for_update: bool) -> QueryBuilder<'static, MySql> {
    let mut builder = loan_product_query_builder();
    builder.push(" WHERE products.id = ");
    builder.push_bind(product_id);
    builder.push(" LIMIT 1");
    if for_update {
        builder.push(" FOR UPDATE");
    }
    builder
}

/// 向构建器追加产品筛选谓词，先写入恒真的 `WHERE 1 = 1` 以便后续条件一律用 AND 拼接。
/// 这样无论有几个可选条件都不需要判断是否为首个谓词，代价是多一个被优化器忽略的恒真式。
/// 所有取值都经 push_bind 作为绑定参数下推，不做字符串插值。
/// 行查询与 COUNT 查询必须各调用一次本函数，才能保证总数与列表口径一致。
fn push_loan_product_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    loan_type: Option<&str>,
    status: Option<&str>,
) {
    builder.push(" WHERE 1 = 1");
    if let Some(loan_type) = loan_type {
        builder.push(" AND products.loan_type = ");
        builder.push_bind(loan_type.to_owned());
    }
    if let Some(status) = status {
        builder.push(" AND products.status = ");
        builder.push_bind(status.to_owned());
    }
}

/// 按编号读取单个产品的完整配置、revision 与资产符号，供详情和事务外只读调用。
/// 查询复用统一产品投影，列表与详情的字段清单不会因手工维护而漂移。
/// 不加行锁，返回值只是即时快照，不能作为并发下单时的条款依据。
/// 编号不存在或资产行缺失导致 INNER JOIN 落空时统一返回 NotFound。
/// 返回的是产品表当前配置，不会覆盖任何订单中已经固化的贷款条款。
pub(crate) async fn load_loan_product_response(
    pool: &Pool<MySql>,
    product_id: u64,
) -> AppResult<LoanProductResponse> {
    loan_product_by_id_query(product_id, false)
        .build_query_as::<LoanProductResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在产品配置事务内按主键回读最新响应，能看到本事务尚未提交的 revision 与字段更新。
/// 本函数不加锁，调用方必须已通过创建或 `lock_loan_product_response_in_tx` 拥有该行的写入边界；
/// 产品或关联资产不存在时返回 NotFound，并使配置与审计事务整体回滚。
pub(crate) async fn load_loan_product_response_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<LoanProductResponse> {
    loan_product_by_id_query(product_id, false)
        .build_query_as::<LoanProductResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 以 `FOR UPDATE` 锁定贷款产品并返回变更前完整快照，所有更新与状态切换必须先走该入口。
/// 行锁把并发管理写串行化，应用层随后比较客户端 revision，条件更新再提供第二道防覆盖保障。
/// 锁定发生在任何产品写入与审计之前；目标不存在时返回 NotFound，不产生部分副作用。
pub(crate) async fn lock_loan_product_response_in_tx(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<LoanProductResponse> {
    loan_product_by_id_query(product_id, true)
        .build_query_as::<LoanProductResponse>()
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在贷款产品配置事务内追加管理员审计，操作人、原因、安全前后快照与请求关联信息原子落库。
/// before/after 由表现层显式白名单生成并包含 revision，不得传入凭据、令牌或密钥明文；
/// HTTP 请求之外没有 task-local 上下文时 IP/request_id 保持 NULL，审计失败会回滚对应配置变更。
pub(crate) async fn insert_loan_product_audit_log_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    product_id: u64,
    before_json: Option<Value>,
    after_json: Option<Value>,
    reason: &str,
) -> AppResult<()> {
    let request_context = crate::infra::admin_request_context::current_admin_request_context();
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, request_id)
           VALUES (?, ?, 'loan_product', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(product_id.to_string())
    .bind(before_json.map(SqlxJson))
    .bind(after_json.map(SqlxJson))
    .bind(reason)
    .bind(
        request_context
            .as_ref()
            .and_then(|context| context.source_ip.as_deref()),
    )
    .bind(
        request_context
            .as_ref()
            .map(|context| context.request_id.as_str()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 读取指定用户的借贷订单列表，user_id 作为首个 WHERE 条件固定拼入，实现归属隔离。
/// 状态筛选先经裁剪归一，纯空白等价于不过滤，未知状态不报错只是查不到数据。
/// 按订单编号倒序并只支持限制条数，不提供偏移翻页，因此等价于查看最近若干笔。
/// 不加行锁也不触发任何资金状态变化，返回的利息与还款额只在订单已结清时才有意义。
pub(crate) async fn list_user_loan_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<String>,
    limit: u32,
) -> AppResult<Vec<LoanOrderResponse>> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    if let Some(status) = optional_string(status) {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY orders.id DESC LIMIT ");
    builder.push_bind(limit as i64);

    Ok(builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_all(pool)
        .await?)
}

/// 后台订单分页查询，五项筛选在同一个循环里同时推给行查询与 COUNT 查询。
/// 三个文本条件先裁剪归一，空白值被丢弃而不会变成匹配空串的无效过滤。
/// 邮箱条件拼成前后通配的 LIKE，因此无法命中索引，是该查询最主要的性能风险点。
/// 其余四项均为等值匹配，条件之间以 AND 连接，全部为空时退化为全量分页。
/// 循环内对文本条件逐次 clone，是因为同一个值要分别绑定进两个独立的构建器。
/// 该只读查询不获取订单或钱包行锁，不触发审核、还款或抵押释放。
pub(crate) async fn list_admin_loan_orders(
    pool: &Pool<MySql>,
    filter: AdminLoanOrdersFilter,
) -> AppResult<(Vec<LoanOrderResponse>, i64)> {
    let email = optional_string(filter.email);
    let loan_type = optional_string(filter.loan_type);
    let status = optional_string(filter.status);
    let mut rows = loan_order_query_builder();
    let mut total = loan_order_count_query_builder();
    for builder in [&mut rows, &mut total] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            builder.push(" AND orders.user_id = ");
            builder.push_bind(user_id);
        }
        if let Some(email) = email.clone() {
            builder.push(" AND users.email LIKE ");
            builder.push_bind(format!("%{email}%"));
        }
        if let Some(product_id) = filter.product_id {
            builder.push(" AND orders.product_id = ");
            builder.push_bind(product_id);
        }
        if let Some(loan_type) = loan_type.clone() {
            builder.push(" AND orders.loan_type = ");
            builder.push_bind(loan_type);
        }
        if let Some(status) = status.clone() {
            builder.push(" AND orders.status = ");
            builder.push_bind(status);
        }
    }

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY orders.id DESC",
        filter.limit,
        filter.offset,
    )
    .await
}

/// 按订单编号读取完整详情，不带用户条件，因此后台可查看任意用户的订单。
/// 各状态迁移用例在提交事务后也用它回读响应，此时读到的是已提交的最新状态。
/// 不加行锁、不计算当前应计利息，利息与还款额两列只在订单已结清时才被写入。
/// 编号不存在时返回 NotFound。
pub(crate) async fn load_loan_order_response(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按用户与编号读取订单，借此强制订单归属隔离。
/// 查询只读订单快照，不获取行锁，也不触发还款、取消或抵押释放。
pub(crate) async fn load_user_loan_order_response(
    pool: &Pool<MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<LoanOrderResponse> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder.push(" AND orders.user_id = ");
    builder.push_bind(user_id);
    builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 按用户与幂等键读取既有借贷订单，用于唯一键冲突后的重放返回。
/// 回读不锁订单也不再次冻结抵押；当前路径不核对本次产品、金额和抵押参数，调用方不得复用键表达不同请求。
pub(crate) async fn load_loan_order_by_idempotency(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<LoanOrderResponse> {
    let mut builder = loan_order_query_builder();
    builder.push(" WHERE orders.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND orders.idempotency_key = ");
    builder.push_bind(idempotency_key.to_owned());
    builder
        .build_query_as::<LoanOrderResponse>()
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 在调用方事务中以 FOR UPDATE 锁定产品行，并取回订单需要快照的条款。
/// 加锁的目的是让条款读取与订单插入之间不被管理端的产品改配置插入，保证同一笔订单条款自洽。
/// 锁定后才检查状态：产品不存在返回 NotFound，存在但非 active 返回参数错误。
/// 这一步是下单事务的第一环，后续依次是插入订单和锁钱包，锁序固定不可调换。
/// 本函数不校验金额、KYC 或抵押，也不产生任何写入。
pub(crate) async fn lock_active_loan_product_terms(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<LoanProductTermsRow> {
    let product = sqlx::query_as::<_, LoanProductTermsRow>(
        r#"SELECT id, loan_type, asset_id, term_days, interest_rate,
                  interest_calculation_mode, min_kyc_level, min_amount, max_amount, status
           FROM loan_products
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != STATUS_ACTIVE {
        return Err(AppError::Validation(
            "loan product is not active".to_owned(),
        ));
    }
    Ok(product)
}

/// 在事务中以 FOR UPDATE 锁定订单行，供管理端审批与拒绝两条路径串行化状态迁移。
/// 不带 user_id 条件，因此后台可操作任意用户订单；订单不存在时返回 NotFound。
/// 持锁后调用方才去读写钱包，这一锁序保证并发审批不会重复放款或重复释放抵押。
/// 只读取判定与算账所需的字段，锁在事务提交或回滚时释放。
pub(crate) async fn lock_loan_order(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<LoanOrderLockRow> {
    sqlx::query_as::<_, LoanOrderLockRow>(
        r#"SELECT id, user_id, asset_id, amount, interest_rate,
                  interest_calculation_mode, term_days, collateral_asset_id,
                  collateral_amount, status, disbursed_at, collateral_released_at
           FROM loan_orders
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 在事务中按用户与编号锁定订单，供用户端取消与还款两条路径使用。
/// 与管理端版本的差别有两处：多了 user_id 条件实现归属隔离，且未命中返回 `None` 而非直接报错，
/// 由调用方统一转成 NotFound，因此他人订单与不存在的订单对外表现一致。
/// 持锁后调用方才去锁钱包，用户侧的锁序同样是订单在前、钱包在后。
/// 只加锁与读取，不做任何状态判断或资金移动。
pub(crate) async fn lock_user_loan_order(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    order_id: u64,
) -> AppResult<Option<LoanOrderLockRow>> {
    sqlx::query_as::<_, LoanOrderLockRow>(
        r#"SELECT id, user_id, asset_id, amount, interest_rate,
                  interest_calculation_mode, term_days, collateral_asset_id,
                  collateral_amount, status, disbursed_at, collateral_released_at
           FROM loan_orders
           WHERE id = ? AND user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Database)
}

/// 在调用方事务中写入订单行，把产品条款与抵押参数一并固化为不可变快照。
/// 状态在 SQL 里写死为 pending，调用方无法指定初始状态。
/// 返回 `sqlx::Error` 而非 `AppError`，是为了让应用层能识别用户维度幂等键的唯一冲突并走重放分支。
/// 插入发生在产品锁之后、钱包锁之前，这一位置保证冲突时尚未发生任何资金移动。
/// 本函数不冻结抵押、不校验余额，抵押冻结由应用层在拿到订单编号后于同一事务内追加。
pub(crate) async fn insert_loan_order_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order: LoanOrderCreate,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"INSERT INTO loan_orders
           (user_id, product_id, loan_type, asset_id, amount, interest_rate,
            interest_calculation_mode, term_days, min_kyc_level, collateral_asset_id,
            collateral_amount, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(order.user_id)
    .bind(order.product_id)
    .bind(&order.loan_type)
    .bind(order.asset_id)
    .bind(&order.amount)
    .bind(&order.interest_rate)
    .bind(&order.interest_calculation_mode)
    .bind(order.term_days)
    .bind(order.min_kyc_level)
    .bind(order.collateral_asset_id)
    .bind(&order.collateral_amount)
    .bind(&order.idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

/// 在调用方事务中把订单置为 cancelled 并记录撤回时刻，时间取数据库当前时间戳而非应用层时钟。
/// 抵押释放必须在同一事务内先行完成，否则会留下已取消但抵押仍被冻结的订单。
/// 不校验原状态，pending 判定由应用层在持锁后完成，本函数无条件执行更新。
/// 不检查受影响行数，因为调用前已锁定该行，行必然存在。
pub(crate) async fn mark_loan_order_cancelled_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE loan_orders SET status = 'cancelled', cancelled_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中把订单置为 disbursed，并一次性写入审批人、审批时刻、放款时刻和到期时刻。
/// 审批与放款共用同一个数据库时间戳，因此这两个时间在数据上总是相等。
/// due_at 由应用层按审批时刻加期限天数算好后传入，逾期扫描任务后续据此判定超期。
/// disbursed_at 是实际天数计息的起点，缺失会导致还款阶段直接被拒绝。
/// 本金入账流水必须在同一事务内先写成功，否则回滚后订单仍保持待审核状态。
pub(crate) async fn mark_loan_order_disbursed_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    admin_id: u64,
    due_at: NaiveDateTime,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'disbursed',
               approved_by = ?,
               approved_at = CURRENT_TIMESTAMP(6),
               disbursed_at = CURRENT_TIMESTAMP(6),
               due_at = ?
           WHERE id = ?"#,
    )
    .bind(admin_id)
    .bind(due_at)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中把订单置为 rejected，记录审核管理员、可选原因和拒绝时刻。
/// 原因已由应用层裁剪归一，为空时绑定 NULL 表示管理员未填写，本函数不做长度或内容校验。
/// 与审批路径不同，这里不写 approved_by 也不发生任何本金放款。
/// 抵押退回必须在同一事务内先完成；状态写入失败会连同 frozen 释放和双桶流水一起回滚，防止单边退款。
pub(crate) async fn mark_loan_order_rejected_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    admin_id: u64,
    reason: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'rejected',
               rejected_by = ?,
               rejected_reason = ?,
               rejected_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(admin_id)
    .bind(reason)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务中把订单置为 repaid，并落库本次实收利息、还款总额与还清时刻。
/// 这两个金额此前一直是零值占位，只有走到这一步才被写入真实结算结果。
/// 传入值已由应用层按贷款资产精度向零截断，本函数不再做任何舍入。
/// 扣款与抵押释放必须在同一事务内先完成，任一步失败都会连同状态写入整体回滚。
/// repaid 是资金流终态，写入后再次调用还款用例会命中幂等分支而不重复扣款。
pub(crate) async fn mark_loan_order_repaid_in_tx(
    tx: &mut Transaction<'_, MySql>,
    order_id: u64,
    interest_amount: &BigDecimal,
    repayment_amount: &BigDecimal,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE loan_orders
           SET status = 'repaid',
               interest_amount = ?,
               repayment_amount = ?,
               repaid_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(interest_amount)
    .bind(repayment_amount)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务内读取资产精度并确认其处于 active，供下单与还款时的金额校验和截断使用。
/// 与连接池版本逻辑完全一致，区别只在于走事务连接，能读到同一事务中尚未提交的改动。
/// 资产缺失返回 NotFound，存在但被禁用返回参数错误，两者都会中止当前资金流程。
/// 在事务中失败意味着整笔订单变更被回滚，不会留下部分写入。
/// 只读元数据，不创建钱包账户，也不对既有余额做任何截断。
pub(crate) async fn load_active_asset_meta_in_tx(
    tx: &mut Transaction<'_, MySql>,
    asset_id: u64,
) -> AppResult<AssetMetaRow> {
    let asset = sqlx::query_as::<_, AssetMetaRow>(
        "SELECT precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

/// 以连接池读取资产精度并确认其处于 active，供产品配置阶段校验最小额与最大额的小数位。
/// 该路径不在事务内，因此只用于产品管理这类无资金移动的场景，下单与还款走事务版本。
/// 资产缺失返回 NotFound，被禁用返回参数错误，两者都会阻止产品配置落库。
/// 只读元数据，不创建钱包账户，也不对提交金额做隐式舍入。
pub(crate) async fn load_active_asset_meta(
    pool: &Pool<MySql>,
    asset_id: u64,
) -> AppResult<AssetMetaRow> {
    let asset = sqlx::query_as::<_, AssetMetaRow>(
        "SELECT precision_scale, status FROM assets WHERE id = ? LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if asset.status != STATUS_ACTIVE {
        return Err(AppError::Validation("asset must be active".to_owned()));
    }
    Ok(asset)
}

/// 在下单事务中确认用户存在且实名等级不低于产品门槛，等级相等即视为达标。
/// 用户行缺失返回未授权而非未找到，避免向调用方暴露用户是否存在。
/// 等级不足返回参数错误并在消息中回显所需等级，便于前端引导用户补充认证。
/// 校验位置在锁定产品之后、插入订单之前，失败时既不会创建订单也不会冻结任何抵押。
/// 门槛值同时会被快照进订单，但仅作留痕，后续审批与还款不再复核 KYC。
pub(crate) async fn ensure_loan_user_kyc_level(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    min_kyc_level: i32,
) -> AppResult<()> {
    let user = sqlx::query_as::<_, UserKycRow>("SELECT kyc_level FROM users WHERE id = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if user.kyc_level < min_kyc_level {
        return Err(AppError::Validation(format!(
            "loan product requires KYC level {min_kyc_level}"
        )));
    }
    Ok(())
}

/// 若订单含未释放抵押，则在调用方事务把 collateral frozen 等额退回 available 并记录释放时间。
/// 调用方须已锁订单，随后锁抵押钱包；释放写 available 正额与 frozen 负额两条同 ref_id 流水，locked 不变。
/// 已释放、无抵押资产或无抵押金额时直接返回；余额不足、流水或状态写入失败由外层事务回滚。
pub(crate) async fn release_loan_collateral_if_needed(
    tx: &mut Transaction<'_, MySql>,
    order: &LoanOrderLockRow,
) -> AppResult<()> {
    let Some(collateral_asset_id) = order.collateral_asset_id else {
        return Ok(());
    };
    let Some(collateral_amount) = order.collateral_amount.as_ref() else {
        return Ok(());
    };
    if order.collateral_released_at.is_some() {
        return Ok(());
    }
    apply_loan_wallet_unfreeze(
        tx,
        order.user_id,
        collateral_asset_id,
        collateral_amount,
        "loan_collateral_release",
        order.id,
    )
    .await?;
    sqlx::query(
        "UPDATE loan_orders SET collateral_released_at = CURRENT_TIMESTAMP(6) WHERE id = ?",
    )
    .bind(order.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在借贷订单事务中把抵押数量从用户可用余额等额迁移到冻结余额。
/// 调用方须传入正数、已按资产精度校验的金额，并保证订单创建和本操作共用事务。
/// 先锁定或创建钱包账户并检查可用余额，再更新两个余额桶并各写一条账后快照流水。
/// 可用额与冻结额一减一增，资产总额必须保持不变，流水引用同一借贷订单。
/// 本函数不独立幂等；余额不足或 SQL 失败向上返回，调用方必须回滚订单及全部资金变更。
pub(crate) async fn apply_loan_wallet_freeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for loan collateral: requested {}, available {}",
            amount, wallet.available
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    let frozen_after = wallet.frozen.clone() + amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

/// 在调用方事务锁定或创建贷款资产钱包后，把本金增加到 available 并写一条正向借贷流水。
/// 调用方须先锁订单并传入正数、已按资产精度确定的金额；frozen/locked 原样进入账后快照。
/// 本函数没有独立幂等键，放款状态负责阻止重放；余额或流水失败由外层事务连同订单状态回滚。
pub(crate) async fn apply_loan_wallet_credit(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

/// 在调用方事务锁定或创建贷款资产钱包后，从 available 扣除已量化的本金加利息并写一条负向流水。
/// frozen/locked 不变；available 不足立即失败且不写流水。本函数无独立幂等键，由已还款状态拦截重放。
/// 扣款、后续抵押释放和订单还清状态是否原子由调用方持有的同一事务保证。
pub(crate) async fn apply_loan_wallet_debit(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for loan repayment: requested {}, available {}",
            amount, wallet.available
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

/// 抵押释放的底层原语，把冻结额等额退回可用额，是 `apply_loan_wallet_freeze` 的严格逆操作。
/// 先锁定或创建钱包行，再要求 frozen 覆盖待释放数量，不足则失败且不写任何流水。
/// available 增加、frozen 减少，两者变动量相同，因此资产总额保持不变，locked 桶完全不参与。
/// 随后写两条同 change_type、同订单引用的流水：一条记 available 正额，一条记 frozen 负额，
/// 两条流水的账后快照都取变动完成后的最终值，因此三桶数据在两行中一致。
/// 本函数没有独立幂等键，重复调用会重复退款，防重放依赖订单的 collateral_released_at 字段。
/// 任何一步失败都向上返回，由调用方持有的事务统一回滚。
async fn apply_loan_wallet_unfreeze(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    if wallet.frozen < *amount {
        return Err(AppError::Validation(format!(
            "insufficient frozen balance for loan collateral: requested {}, frozen {}",
            amount, wallet.frozen
        )));
    }
    let available_after = wallet.available.clone() + amount.clone();
    let frozen_after = wallet.frozen.clone() - amount.clone();
    sqlx::query(
        "UPDATE wallet_accounts SET available = ?, frozen = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&available_after)
    .bind(&frozen_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await?;
    insert_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "frozen",
        &frozen_after,
        &available_after,
        &frozen_after,
        &wallet.locked,
        change_type,
        order_id,
    )
    .await
}

/// 取得某个用户资产维度钱包行的排他锁并返回三桶余额，是四个资金原语共同的第一步。
/// 先用 INSERT IGNORE 幂等建行，让首次接触该资产的用户也能直接放款或退还抵押；
/// 已存在时该语句什么都不做，绝不会把既有余额重置为零。
/// 随后以 SELECT ... FOR UPDATE 锁行，锁在调用方事务提交或回滚时释放。
/// 建行后仍查不到行属于异常状态，按参数错误上报并让整个事务回滚。
/// 加锁顺序由调用方决定，本函数不参与死锁规避策略。
async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<WalletRow> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, WalletRow>(
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
    .ok_or_else(|| AppError::Validation("wallet account is required".to_owned()))
}

/// 写入一条借贷相关的钱包流水，是四个资金原语记账的唯一出口。
/// `amount` 带符号，出账为负、入账为正；`balance_type` 标明这条流水描述的是哪个余额桶。
/// `balance_after` 是该桶变动后的值，另外三个 after 参数记录变动完成时三桶的完整快照，
/// 因此双桶迁移写出的两条流水会携带相同的快照，只有 amount 与 balance_type 不同。
/// 引用类型固定为 loan_order，引用编号取订单主键并转为字符串以适配通用引用列。
/// 调用方必须保证流水与余额更新在同一事务内，否则会出现账实不符。
#[allow(clippy::too_many_arguments)]
async fn insert_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    order_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(REF_TYPE_LOAN_ORDER)
    .bind(order_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 构造订单查询的 SELECT 与 JOIN 前缀，列顺序和别名必须与 `LoanOrderResponse` 严格对应。
/// 用户、产品、贷款资产三张表用 INNER JOIN，任一关联行缺失都会让订单从结果中消失；
/// 抵押资产用 LEFT JOIN，因为信用贷订单的抵押资产编号本就为空。
/// 产品名称取自产品表当前值而非订单快照，因此改名会同时影响历史订单的展示。
/// 只返回未附加 WHERE 的构建器，筛选、排序与分页由调用方推入。
fn loan_order_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT orders.id, orders.user_id, users.email AS user_email,
                  orders.product_id, products.name AS product_name,
                  products.name_json AS product_name_json,
                  orders.loan_type, orders.asset_id, assets.symbol AS asset_symbol,
                  orders.amount, orders.interest_rate, orders.interest_calculation_mode,
                  orders.term_days, orders.min_kyc_level,
                  orders.collateral_asset_id, collateral_assets.symbol AS collateral_asset_symbol,
                  orders.collateral_amount, orders.status, orders.interest_amount,
                  orders.repayment_amount, orders.approved_by, orders.rejected_by,
                  orders.rejected_reason, orders.approved_at, orders.rejected_at,
                  orders.disbursed_at, orders.due_at, orders.overdue_at,
                  orders.cancelled_at, orders.repaid_at,
                  orders.collateral_released_at, orders.created_at, orders.updated_at
           FROM loan_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN loan_products products ON products.id = orders.product_id
           INNER JOIN assets ON assets.id = orders.asset_id
           LEFT JOIN assets collateral_assets ON collateral_assets.id = orders.collateral_asset_id"#,
    )
}

/// 构造订单计数查询的前缀，JOIN 结构必须与行查询完全一致，否则总数会与列表口径不符。
/// 尤其是用户与产品的 INNER JOIN 会过滤掉关联缺失的订单，计数必须复现同样的过滤效果。
/// 只换掉投影列，其余部分与 `loan_order_query_builder` 保持同步，两处需一起修改。
fn loan_order_count_query_builder() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM loan_orders orders
           INNER JOIN users ON users.id = orders.user_id
           INNER JOIN loan_products products ON products.id = orders.product_id
           INNER JOIN assets ON assets.id = orders.asset_id
           LEFT JOIN assets collateral_assets ON collateral_assets.id = orders.collateral_asset_id"#,
    )
}

/// 分页排序必须带唯一列 id，否则同一排序值的行会在页间重复或丢失。
const LOAN_PRODUCT_ORDER_BY: &str = " ORDER BY products.id DESC";

/// 统一收口后台分页：向行查询追加排序与 LIMIT OFFSET，再执行计数查询，一并返回。
/// 调用方必须已经用同一组谓词构建好两个构建器，本函数只负责分页与执行，不再补充筛选条件。
/// 排序子句由调用方传入而非写死，因为产品与订单分页的排序列不同。
/// 两条查询各自独立执行且不在事务内，因此高并发写入时总数与当前页内容可能存在瞬时不一致。
/// 泛型行类型只要求可从 MySQL 行反序列化，产品与订单两种响应共用这一条路径。
async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

/// 本层自用的文本归一：裁剪首尾空白并把空串折成 `None`，避免空白值被当成有效过滤条件。
/// 与服务层的同名函数逻辑一致但各自独立，是为了让基础设施层不反向依赖服务层。
/// 只做裁剪与判空，不校验枚举合法性，未知取值会照常拼进查询条件。
fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 判断错误是否为 MySQL 的唯一键冲突，用于把订单幂等键重复识别为可重放而非真正的故障。
/// 同时接受 1062 与 23000 两种编码：前者是 MySQL 原生错误号，后者是完整性约束的 SQLSTATE。
/// 非数据库错误或取不到错误码时一律返回假，调用方会把它当作普通数据库故障向上抛出。
/// 该判定不区分是哪个唯一索引冲突，因此调用方需保证该语句上只有幂等键一个可能冲突的约束。
pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|db_error| db_error.code())
        .is_some_and(|code| code == "1062" || code == "23000")
}
