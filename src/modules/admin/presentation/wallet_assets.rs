//! 后台资产定义、精度和充提费配置 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct AdminAssetQuery {
    pub(crate) symbol: Option<String>,
    pub(crate) asset_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminAssetQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAssetRequest {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) deposit_enabled: Option<bool>,
    pub(crate) withdraw_enabled: Option<bool>,
    pub(crate) margin_transfer_enabled: Option<bool>,
    pub(crate) min_deposit_amount: Option<BigDecimal>,
    pub(crate) deposit_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee_tiers: Option<Vec<WithdrawFeeTier>>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for CreateAssetRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateAssetRequest {
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: Option<bool>,
    pub(crate) withdraw_enabled: Option<bool>,
    pub(crate) margin_transfer_enabled: Option<bool>,
    pub(crate) min_deposit_amount: Option<BigDecimal>,
    pub(crate) deposit_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee: Option<BigDecimal>,
    pub(crate) withdraw_fee_tiers: Option<Vec<WithdrawFeeTier>>,
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for UpdateAssetRequest {}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteAssetRequest {
    pub(crate) reason: Option<String>,
}

impl PresentationLayer for DeleteAssetRequest {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminAssetResponse {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) logo_url: Option<String>,
    pub(crate) precision_scale: i32,
    pub(crate) asset_type: String,
    pub(crate) status: String,
    pub(crate) deposit_enabled: bool,
    pub(crate) withdraw_enabled: bool,
    pub(crate) margin_transfer_enabled: bool,
    pub(crate) min_deposit_amount: BigDecimal,
    pub(crate) deposit_fee: BigDecimal,
    pub(crate) withdraw_fee: BigDecimal,
    pub(crate) withdraw_fee_tiers: SqlxJson<Vec<WithdrawFeeTier>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminAssetResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminAssetsResponse {
    pub(crate) assets: Vec<AdminAssetResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminAssetsResponse {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminWalletAccountQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) include_empty: Option<bool>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminWalletAccountQuery {}

#[derive(Debug, Deserialize)]
pub(crate) struct AdminWalletLedgerQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) asset_id: Option<u64>,
    pub(crate) change_type: Option<String>,
    pub(crate) ref_type: Option<String>,
    pub(crate) include_internal: Option<bool>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl PresentationLayer for AdminWalletLedgerQuery {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminWalletAccountResponse {
    pub(crate) id: Option<u64>,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked: BigDecimal,
    pub(crate) account_exists: bool,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

impl PresentationLayer for AdminWalletAccountResponse {}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub(crate) struct AdminWalletLedgerResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) user_email: String,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) change_type: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) amount: BigDecimal,
    pub(crate) balance_type: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) balance_after: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) available_after: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) frozen_after: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) locked_after: BigDecimal,
    pub(crate) ref_type: String,
    pub(crate) ref_id: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
}

impl PresentationLayer for AdminWalletLedgerResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminWalletAccountsResponse {
    pub(crate) accounts: Vec<AdminWalletAccountResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminWalletAccountsResponse {}

#[derive(Debug, Serialize)]
pub(crate) struct AdminWalletLedgerResponseList {
    pub(crate) ledger: Vec<AdminWalletLedgerResponse>,
    pub(crate) total: i64,
}

impl PresentationLayer for AdminWalletLedgerResponseList {}
