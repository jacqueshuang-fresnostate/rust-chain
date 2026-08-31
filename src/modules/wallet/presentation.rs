//! wallet bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 充值地址、提现申请、链事件、钱包账户、流水分页与已实现收益的入参出参结构集中在本文件定义。
//! 金额字段沿用后端 18 位小数定点口径：收益类响应统一序列化为补零字符串，避免前端浮点解析丢精度。
//! 本层只做结构声明与格式转换，不校验业务规则、不访问数据库，也不参与任何余额或状态变更。

use super::WithdrawFeeTier;
use crate::modules::security::SecurityVerificationMethod;
use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};

/// 把定点金额渲染为固定带小数点的字符串，小数位右侧补零至至少 18 位。
/// 整数值补出完整 18 位零尾；原始小数位超过 18 位时按原样保留，本函数不做截断或四舍五入。
/// 符号沿用 BigDecimal 自身文本，因此负零会输出前导负号，收益计算须在调用前完成零值归一化。
fn decimal_18_string(value: &BigDecimal) -> String {
    let value = value.to_string();
    let (whole, fraction) = value.split_once('.').unwrap_or((&value, ""));
    format!("{whole}.{fraction:0<18}")
}

/// 将必填定点金额以 18 位补零字符串写入 JSON，供收益类响应保持稳定精度契约。
/// 输出始终是字符串而非数字，前端不得按浮点解析；本函数不改变数值本身。
fn serialize_decimal_18<S>(value: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&decimal_18_string(value))
}

/// 将可空定点金额序列化为 18 位补零字符串，缺价场景写出 JSON null。
/// 收益历史用 null 表达该日或该汇总因缺少报价而未知，调用方不得把 null 视作零收益。
fn serialize_optional_decimal_18<S>(
    value: &Option<BigDecimal>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(&decimal_18_string(value)),
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Deserialize)]
pub struct DepositAddressRequest {
    pub asset_symbol: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct DepositAddressResponse {
    pub id: u64,
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    #[serde(with = "unix_millis")]
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWithdrawalQuoteRequest {
    pub asset_symbol: String,
    pub network: String,
    pub amount: BigDecimal,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WithdrawalQuoteResponse {
    pub quote_id: String,
    pub asset_symbol: String,
    pub network: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub net: BigDecimal,
    pub total_reserved: BigDecimal,
    pub fee_config_version: String,
    #[serde(with = "unix_millis")]
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWithdrawalRequest {
    pub quote_id: String,
    pub asset_symbol: String,
    pub network: Option<String>,
    pub address: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub idempotency_key: String,
    pub fund_password: Option<String>,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WithdrawalRequestResponse {
    pub id: u64,
    pub quote_id: String,
    pub status: String,
    pub asset_symbol: String,
    pub network: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub net: BigDecimal,
    pub total_reserved: BigDecimal,
    pub fee_config_version: String,
    #[serde(with = "unix_millis")]
    pub expires_at: DateTime<Utc>,
    pub security_method: SecurityVerificationMethod,
}

#[derive(Debug, Deserialize)]
pub struct WalletWithdrawalQuery {
    pub status: Option<String>,
    pub user_id: Option<u64>,
    pub limit: Option<u32>,
}

/// 后台充提列表查询参数：用户端不暴露 offset，避免公开接口承诺未实现的翻页语义。
#[derive(Debug, Deserialize)]
pub struct AdminWalletListQuery {
    pub status: Option<String>,
    pub user_id: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct WalletWithdrawalsResponse {
    pub withdrawals: Vec<WalletWithdrawalResponse>,
}

#[derive(Debug, Serialize)]
pub struct AdminWalletWithdrawalsResponse {
    pub withdrawals: Vec<WalletWithdrawalResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WalletWithdrawalResponse {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub asset_symbol: String,
    pub network: Option<String>,
    pub address: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub total_reserved: BigDecimal,
    pub status: String,
    pub security_method: String,
    pub idempotency_key: String,
    pub gateway_request_id: String,
    pub withdrawal_quote_id: Option<String>,
    pub tx_hash: Option<String>,
    pub block_height: Option<u64>,
    pub confirmations: u32,
    pub failure_reason: Option<String>,
    pub broadcast_error_class: Option<String>,
    pub broadcast_last_error: Option<String>,
    pub broadcast_resolution: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub acceptance_evidence_at: Option<DateTime<Utc>>,
    pub review_reason: Option<String>,
    pub reviewed_by: Option<u64>,
    pub broadcasted_by: Option<u64>,
    pub confirmed_by: Option<u64>,
    pub failed_by: Option<u64>,
    pub retry_count: u32,
    pub gateway_query_count: u32,
    #[serde(default, with = "option_unix_millis")]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub broadcast_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub confirmed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub failed_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub released_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub last_gateway_query_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub manual_review_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewWithdrawalRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastWithdrawalRequest {
    pub tx_hash: String,
    pub block_height: Option<u64>,
    pub confirmations: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmWithdrawalRequest {
    pub block_height: Option<u64>,
    pub confirmations: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct FailWithdrawalRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ObserveDepositRequest {
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    pub tx_hash: String,
    #[serde(default)]
    pub event_index: u32,
    pub amount: BigDecimal,
    pub block_height: Option<u64>,
    #[serde(default)]
    pub confirmations: u32,
}

#[derive(Debug, Deserialize)]
pub struct ReverseDepositRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct WalletDepositsResponse {
    pub deposits: Vec<WalletDepositEventResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WalletDepositEventResponse {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub asset_symbol: String,
    pub network: String,
    pub address: String,
    pub memo: Option<String>,
    pub tx_hash: String,
    pub event_index: u32,
    pub amount: BigDecimal,
    pub block_height: Option<u64>,
    pub confirmations: u32,
    pub required_confirmations: u32,
    pub status: String,
    pub failure_reason: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub credited_at: Option<DateTime<Utc>>,
    #[serde(default, with = "option_unix_millis")]
    pub reversed_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WalletAccountsResponse {
    pub accounts: Vec<WalletAccountResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TodayReturnStatus {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TodayReturnResponse {
    pub(crate) scope: &'static str,
    pub(crate) reporting_asset: &'static str,
    #[serde(serialize_with = "serialize_decimal_18")]
    pub(crate) amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_18")]
    pub(crate) basis_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_18")]
    pub(crate) rate: BigDecimal,
    #[serde(with = "unix_millis")]
    pub(crate) period_start_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) calculated_at: DateTime<Utc>,
    pub(crate) status: TodayReturnStatus,
    pub(crate) missing_price_assets: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReturnHistoryQuery {
    pub(crate) days: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReturnHistorySummary {
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) amount: Option<BigDecimal>,
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) basis_amount: Option<BigDecimal>,
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) rate: Option<BigDecimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReturnHistoryMissingPrice {
    #[serde(with = "unix_millis")]
    pub(crate) day_start_at: DateTime<Utc>,
    pub(crate) asset_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReturnHistoryPoint {
    #[serde(with = "unix_millis")]
    pub(crate) day_start_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) valued_at: DateTime<Utc>,
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) amount: Option<BigDecimal>,
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) basis_amount: Option<BigDecimal>,
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) rate: Option<BigDecimal>,
    #[serde(serialize_with = "serialize_optional_decimal_18")]
    pub(crate) cumulative_amount: Option<BigDecimal>,
    pub(crate) status: TodayReturnStatus,
    pub(crate) missing_price_assets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReturnHistoryResponse {
    pub(crate) scope: &'static str,
    pub(crate) reporting_asset: &'static str,
    pub(crate) period_days: u16,
    #[serde(with = "unix_millis")]
    pub(crate) period_start_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) calculated_at: DateTime<Utc>,
    pub(crate) status: TodayReturnStatus,
    pub(crate) summary: ReturnHistorySummary,
    pub(crate) missing_prices: Vec<ReturnHistoryMissingPrice>,
    pub(crate) points: Vec<ReturnHistoryPoint>,
}

#[derive(Debug, Serialize)]
pub struct WalletAccountResponse {
    pub user_id: u64,
    pub asset_id: u64,
    pub symbol: String,
    pub logo_url: Option<String>,
    pub available: BigDecimal,
    pub frozen: BigDecimal,
    pub locked: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub struct WalletLedgerQuery {
    pub asset_id: Option<u64>,
    pub asset_symbol: Option<String>,
    pub change_type: Option<String>,
    pub category: Option<String>,
    pub account_type: Option<String>,
    pub ref_type: Option<String>,
    pub ref_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct WalletLedgerResponse {
    pub entries: Vec<WalletLedgerEntryResponse>,
    pub page: WalletLedgerPageResponse,
}

#[derive(Debug, Serialize)]
pub struct WalletLedgerPageResponse {
    pub number: u32,
    pub size: u32,
    pub total_elements: u64,
    pub total_pages: u32,
}

#[derive(Debug, Serialize)]
pub struct WalletLedgerEntryResponse {
    pub id: u64,
    pub account_type: String,
    pub user_id: u64,
    pub asset_id: u64,
    pub symbol: String,
    pub change_type: String,
    pub category: String,
    pub amount: BigDecimal,
    pub balance_type: String,
    pub balance_after: BigDecimal,
    pub available_after: BigDecimal,
    pub frozen_after: BigDecimal,
    pub locked_after: BigDecimal,
    pub fee: BigDecimal,
    pub ref_type: String,
    pub ref_id: String,
    #[serde(with = "unix_millis")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DepositNetworksQuery {
    pub asset_symbol: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DepositAssetsResponse {
    pub assets: Vec<DepositAssetResponse>,
}

#[derive(Debug, Serialize)]
pub struct DepositNetworksResponse {
    pub networks: Vec<DepositNetworkResponse>,
}

#[derive(Debug, Serialize)]
pub struct DepositNetworkResponse {
    pub network: String,
    pub display_name: String,
    pub address_group_code: String,
    pub address_group_name: Option<String>,
    pub asset_symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DepositAssetResponse {
    pub symbol: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub precision_scale: i32,
    pub deposit_enabled: bool,
    pub withdraw_enabled: bool,
    pub min_deposit_amount: BigDecimal,
    pub deposit_fee: BigDecimal,
    pub withdraw_fee: BigDecimal,
    pub withdraw_fee_tiers: Vec<WithdrawFeeTier>,
}
