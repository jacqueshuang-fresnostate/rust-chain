//! earn bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。
//!
//! 现阶段这里只承载数据形态而没有 trait 端口：几个 `*Write` 结构描述写入载荷，
//! 几个 `*Row` 结构描述查询取回的行。它们刻意与表现层 DTO 分开，
//! 使得校验归一后的内部形态与对外报文可以各自演进。
//! 其中费率相关结构是理财「快照语义」的落点：申购时把这些值复制进订阅行后即固化。

use bigdecimal::BigDecimal;
use serde_json::Value;

/// 申购事务内锁定产品后取回的条款行，只含下单判定与订阅快照真正需要的列。
/// 这些值随即被逐字复制进 `earn_subscriptions`，之后产品改配置不再影响该订阅。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct EarnProductRuleRow {
    pub(crate) id: u64,
    /// 申购与赎回共用的资产。
    pub(crate) asset_id: u64,
    /// 期限天数，用于推算到期时刻与到期计息比例。
    pub(crate) term_days: u32,
    /// 年化收益率。
    pub(crate) apr_rate: BigDecimal,
    /// 通用赎回费率，赎回时对本金加毛收益整体计费。
    pub(crate) redemption_fee_rate: BigDecimal,
    /// 到期利润手续费率，只在到期赎回时对毛收益计费。
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    /// 提前赎回费基准，取 none、principal 或 profit。
    pub(crate) early_redeem_fee_basis: String,
    /// 提前赎回费率，基准为 none 时已在配置阶段被强制归零。
    pub(crate) early_redeem_fee_rate: BigDecimal,
    /// 单笔最小申购额。
    pub(crate) min_subscribe: BigDecimal,
    /// 单笔最大申购额，为空表示不限。
    pub(crate) max_subscribe: Option<BigDecimal>,
    /// 产品状态，非 active 时不允许申购。
    pub(crate) status: String,
}

/// 归一后的四项费用配置，创建与更新产品共用同一形态。
/// 缺省项已补零、基准已校验，且基准为 none 时提前赎回费率必定为零。
#[derive(Debug, Clone)]
pub(crate) struct EarnProductFeeConfig {
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
}

/// 分类的写入载荷，创建与更新共用；更新时 code 由调用方从已锁定的旧行原样带回。
#[derive(Debug, Clone)]
pub(crate) struct EarnCategoryWrite {
    /// 稳定分类代码，产品通过它引用分类，创建后不可变更。
    pub(crate) code: String,
    /// 多语言名称结构，缺省时按 code 生成中文兜底条目。
    pub(crate) name_json: Value,
    /// 列表排序权重，升序排列。
    pub(crate) sort_order: i32,
    /// 启停状态，仅 active 分类可被新产品引用。
    pub(crate) status: String,
}

/// 产品的写入载荷，字段均已通过应用层校验与归一，创建与整体更新共用。
/// 四项费率此处即定稿，后续申购会把它们复制进订阅快照。
#[derive(Debug, Clone)]
pub(crate) struct EarnProductWrite {
    pub(crate) asset_id: u64,
    /// 已裁剪的产品名，长度不超过 128 字符。
    pub(crate) name: String,
    /// 横幅图地址，可空，长度不超过 2048 字符。
    pub(crate) banner_url: Option<String>,
    /// 小图标地址，可空，同样限长 2048 字符。
    pub(crate) small_logo_url: Option<String>,
    /// 引用的分类代码，写入前会校验该分类存在且处于 active。
    pub(crate) category: String,
    /// 多语言富文本介绍，节点类型受白名单限制。
    pub(crate) introduction_json: Value,
    pub(crate) term_days: u32,
    pub(crate) apr_rate: BigDecimal,
    pub(crate) redemption_fee_rate: BigDecimal,
    pub(crate) maturity_profit_fee_rate: BigDecimal,
    pub(crate) early_redeem_fee_basis: String,
    pub(crate) early_redeem_fee_rate: BigDecimal,
    pub(crate) min_subscribe: BigDecimal,
    pub(crate) max_subscribe: Option<BigDecimal>,
    pub(crate) status: String,
}

/// 资金事务内锁定钱包行后取回的三桶余额。
/// 理财只增减 available，frozen 与 locked 读出来仅用于写流水时的账后快照。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct EarnWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}
