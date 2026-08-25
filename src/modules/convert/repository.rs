//! convert bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//!
//! MySQL `convert_quotes` 是线上资金确认的唯一权威事实，保存 owner、指纹、价格证据、
//! 过期与消费状态；Redis 端口只保留为可丢失的展示缓存和纯内存服务测试适配器。
//! `ConvertOrderRepository` 的旧兼容端口仍由 MySQL 唯一键提供幂等，但线上确认路径会在
//! 同一个事务内锁 quote、按稳定资产顺序锁钱包并完成消费。数据库行到内存规则的中间
//! 结构也在本文件声明，供服务层做正反向换算。

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

use crate::modules::convert::domain::{
    ConvertConfirmationInsert, ConvertQuoteCacheEntry, ConvertQuoteConfirmationRecord,
    ConvertRepositoryError, QuoteId,
};

/// 可丢失报价缓存的读写端口，生产实现为 Redis，测试可替换为内存 Map。
/// 该端口不参与线上资金确认；缓存命中也不能覆盖 MySQL 的 owner、过期或消费状态。
pub trait ConvertQuoteRepository {
    /// 保存限时报价缓存快照，TTL 与 expires_at 必须表达同一过期边界。
    /// 实现失败返回仓储错误；不得把缺少缓存的报价伪装成可确认状态。
    fn save_quote_ttl(
        &mut self,
        entry: ConvertQuoteCacheEntry,
    ) -> Result<(), ConvertRepositoryError>;

    /// 按报价标识读取限时缓存；未命中返回空，存储或反序列化失败必须区分为错误。
    fn get_quote_ttl(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<ConvertQuoteCacheEntry>, ConvertRepositoryError>;
}

/// 确认记录的写入端口，是闪兑资金幂等的唯一守门人。
/// 生产实现依赖 `convert_orders` 表上 quote_id 的唯一约束把并发重放收敛为一次入账。
pub trait ConvertOrderRepository {
    /// 写入报价确认记录并以唯一键区分首次确认和重复确认。
    /// 实现必须让重放返回 Duplicate，禁止同一报价生成第二份订单或资金副作用。
    fn insert_quote_confirmation(
        &mut self,
        record: ConvertQuoteConfirmationRecord,
    ) -> Result<ConvertConfirmationInsert, ConvertRepositoryError>;
}

/// 写入 `convert_quotes` 表的一行报价快照，全部数值均为报价时刻固化的结果。
/// 汇率、价差与费率在此处落库，后续修改交易对配置不会改写已有报价行。
#[derive(Debug, Clone)]
pub struct ConvertQuoteInsert {
    /// 报价标识，表上有唯一约束，重复插入会退化为回读既有行。
    pub quote_id: QuoteId,
    /// 生成该报价所依据的 `convert_pairs` 配置行编号。
    pub convert_pair_id: u64,
    /// 报价归属用户。
    pub user_id: u64,
    /// 源资产编号，扣款方向。
    pub from_asset_id: u64,
    /// 目标资产编号，入账方向。
    pub to_asset_id: u64,
    /// 用户提交的源资产数量。
    pub from_amount: BigDecimal,
    /// 已扣手续费并按目标资产精度截断后的到账数量。
    pub to_amount: BigDecimal,
    /// 未叠加价差的原始汇率快照。
    pub rate: BigDecimal,
    /// 价差比例快照，已在 to_amount 计算中生效。
    pub spread_rate: BigDecimal,
    /// 手续费率快照。
    pub fee_rate: BigDecimal,
    /// 以源资产计价的手续费金额，已从净额中扣除。
    pub fee_amount: BigDecimal,
    /// 对报价全部资金字段与价格证据计算出的 SHA-256 指纹。
    pub request_fingerprint: String,
    /// 固定配置或行情供应商来源。
    pub price_source: String,
    /// 市场计价交易对；固定计价为空。
    pub price_symbol: Option<String>,
    /// 价格或固定配置的观察时刻。
    pub price_observed_at: DateTime<Utc>,
    /// 价格事件摘要或固定配置版本。
    pub price_version: String,
    /// 报价绝对到期时刻，与 Redis 键 TTL 表达同一边界。
    pub expires_at: chrono::DateTime<Utc>,
}

/// 报价落库结果，用于区分「本次真的插入了新行」和「唯一键命中后回读」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertQuoteInsertResult {
    /// `convert_quotes` 的自增主键，重复插入时通过回查补齐。
    pub quote_row_id: u64,
    /// 是否由本次调用真正写入了新行。
    pub inserted: bool,
}

/// `load_pair_rule` 联表查询的原始行，同时携带正向与反向两套限额以及市场计价线索。
/// 该结构保留数据库视角的字段方向，尚未按用户请求方向做换算。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertPairRuleDbRecord {
    pub(crate) id: u64,
    /// 闪兑对当前是否启用；确认阶段锁行后必须再次校验。
    pub(crate) enabled: bool,
    /// 配置中定义的正向源资产，未必等于用户请求的源资产。
    pub(crate) from_asset_id: u64,
    /// 配置中定义的正向目标资产。
    pub(crate) to_asset_id: u64,
    /// 计价模式，仅支持 fixed 与 market 两种取值。
    pub(crate) pricing_mode: String,
    /// 价差比例，最终汇率为 `rate * (1 - spread_rate)`。
    pub(crate) spread_rate: BigDecimal,
    /// 手续费率，须落在 `[0, 1)` 区间。
    pub(crate) fee_rate: BigDecimal,
    /// 正向最小下单额，以正向源资产计价。
    pub(crate) min_amount: BigDecimal,
    /// 正向最大下单额，为空表示不限。
    pub(crate) max_amount: Option<BigDecimal>,
    /// 反向最小下单额，以正向目标资产计价。
    pub(crate) target_min_amount: BigDecimal,
    /// 反向最大下单额，为空表示不限。
    pub(crate) target_max_amount: Option<BigDecimal>,
    /// 来自活动 `new_coin_convert_rules` 的固定汇率，仅 fixed 模式使用。
    pub(crate) fixed_rate: Option<BigDecimal>,
    /// 关联的活动现货交易对符号，仅 market 模式使用。
    pub(crate) market_pair_symbol: Option<String>,
    /// 该现货交易对的 base 资产编号，用于判定行情方向。
    pub(crate) market_base_asset_id: Option<u64>,
    /// 该现货交易对的 quote 资产编号，用于判定行情方向。
    pub(crate) market_quote_asset_id: Option<u64>,
    /// pair 与固定规则中较新的配置更新时间，用作 fixed quote 版本。
    pub(crate) pricing_updated_at: DateTime<Utc>,
}

/// 已按用户请求方向归一化的报价规则：限额换成请求方向那一侧，固定汇率在反向时取倒数。
/// 服务层与应用层只面向该结构做校验和计价，不再关心配置里资产的原始排列顺序。
#[derive(Debug, Clone)]
pub(crate) struct ConvertPairRule {
    pub(crate) id: u64,
    /// 用户请求的源资产。
    pub(crate) from_asset_id: u64,
    /// 用户请求的目标资产。
    pub(crate) to_asset_id: u64,
    pub(crate) pricing_mode: String,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    /// 请求方向对应的最小下单额。
    pub(crate) min_amount: BigDecimal,
    /// 请求方向对应的最大下单额，为空表示不限。
    pub(crate) max_amount: Option<BigDecimal>,
    /// 请求方向对应的固定汇率，反向请求时已取过倒数。
    pub(crate) fixed_rate: Option<BigDecimal>,
    pub(crate) market_pair_symbol: Option<String>,
    pub(crate) market_base_asset_id: Option<u64>,
    pub(crate) market_quote_asset_id: Option<u64>,
    pub(crate) pricing_updated_at: DateTime<Utc>,
}

/// MySQL 行锁下读取的权威闪兑报价，确认阶段不再读取 Redis。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertAuthoritativeQuoteRecord {
    pub(crate) quote_id: String,
    pub(crate) convert_pair_id: u64,
    pub(crate) user_id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) from_amount: BigDecimal,
    pub(crate) to_amount: BigDecimal,
    pub(crate) rate: BigDecimal,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) request_fingerprint: String,
    pub(crate) price_source: String,
    pub(crate) price_symbol: Option<String>,
    pub(crate) price_observed_at: DateTime<Utc>,
    pub(crate) price_version: String,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) consumed_at: Option<DateTime<Utc>>,
}

/// MySQL 行情历史中用于市场闪兑计价的不可变快照。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertMarketPriceSnapshot {
    pub(crate) price: BigDecimal,
    pub(crate) source: String,
    pub(crate) symbol: String,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) source_version: String,
}

/// 报价阶段非锁定读取的钱包余额快照，只用于提示性校验，确认时会在事务内重新锁行复核。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct WalletBalanceRecord {
    /// 可自由动用余额。
    pub(crate) available: BigDecimal,
    /// 被其他业务占用的余额，仅作为错误上下文回显。
    pub(crate) locked: BigDecimal,
}

/// 结算事务内以 `FOR UPDATE` 锁定的 pending 订单快照，金额来自报价固化值而非实时重算。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertSettlementOrderRecord {
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    /// 需从源资产 available 全额扣除的数量。
    pub(crate) from_amount: BigDecimal,
    /// 需向目标资产 available 增加的数量。
    pub(crate) to_amount: BigDecimal,
}

/// 结算事务内以 `FOR UPDATE` 锁定的钱包三段余额，frozen 与 locked 在闪兑中保持不变，只用于写流水快照。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ConvertSettlementWalletRecord {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}
