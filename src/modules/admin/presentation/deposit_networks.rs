//! 后台充值网络配置与地址池管理 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminDepositNetworkConfigQuery {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminDepositNetworkConfigQuery {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositNetworkConfigRequest {
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: Option<String>,
    pub(crate) sort_order: Option<i32>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateDepositNetworkConfigRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateDepositNetworkConfigRequest {
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateDepositNetworkConfigRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDepositNetworkConfigResponse {
    pub(crate) id: u64,
    pub(crate) network: String,
    pub(crate) display_name: String,
    pub(crate) address_group_code: String,
    pub(crate) address_group_name: Option<String>,
    pub(crate) asset_symbols: SqlxJson<Vec<String>>,
    pub(crate) status: String,
    pub(crate) sort_order: i32,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminDepositNetworkConfigResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDepositNetworkConfigResponseList {
    pub(crate) configs: Vec<AdminDepositNetworkConfigResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminDepositNetworkConfigResponseList {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminDepositAddressPoolQuery {
    pub(crate) network: Option<String>,
    pub(crate) address_group_code: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) assigned_user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) address: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminDepositAddressPoolQuery {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositAddressPoolRequest {
    pub(crate) network: String,
    pub(crate) address_group_code: Option<String>,
    pub(crate) address: String,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: Option<String>,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateDepositAddressPoolRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateDepositAddressPoolRequest {
    pub(crate) network: String,
    pub(crate) address_group_code: Option<String>,
    pub(crate) address: String,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateDepositAddressPoolRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReclaimDepositAddressPoolRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for ReclaimDepositAddressPoolRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositAddressPoolBatchRequest {
    pub(crate) network: String,
    pub(crate) address_group_code: Option<String>,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: Option<Vec<String>>,
    pub(crate) status: Option<String>,
    pub(crate) entries: Vec<CreateDepositAddressPoolEntryRequest>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateDepositAddressPoolBatchRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDepositAddressPoolEntryRequest {
    pub(crate) address: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
}

impl PresentationLayer for CreateDepositAddressPoolEntryRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminDepositAddressPoolResponse {
    pub(crate) id: u64,
    pub(crate) network: String,
    pub(crate) address_group_code: String,
    pub(crate) address: String,
    pub(crate) asset_symbol: Option<String>,
    pub(crate) asset_symbols: SqlxJson<Vec<String>>,
    pub(crate) status: String,
    pub(crate) assigned_user_id: Option<u64>,
    pub(crate) assigned_user_email: Option<String>,
    pub(crate) assigned_asset_symbol: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) assigned_at: Option<DateTime<Utc>>,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminDepositAddressPoolResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDepositAddressPoolResponseList {
    pub(crate) addresses: Vec<AdminDepositAddressPoolResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminDepositAddressPoolResponseList {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminDepositAddressPoolBatchResponse {
    pub(crate) addresses: Vec<AdminDepositAddressPoolResponse>,
}

impl PresentationLayer for AdminDepositAddressPoolBatchResponse {}
