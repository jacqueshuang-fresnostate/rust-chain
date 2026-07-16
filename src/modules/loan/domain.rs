//! loan bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。

/// 贷款类型常量。
pub const LOAN_TYPE_CREDIT: &str = "credit";

/// 保证金借贷（有抵押）类型常量。
pub const LOAN_TYPE_COLLATERALIZED: &str = "collateralized";

/// 利息结算模式常量：按全期比例计息。
pub const INTEREST_MODE_FULL_TERM: &str = "full_term";

/// 利息结算模式常量：按实际天数比例计息。
pub const INTEREST_MODE_ACTUAL_DAYS: &str = "actual_days";

/// 贷款状态常量。
pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_DISABLED: &str = "disabled";
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_DISBURSED: &str = "disbursed";
pub const STATUS_REJECTED: &str = "rejected";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const STATUS_REPAID: &str = "repaid";

/// 产品标题 JSON 的最大长度。
pub const LOAN_PRODUCT_NAME_TITLE_MAX_LEN: usize = 128;
