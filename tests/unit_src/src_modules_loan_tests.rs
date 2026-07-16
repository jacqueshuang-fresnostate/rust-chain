use super::service::calculate_interest_amount;
use super::{
    domain::{INTEREST_MODE_ACTUAL_DAYS, INTEREST_MODE_FULL_TERM},
    *,
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

#[test]
fn full_term_interest_is_truncated_to_asset_precision() {
    let interest = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.123456789"),
        INTEREST_MODE_FULL_TERM,
        30,
        Utc::now(),
        Utc::now(),
        4,
    )
    .expect("interest amount");

    assert_eq!(interest, decimal("12.3456"));
}

#[test]
fn actual_days_interest_charges_at_least_one_day_and_clamps_to_term() {
    let disbursed_at = Utc::now();
    let one_day_interest = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.30"),
        INTEREST_MODE_ACTUAL_DAYS,
        30,
        disbursed_at,
        disbursed_at,
        2,
    )
    .expect("one day interest amount");
    let full_term_interest = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.30"),
        INTEREST_MODE_ACTUAL_DAYS,
        30,
        disbursed_at,
        disbursed_at + TimeDelta::days(45),
        2,
    )
    .expect("full term interest amount");

    assert_eq!(one_day_interest, decimal("1.00"));
    assert_eq!(full_term_interest, decimal("30.00"));
}

#[test]
fn default_product_name_json_uses_chinese_locale() {
    let name_json = normalized_product_name_json(None, "信用贷").expect("default name json");

    assert_eq!(name_json["version"], json!(1));
    assert_eq!(name_json["default_locale"], json!("zh-CN"));
    assert_eq!(name_json["items"][0]["country"], json!("CN"));
    assert_eq!(name_json["items"][0]["title"], json!("信用贷"));
    assert_eq!(product_default_name(&name_json), Some("信用贷".to_owned()));
}

#[test]
fn product_name_json_requires_default_locale_item() {
    let name_json = json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            { "locale": "en-US", "country": "US", "title": "Loan" }
        ]
    });

    let error =
        super::service::validate_product_name_json(&name_json).expect_err("missing default locale");
    assert!(format!("{error:?}").contains("default_locale must exist"));
}
