//! quick_recharge bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use super::repository::{QuickRechargeConfigRow, QuickRechargeOrderRow};
use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize)]
pub struct SaveQuickRechargeConfigRequest {
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    pub(crate) merchant_secret: Option<String>,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) notify_url: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) pc_app_redirect_url: Option<String>,
    pub(crate) mac_app_redirect_url: Option<String>,
    pub(crate) ios_app_redirect_url: Option<String>,
    pub(crate) android_app_redirect_url: Option<String>,
    pub(crate) mobile_web_redirect_url: Option<String>,
    pub(crate) desktop_web_redirect_url: Option<String>,
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestQuickRechargeConfigRequest {
    pub(crate) amount: BigDecimal,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteQuickRechargeOrderRequest {
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQuickRechargeOrderRequest {
    pub(crate) amount: BigDecimal,
    pub(crate) return_target: Option<QuickRechargeReturnTarget>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuickRechargeReturnTarget {
    PcApp,
    MacApp,
    IosApp,
    AndroidApp,
    MobileWeb,
    DesktopWeb,
}

impl QuickRechargeReturnTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PcApp => "pc_app",
            Self::MacApp => "mac_app",
            Self::IosApp => "ios_app",
            Self::AndroidApp => "android_app",
            Self::MobileWeb => "mobile_web",
            Self::DesktopWeb => "desktop_web",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct QuickRechargeOrdersQuery {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) order_id: Option<String>,
    pub(crate) provider_trade_id: Option<String>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct QuickRechargeConfigResponse {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) provider: String,
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    pub(crate) merchant_secret_mask: Option<String>,
    pub(crate) merchant_secret_set: bool,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) notify_url: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) pc_app_redirect_url: Option<String>,
    pub(crate) mac_app_redirect_url: Option<String>,
    pub(crate) ios_app_redirect_url: Option<String>,
    pub(crate) android_app_redirect_url: Option<String>,
    pub(crate) mobile_web_redirect_url: Option<String>,
    pub(crate) desktop_web_redirect_url: Option<String>,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) min_amount: BigDecimal,
    #[serde(serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) updated_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserQuickRechargeConfigResponse {
    pub(crate) enabled: bool,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) min_amount: BigDecimal,
    #[serde(serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) max_amount: Option<BigDecimal>,
}

#[derive(Debug, Serialize, Clone)]
pub struct QuickRechargeOrderResponse {
    pub(crate) id: u64,
    pub(crate) order_id: String,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) fiat_amount: BigDecimal,
    #[serde(serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) actual_amount: Option<BigDecimal>,
    pub(crate) provider_trade_id: Option<String>,
    pub(crate) receive_address: Option<String>,
    pub(crate) payment_url: Option<String>,
    pub(crate) return_target: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) expiration_time: Option<i64>,
    pub(crate) status: String,
    pub(crate) block_transaction_id: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) paid_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TestQuickRechargeConfigResponse {
    pub(crate) order_id: String,
    pub(crate) provider_trade_id: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) fiat_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) actual_amount: BigDecimal,
    pub(crate) receive_address: String,
    pub(crate) payment_url: String,
    pub(crate) expiration_time: Option<i64>,
    pub(crate) tested_at: i64,
}

#[derive(Debug, Serialize)]
pub struct QuickRechargeOrdersResponse {
    pub(crate) orders: Vec<QuickRechargeOrderResponse>,
}

impl From<QuickRechargeConfigRow> for QuickRechargeConfigResponse {
    fn from(row: QuickRechargeConfigRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            provider: row.provider,
            enabled: row.enabled,
            api_base_url: row.api_base_url,
            merchant_pid: row.merchant_pid,
            merchant_secret_mask: row.merchant_secret_mask,
            merchant_secret_set: row.merchant_secret_ciphertext.is_some(),
            currency: row.currency,
            token: row.token,
            network: row.network,
            notify_url: row.notify_url,
            redirect_url: row.redirect_url,
            pc_app_redirect_url: row.pc_app_redirect_url,
            mac_app_redirect_url: row.mac_app_redirect_url,
            ios_app_redirect_url: row.ios_app_redirect_url,
            android_app_redirect_url: row.android_app_redirect_url,
            mobile_web_redirect_url: row.mobile_web_redirect_url,
            desktop_web_redirect_url: row.desktop_web_redirect_url,
            min_amount: row.min_amount,
            max_amount: row.max_amount,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<QuickRechargeOrderRow> for QuickRechargeOrderResponse {
    fn from(row: QuickRechargeOrderRow) -> Self {
        Self {
            id: row.id,
            order_id: row.order_id,
            user_id: row.user_id,
            user_email: row.user_email,
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            currency: row.currency,
            token: row.token,
            network: row.network,
            fiat_amount: row.fiat_amount,
            actual_amount: row.actual_amount,
            provider_trade_id: row.provider_trade_id,
            receive_address: row.receive_address,
            payment_url: row.payment_url,
            return_target: row.return_target,
            redirect_url: row.redirect_url,
            expiration_time: row.expiration_time,
            status: row.status,
            block_transaction_id: row.block_transaction_id,
            paid_at: row.paid_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}

fn serialize_optional_decimal_amount<S>(
    amount: &Option<BigDecimal>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match amount {
        Some(amount) => serializer.serialize_some(&format!("{amount:.18}")),
        None => serializer.serialize_none(),
    }
}
