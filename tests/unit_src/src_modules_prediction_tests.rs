use crate::error::AppError;
use chrono::{TimeDelta, TimeZone, Utc};
use serde_json::{Value, json};

#[test]
fn extracts_markets_from_polymarket_events_with_context() {
    let payload = json!({
        "events": [
            {
                "id": "event-1",
                "slug": "sample-event",
                "category": "crypto",
                "tags": [{"label": "Bitcoin"}],
                "markets": [
                    {
                        "id": "market-1",
                        "question": "Will BTC close above 100k?",
                        "outcomes": "[\"Yes\",\"No\"]",
                        "outcomePrices": "[\"0.42\",\"0.58\"]"
                    }
                ]
            }
        ]
    });

    let markets = super::service::extract_market_values(payload);

    assert_eq!(markets.len(), 1);
    assert_eq!(
        markets[0].get("eventId").and_then(Value::as_str),
        Some("event-1")
    );
    assert_eq!(
        markets[0].get("category").and_then(Value::as_str),
        Some("crypto")
    );
    assert!(markets[0].get("tags").and_then(Value::as_array).is_some());

    let parsed = super::service::parse_polymarket_market(&markets[0]).expect("market should parse");
    assert_eq!(parsed.external_event_id.as_deref(), Some("event-1"));
    assert_eq!(parsed.external_market_id, "market-1");
    assert_eq!(parsed.outcome_yes_label, "Yes");
    assert_eq!(parsed.outcome_no_label, "No");
    assert_eq!(parsed.yes_price, super::service::decimal_str("0.42"));
}

#[test]
fn closed_polymarket_market_uses_final_binary_prices_for_resolution() {
    let market = json!({
        "id": "closed-market-1",
        "question": "Did the event resolve?",
        "outcomes": "[\"Yes\",\"No\"]",
        "outcomePrices": "[\"1\",\"0\"]",
        "closed": true
    });

    let parsed = super::service::parse_polymarket_market(&market).expect("market should parse");

    assert_eq!(parsed.source_status, super::service::STATUS_HIDDEN);
    assert_eq!(
        parsed.external_resolution.as_deref(),
        Some(super::service::OUTCOME_YES)
    );
}

#[test]
fn admin_asset_config_query_does_not_require_assets_updated_at() {
    assert!(!super::infrastructure::ADMIN_ASSET_CONFIGS_SQL.contains("assets.updated_at"));
    assert!(
        super::infrastructure::ADMIN_ASSET_CONFIGS_SQL
            .contains("COALESCE(configs.updated_at, assets.created_at)")
    );
    assert!(
        super::infrastructure::ADMIN_ASSET_CONFIGS_SQL
            .contains("COALESCE(configs.revision, CAST(0 AS UNSIGNED)) AS revision")
    );
}

#[test]
fn prediction_user_subject_uses_sa_token_user_prefix() {
    assert_eq!(super::service::user_id_from_subject("user:79").unwrap(), 79);
    assert!(matches!(
        super::service::user_id_from_subject("79"),
        Err(AppError::Unauthorized)
    ));
}

#[test]
fn prediction_admin_subject_is_parsed_only_from_admin_session_shape() {
    assert_eq!(
        super::service::admin_id_from_subject("admin:81").unwrap(),
        81
    );
    for invalid in ["81", "user:81", "admin:", "admin:not-a-number"] {
        assert!(matches!(
            super::service::admin_id_from_subject(invalid),
            Err(AppError::Unauthorized)
        ));
    }
}

#[test]
fn prediction_admin_reason_is_trimmed_required_and_bounded() {
    assert_eq!(
        super::service::required_admin_reason(Some("  调整预测费率  ".to_owned())).unwrap(),
        "调整预测费率"
    );
    for missing in [None, Some(String::new()), Some("   ".to_owned())] {
        assert!(matches!(
            super::service::required_admin_reason(missing),
            Err(AppError::Validation(message)) if message == "reason is required"
        ));
    }
    assert!(matches!(
        super::service::required_admin_reason(Some("理".repeat(513))),
        Err(AppError::Validation(message)) if message == "reason is too long"
    ));
}

#[test]
fn prediction_trading_window_is_left_open_at_end_and_rejects_stale_sync() {
    let now = Utc.with_ymd_and_hms(2026, 8, 24, 12, 0, 0).unwrap();
    assert!(
        super::service::validate_market_trading_window(
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_OPEN,
            Some(now + TimeDelta::microseconds(1)),
            Some(now - TimeDelta::seconds(60)),
            now,
            60,
        )
        .is_ok()
    );
    assert!(
        super::service::validate_market_trading_window(
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_OPEN,
            Some(now),
            Some(now),
            now,
            60,
        )
        .is_err()
    );
    assert!(
        super::service::validate_market_trading_window(
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_OPEN,
            Some(now + TimeDelta::minutes(10)),
            Some(now - TimeDelta::seconds(60) - TimeDelta::microseconds(1)),
            now,
            60,
        )
        .is_err()
    );
    assert!(
        super::service::validate_market_trading_window(
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_OPEN,
            Some(now + TimeDelta::minutes(10)),
            Some(now + TimeDelta::microseconds(1)),
            now,
            60,
        )
        .is_err()
    );

    for (display_status, settlement_status, end_at, last_synced_at) in [
        (
            super::service::STATUS_HIDDEN,
            super::service::SETTLEMENT_OPEN,
            Some(now + TimeDelta::minutes(10)),
            Some(now),
        ),
        (
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_PENDING_CONFIRMATION,
            Some(now + TimeDelta::minutes(10)),
            Some(now),
        ),
        (
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_OPEN,
            None,
            Some(now),
        ),
        (
            super::service::STATUS_ACTIVE,
            super::service::SETTLEMENT_OPEN,
            Some(now + TimeDelta::minutes(10)),
            None,
        ),
    ] {
        assert!(
            super::service::validate_market_trading_window(
                display_status,
                settlement_status,
                end_at,
                last_synced_at,
                now,
                60,
            )
            .is_err()
        );
    }
}
