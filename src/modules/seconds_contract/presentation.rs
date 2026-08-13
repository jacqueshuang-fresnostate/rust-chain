//! seconds_contract bounded context presentation layer.
//!
//! 表现层：定义秒合约对外的请求与响应 DTO，是 HTTP 报文与内部类型之间的唯一转换边界。
//! 请求类型只做反序列化，不带任何默认值补全与取值校验，归一和校验统一由服务层负责，
//! 因此这里大量使用 `Option` 表达「客户端未提供」而非「业务默认值」。
//! 响应类型中的金额与赔率使用 `BigDecimal` 序列化以保留完整精度；
//! 所有时间字段统一用 `unix_millis` 序列化成毫秒时间戳，避免客户端因时区解析出现到期时刻偏差。
//! 本文件不含任何业务逻辑、数据库访问或资金计算。

use crate::time::unix_millis;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 用户侧产品目录与订单历史共用的查询串，只支持限制条数。
/// 缺省时由 `route_limit` 补默认 50 并封顶 100；本类型不支持偏移分页。
#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    /// 期望返回的条数上限，未给出时使用服务端默认值。
    pub(crate) limit: Option<u32>,
}

/// 后台产品列表查询串，比用户侧多出偏移分页能力。
/// 不含状态筛选字段，后台列表固定返回含已禁用产品在内的全量结果。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminProductsQuery {
    /// 单页条数，归一后夹在 1 到 100 之间。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，归一后截断到 100000 以内。
    pub(crate) offset: Option<u32>,
}

/// 后台订单列表查询串，支持分页与三个可选筛选维度的组合。
/// 三个筛选项同时给出时按 AND 叠加，均为空则返回全量订单。
#[derive(Debug, Deserialize)]
pub(crate) struct AdminOrdersQuery {
    /// 单页条数，归一后夹在 1 到 100 之间。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，归一后截断到 100000 以内。
    pub(crate) offset: Option<u32>,
    /// 按下单用户编号精确筛选。
    pub(crate) user_id: Option<u64>,
    /// 按账号邮箱精确匹配，空白串会被裁剪成不筛选。
    pub(crate) email: Option<String>,
    /// 按订单状态筛选，空白串同样降级为不筛选。
    pub(crate) status: Option<String>,
}

/// 用户开仓请求体，不含用户维度字段，下单人始终由鉴权令牌确定。
/// 请求体中也没有价格字段，开仓价由服务端从行情缓存读取，客户端无法指定。
#[derive(Debug, Deserialize)]
pub(crate) struct OpenSecondsContractOrderRequest {
    /// 目标秒合约产品编号。
    pub(crate) product_id: u64,
    /// 选择的周期时长，单位为秒；不给出时回落到产品默认周期，兼容旧版客户端。
    pub(crate) duration_seconds: Option<u32>,
    /// 看涨或看跌，服务端会去空白转小写后只接受 `up` 与 `down`。
    pub(crate) direction: String,
    /// 投注本金，必须为正数并符合质押资产精度与该周期的投注区间。
    pub(crate) stake_amount: BigDecimal,
    /// 客户端生成的幂等键，同一用户下重复提交同键请求不会二次扣款。
    pub(crate) idempotency_key: String,
}

/// 后台配置产品周期时的单条周期入参，四个字段全部可空以便服务端给出精确的缺字段提示。
/// 除最大投注额外其余三项实为必填，缺失会在校验阶段返回参数错误而不是套用默认值。
#[derive(Debug, Deserialize)]
pub(crate) struct SecondsContractProductCycleInput {
    /// 周期时长，单位为秒，必须为正且在同一产品内唯一。
    pub(crate) duration_seconds: Option<u32>,
    /// 赢单净收益率，允许为零但不允许为负。
    pub(crate) payout_rate: Option<BigDecimal>,
    /// 该周期的单笔最小投注额，必须为正数。
    pub(crate) min_stake: Option<BigDecimal>,
    /// 该周期的单笔最大投注额，真正可省略，省略表示不设上限。
    pub(crate) max_stake: Option<BigDecimal>,
}

/// 对外返回的单条产品周期配置，同时用作周期子表的查询映射。
/// 产品无周期子表记录时，读取层会用产品主记录合成一条虚拟周期，此时主键与排序号均为 0。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct SecondsContractProductCycleResponse {
    /// 周期记录主键；取值为 0 表示这是由主记录兜底合成的虚拟周期而非真实子表行。
    pub(crate) id: u64,
    /// 所属产品编号。
    pub(crate) product_id: u64,
    /// 周期时长，单位为秒，也是客户端下单时用来选档的键。
    pub(crate) duration_seconds: u32,
    /// 该周期的赢单净收益率，不含本金。
    pub(crate) payout_rate: BigDecimal,
    /// 该周期允许的最小投注额。
    pub(crate) min_stake: BigDecimal,
    /// 该周期允许的最大投注额，`None` 表示不限。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 展示排序号，写入时取周期切片下标，排序后的首条即产品默认档位。
    pub(crate) sort_order: u32,
}

/// 后台新建秒合约产品的请求体，同时兼容多周期与旧版单周期两种配置写法。
/// 优先采用 `cycles`；未给出时才用 `duration_seconds` 等旧字段合成唯一周期，此时这三项变为必填。
#[derive(Debug, Deserialize)]
pub(crate) struct CreateSecondsContractProductRequest {
    /// 挂靠的交易对编号，不得为零且必须真实存在。
    pub(crate) pair_id: u64,
    /// 质押资产编号，不得为零且必须真实存在。
    pub(crate) stake_asset: u64,
    /// 产品图标地址，可省略，长度上限 2048 字符。
    pub(crate) logo_url: Option<String>,
    /// 旧版单周期时长，仅在未提供 `cycles` 时使用。
    pub(crate) duration_seconds: Option<u32>,
    /// 旧版单周期赔率，仅在未提供 `cycles` 时使用。
    pub(crate) payout_rate: Option<BigDecimal>,
    /// 旧版单周期最小投注额，仅在未提供 `cycles` 时使用。
    pub(crate) min_stake: Option<BigDecimal>,
    /// 旧版单周期最大投注额，仅在未提供 `cycles` 时使用。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 多周期配置集合，给出时不得为空数组，且各条时长必须互不相同。
    pub(crate) cycles: Option<Vec<SecondsContractProductCycleInput>>,
    /// 初始上下架状态，省略时按 `active` 处理，即创建后立即可下单。
    pub(crate) status: Option<String>,
    /// 审计原因，字段可空但业务上必填，缺失会在校验阶段被拒绝。
    pub(crate) reason: Option<String>,
}

/// 后台更新秒合约产品的请求体，按整体覆盖语义处理，未列出的周期会被删除。
/// 与创建请求的唯一结构差异是 `status` 从可选变为必填，前端必须显式表明更新后的上下架状态。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSecondsContractProductRequest {
    /// 更新后的交易对编号，允许改挂到其他交易对。
    pub(crate) pair_id: u64,
    /// 更新后的质押资产编号。
    pub(crate) stake_asset: u64,
    /// 更新后的图标地址，传空白等同于清空。
    pub(crate) logo_url: Option<String>,
    /// 旧版单周期时长，仅在未提供 `cycles` 时生效。
    pub(crate) duration_seconds: Option<u32>,
    /// 旧版单周期赔率，仅在未提供 `cycles` 时生效。
    pub(crate) payout_rate: Option<BigDecimal>,
    /// 旧版单周期最小投注额，仅在未提供 `cycles` 时生效。
    pub(crate) min_stake: Option<BigDecimal>,
    /// 旧版单周期最大投注额，仅在未提供 `cycles` 时生效。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 更新后的完整周期集合，按覆盖处理，遗漏的周期视为删除。
    pub(crate) cycles: Option<Vec<SecondsContractProductCycleInput>>,
    /// 更新后的上下架状态，必填。
    pub(crate) status: String,
    /// 审计原因，业务上必填。
    pub(crate) reason: Option<String>,
}

/// 产品上下架请求体，只携带目标状态与原因，不涉及任何交易参数。
/// 单独设一个精简 DTO，使运营快速下架时无需回填完整产品配置。
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSecondsContractProductStatusRequest {
    /// 目标状态，只接受 `active` 或 `disabled`。
    pub(crate) status: String,
    /// 审计原因，业务上必填，用于记录下架或恢复的责任说明。
    pub(crate) reason: Option<String>,
}

/// 产品删除请求体，仅承载审计原因。
/// DELETE 请求之所以带请求体，正是因为原因属于必填的审计信息，无法省略。
#[derive(Debug, Deserialize)]
pub(crate) struct DeleteSecondsContractProductRequest {
    /// 审计原因，业务上必填。
    pub(crate) reason: Option<String>,
}

/// 对外返回的秒合约产品完整视图，用户目录与后台管理页共用同一结构。
/// 顶层的时长、赔率与投注区间是 `cycles` 首条的冗余展开，供不解析周期数组的旧客户端直接读取。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SecondsContractProductResponse {
    /// 产品主键。
    pub(crate) id: u64,
    /// 交易对编号。
    pub(crate) pair_id: u64,
    /// 交易对展示符号。
    pub(crate) symbol: String,
    /// 质押资产编号。
    pub(crate) stake_asset: u64,
    /// 质押资产展示符号。
    pub(crate) stake_asset_symbol: String,
    /// 产品图标地址，为空时前端使用默认图。
    pub(crate) logo_url: Option<String>,
    /// 默认周期时长，等于 `cycles` 首条的时长。
    pub(crate) duration_seconds: u32,
    /// 默认周期赔率。
    pub(crate) payout_rate: BigDecimal,
    /// 默认周期最小投注额。
    pub(crate) min_stake: BigDecimal,
    /// 默认周期最大投注额，`None` 表示不限。
    pub(crate) max_stake: Option<BigDecimal>,
    /// 全部可选周期档位，读取层保证至少含一条。
    pub(crate) cycles: Vec<SecondsContractProductCycleResponse>,
    /// 上下架状态；用户目录只会返回 `active`，后台可见 `disabled`。
    pub(crate) status: String,
}

/// 行情缓存中 ticker 快照的最小反序列化视图，只取秒合约开仓定价所需的两个字段。
/// 该结构对应 Redis 里由行情模块写入的 JSON，字段缺失或类型不符会被判定为数据契约破损。
#[derive(Debug, Deserialize)]
pub(crate) struct CachedTickerPayload {
    /// 最新成交价，秒合约据此确定开仓价；非正数会被拒绝下单。
    pub(crate) last_price: BigDecimal,
    /// 该报价的观测时刻，以毫秒时间戳传输；早于当前 60 秒即视为陈旧行情并拒绝开仓。
    #[serde(with = "unix_millis")]
    pub(crate) observed_at: DateTime<Utc>,
}

/// 用户侧产品目录响应，只含产品数组而不返回总数，因为该接口不支持偏移分页。
#[derive(Debug, Serialize)]
pub(crate) struct SecondsContractProductsResponse {
    /// 当前可下单的产品列表。
    pub(crate) products: Vec<SecondsContractProductResponse>,
}

/// 后台产品列表响应，比用户侧多返回筛选后的总数以支撑分页控件。
#[derive(Debug, Serialize)]
pub(crate) struct AdminSecondsContractProductsResponse {
    /// 当前页的产品列表，含已禁用产品。
    pub(crate) products: Vec<SecondsContractProductResponse>,
    /// 与当前筛选条件一致的产品总数，不受分页参数影响。
    pub(crate) total: i64,
}

/// 用户侧订单历史响应，包含持仓中与已结算订单，不返回总数。
#[derive(Debug, Serialize)]
pub(crate) struct SecondsContractOrdersResponse {
    /// 按创建时间倒序排列的订单列表。
    pub(crate) orders: Vec<SecondsContractOrderResponse>,
}

/// 后台订单列表响应，附带匹配总数供客服与风控分页翻查。
#[derive(Debug, Serialize)]
pub(crate) struct AdminSecondsContractOrdersResponse {
    /// 当前页的订单列表，含账号邮箱等后台专用展示字段。
    pub(crate) orders: Vec<SecondsContractOrderResponse>,
    /// 与当前筛选条件一致的订单总数。
    pub(crate) total: i64,
}

/// 秒合约订单的对外视图，同时用作订单查询的 `FromRow` 映射，因此字段顺序与查询列表严格对应。
/// 结构在用户侧与后台侧共用，差别在于用户侧查询把 `email` 固定选为 NULL，不回显账号。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct SecondsContractOrderResponse {
    /// 订单主键，同时是资金流水的引用编号。
    pub(crate) id: u64,
    /// 下单用户编号。
    pub(crate) user_id: u64,
    /// 账号邮箱，仅后台查询回显；用户侧查询恒为 `None`。
    pub(crate) email: Option<String>,
    /// 产品编号。
    pub(crate) product_id: u64,
    /// 交易对编号。
    pub(crate) pair_id: u64,
    /// 交易对展示符号。
    pub(crate) symbol: String,
    /// 质押资产编号。
    pub(crate) stake_asset: u64,
    /// 质押资产展示符号。
    pub(crate) stake_asset_symbol: String,
    /// 下单方向，取值为 `up` 或 `down`。
    pub(crate) direction: String,
    /// 投注本金，开仓时已从可用余额扣除。
    pub(crate) stake_amount: BigDecimal,
    /// 该单的周期时长，单位为秒。
    pub(crate) duration_seconds: u32,
    /// 下单时固化的赔率，结算按此值计算赔付而非产品当前配置。
    pub(crate) payout_rate: BigDecimal,
    /// 服务端认定的开仓价，理论上开仓成功即有值。
    pub(crate) entry_price: Option<BigDecimal>,
    /// 结算价，未结算订单为 `None`；与开仓价比对即可复核胜负判定。
    pub(crate) settlement_price: Option<BigDecimal>,
    /// 订单状态，`opened` 为持仓中，`settled` 为已结算。
    pub(crate) status: String,
    /// 胜负结果，`win` 或 `loss`；未结算时为 `None`。
    pub(crate) result: Option<String>,
    /// 下单时使用的幂等键，可用于客户端核对重放是否命中原单。
    pub(crate) idempotency_key: String,
    /// 到期时刻，以毫秒时间戳输出，前端据此展示倒计时。
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
    /// 下单时刻，以毫秒时间戳输出，与到期时刻之差即周期时长。
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

/// 开仓接口响应，只回单笔订单快照。
/// 幂等重放命中时返回的是既有订单而非新建订单，客户端可用订单主键或创建时间加以区分。
#[derive(Debug, Serialize)]
pub(crate) struct OpenSecondsContractOrderResponse {
    /// 本次开仓成功或重放命中的订单。
    pub(crate) order: SecondsContractOrderResponse,
}

/// 后台人工结算请求体，胜负由调用方给出，服务端不自行比价推导。
#[derive(Debug, Deserialize)]
pub(crate) struct SettleSecondsContractOrderRequest {
    /// 结算结果，只接受 `win` 或 `loss`；与既有结算结果冲突时请求会被拒绝。
    pub(crate) result: String,
    /// 审计原因，业务上必填，用于记录人工介入结算的依据。
    pub(crate) reason: Option<String>,
}

/// 结算接口响应，除订单终态外单独返回本次赔付金额，便于前端直接展示到账数额。
/// 输单时该金额为按资产精度规整后的零，表示本金不退回。
#[derive(Debug, Serialize)]
pub(crate) struct SettleSecondsContractOrderResponse {
    /// 结算后的订单快照，含结算价与胜负结果。
    pub(crate) order: SecondsContractOrderResponse,
    /// 实际入账的赔付额，赢单为含本金的总额，输单为零。
    pub(crate) payout_amount: BigDecimal,
}
