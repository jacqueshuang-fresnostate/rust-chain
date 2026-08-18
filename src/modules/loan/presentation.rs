//! loan bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//!
//! 这里只描述 HTTP 报文形状，不承载校验规则：枚举合法性、金额区间、小数位与 KYC 门槛
//! 全部由服务层和应用层判定。金额统一用 `BigDecimal` 透传以避免浮点误差，
//! 时间字段统一序列化为 Unix 毫秒，尚未发生的阶段时间以可空形式表达。
//! 响应结构体同时用作 `sqlx::FromRow` 目标，字段名与查询别名严格对应，改名会直接影响取数。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::types::Json as SqlxJson;

/// 用户端产品列表的查询串，只支持限制条数，不支持偏移翻页。
#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    /// 期望条数，缺省 50，越界夹紧到 1..=200。
    pub(crate) limit: Option<u32>,
}

/// 后台产品列表的查询串，两个筛选项都会先做枚举校验，非法取值直接报错。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminLoanProductsQuery {
    /// 期望条数，缺省 50，夹紧到 1..=200。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，缺省 0，硬性截断到十万以避免深分页拖垮查询。
    pub(crate) offset: Option<u32>,
    /// 借贷类型筛选，仅接受 credit 或 collateralized。
    pub(crate) loan_type: Option<String>,
    /// 产品状态筛选，仅接受 active 或 disabled，与订单状态不是同一取值空间。
    pub(crate) status: Option<String>,
}

/// 后台订单列表的查询串，各筛选项之间是「与」关系，均为可选。
/// 与产品列表不同，这里的枚举值不做校验，未知取值只会查不到数据。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminLoanOrdersQuery {
    /// 期望条数，缺省 50，夹紧到 1..=200。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，缺省 0，截断到十万。
    pub(crate) offset: Option<u32>,
    /// 按用户编号精确筛选。
    pub(crate) user_id: Option<u64>,
    /// 按用户邮箱模糊筛选，实际拼成前后通配的 LIKE 条件。
    pub(crate) email: Option<String>,
    /// 按借贷产品编号精确筛选。
    pub(crate) product_id: Option<u64>,
    /// 按借贷类型精确筛选。
    pub(crate) loan_type: Option<String>,
    /// 按订单状态精确筛选，取值来自订单状态机而非产品启停状态。
    pub(crate) status: Option<String>,
}

/// 用户端订单列表的查询串，用户维度由 JWT 强制注入，不出现在查询参数中。
#[derive(Debug, Deserialize)]
pub(crate) struct UserLoanOrdersQuery {
    /// 期望条数，缺省 50，夹紧到 1..=200。
    pub(crate) limit: Option<u32>,
    /// 订单状态筛选，空白等价于不过滤，不校验枚举合法性。
    pub(crate) status: Option<String>,
}

/// 新建借贷产品的请求体，只有 status 可省略，其余字段均为必填。
#[derive(Debug, Deserialize)]
pub(crate) struct CreateLoanProductRequest {
    /// 借贷类型，credit 或 collateralized。
    pub(crate) loan_type: String,
    /// 放款资产编号，必须处于 active，其精度决定额度字段允许的小数位。
    pub(crate) asset_id: u64,
    /// 纯文本产品名，裁剪后不得为空；若多语言结构含默认标题则以后者为准。
    pub(crate) name: String,
    /// 多语言名称结构，缺省时按 name 自动生成简体中文兜底条目。
    pub(crate) name_json: Option<Value>,
    /// 借款期限天数，必须为正，审批时用于推算 due_at。
    pub(crate) term_days: u32,
    /// 期内利率，允许为零表示免息，不得为负。
    pub(crate) interest_rate: BigDecimal,
    /// 计息模式，full_term 或 actual_days。
    pub(crate) interest_calculation_mode: String,
    /// 申请所需的最低 KYC 等级，不得为负，下单事务内与用户实际等级比对。
    pub(crate) min_kyc_level: i32,
    /// 单笔最低借款额，必须为正。
    pub(crate) min_amount: BigDecimal,
    /// 单笔最高借款额，省略表示不限；给出时须为正且不小于最低额。
    pub(crate) max_amount: Option<BigDecimal>,
    /// 上下架状态，省略时默认 active。
    pub(crate) status: Option<String>,
    /// 管理员变更原因，传输层允许缺省以返回统一校验错误，应用层要求裁剪后非空。
    pub(crate) reason: Option<String>,
}

/// 整体覆盖借贷产品的请求体，字段含义与创建请求一致，差别只在 status 为必填。
/// 覆盖语义意味着缺字段等同于置空，服务端不会从数据库读回旧值做合并。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateLoanProductRequest {
    pub(crate) loan_type: String,
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) name_json: Option<Value>,
    pub(crate) term_days: u32,
    pub(crate) interest_rate: BigDecimal,
    pub(crate) interest_calculation_mode: String,
    pub(crate) min_kyc_level: i32,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    /// 上下架状态，此处必填，不再有默认值。
    pub(crate) status: String,
    /// 客户端读取该产品时获得的版本；缺失、零值或落后于数据库当前版本都会拒绝覆盖。
    pub(crate) revision: Option<u64>,
    /// 管理员变更原因，应用层要求裁剪后非空并写入同事务审计。
    pub(crate) reason: Option<String>,
}

/// 只切换产品上下架状态的轻量请求体，用于运营快速停售而无需重传全部配置。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateLoanProductStatusRequest {
    /// 目标状态，仅接受 active 或 disabled。
    pub(crate) status: String,
    /// 客户端读取该产品时获得的版本，用于阻止旧页面覆盖新的上下架结果。
    pub(crate) revision: Option<u64>,
    /// 管理员变更原因，应用层要求裁剪后非空并写入同事务审计。
    pub(crate) reason: Option<String>,
}

/// 用户提交借款申请的请求体，用户维度取自 JWT 而不在此结构中。
#[derive(Debug, Deserialize)]
pub(crate) struct CreateLoanOrderRequest {
    /// 目标产品编号，下单事务内会锁定该产品并快照其条款。
    pub(crate) product_id: u64,
    /// 申请借款金额，须为正、落在产品额度区间且小数位不超过放款资产精度。
    pub(crate) amount: BigDecimal,
    /// 抵押资产编号，抵押类产品必填，信用类产品忽略。
    pub(crate) collateral_asset_id: Option<u64>,
    /// 抵押数量，抵押类产品必填且须为正，成功后即从 available 冻结到 frozen。
    pub(crate) collateral_amount: Option<BigDecimal>,
    /// 用户维度幂等键，裁剪后非空且不超过 255 字节；重放会回读旧订单而不重复冻结抵押。
    pub(crate) idempotency_key: String,
}

/// 管理端审核动作的请求体，目前只被拒绝接口使用，审批接口不读取任何请求体字段。
#[derive(Debug, Deserialize)]
pub(crate) struct ReviewLoanOrderRequest {
    /// 拒绝原因，可省略；裁剪后为空则按未填写存入，不做长度或内容校验。
    pub(crate) reason: Option<String>,
}

/// 用户端产品列表响应，不含总数，因为该接口不支持偏移翻页。
#[derive(Debug, Serialize)]
pub(crate) struct LoanProductsResponse {
    pub(crate) products: Vec<LoanProductResponse>,
}

/// 后台产品列表响应，total 与当前筛选口径一致而非全表行数。
#[derive(Debug, Serialize)]
pub(crate) struct AdminLoanProductsResponse {
    pub(crate) products: Vec<LoanProductResponse>,
    /// 命中同一组筛选谓词的总行数，用于前端计算页数。
    pub(crate) total: i64,
}

/// 用户端订单列表响应，按订单编号倒序排列。
#[derive(Debug, Serialize)]
pub(crate) struct LoanOrdersResponse {
    pub(crate) orders: Vec<LoanOrderResponse>,
}

/// 后台订单列表响应，total 同样跟随当前筛选。
#[derive(Debug, Serialize)]
pub(crate) struct AdminLoanOrdersResponse {
    pub(crate) orders: Vec<LoanOrderResponse>,
    pub(crate) total: i64,
}

/// 状态迁移类接口的统一响应，创建、取消、还款、审批、拒绝五个入口共用。
/// `changed` 区分「本次真的改变了状态」与「命中终态的幂等重放」，两者都返回 HTTP 成功。
#[derive(Debug, Serialize)]
pub(crate) struct LoanOrderActionResponse {
    /// 操作完成后回读的订单最新快照。
    pub(crate) order: LoanOrderResponse,
    /// 为假表示订单此前已处于目标状态，本次调用未产生任何资金或状态副作用。
    pub(crate) changed: bool,
}

/// 借贷产品的对外视图，同时用作产品查询的 FromRow 目标。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct LoanProductResponse {
    id: u64,
    loan_type: String,
    asset_id: u64,
    asset_symbol: String,
    name: String,
    name_json: SqlxJson<Value>,
    term_days: u32,
    interest_rate: BigDecimal,
    interest_calculation_mode: String,
    min_kyc_level: i32,
    min_amount: BigDecimal,
    max_amount: Option<BigDecimal>,
    status: String,
    /// 配置乐观并发版本；创建初始为一，每次完整更新或状态变更成功后加一。
    revision: u64,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    updated_at: DateTime<Utc>,
}

impl LoanProductResponse {
    /// 返回当前配置 revision，供应用层在持有产品行锁后校验客户端基线。
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// 生成后台审计专用的显式白名单快照，只包含贷款产品公开配置与 revision。
    /// 快照刻意不复用未来可能扩展的任意对象序列化，`name_json` 也会逐层投影允许字段，
    /// 避免凭据、令牌或内部密钥被扩展键意外带入审计明文。
    pub(crate) fn audit_snapshot(&self) -> Value {
        json!({
            "id": self.id,
            "loan_type": self.loan_type,
            "asset_id": self.asset_id,
            "asset_symbol": self.asset_symbol,
            "name": self.name,
            "name_json": audit_product_name_json(&self.name_json.0),
            "term_days": self.term_days,
            "interest_rate": self.interest_rate,
            "interest_calculation_mode": self.interest_calculation_mode,
            "min_kyc_level": self.min_kyc_level,
            "min_amount": self.min_amount,
            "max_amount": self.max_amount,
            "status": self.status,
            "revision": self.revision,
        })
    }
}

/// 把产品多语言名称投影为审计允许的固定结构，忽略顶层与条目中的所有扩展键。
/// 历史脏数据缺少字段时对应值写为 null 或空数组而不会阻断管理员修正配置；
/// locale、country 与 title 是公开展示数据，因此可以保留，其他任意内容一律不进入审计。
fn audit_product_name_json(name_json: &Value) -> Value {
    let items = name_json
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    json!({
                        "locale": item.get("locale").and_then(Value::as_str),
                        "country": item.get("country").and_then(Value::as_str),
                        "title": item.get("title").and_then(Value::as_str),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "version": name_json.get("version").and_then(Value::as_u64),
        "default_locale": name_json.get("default_locale").and_then(Value::as_str),
        "items": items,
    })
}

/// 借贷订单的对外视图，兼作订单查询的 FromRow 目标。
/// 利率、计息模式、期限和 KYC 门槛都是下单时从产品复制的快照，产品后续改配置不会回溯改写。
/// 各阶段时间戳只在对应状态迁移真正发生时才有值，未发生的阶段保持为空。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct LoanOrderResponse {
    id: u64,
    user_id: u64,
    /// 借款人邮箱，供后台检索与展示，用户端同样返回。
    user_email: Option<String>,
    product_id: u64,
    /// 产品纯文本名，取自产品表当前值而非下单时快照。
    product_name: String,
    /// 产品多语言名称，同样取自产品表当前值。
    product_name_json: SqlxJson<Value>,
    loan_type: String,
    asset_id: u64,
    asset_symbol: String,
    /// 借款本金，放款时全额计入该资产的 available。
    amount: BigDecimal,
    /// 下单时快照的期内利率。
    interest_rate: BigDecimal,
    /// 下单时快照的计息模式，决定还款时利息是否按实际天数折算。
    interest_calculation_mode: String,
    /// 下单时快照的期限天数，审批时用于推算 due_at。
    term_days: u32,
    /// 下单时快照的 KYC 门槛，仅作留痕，还款与审批不再复核。
    min_kyc_level: i32,
    /// 抵押资产编号，信用贷为空。
    collateral_asset_id: Option<u64>,
    /// 抵押资产符号，无抵押时为空。
    collateral_asset_symbol: Option<String>,
    /// 抵押数量，下单成功即从 available 冻结到 frozen，直到取消、拒绝或还款时释放。
    collateral_amount: Option<BigDecimal>,
    /// 订单当前状态，取值为 pending、disbursed、rejected、cancelled、repaid、overdue 之一。
    status: String,
    /// 实际收取的利息，仅在还款成功时写入，未结清订单不代表当前应计利息。
    interest_amount: BigDecimal,
    /// 实际扣除的还款总额，等于本金加利息按资产精度截断后的结果，同样只在还款时写入。
    repayment_amount: BigDecimal,
    /// 审批放款的管理员编号。
    approved_by: Option<u64>,
    /// 驳回申请的管理员编号。
    rejected_by: Option<u64>,
    /// 驳回原因，管理员未填写时为空。
    rejected_reason: Option<String>,
    /// 审批通过时刻，与放款时刻在同一条 UPDATE 中写入。
    #[serde(default, with = "option_unix_millis")]
    approved_at: Option<DateTime<Utc>>,
    /// 驳回时刻。
    #[serde(default, with = "option_unix_millis")]
    rejected_at: Option<DateTime<Utc>>,
    /// 本金入账时刻，实际天数计息以此为起点。
    #[serde(default, with = "option_unix_millis")]
    disbursed_at: Option<DateTime<Utc>>,
    /// 到期时刻，等于审批时刻加期限天数，逾期扫描任务据此判定。
    #[serde(default, with = "option_unix_millis")]
    due_at: Option<DateTime<Utc>>,
    /// 被标记为逾期的时刻，由后台逾期任务写入。
    #[serde(default, with = "option_unix_millis")]
    overdue_at: Option<DateTime<Utc>>,
    /// 用户撤回时刻。
    #[serde(default, with = "option_unix_millis")]
    cancelled_at: Option<DateTime<Utc>>,
    /// 结清时刻。
    #[serde(default, with = "option_unix_millis")]
    repaid_at: Option<DateTime<Utc>>,
    /// 抵押释放时刻，取消、驳回或还款任一路径都会写入，非空即表示不会再次释放。
    #[serde(default, with = "option_unix_millis")]
    collateral_released_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    updated_at: DateTime<Utc>,
}
