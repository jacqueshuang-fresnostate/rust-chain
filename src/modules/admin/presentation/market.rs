//! 后台交易对与做市策略 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminTradingPairQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminTradingPairQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTradingPairRequest {
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: Option<String>,
    pub(crate) market_type: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateTradingPairRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateTradingPairStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateTradingPairStatusRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateTradingPairRequest {
    pub(crate) logo_url: Option<String>,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateTradingPairRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminTradingPairResponse {
    pub(crate) id: u64,
    pub(crate) base_asset_id: u64,
    pub(crate) quote_asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) base_asset: String,
    pub(crate) quote_asset: String,
    pub(crate) price_precision: i32,
    pub(crate) qty_precision: i32,
    pub(crate) min_order_value: BigDecimal,
    pub(crate) status: String,
    pub(crate) market_type: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminTradingPairResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminTradingPairsResponse {
    pub(crate) pairs: Vec<AdminTradingPairResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminTradingPairsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarketStrategyQuery {
    pub(crate) pair_id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminMarketStrategyQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMarketStrategyRequest {
    pub(crate) pair_id: u64,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    #[serde(default)]
    pub(crate) nodes: Vec<MarketStrategyNodeRequest>,
    pub(crate) status: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateMarketStrategyRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarketStrategyRequest {
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    #[serde(default)]
    pub(crate) nodes: Vec<MarketStrategyNodeRequest>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateMarketStrategyRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMarketStrategyStatusRequest {
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateMarketStrategyStatusRequest {}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct AdminMarketStrategyResponse {
    pub(crate) id: u64,
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) market_type: String,
    pub(crate) strategy_type: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) target_price: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) start_time: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) end_time: DateTime<Utc>,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: BigDecimal,
    pub(crate) volume_max: BigDecimal,
    pub(crate) status: String,
    pub(crate) run_status: Option<String>,
    pub(crate) active_version: Option<i32>,
    pub(crate) current_price: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_generated_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_kline_open_time: Option<DateTime<Utc>>,
    pub(crate) recovery_status: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminMarketStrategyResponse {}

/// 策略详情将旧的主表/运行快照与独立关系表节点组装返回；列表行继续使用旧 DTO，
/// 避免在 SQLx `FromRow` 中伪造不存在的集合列，也保持无节点旧数据为空数组。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AdminMarketStrategyDetailResponse {
    #[serde(flatten)]
    pub(crate) strategy: AdminMarketStrategyResponse,
    pub(crate) nodes: Vec<AdminMarketStrategyNodeResponse>,
}

impl PresentationLayer for AdminMarketStrategyDetailResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminMarketStrategiesResponse {
    pub(crate) strategies: Vec<AdminMarketStrategyResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminMarketStrategiesResponse {}

/// 管理员写入的单个策略目标节点；数组顺序即持久化顺序，时间、目标类型、
/// 执行模式与数值不变量由 service 统一校验，并依起始价及前一节点解析最终正价格。
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub(crate) struct MarketStrategyNodeRequest {
    #[serde(with = "unix_millis")]
    pub(crate) target_time: DateTime<Utc>,
    pub(crate) target_type: String,
    pub(crate) target_value: BigDecimal,
    pub(crate) execution_mode: String,
    pub(crate) tolerance: BigDecimal,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: Option<BigDecimal>,
    pub(crate) volume_max: Option<BigDecimal>,
}

impl PresentationLayer for MarketStrategyNodeRequest {}

/// 后台策略读模型中的已持久化节点；`sequence_no` 为稳定排序依据，
/// 其余字段保留版本快照所使用的原始精度。
#[derive(Debug, Clone, Serialize, sqlx::FromRow, PartialEq)]
pub(crate) struct AdminMarketStrategyNodeResponse {
    pub(crate) id: u64,
    pub(crate) sequence_no: u32,
    #[serde(with = "unix_millis")]
    pub(crate) target_time: DateTime<Utc>,
    pub(crate) target_type: String,
    pub(crate) target_value: BigDecimal,
    pub(crate) execution_mode: String,
    pub(crate) tolerance: BigDecimal,
    pub(crate) volatility: BigDecimal,
    pub(crate) volume_min: Option<BigDecimal>,
    pub(crate) volume_max: Option<BigDecimal>,
}

impl PresentationLayer for AdminMarketStrategyNodeResponse {}

/// 缺口检测的可选 UTC 范围；边界缺省时由应用层收敛到策略有效时段与已闭合分钟。
#[derive(Debug, Deserialize)]
pub(crate) struct MarketStrategyGapQuery {
    #[serde(default, with = "option_unix_millis")]
    pub(crate) range_start: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) range_end: Option<DateTime<Utc>>,
}

impl PresentationLayer for MarketStrategyGapQuery {}

/// 一段连续缺失的权威 1m K 线范围，采用 `[range_start, range_end)` 半开区间；
/// 两端均为 UTC 分钟边界，`one_minute_count` 与区间分钟差保持一致。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct MarketStrategyGapRangeResponse {
    #[serde(with = "unix_millis")]
    pub(crate) range_start: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) one_minute_count: u32,
}

impl PresentationLayer for MarketStrategyGapRangeResponse {}

/// 缺口检测汇总；仅描述 Mongo 已闭合 1m 数据缺失情况，不生成 K 线、
/// 不签发预览令牌，也不改变运行检查点。
#[derive(Debug, Serialize)]
pub(crate) struct MarketStrategyGapsResponse {
    pub(crate) strategy_id: u64,
    pub(crate) config_version: i32,
    pub(crate) gaps: Vec<MarketStrategyGapRangeResponse>,
    pub(crate) total_1m_count: u32,
}

impl PresentationLayer for MarketStrategyGapsResponse {}

/// 预览指定缺口范围的请求；必须显式提供 `[range_start, range_end)` UTC 分钟范围。
#[derive(Debug, Deserialize)]
pub(crate) struct PreviewMarketStrategyRecoveryRequest {
    #[serde(with = "unix_millis")]
    pub(crate) range_start: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) range_end: DateTime<Utc>,
}

impl PresentationLayer for PreviewMarketStrategyRecoveryRequest {}

/// 预览或补偿返回的有限 OHLCV 样本；价格与成交量保持确定性生成器精度。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct MarketStrategyRecoverySampleResponse {
    #[serde(with = "unix_millis")]
    pub(crate) open_time: DateTime<Utc>,
    pub(crate) open: BigDecimal,
    pub(crate) high: BigDecimal,
    pub(crate) low: BigDecimal,
    pub(crate) close: BigDecimal,
    pub(crate) volume: BigDecimal,
}

impl PresentationLayer for MarketStrategyRecoverySampleResponse {}

/// 手动补偿预览快照；令牌绑定策略版本、范围和内容摘要，只能作为后续执行确认，
/// 此响应本身不写 Mongo、Redis、策略检查点或任务表。
#[derive(Debug, Serialize)]
pub(crate) struct MarketStrategyRecoveryPreviewResponse {
    pub(crate) strategy_id: u64,
    pub(crate) config_version: i32,
    #[serde(with = "unix_millis")]
    pub(crate) range_start: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) one_minute_count: u32,
    pub(crate) aggregate_intervals: Vec<String>,
    pub(crate) first_price: BigDecimal,
    pub(crate) last_price: BigDecimal,
    pub(crate) samples: Vec<MarketStrategyRecoverySampleResponse>,
    pub(crate) preview_token: String,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
}

impl PresentationLayer for MarketStrategyRecoveryPreviewResponse {}

/// 执行手动补偿时携带的一次性预览令牌与必填审计原因；应用层会再次校验版本和缺口摘要。
#[derive(Debug, Deserialize)]
pub(crate) struct ExecuteMarketStrategyRecoveryRequest {
    pub(crate) preview_token: String,
    pub(crate) reason: String,
}

impl PresentationLayer for ExecuteMarketStrategyRecoveryRequest {}

/// 补偿任务列表的可选状态和分页条件；策略 ID 由路径提供，避免查询跨策略数据。
#[derive(Debug, Deserialize)]
pub(crate) struct MarketStrategyRecoveryJobsQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for MarketStrategyRecoveryJobsQuery {}

/// 手动 K 线补偿任务的可审计读模型；不返回预览令牌或其哈希，避免已消费确认凭证泄露。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct MarketStrategyRecoveryJobResponse {
    pub(crate) id: u64,
    pub(crate) strategy_id: u64,
    pub(crate) requested_by: u64,
    pub(crate) config_version: i32,
    #[serde(with = "unix_millis")]
    pub(crate) range_start: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) reason: String,
    pub(crate) status: String,
    pub(crate) expected_1m_count: u32,
    pub(crate) actual_1m_count: u32,
    pub(crate) actual_aggregate_count: u32,
    pub(crate) error_message: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) started_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) completed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for MarketStrategyRecoveryJobResponse {}

/// 单策略补偿任务分页响应；`total` 使用与数据行相同的策略和状态谓词。
#[derive(Debug, Serialize)]
pub(crate) struct MarketStrategyRecoveryJobsResponse {
    pub(crate) jobs: Vec<MarketStrategyRecoveryJobResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for MarketStrategyRecoveryJobsResponse {}
