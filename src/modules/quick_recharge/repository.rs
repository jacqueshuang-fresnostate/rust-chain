//! quick_recharge bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::architecture::RepositoryLayer;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeUserOrderFilter {
    pub(crate) user_id: u64,
    pub(crate) status: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeAdminOrderFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) order_id: Option<String>,
    pub(crate) provider_trade_id: Option<String>,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderCreateWrite {
    pub(crate) order_id: String,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) fiat_amount: BigDecimal,
    pub(crate) return_target: Option<String>,
    pub(crate) redirect_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderProviderUpdate {
    pub(crate) order_id: String,
    pub(crate) provider_trade_id: String,
    pub(crate) actual_amount: BigDecimal,
    pub(crate) receive_address: String,
    pub(crate) payment_url: String,
    pub(crate) expiration_time: Option<i64>,
    pub(crate) currency: String,
    pub(crate) token: String,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderPaidUpdate {
    pub(crate) order_id: String,
    pub(crate) provider_trade_id: String,
    pub(crate) actual_amount: BigDecimal,
    pub(crate) receive_address: Option<String>,
    pub(crate) block_transaction_id: Option<String>,
    pub(crate) callback_payload_json: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeConfigWrite {
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    pub(crate) merchant_secret_ciphertext: Option<String>,
    pub(crate) merchant_secret_mask: Option<String>,
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
    pub(crate) updated_by: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeConfigRow {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) provider: String,
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    pub(crate) merchant_secret_ciphertext: Option<String>,
    pub(crate) merchant_secret_mask: Option<String>,
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
    pub(crate) updated_by: Option<u64>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderRow {
    pub(crate) id: u64,
    pub(crate) order_id: String,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) fiat_amount: BigDecimal,
    pub(crate) actual_amount: Option<BigDecimal>,
    pub(crate) provider_trade_id: Option<String>,
    pub(crate) receive_address: Option<String>,
    pub(crate) payment_url: Option<String>,
    pub(crate) return_target: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) expiration_time: Option<i64>,
    pub(crate) status: String,
    pub(crate) block_transaction_id: Option<String>,
    pub(crate) paid_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeAssetRow {
    pub(crate) id: u64,
    pub(crate) symbol: String,
}

#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeWalletRow {
    pub(crate) available: BigDecimal,
    pub(crate) frozen: BigDecimal,
    pub(crate) locked: BigDecimal,
}
