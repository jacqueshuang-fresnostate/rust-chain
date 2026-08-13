//! convert bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//!
//! 这里的结构只描述 HTTP 报文形状，不承载任何校验或计算规则：金额区间、精度、
//! 汇率与手续费一律由服务层和基础设施层决定。所有时间字段统一以 Unix 毫秒序列化，
//! `BigDecimal` 直接透传避免浮点误差，响应中的费率与金额都是下单时刻的固化快照。

use crate::time::unix_millis;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 交易对列表的查询串，只有一个可选分页量。
#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    /// 期望返回条数，缺省 50，超出 1..=100 时被服务层夹紧而不是报错。
    pub(crate) limit: Option<u32>,
}

/// 订单列表的查询串，用户维度不在此处，由 JWT claims 强制注入。
#[derive(Debug, Deserialize)]
pub(crate) struct ConvertOrdersQuery {
    /// 订单状态过滤词，空白串等价于不过滤；不校验枚举合法性，非法值只会查不到数据。
    pub(crate) status: Option<String>,
    /// 期望返回条数，缺省 50，夹紧到 1..=100。
    pub(crate) limit: Option<u32>,
}

/// 创建报价的请求体。汇率不可由客户端指定，只能提交方向与源金额。
#[derive(Debug, Deserialize)]
pub(crate) struct CreateConvertQuoteRequest {
    /// 源资产编号，配合目标资产定位启用中的正向或反向闪兑规则。
    pub(crate) from_asset_id: u64,
    /// 目标资产编号。
    pub(crate) to_asset_id: u64,
    /// 源资产扣减数量，须为正、落在该方向限额内且有效小数位不超过源资产精度。
    pub(crate) from_amount: BigDecimal,
}

/// 确认报价的请求体，只需报价标识；金额和汇率一律以服务端缓存快照为准。
#[derive(Debug, Deserialize)]
pub(crate) struct ConfirmConvertQuoteRequest {
    /// 报价 UUID 字符串，解析失败在读取 Redis 前即返回参数错误。
    pub(crate) quote_id: String,
}

/// 单个可用闪兑交易对的对外视图，包含双侧资产 Logo 以便前端直接渲染。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertPairResponse {
    pub(crate) id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) from_asset_symbol: String,
    /// 源资产图标地址，直接取自 assets 表，不由符号拼接推导，可能为空。
    pub(crate) from_asset_logo_url: Option<String>,
    pub(crate) to_asset_id: u64,
    pub(crate) to_asset_symbol: String,
    /// 目标资产图标地址，同样允许为空由前端兜底。
    pub(crate) to_asset_logo_url: Option<String>,
    /// 计价模式，fixed 走配置固定汇率，market 走缓存行情。
    pub(crate) pricing_mode: String,
    /// 价差比例，实际汇率为原始汇率乘以 `1 - spread_rate`。
    pub(crate) spread_rate: BigDecimal,
    /// 手续费率，以源资产计费。
    pub(crate) fee_rate: BigDecimal,
    /// 正向下单最小额，以 from_asset 计价。
    pub(crate) min_amount: BigDecimal,
    /// 正向下单最大额，为空表示不设上限。
    pub(crate) max_amount: Option<BigDecimal>,
    /// 反向下单最小额，以 to_asset 计价，供前端切换方向时使用。
    pub(crate) target_min_amount: BigDecimal,
    /// 反向下单最大额，为空表示不设上限。
    pub(crate) target_max_amount: Option<BigDecimal>,
    /// 是否启用；列表接口已按启用过滤，此处恒为真，保留供后台复用同一结构。
    pub(crate) enabled: bool,
}

/// 交易对列表响应包装，按配置行编号倒序排列。
#[derive(Debug, Serialize)]
pub(crate) struct ConvertPairsResponse {
    pub(crate) pairs: Vec<ConvertPairResponse>,
}

/// 单笔闪兑订单的对外视图，所有金额与费率都是确认时固化的历史值。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct ConvertOrderResponse {
    pub(crate) id: u64,
    /// 关联的报价标识，同时是该订单的幂等唯一键。
    pub(crate) quote_id: String,
    pub(crate) convert_pair_id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    /// 实际从源资产 available 扣除的数量。
    pub(crate) from_amount: BigDecimal,
    /// 实际计入目标资产 available 的数量，已扣除手续费。
    pub(crate) to_amount: BigDecimal,
    /// 未叠加价差的原始汇率快照。
    pub(crate) rate: BigDecimal,
    /// 下单时刻的手续费率快照。
    pub(crate) fee_rate: BigDecimal,
    /// 手续费金额，已折入 to_amount，不产生独立钱包流水。
    pub(crate) fee_amount: BigDecimal,
    /// 订单状态，插入时为 pending，结算成功后改为 completed。
    pub(crate) status: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

/// 订单列表响应包装，按订单自增编号倒序排列。
#[derive(Debug, Serialize)]
pub(crate) struct ConvertOrdersResponse {
    pub(crate) orders: Vec<ConvertOrderResponse>,
}

/// 新建报价的响应，字段与落库快照逐一对应，客户端应原样展示不再自行换算。
#[derive(Debug, Serialize)]
pub(crate) struct ConvertQuoteResponse {
    /// 报价 UUID，确认接口凭此兑现。
    pub(crate) quote_id: String,
    pub(crate) convert_pair_id: u64,
    pub(crate) from_asset_id: u64,
    pub(crate) to_asset_id: u64,
    pub(crate) from_amount: BigDecimal,
    /// 预计到账数量，已按目标资产精度向零截断。
    pub(crate) to_amount: BigDecimal,
    pub(crate) rate: BigDecimal,
    pub(crate) spread_rate: BigDecimal,
    pub(crate) fee_rate: BigDecimal,
    /// 以源资产计价的手续费，已按源资产精度向零截断。
    pub(crate) fee_amount: BigDecimal,
    /// 报价失效时刻，到点后确认接口一律拒绝。
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
}

/// 确认接口的响应，成功路径下 `confirmed` 恒为真；重复确认走冲突错误而非返回假值。
#[derive(Debug, Serialize)]
pub(crate) struct ConfirmConvertQuoteResponse {
    pub(crate) quote_id: String,
    pub(crate) confirmed: bool,
}
