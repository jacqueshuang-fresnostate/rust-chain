//! 后台行情源配置、凭证与运行状态 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub struct SaveMarketFeedConfigRequest {
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveMarketFeedConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketFeedConfigResponse {
    pub id: u64,
    pub name: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub version: u64,
    pub applied_version: Option<u64>,
    pub needs_reload: bool,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub last_reloaded_at: Option<DateTime<Utc>>,
}

impl PresentationLayer for MarketFeedConfigResponse {}

#[derive(Debug, Serialize)]
pub struct MarketFeedStatusResponse {
    pub saved_config: Option<MarketFeedConfigResponse>,
    pub runtime: MarketFeedRuntimeStatus,
    /// 数据库权威摄取记录与策略检查点组成的健康快照。
    /// 该字段随状态接口一起返回，避免前端只看到 supervisor 已启动却不知道实际行情是否已经断流。
    pub health: MarketFeedHealthResponse,
}

impl PresentationLayer for MarketFeedStatusResponse {}

/// 行情订阅的可观测健康快照。
///
/// `healthy` 只在所有已配置交易对最近有摄取、运行时就绪且没有待补 K 线/失败恢复任务时为 true；
/// `stale_symbols` 明确列出缺少或超过阈值未摄取的交易对，便于后台直接定位故障而不需要猜测。
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketFeedHealthResponse {
    /// `healthy`、`degraded`、`stale` 或 `not_configured`。
    pub status: String,
    pub healthy: bool,
    pub stale_after_seconds: u64,
    pub configured_symbols: u32,
    pub healthy_symbols: u32,
    pub stale_symbols: Vec<String>,
    /// 上游行情事件本身最近发生的时间；断流判定以此字段为准。
    #[serde(default, with = "option_unix_millis")]
    pub last_observed_at: Option<DateTime<Utc>>,
    /// 本服务最近把行情写入数据库的时间，仅用于区分上游断流和本地摄取阻塞。
    #[serde(default, with = "option_unix_millis")]
    pub last_ingested_at: Option<DateTime<Utc>>,
    /// 尚未闭合的策略 K 线缺口数量（按策略计数，而非按蜡烛根数计数）。
    pub kline_gap_count: i64,
    /// 策略运行状态或手动补偿任务中处于 failed 的数量。
    pub kline_recovery_failed_count: i64,
    #[serde(with = "unix_millis")]
    pub checked_at: DateTime<Utc>,
}

impl PresentationLayer for MarketFeedHealthResponse {}

#[derive(Debug, Deserialize)]
pub struct ReloadMarketFeedRequest {
    pub reason: String,
}

impl PresentationLayer for ReloadMarketFeedRequest {}

#[derive(Debug, Serialize)]
pub struct ReloadMarketFeedResponse {
    pub config: MarketFeedConfigResponse,
    pub runtime: MarketFeedRuntimeStatus,
}

impl PresentationLayer for ReloadMarketFeedResponse {}

#[derive(Debug, Deserialize)]
pub struct UpsertMarketSourceCredentialRequest {
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub enabled: bool,
    pub reason: String,
}

impl PresentationLayer for UpsertMarketSourceCredentialRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketSourceCredentialResponse {
    pub provider: String,
    pub auth_type: String,
    pub api_key_mask: Option<String>,
    pub enabled: bool,
}

impl PresentationLayer for MarketSourceCredentialResponse {}

#[derive(Debug, Serialize)]
pub struct MarketSourceCredentialsResponse {
    pub credentials: Vec<MarketSourceCredentialResponse>,
}

impl PresentationLayer for MarketSourceCredentialsResponse {}

#[derive(Debug, Clone)]
pub struct MarketSourceCredentialSecret {
    pub provider: String,
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
}
