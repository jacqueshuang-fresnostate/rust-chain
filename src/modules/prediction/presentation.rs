//! prediction 模块表现层。
//!
//! 负责 HTTP 请求/响应 DTO 与查询模型。
//! 本文件定义预测模块的查询参数、请求体、响应体，以及从仓储行模型到响应体的转换。
//! 转换基本是逐字段平移，唯一的例外是设置响应会把两个 JSON 列解析成数组，
//! 让前端拿到的是标签与资产编号的真实列表而不是原始 JSON 文本。
//! 时间字段统一经 `unix_millis` 与 `option_unix_millis` 序列化为毫秒时间戳，
//! 空值保持为 null，因此「未结束」「未结算」与「纪元起始」不会被混淆。
//! 金额与概率以 `BigDecimal` 承载并按字符串形式输出，不转浮点也不截断小数位。
//! 用户端与管理端共用同一批响应结构，管理端专有的覆盖配置字段对用户端同样可见，
//! 可见性差异由 application 层的查询条件控制，而不是靠这里裁剪字段。

use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

use super::{
    repository::{
        PredictionAssetConfigRow, PredictionMarketRow, PredictionOrderRow, PredictionSettingsRow,
        PredictionStakeAssetRow, PredictionSyncLogRow,
    },
    service,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminListQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminMarketQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) display_status: Option<String>,
    pub(crate) settlement_status: Option<String>,
    pub(crate) keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OrdersQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) status: Option<String>,
    pub(crate) market_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminOrdersQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) status: Option<String>,
    pub(crate) market_id: Option<u64>,
    pub(crate) email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SavePredictionSettingsRequest {
    pub(crate) sync_enabled: bool,
    pub(crate) sync_interval_seconds: u32,
    pub(crate) sync_tags: Vec<String>,
    pub(crate) allowed_asset_ids: Vec<u64>,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) default_settlement_mode: String,
    pub(crate) default_invalid_refund_policy: String,
    pub(crate) quote_ttl_seconds: u32,
    pub(crate) revision: u64,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertPredictionAssetConfigRequest {
    pub(crate) asset_id: u64,
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
    pub(crate) revision: u64,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdatePredictionAssetConfigRequest {
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
    pub(crate) revision: u64,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdatePredictionMarketRequest {
    pub(crate) display_status: String,
    pub(crate) settlement_mode_override: Option<String>,
    pub(crate) allowed_asset_ids_override: Option<Vec<u64>>,
    pub(crate) payout_cap_overrides: Option<Value>,
    pub(crate) fee_rate_override: Option<BigDecimal>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePredictionQuoteRequest {
    pub(crate) market_id: u64,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) stake_amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePredictionOrderRequest {
    pub(crate) quote_id: String,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettlePredictionMarketRequest {
    pub(crate) result: String,
    pub(crate) invalid_refund_policy: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSettingsResponse {
    pub(crate) sync_enabled: bool,
    pub(crate) sync_interval_seconds: u32,
    pub(crate) sync_tags: Vec<String>,
    pub(crate) allowed_asset_ids: Vec<u64>,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) default_settlement_mode: String,
    pub(crate) default_invalid_refund_policy: String,
    pub(crate) quote_ttl_seconds: u32,
    pub(crate) revision: u64,
    pub(crate) last_sync_status: Option<String>,
    pub(crate) last_sync_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_sync_started_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_sync_finished_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_successful_sync_at: Option<DateTime<Utc>>,
    pub(crate) last_sync_imported_count: u32,
    pub(crate) last_sync_updated_count: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionStakeAssetResponse {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) max_payout_amount: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionUserConfigResponse {
    pub(crate) allowed_assets: Vec<PredictionStakeAssetResponse>,
    pub(crate) default_fee_rate: BigDecimal,
    pub(crate) quote_ttl_seconds: u32,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionMarketResponse {
    pub(crate) id: u64,
    pub(crate) source: String,
    pub(crate) external_event_id: Option<String>,
    pub(crate) external_market_id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) tags_json: SqlxJson<Value>,
    pub(crate) outcome_yes_label: String,
    pub(crate) outcome_no_label: String,
    pub(crate) yes_price: BigDecimal,
    pub(crate) no_price: BigDecimal,
    pub(crate) volume: Option<BigDecimal>,
    pub(crate) liquidity: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) end_at: Option<DateTime<Utc>>,
    pub(crate) source_status: String,
    pub(crate) display_status: String,
    pub(crate) external_resolution: Option<String>,
    pub(crate) local_resolution: Option<String>,
    pub(crate) settlement_status: String,
    pub(crate) settlement_mode_override: Option<String>,
    pub(crate) allowed_asset_ids_override_json: Option<SqlxJson<Value>>,
    pub(crate) payout_cap_overrides_json: Option<SqlxJson<Value>>,
    pub(crate) fee_rate_override: Option<BigDecimal>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) last_synced_at: Option<DateTime<Utc>>,
    /// 每次上游刷新或本地关盘都会递增，quote 消费必须与创建时版本一致。
    pub(crate) market_version: u64,
    /// 本地 DB 时间关盘时刻；未到期或由上游先关闭时可为空。
    #[serde(default, with = "option_unix_millis")]
    pub(crate) locally_closed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionMarketsResponse {
    pub(crate) markets: Vec<PredictionMarketResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminPredictionMarketsResponse {
    pub(crate) markets: Vec<PredictionMarketResponse>,
    pub(crate) total: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionAssetConfigsResponse {
    pub(crate) configs: Vec<PredictionAssetConfigResponse>,
    pub(crate) total: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionAssetConfigResponse {
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) enabled: bool,
    pub(crate) max_payout_amount: BigDecimal,
    pub(crate) revision: u64,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionQuoteResponse {
    pub(crate) quote_id: String,
    pub(crate) market_id: u64,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) accepted_price: BigDecimal,
    pub(crate) shares: BigDecimal,
    pub(crate) theoretical_payout: BigDecimal,
    pub(crate) effective_payout_cap: BigDecimal,
    pub(crate) market_version: u64,
    #[serde(with = "unix_millis")]
    pub(crate) market_last_synced_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionOrderResponse {
    pub(crate) id: u64,
    pub(crate) order_no: Option<String>,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) market_id: u64,
    pub(crate) market_title: String,
    pub(crate) outcome: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) stake_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
    pub(crate) accepted_price: BigDecimal,
    pub(crate) shares: BigDecimal,
    pub(crate) theoretical_payout: BigDecimal,
    pub(crate) effective_payout_cap: BigDecimal,
    pub(crate) status: String,
    pub(crate) result: Option<String>,
    pub(crate) payout_amount: BigDecimal,
    pub(crate) refund_amount: BigDecimal,
    pub(crate) fee_refund_amount: BigDecimal,
    pub(crate) invalid_refund_policy_used: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) settled_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionOrdersResponse {
    pub(crate) orders: Vec<PredictionOrderResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminPredictionOrdersResponse {
    pub(crate) orders: Vec<PredictionOrderResponse>,
    pub(crate) total: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionOrderActionResponse {
    pub(crate) order: PredictionOrderResponse,
    pub(crate) changed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSettlementResponse {
    pub(crate) market: PredictionMarketResponse,
    pub(crate) settled_orders: u32,
    pub(crate) changed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSyncResponse {
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
    pub(crate) status: String,
    pub(crate) error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct PredictionSyncLogResponse {
    pub(crate) id: u64,
    pub(crate) trigger_type: String,
    pub(crate) status: String,
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
    pub(crate) error_message: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) started_at: DateTime<Utc>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PredictionSyncLogsResponse {
    pub(crate) logs: Vec<PredictionSyncLogResponse>,
    pub(crate) total: i64,
}

impl From<PredictionSettingsRow> for PredictionSettingsResponse {
    /// 把设置单例行转成后台响应，是本文件唯一做解析而非纯平移的转换。
    /// 同步标签与允许资产两列在库中是 JSON，这里解析成字符串数组与数字数组再输出，
    /// 解析失败或结构不符时退化为空数组，不会让整个设置接口报错。
    /// 需要注意空数组在两处含义不同：标签为空表示不按标签过滤即全量拉取，
    /// 而允许资产为空表示任何资产都不可下注，前端不应把两者作同样处理。
    /// 其余字段包括费率、结算模式、退款策略、报价有效期和最近一次同步的状态与计数均原样透传。
    fn from(row: PredictionSettingsRow) -> Self {
        Self {
            sync_enabled: row.sync_enabled,
            sync_interval_seconds: row.sync_interval_seconds,
            sync_tags: service::json_string_array(&row.sync_tags_json),
            allowed_asset_ids: service::json_u64_array(&row.allowed_asset_ids_json),
            default_fee_rate: row.default_fee_rate,
            default_settlement_mode: row.default_settlement_mode,
            default_invalid_refund_policy: row.default_invalid_refund_policy,
            quote_ttl_seconds: row.quote_ttl_seconds,
            revision: row.revision,
            last_sync_status: row.last_sync_status,
            last_sync_error: row.last_sync_error,
            last_sync_started_at: row.last_sync_started_at,
            last_sync_finished_at: row.last_sync_finished_at,
            last_successful_sync_at: row.last_successful_sync_at,
            last_sync_imported_count: row.last_sync_imported_count,
            last_sync_updated_count: row.last_sync_updated_count,
        }
    }
}

impl From<PredictionStakeAssetRow> for PredictionStakeAssetResponse {
    /// 平移可下注资产条目，输出资产编号、符号与该资产的赔付上限三项。
    /// 上限为零表示不设封顶而非禁止赔付，前端不应据零值提示用户无法下注。
    /// 不含启用标记，因为能出现在这份清单里本身就意味着已启用。
    fn from(row: PredictionStakeAssetRow) -> Self {
        Self {
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            max_payout_amount: row.max_payout_amount,
        }
    }
}

impl From<PredictionMarketRow> for PredictionMarketResponse {
    /// 平移市场行到响应体，三个 JSON 列以原始结构重新包装后输出而不解析成具体类型。
    /// 标签恒为非空 JSON，两项覆盖配置则保留其可空性：为空表示该市场未设覆盖，走全局默认。
    /// 上游结果与本地结果分列两个字段，前者只是同步来的参考值，
    /// 只有后者才是真正决定派奖的权威口径，前端展示结论时应以本地结果为准。
    /// 上游状态与展示状态同理并存，前者反映 Polymarket 侧是否关闭，后者才决定用户能否看到。
    /// 成交量与流动性可空，为空表示上游未提供而非确为零，转换不把它们折成零。
    fn from(row: PredictionMarketRow) -> Self {
        Self {
            id: row.id,
            source: row.source,
            external_event_id: row.external_event_id,
            external_market_id: row.external_market_id,
            slug: row.slug,
            title: row.title,
            description: row.description,
            image_url: row.image_url,
            category: row.category,
            tags_json: SqlxJson(row.tags_json),
            outcome_yes_label: row.outcome_yes_label,
            outcome_no_label: row.outcome_no_label,
            yes_price: row.yes_price,
            no_price: row.no_price,
            volume: row.volume,
            liquidity: row.liquidity,
            end_at: row.end_at,
            source_status: row.source_status,
            display_status: row.display_status,
            external_resolution: row.external_resolution,
            local_resolution: row.local_resolution,
            settlement_status: row.settlement_status,
            settlement_mode_override: row.settlement_mode_override,
            allowed_asset_ids_override_json: row.allowed_asset_ids_override_json.map(SqlxJson),
            payout_cap_overrides_json: row.payout_cap_overrides_json.map(SqlxJson),
            fee_rate_override: row.fee_rate_override,
            last_synced_at: row.last_synced_at,
            market_version: row.market_version,
            locally_closed_at: row.locally_closed_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<PredictionAssetConfigRow> for PredictionAssetConfigResponse {
    /// 平移后台资产配置条目，含启用标记、赔付上限与两个时间戳。
    /// 由于列表查询以资产表左连配置表，尚未配置过的资产也会走到这里，
    /// 其启用标记为假、上限为零，时间戳回退为资产自身的创建时间而非配置创建时间。
    /// 因此不能凭时间戳判断该资产是否被配置过，只能依据启用标记与上限是否被显式设置过。
    fn from(row: PredictionAssetConfigRow) -> Self {
        Self {
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            enabled: row.enabled,
            max_payout_amount: row.max_payout_amount,
            revision: row.revision,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<PredictionOrderRow> for PredictionOrderResponse {
    /// 平移订单行到响应体，同时带出用户邮箱、市场标题与资产符号三个连表展示字段。
    /// 下单侧字段全部是建单时固化的快照：本金、手续费、接受价格、份额、理论赔付与赔付上限，
    /// 它们不随行情或后台改配置而变化，可直接用于对账。
    /// 结算侧字段在订单终结前均为空，包括结果、派奖额、退款额、手续费退款额、
    /// 实际使用的无效退款策略与结算时间，因此判断是否已结算应看状态而非金额是否为零。
    /// 理论赔付与派奖额可能不等，差额来自赔付上限封顶；两者并存正是为了让这一截断可被审计。
    fn from(row: PredictionOrderRow) -> Self {
        Self {
            id: row.id,
            order_no: row.order_no,
            user_id: row.user_id,
            user_email: row.user_email,
            market_id: row.market_id,
            market_title: row.market_title,
            outcome: row.outcome,
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            stake_amount: row.stake_amount,
            fee_amount: row.fee_amount,
            accepted_price: row.accepted_price,
            shares: row.shares,
            theoretical_payout: row.theoretical_payout,
            effective_payout_cap: row.effective_payout_cap,
            status: row.status,
            result: row.result,
            payout_amount: row.payout_amount,
            refund_amount: row.refund_amount,
            fee_refund_amount: row.fee_refund_amount,
            invalid_refund_policy_used: row.invalid_refund_policy_used,
            settled_at: row.settled_at,
            created_at: row.created_at,
        }
    }
}

impl From<PredictionSyncLogRow> for PredictionSyncLogResponse {
    /// 平移同步日志条目，输出触发来源、状态、导入与更新计数、错误文本及起止时间。
    /// 触发来源区分定时轮询与后台手动触发；导入计数对应本轮新建的市场，更新计数对应命中既有市场。
    /// 结束时间为空且状态仍是 running，既可能表示同步正在进行，
    /// 也可能表示进程在同步途中退出而未能回填，转换不区分这两种情况。
    /// 错误文本在写入时已压缩为单行并截断，此处原样输出不再处理。
    fn from(row: PredictionSyncLogRow) -> Self {
        Self {
            id: row.id,
            trigger_type: row.trigger_type,
            status: row.status,
            imported_count: row.imported_count,
            updated_count: row.updated_count,
            error_message: row.error_message,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}
