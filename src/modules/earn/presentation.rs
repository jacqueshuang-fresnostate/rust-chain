//! earn bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//!
//! 只描述 HTTP 报文形状，费率取值范围、金额精度、富文本节点白名单等规则一律由服务层判定。
//! 管理端的五个写请求都带一个 reason 字段，用于落入管理员审计日志，且在应用层被视为必填。
//! 需要特别区分两组同名概念：产品响应里的费率是当前配置，
//! 订阅响应里的同名费率是申购时复制的快照，二者可能不同，前端不应混用。
//! 时间统一序列化为 Unix 毫秒，尚未发生的赎回时间以可空形式表达。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

/// 用户端产品列表与订阅列表共用的查询串，只支持限制条数。
#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    /// 期望条数，缺省 50，越界夹紧到 1..=100。
    pub(crate) limit: Option<u32>,
}

/// 后台产品列表的查询串，不提供状态筛选，因此上下架产品一并返回。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminProductsQuery {
    /// 期望条数，缺省 50，夹紧到 1..=100。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，缺省 0，截断到十万。
    pub(crate) offset: Option<u32>,
}

/// 后台分类列表的查询串。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminCategoriesQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    /// 分类启停状态筛选，空白等价于不过滤，不校验枚举合法性。
    pub(crate) status: Option<String>,
}

/// 后台订阅列表的查询串，三项筛选之间为「与」关系。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminSubscriptionsQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    /// 按用户编号精确筛选。
    pub(crate) user_id: Option<u64>,
    /// 按用户邮箱精确匹配，非模糊查询，必须填写完整邮箱。
    pub(crate) email: Option<String>,
    /// 订阅状态筛选，取值为 subscribed 或 redeemed。
    pub(crate) status: Option<String>,
}

/// 申购请求体，用户维度取自 JWT 而不在此结构中。
#[derive(Debug, Deserialize)]
pub(crate) struct SubscribeEarnRequest {
    /// 目标产品编号，申购事务内会锁定该产品并复制其费率快照。
    pub(crate) product_id: u64,
    /// 申购本金，须为正、落在产品额度区间，且小数位不超过 18 位、整数位不超过 20 位。
    pub(crate) amount: BigDecimal,
    /// 用户维度幂等键，裁剪后非空且不超过 255 字节；重放时会核对产品与金额是否一致。
    pub(crate) idempotency_key: String,
}

/// 新建理财产品的请求体，四项费率与分类、介绍、图片均可省略并各有缺省行为。
#[derive(Debug, Deserialize)]
pub(crate) struct CreateEarnProductRequest {
    /// 申购与赎回所用资产，写入前会校验该资产存在。
    pub(crate) asset_id: u64,
    /// 产品名，裁剪后不得为空且不超过 128 字符。
    pub(crate) name: String,
    /// 横幅图地址，可空，限长 2048 字符。
    pub(crate) banner_url: Option<String>,
    /// 小图标地址，可空，限长 2048 字符。
    pub(crate) small_logo_url: Option<String>,
    /// 分类代码，省略时回退到 fixed_term；引用的分类必须存在且处于 active。
    pub(crate) category: Option<String>,
    /// 多语言富文本介绍，省略时按产品名生成中文兜底段落。
    pub(crate) introduction_json: Option<Value>,
    /// 期限天数，须为正且不超过 3650。
    pub(crate) term_days: u32,
    /// 年化收益率，不得为负，最多 8 位小数、整数位不超过 10 位。
    pub(crate) apr_rate: BigDecimal,
    /// 通用赎回费率，省略按零处理；须落在 0..=1 且最多 8 位小数。
    pub(crate) redemption_fee_rate: Option<BigDecimal>,
    /// 到期利润手续费率，省略按零处理，取值约束同上。
    pub(crate) maturity_profit_fee_rate: Option<BigDecimal>,
    /// 提前赎回费基准，省略按 none 处理，仅接受 none、principal、profit。
    pub(crate) early_redeem_fee_basis: Option<String>,
    /// 提前赎回费率，基准为 none 时会被强制归零而非报错。
    pub(crate) early_redeem_fee_rate: Option<BigDecimal>,
    /// 单笔最小申购额，须为正。
    pub(crate) min_subscribe: BigDecimal,
    /// 单笔最大申购额，省略表示不限；给出时须为正且不小于最小额。
    pub(crate) max_subscribe: Option<BigDecimal>,
    /// 上下架状态，省略时默认 active。
    pub(crate) status: Option<String>,
    /// 管理员操作原因，序列化上可空但应用层视为必填，裁剪后非空且不超过 512 字符。
    pub(crate) reason: Option<String>,
}

/// 整体覆盖理财产品的请求体，字段含义与创建请求一致，差别只在 status 为必填。
/// 覆盖语义意味着缺省字段按各自的缺省规则重新取值，而不是保留数据库中的旧值。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnProductRequest {
    pub(crate) asset_id: u64,
    pub(crate) name: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) introduction_json: Option<Value>,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: Option<BigDecimal>,
    pub(crate) maturity_profit_fee_rate: Option<BigDecimal>,
    pub(crate) early_redeem_fee_basis: Option<String>,
    pub(crate) early_redeem_fee_rate: Option<BigDecimal>,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    /// 上下架状态，此处必填，不再有默认值。
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

/// 只切换产品上下架状态的轻量请求体。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnProductStatusRequest {
    /// 目标状态，仅接受 active 或 disabled。
    pub(crate) status: String,
    /// 管理员操作原因，应用层必填，写入审计日志。
    pub(crate) reason: Option<String>,
}

/// 新建分类的请求体。
#[derive(Debug, Deserialize)]
pub(crate) struct CreateEarnCategoryRequest {
    /// 稳定分类代码，只允许字母、数字、下划线和连字符，不超过 64 字符，创建后不可修改。
    pub(crate) code: String,
    /// 多语言名称，省略时按 code 生成中文兜底条目。
    pub(crate) name_json: Option<Value>,
    /// 列表排序权重，省略按 0 处理，升序排列。
    pub(crate) sort_order: Option<i32>,
    /// 启停状态，省略时默认 active。
    pub(crate) status: Option<String>,
    /// 管理员操作原因，应用层必填。
    pub(crate) reason: Option<String>,
}

/// 更新分类的请求体，刻意不包含 code 字段以保证分类代码的引用稳定性。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnCategoryRequest {
    /// 多语言名称，省略时按数据库中锁定到的旧代码重新生成兜底条目。
    pub(crate) name_json: Option<Value>,
    /// 排序权重，此处必填。
    pub(crate) sort_order: i32,
    /// 启停状态，此处必填。
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

/// 只切换分类启停状态的轻量请求体。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEarnCategoryStatusRequest {
    /// 目标状态，仅接受 active 或 disabled；置为 disabled 不影响已引用它的存量产品。
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

/// 理财分类的对外视图，兼作分类查询的 FromRow 目标。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct EarnCategoryResponse {
    pub(crate) id: u64,
    /// 稳定分类代码，产品通过它引用分类。
    pub(crate) code: String,
    /// 完整多语言名称结构，供前端按语言自行挑选。
    pub(crate) name_json: SqlxJson<Value>,
    /// 由 SQL 取多语言结构首个条目的标题得到，取不到时回退为 code。
    pub(crate) default_name: String,
    pub(crate) sort_order: i32,
    pub(crate) status: String,
}

/// 分类列表响应，按 sort_order 升序再按编号升序，total 跟随当前状态筛选。
#[derive(Debug, Serialize)]
pub(crate) struct EarnCategoriesResponse {
    pub(crate) categories: Vec<EarnCategoryResponse>,
    pub(crate) total: i64,
}

/// 理财产品的对外视图，兼作产品查询的 FromRow 目标。
/// 这里的费率是产品当前配置；已有订阅的结算费率以订阅快照为准，两者可能不同。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct EarnProductResponse {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) name: String,
    pub(crate) banner_url: Option<String>,
    pub(crate) small_logo_url: Option<String>,
    /// 分类代码，即使对应分类行已被删除也保留原值。
    pub(crate) category: String,
    /// 分类展示名，取多语言结构首个条目标题，关联不到分类时回退为分类代码。
    pub(crate) category_name: String,
    /// 分类完整多语言结构，分类行缺失时为空。
    pub(crate) category_name_json: Option<SqlxJson<Value>>,
    /// 多语言富文本介绍。
    pub(crate) introduction_json: SqlxJson<Value>,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    /// 通用赎回费率，赎回时对本金加毛收益整体计费。
    pub(crate) redemption_fee_rate: BigDecimal,
    /// 到期利润手续费率，只在到期赎回时对毛收益计费。
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    /// 提前赎回费基准，取 none、principal 或 profit。
    pub(crate) early_redeem_fee_basis: String,
    /// 提前赎回费率，基准为 none 时恒为零。
    pub(crate) early_redeem_fee_rate: BigDecimal,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: String,
}

/// 用户端产品列表响应，不含总数，因为该接口不支持偏移翻页。
#[derive(Debug, Serialize)]
pub(crate) struct EarnProductsResponse {
    pub(crate) products: Vec<EarnProductResponse>,
}

/// 后台产品列表响应，total 与当前筛选一致。
#[derive(Debug, Serialize)]
pub(crate) struct AdminEarnProductsResponse {
    pub(crate) products: Vec<EarnProductResponse>,
    pub(crate) total: i64,
}

/// 用户端订阅列表响应，按创建时间倒序。
#[derive(Debug, Serialize)]
pub(crate) struct EarnSubscriptionsResponse {
    pub(crate) subscriptions: Vec<EarnSubscriptionResponse>,
}

/// 后台订阅列表响应，total 跟随用户、邮箱、状态三项筛选。
#[derive(Debug, Serialize)]
pub(crate) struct AdminEarnSubscriptionsResponse {
    pub(crate) subscriptions: Vec<EarnSubscriptionResponse>,
    pub(crate) total: i64,
}

/// 理财订阅的对外视图，兼作订阅查询的 FromRow 目标。
/// 其中 APR、期限和四项费率都是申购时从产品复制的快照，是赎回结算的唯一依据，
/// 后台修改产品配置不会回溯改写这些值。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct EarnSubscriptionResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) product_id: u64,
    pub(crate) asset_id: u64,
    /// 申购本金，赎回时原样作为本金部分返还。
    pub(crate) amount: BigDecimal,
    /// 申购时快照的年化收益率。
    pub(crate) apr_rate: BigDecimal,
    /// 申购时快照的通用赎回费率。
    pub(crate) redemption_fee_rate: BigDecimal,
    /// 申购时快照的到期利润手续费率。
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    /// 申购时快照的提前赎回费基准。
    pub(crate) early_redeem_fee_basis: String,
    /// 申购时快照的提前赎回费率。
    pub(crate) early_redeem_fee_rate: BigDecimal,
    /// 申购时快照的期限天数，到期计息按该天数占一年的比例折算。
    pub(crate) term_days: u32,
    /// 订阅状态，subscribed 表示持有中，redeemed 表示已赎回。
    pub(crate) status: String,
    /// 申购时使用的用户维度幂等键。
    pub(crate) idempotency_key: String,
    /// 申购时刻，提前赎回按从该时刻起的实际秒数计息。
    #[serde(with = "unix_millis")]
    pub(crate) subscribed_at: DateTime<Utc>,
    /// 到期时刻，等于申购时刻加期限天数，也是提前与到期两种计费口径的分界点。
    #[serde(with = "unix_millis")]
    pub(crate) matures_at: DateTime<Utc>,
    /// 赎回完成时刻，未赎回时为空。
    #[serde(default, with = "option_unix_millis")]
    pub(crate) redeemed_at: Option<DateTime<Utc>>,
}

/// 申购响应，只回传订阅快照；本次是否为新建由服务端内部判断，不体现在报文中。
#[derive(Debug, Serialize)]
pub(crate) struct SubscribeEarnResponse {
    pub(crate) subscription: EarnSubscriptionResponse,
}

/// 赎回响应，除订阅最新状态外还给出逐项金额明细供前端展示。
/// 只有 `redeem_amount` 真正进入钱包，三类费用不产生独立的钱包流水。
#[derive(Debug, Serialize)]
pub(crate) struct RedeemEarnResponse {
    pub(crate) subscription: EarnSubscriptionResponse,
    /// 返还的申购本金。
    pub(crate) principal_amount: BigDecimal,
    /// 未扣任何费用的毛收益。
    pub(crate) gross_yield_amount: BigDecimal,
    /// 展示口径的净收益，只扣以毛收益为基准的费用，不扣通用赎回费。
    pub(crate) yield_amount: BigDecimal,
    /// 通用赎回费，基数为本金加毛收益。
    pub(crate) redemption_fee_amount: BigDecimal,
    /// 到期利润手续费，提前赎回时为零。
    pub(crate) maturity_profit_fee_amount: BigDecimal,
    /// 提前赎回费，到期赎回或基准为 none 时为零。
    pub(crate) early_redeem_fee_amount: BigDecimal,
    /// 三类费用之和。
    pub(crate) fee_amount: BigDecimal,
    /// 实际入账 available 的净到账额，下限为零。
    pub(crate) redeem_amount: BigDecimal,
}
