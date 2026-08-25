//! loan bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//!
//! 借贷领域没有富实体，只沉淀一组与数据库枚举列逐字对应的字符串常量。
//! 它们分属三套互不相干的取值空间：借贷类型、计息模式、以及状态。
//! 其中状态又细分为两类，active 与 disabled 描述产品能否被申请，
//! pending、disbursed、rejected、cancelled、repaid、overdue 描述单笔订单的生命周期。
//! 常量以字符串而非枚举表达，是为了直接绑定进 SQL 且与历史数据保持兼容。

/// 信用借贷类型：无需抵押物，仅凭 KYC 等级准入。
pub const LOAN_TYPE_CREDIT: &str = "credit";

/// 保证金借贷（有抵押）类型：下单时必须提供抵押资产与正数抵押金额，并在同一事务冻结。
pub const LOAN_TYPE_COLLATERALIZED: &str = "collateralized";

/// 利息结算模式：按全期计息，无论提前多久还款都收取本金乘利率的完整利息。
pub const INTEREST_MODE_FULL_TERM: &str = "full_term";

/// 利息结算模式：按实际占用天数比例计息，天数向上取整、下限一天且不超过产品期限。
pub const INTEREST_MODE_ACTUAL_DAYS: &str = "actual_days";

/// 产品状态：在售，只有该状态的产品允许被申请。
pub const STATUS_ACTIVE: &str = "active";
/// 产品状态：下架，阻断新申请但不影响已存在订单的审批、计息与还款。
pub const STATUS_DISABLED: &str = "disabled";
/// 订单状态：已提交待审核，是唯一可迁移到取消、拒绝或放款的状态。
pub const STATUS_PENDING: &str = "pending";
/// 订单状态：审核通过且本金已入账，此时 due_at 已按审核时刻加期限天数写入。
pub const STATUS_DISBURSED: &str = "disbursed";
/// 订单状态：审核驳回，抵押已退回，不发生本金放款。
pub const STATUS_REJECTED: &str = "rejected";
/// 订单状态：用户在审核前主动撤回，抵押已退回。
pub const STATUS_CANCELLED: &str = "cancelled";
/// 订单状态：本金与利息已结清且抵押已释放，是资金流的终态。
pub const STATUS_REPAID: &str = "repaid";
/// 订单状态：超过 due_at 仍未结清，由逾期扫描任务标记，仍然允许还款。
pub const STATUS_OVERDUE: &str = "overdue";
/// 订单状态：抵押物已进入平台清算，回收与坏账已全部记账的终态。
pub const STATUS_LIQUIDATED: &str = "liquidated";

/// 抵押贷当前支持的唯一服务端行情适配器标识。
pub const LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS: &str = "market_ticker_redis";

/// 抵押率低于维持线，订单处于正常区间。
pub const LOAN_RISK_STATE_HEALTHY: &str = "healthy";
/// 抵押率达到维持线但未达强制清算线。
pub const LOAN_RISK_STATE_MARGIN_CALL: &str = "margin_call";
/// 抵押率达到强制清算线。
pub const LOAN_RISK_STATE_LIQUIDATABLE: &str = "liquidatable";

/// 多语言产品标题的最大长度，按字符数而非字节数计算，中英文使用同一口径。
pub const LOAN_PRODUCT_NAME_TITLE_MAX_LEN: usize = 128;

/// 贷款产品后台变更原因的最大字符数，与 `admin_audit_logs.reason` 的 512 字符容量保持一致。
pub const LOAN_PRODUCT_AUDIT_REASON_MAX_LEN: usize = 512;
