//! seconds_contract bounded context repository layer.
//!
//! 仓储层：定义秒合约与 MySQL 之间的读写契约，只描述数据形状，不含任何 SQL 与业务判定。
//! 类型分三组：以 `Row` 结尾的是 `sqlx::FromRow` 查询映射，`Write`/`Insert` 结尾的是写入参数聚合，
//! `Filter` 结尾的是已归一化的查询条件。
//! 所有金额与赔率统一使用 `BigDecimal` 承载，禁止在本上下文中降级为浮点，
//! 因为秒合约赔付直接进用户钱包，任何二进制浮点误差都会变成真实的资金差额。

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

/// 秒合约产品主表连表查询结果，用于面向展示的产品目录与后台列表。
/// 其中的时长、赔率与投注上下限是周期集合首条的冗余副本，供不支持多周期的旧客户端读取，
/// 不能直接当作下单校验依据，下单必须使用带资产精度的规则行。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SecondsContractProductRow {
    /// 产品主键。
    pub(crate) id: u64,
    /// 该产品挂靠的交易对编号，决定开仓价与结算价取自哪条行情。
    pub(crate) pair_id: u64,
    /// 交易对展示符号，来自连表而非产品表自身字段。
    pub(crate) symbol: String,
    /// 投注与派奖使用的质押资产编号，也是钱包扣款和入账的资产维度。
    pub(crate) stake_asset: u64,
    /// 质押资产展示符号，来自连表。
    pub(crate) stake_asset_symbol: String,
    /// 产品图标地址，为空表示前端使用默认图。
    pub(crate) logo_url: Option<String>,
    /// 默认周期时长，单位为秒。
    pub(crate) duration_seconds: u32,
    /// 默认周期的赢单净收益率，不含本金。
    pub(crate) payout_rate: BigDecimal,
    /// 默认周期的单笔最小投注额。
    pub(crate) min_stake: BigDecimal,
    /// 默认周期的单笔最大投注额，`None` 表示不限。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 上下架状态，取值为 `active` 或 `disabled`。
    pub(crate) status: String,
}

/// 下单路径专用的产品规则行，由锁定查询产出，代表本次下单实际适用的那一条周期规则。
/// 与展示用的产品行相比，这里没有图标等展示字段，额外带上质押资产精度，
/// 且时长、赔率与投注区间已被解析为选中周期的取值而非产品默认档位。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SecondsContractProductRuleRow {
    /// 产品主键，写入订单的 `product_id`。
    pub(crate) id: u64,
    /// 交易对编号，同时决定读取哪个行情缓存键取开仓价。
    pub(crate) pair_id: u64,
    /// 交易对符号，用于拼接行情缓存键。
    pub(crate) symbol: String,
    /// 质押资产编号，即扣本金和派奖所针对的钱包资产。
    pub(crate) stake_asset: u64,
    /// 质押资产的小数位精度，用于投注额合法性校验与赔付金额向零截断。
    pub(crate) stake_asset_precision: i32,
    /// 本次下单选中周期的时长，单位为秒，与开仓时间相加得到到期时刻。
    pub(crate) duration_seconds: u32,
    /// 本次下单选中周期的赔率，会被固化进订单快照，后续改配置不影响该单结算。
    pub(crate) payout_rate: BigDecimal,
    /// 选中周期的最小投注额。
    pub(crate) min_stake: BigDecimal,
    /// 选中周期的最大投注额，`None` 表示该周期不设上限。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 产品状态；锁定查询已过滤为 `active`，此处保留取值供审计与断言。
    pub(crate) status: String,
}

/// 钱包账户在行锁保护下的余额快照，秒合约只读取不直接修改。
/// 三项余额之和构成用户在该资产上的总持有量，秒合约的扣款与派奖只作用于可用余额，
/// 冻结与锁定余额原样带出，仅用于写资金流水时记录当时的完整余额分布。
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SecondsContractWalletRow {
    /// 可用余额，开仓扣本金与结算派奖的唯一操作对象。
    pub(crate) available: BigDecimal,
    /// 冻结余额，由其他业务占用，秒合约不改动。
    pub(crate) frozen: BigDecimal,
    /// 锁定余额，由风控或理财等场景占用，秒合约不改动。
    pub(crate) locked: BigDecimal,
}

/// 后台订单查询的归一化筛选条件，字段均已由应用层裁剪处理。
/// 三个可选筛选项同时给出时按 AND 叠加，全部为空则返回全量订单。
#[derive(Debug, Clone)]
pub(crate) struct SecondsContractAdminOrderFilter {
    /// 按下单用户编号精确筛选。
    pub(crate) user_id: Option<u64>,
    /// 按账号邮箱精确匹配，空白串已被降级为 `None` 而非空串匹配。
    pub(crate) email: Option<String>,
    /// 按订单状态筛选，如 `opened` 或 `settled`。
    pub(crate) status: Option<String>,
    /// 单页条数，已被夹在 1 到 100 之间。
    pub(crate) limit: u32,
    /// 分页偏移，已被截断到 100000 以内。
    pub(crate) offset: u32,
}

/// 产品主表的写入参数，创建与更新共用，两种场景都按全字段覆盖语义使用。
/// 其中的周期相关字段取自周期集合首条，与周期子表由同一次事务保持同步。
#[derive(Debug, Clone)]
pub(crate) struct SecondsContractProductWrite {
    /// 目标交易对编号，写入前已确认该交易对存在。
    pub(crate) pair_id: u64,
    /// 目标质押资产编号，写入前已确认该资产存在。
    pub(crate) stake_asset: u64,
    /// 产品图标地址，已裁剪空白并校验长度，`None` 表示清空该字段。
    pub(crate) logo_url: Option<String>,
    /// 默认周期时长，取自周期集合首条。
    pub(crate) duration_seconds: u32,
    /// 默认周期赔率，取自周期集合首条。
    pub(crate) payout_rate: BigDecimal,
    /// 默认周期最小投注额。
    pub(crate) min_stake: BigDecimal,
    /// 默认周期最大投注额，`None` 表示不限。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 归一化后的上下架状态。
    pub(crate) status: String,
}

/// 开仓订单的插入参数，承载下单当刻被固化下来的全部快照值。
/// 时长、赔率与开仓价一旦随本结构落库就不再变化，产品后续改配置不影响该订单的结算口径。
#[derive(Debug, Clone)]
pub(crate) struct SecondsContractOrderInsert {
    /// 下单用户编号，来自鉴权令牌而非请求体。
    pub(crate) user_id: u64,
    /// 产品编号。
    pub(crate) product_id: u64,
    /// 交易对编号，冗余存储以便订单查询免去连产品表。
    pub(crate) pair_id: u64,
    /// 质押资产编号，决定从哪个钱包扣本金。
    pub(crate) stake_asset: u64,
    /// 看涨或看跌方向，已归一化为 `up` 或 `down`。
    pub(crate) direction: String,
    /// 投注本金，开仓时从可用余额等额扣减。
    pub(crate) stake_amount: BigDecimal,
    /// 选中周期时长，单位为秒。
    pub(crate) duration_seconds: u32,
    /// 该周期的赔率快照，结算时按此值而非产品当前配置计算赔付。
    pub(crate) payout_rate: BigDecimal,
    /// 服务端从行情缓存取得的开仓价，客户端上送值一律不采纳。
    pub(crate) entry_price: BigDecimal,
    /// 下单幂等键，与用户编号共同构成唯一约束，是防重复扣款的依据。
    pub(crate) idempotency_key: String,
    /// 到期时刻，由下单时间加周期时长算出，结算取价以此为时间锚点。
    pub(crate) expires_at: DateTime<Utc>,
}

/// 秒合约资金流水的写入参数，开仓扣款与结算派奖共用同一结构。
/// 各项 `*_after` 必须是同一次钱包更新之后的余额，调用方要在持有钱包行锁期间算好再传入。
#[derive(Debug, Clone)]
pub(crate) struct SecondsContractWalletLedgerWrite {
    /// 资金归属用户编号。
    pub(crate) user_id: u64,
    /// 发生变动的资产编号。
    pub(crate) asset_id: u64,
    /// 变动类型，开仓为 `seconds_contract_open`，赢单派奖为 `seconds_contract_settle_win`。
    pub(crate) change_type: &'static str,
    /// 本次变动额，开仓扣款为负数，派奖入账为正数。
    pub(crate) amount: BigDecimal,
    /// 变动后的可用余额，同时也被写入流水的通用余额字段。
    pub(crate) available_after: BigDecimal,
    /// 变动后的冻结余额，秒合约不改动该项，此处记录当时快照。
    pub(crate) frozen_after: BigDecimal,
    /// 变动后的锁定余额，秒合约不改动该项，此处记录当时快照。
    pub(crate) locked_after: BigDecimal,
    /// 关联订单主键的字符串形式，配合固定的 `seconds_contract_order` 引用类型用于反查对账。
    pub(crate) ref_id: String,
}
