//! prediction bounded context service layer.
//!
//! 服务层：封装可复用业务规则、数据格式化与纯计算逻辑。

use crate::{
    architecture::ServiceLayer,
    error::{AppError, AppResult},
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::HashSet, str::FromStr};

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

pub(crate) const STATUS_ACTIVE: &str = "active";
pub(crate) const STATUS_HIDDEN: &str = "hidden";
pub(crate) const SETTLEMENT_OPEN: &str = "open";
pub(crate) const SETTLEMENT_PENDING_CONFIRMATION: &str = "pending_confirmation";
pub(crate) const SETTLEMENT_SETTLED: &str = "settled";
pub(crate) const SETTLEMENT_REFUNDED: &str = "refunded";
pub(crate) const ORDER_STATUS_OPEN: &str = "open";
pub(crate) const OUTCOME_YES: &str = "yes";
pub(crate) const OUTCOME_NO: &str = "no";
pub(crate) const OUTCOME_INVALID: &str = "invalid";
pub(crate) const SETTLEMENT_MODE_MANUAL: &str = "manual_confirm";
pub(crate) const SETTLEMENT_MODE_AUTO: &str = "auto";
pub(crate) const REFUND_STAKE_AND_FEE: &str = "refund_stake_and_fee";
pub(crate) const REFUND_STAKE_ONLY: &str = "refund_stake_only";
pub(crate) const REFUND_MANUAL: &str = "manual";
pub(crate) const DEFAULT_SYNC_POLL_SECONDS: u64 = 30;
pub(crate) const DEFAULT_SYNC_LIMIT: &str = "100";
pub(crate) const POLYMARKET_GAMMA_EVENTS_URL: &str = "https://gamma-api.polymarket.com/events";
pub(crate) const REF_TYPE_PREDICTION_ORDER: &str = "prediction_order";

#[derive(Debug, Default)]
pub(crate) struct SyncCounts {
    pub(crate) imported_count: u32,
    pub(crate) updated_count: u32,
}

#[derive(Debug)]
pub(crate) struct EffectiveMarketConfig {
    pub(crate) allowed_asset_ids: Vec<u64>,
    pub(crate) fee_rate: BigDecimal,
    pub(crate) payout_cap_overrides: Option<Value>,
}

#[derive(Debug)]
pub(crate) struct ParsedPolymarketMarket {
    pub(crate) external_event_id: Option<String>,
    pub(crate) external_market_id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) tags_json: Value,
    pub(crate) outcome_yes_label: String,
    pub(crate) outcome_no_label: String,
    pub(crate) yes_price: BigDecimal,
    pub(crate) no_price: BigDecimal,
    pub(crate) volume: Option<BigDecimal>,
    pub(crate) liquidity: Option<BigDecimal>,
    pub(crate) end_at: Option<DateTime<Utc>>,
    pub(crate) source_status: String,
    pub(crate) external_resolution: Option<String>,
    pub(crate) payload: Value,
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

pub(crate) fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
}

pub(crate) fn required_text(value: String, field: &str, max_len: usize) -> AppResult<String> {
    let normalized = value.trim().to_owned();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if normalized.len() > max_len {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(normalized)
}

pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn ensure_positive_amount(amount: &BigDecimal, field: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(format!("{field} must be positive")));
    }
    Ok(())
}

pub(crate) fn ensure_non_negative_decimal(value: &BigDecimal, field: &str) -> AppResult<()> {
    if value < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{field} must not be negative"
        )));
    }
    Ok(())
}

pub(crate) fn ensure_amount_precision(
    amount: &BigDecimal,
    precision_scale: i32,
    field: &str,
) -> AppResult<()> {
    use crate::modules::wallet::amount_fits_asset_precision;

    if !amount_fits_asset_precision(amount, precision_scale) {
        return Err(AppError::Validation(format!(
            "{field} exceeds asset precision scale {precision_scale}"
        )));
    }
    Ok(())
}

pub(crate) fn ensure_probability_price(price: &BigDecimal) -> AppResult<()> {
    if price <= &BigDecimal::from(0) || price >= &BigDecimal::from(1) {
        return Err(AppError::Validation(
            "prediction probability price must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn normalize_binary_outcome(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(OUTCOME_YES.to_owned()),
        "no" => Ok(OUTCOME_NO.to_owned()),
        _ => Err(AppError::Validation(
            "prediction outcome must be yes or no".to_owned(),
        )),
    }
}

pub(crate) fn normalize_settlement_result(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(OUTCOME_YES.to_owned()),
        "no" => Ok(OUTCOME_NO.to_owned()),
        "invalid" | "cancelled" | "canceled" => Ok(OUTCOME_INVALID.to_owned()),
        _ => Err(AppError::Validation(
            "prediction settlement result must be yes, no, or invalid".to_owned(),
        )),
    }
}

pub(crate) fn normalize_settlement_mode(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        SETTLEMENT_MODE_MANUAL => Ok(SETTLEMENT_MODE_MANUAL.to_owned()),
        SETTLEMENT_MODE_AUTO => Ok(SETTLEMENT_MODE_AUTO.to_owned()),
        _ => Err(AppError::Validation(
            "settlement mode must be manual_confirm or auto".to_owned(),
        )),
    }
}

pub(crate) fn normalize_invalid_refund_policy(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        REFUND_STAKE_AND_FEE => Ok(REFUND_STAKE_AND_FEE.to_owned()),
        REFUND_STAKE_ONLY => Ok(REFUND_STAKE_ONLY.to_owned()),
        REFUND_MANUAL => Ok(REFUND_MANUAL.to_owned()),
        _ => Err(AppError::Validation(
            "invalid refund policy is unsupported".to_owned(),
        )),
    }
}

pub(crate) fn normalize_display_status(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        STATUS_ACTIVE => Ok(STATUS_ACTIVE.to_owned()),
        STATUS_HIDDEN => Ok(STATUS_HIDDEN.to_owned()),
        _ => Err(AppError::Validation(
            "display_status must be active or hidden".to_owned(),
        )),
    }
}

pub(crate) fn normalize_external_resolution(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(OUTCOME_YES.to_owned()),
        "no" => Some(OUTCOME_NO.to_owned()),
        "invalid" | "canceled" | "cancelled" => Some(OUTCOME_INVALID.to_owned()),
        _ => None,
    }
}

pub(crate) fn unique_u64_list(values: Vec<u64>) -> Vec<u64> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| *value > 0 && seen.insert(*value))
        .collect()
}

pub(crate) fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
}

pub(crate) fn json_u64_array(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_u64().or_else(|| item.as_str()?.parse::<u64>().ok()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn json_string_array(value: &Value) -> Vec<String> {
    let parsed = match value {
        Value::String(text) => {
            serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::Array(Vec::new()))
        }
        other => other.clone(),
    };
    parsed
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(text) = item.as_str() {
                        Some(text.to_owned())
                    } else {
                        item.get("label")
                            .or_else(|| item.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn json_decimal_array(value: &Value) -> Vec<BigDecimal> {
    let parsed = match value {
        Value::String(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
        other => other.clone(),
    };
    parsed
        .as_array()
        .map(|items| items.iter().filter_map(decimal_from_json).collect())
        .unwrap_or_default()
}

pub(crate) fn decimal_from_json(value: &Value) -> Option<BigDecimal> {
    match value {
        Value::Number(number) => BigDecimal::from_str(&number.to_string()).ok(),
        Value::String(text) => BigDecimal::from_str(text.trim()).ok(),
        _ => None,
    }
}

pub(crate) fn first_jsonish_value(value: &Value, keys: &[&str]) -> Option<Value> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str()
            && let Ok(parsed) = serde_json::from_str::<Value>(text)
        {
            return Some(parsed);
        }
        return Some(candidate.clone());
    }
    None
}

pub(crate) fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(candidate) = value.get(*key) else {
            continue;
        };
        if let Some(text) = candidate.as_str() {
            return Some(text.to_owned());
        }
        if candidate.is_number() || candidate.is_boolean() {
            return Some(candidate.to_string());
        }
    }
    None
}

pub(crate) fn first_decimal(value: &Value, keys: &[&str]) -> Option<BigDecimal> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(decimal_from_json))
}

pub(crate) fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn extract_market_values(payload: Value) -> Vec<Value> {
    if let Some(items) = payload.as_array() {
        return items
            .iter()
            .flat_map(extract_market_values_from_item)
            .collect();
    }
    if let Some(markets) = payload.get("markets").and_then(Value::as_array) {
        return markets
            .iter()
            .map(|market| merge_event_context(&payload, market))
            .collect();
    }
    for key in ["events", "data", "items"] {
        if let Some(items) = payload.get(key).and_then(Value::as_array) {
            return items
                .iter()
                .flat_map(extract_market_values_from_item)
                .collect();
        }
    }
    Vec::new()
}

pub(crate) fn extract_market_values_from_item(item: &Value) -> Vec<Value> {
    if let Some(markets) = item.get("markets").and_then(Value::as_array) {
        return markets
            .iter()
            .map(|market| merge_event_context(item, market))
            .collect();
    }
    vec![item.clone()]
}

pub(crate) fn merge_event_context(event: &Value, market: &Value) -> Value {
    let mut merged = market.clone();
    let Some(object) = merged.as_object_mut() else {
        return merged;
    };
    for (target, keys) in [
        ("eventId", &["id", "eventId", "event_id"][..]),
        ("eventSlug", &["slug", "eventSlug", "event_slug"][..]),
        ("category", &["category", "categorySlug"][..]),
        ("image", &["image", "icon", "imageUrl"][..]),
    ] {
        if object.get(target).is_none()
            && let Some(value) = keys.iter().find_map(|key| event.get(*key)).cloned()
        {
            object.insert(target.to_owned(), value);
        }
    }
    if object.get("tags").is_none()
        && let Some(tags) = event.get("tags").cloned()
    {
        object.insert("tags".to_owned(), tags);
    }
    merged
}

pub(crate) fn parse_polymarket_market(value: &Value) -> AppResult<ParsedPolymarketMarket> {
    let external_market_id = first_string(value, &["id", "conditionId", "questionID"])
        .ok_or_else(|| AppError::Validation("polymarket market id is missing".to_owned()))?;
    let external_event_id = first_string(value, &["eventId", "event_id", "groupItemTitle"]);
    let title = first_string(value, &["question", "title", "name"])
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| AppError::Validation("polymarket market title is missing".to_owned()))?;
    let outcome_labels = json_string_array(
        &first_jsonish_value(value, &["outcomes", "tokens"]).unwrap_or(Value::Null),
    );
    let outcome_yes_label = outcome_labels
        .first()
        .cloned()
        .unwrap_or_else(|| "Yes".to_owned());
    let outcome_no_label = outcome_labels
        .get(1)
        .cloned()
        .unwrap_or_else(|| "No".to_owned());
    let prices = json_decimal_array(
        &first_jsonish_value(value, &["outcomePrices", "prices"]).unwrap_or(Value::Null),
    );
    let yes_price = prices
        .first()
        .cloned()
        .unwrap_or_else(|| decimal_str("0.5"));
    let no_price = prices
        .get(1)
        .cloned()
        .unwrap_or_else(|| decimal_str("1") - yes_price.clone());
    let is_closed = bool_field(value, "closed") || bool_field(value, "archived");
    let source_status = if is_closed {
        STATUS_HIDDEN.to_owned()
    } else {
        STATUS_ACTIVE.to_owned()
    };
    let external_resolution = first_string(
        value,
        &["resolutionOutcome", "resolvedOutcome", "winningOutcome", "outcome"],
    )
    .and_then(|outcome| normalize_external_resolution(&outcome))
    .or_else(|| closed_binary_price_resolution(is_closed, &prices));

    Ok(ParsedPolymarketMarket {
        external_event_id,
        external_market_id,
        slug: first_string(value, &["slug"]),
        title,
        description: first_string(value, &["description"]),
        image_url: first_string(value, &["image", "icon", "imageUrl"]),
        category: first_string(value, &["category", "categorySlug"]),
        tags_json: first_jsonish_value(value, &["tags"])
            .unwrap_or_else(|| Value::Array(Vec::new())),
        outcome_yes_label,
        outcome_no_label,
        yes_price: clamp_probability(yes_price),
        no_price: clamp_probability(no_price),
        volume: first_decimal(value, &["volume", "volumeNum", "volume24hr"]),
        liquidity: first_decimal(value, &["liquidity", "liquidityNum"]),
        end_at: first_string(value, &["endDate", "end_date"])
            .and_then(|text| parse_datetime(&text)),
        source_status,
        external_resolution,
        payload: value.clone(),
    })
}

fn closed_binary_price_resolution(is_closed: bool, prices: &[BigDecimal]) -> Option<String> {
    if !is_closed || prices.len() < 2 {
        return None;
    }
    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);
    match (&prices[0], &prices[1]) {
        (yes, no) if yes == &one && no == &zero => Some(OUTCOME_YES.to_owned()),
        (yes, no) if yes == &zero && no == &one => Some(OUTCOME_NO.to_owned()),
        _ => None,
    }
}

pub(crate) fn capped_payout(theoretical_payout: &BigDecimal, cap: &BigDecimal) -> BigDecimal {
    if cap > &BigDecimal::from(0) && theoretical_payout > cap {
        cap.clone()
    } else {
        theoretical_payout.clone()
    }
}

pub(crate) fn prediction_order_no(order_id: u64) -> String {
    format!("PM{}{:08}", Utc::now().format("%Y%m%d"), order_id)
}

pub(crate) fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
}

pub(crate) fn compact_error_message(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(512).collect()
}

pub(crate) fn is_duplicate_key_error(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

pub(crate) fn decimal_str(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap_or_else(|_| BigDecimal::from(0))
}

pub(crate) fn clamp_probability(value: BigDecimal) -> BigDecimal {
    if value <= BigDecimal::from(0) {
        decimal_str("0.01")
    } else if value >= BigDecimal::from(1) {
        decimal_str("0.99")
    } else {
        value.with_scale(8)
    }
}
