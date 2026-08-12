//! admin bounded context presentation layer.
//!
//! 表现层兼容 façade：按后台子域组织请求/响应 DTO，并稳定重导出现有符号路径。

mod agents;
mod convert;
mod countries;
mod dashboard_audit;
mod deposit_networks;
mod market;
mod market_feed;
mod new_coin;
mod news;
mod risk_security;
mod system_config;
mod users;
mod wallet_assets;

pub(crate) use self::{
    agents::*, convert::*, countries::*, dashboard_audit::*, deposit_networks::*, market::*,
    new_coin::*, news::*, risk_security::*, users::*, wallet_assets::*,
};
pub use self::{market_feed::*, system_config::*};

use crate::{
    architecture::PresentationLayer,
    error::{AppError, AppResult},
    infra::email::VerificationCodeTemplate,
    modules::{
        security::{LoginTwoFactorMode, PaymentPolicies, ThirdPartyBindingPolicy},
        wallet::WithdrawFeeTier,
    },
    time::{option_unix_millis, unix_millis},
    workers::market_feed::MarketFeedRuntimeStatus,
};
use axum::extract::Multipart;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sqlx::types::Json as SqlxJson;

/// 区分「字段缺省」与「显式 null」：缺省 → None（保持原值），null → Some(None)（清空）。
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_presentation_tests.rs"]
mod tests;
