use crate::error::AppError;
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
    assert_eq!(parsed.external_resolution.as_deref(), Some(super::service::OUTCOME_YES));
}

#[test]
fn admin_asset_config_query_does_not_require_assets_updated_at() {
    assert!(!super::infrastructure::ADMIN_ASSET_CONFIGS_SQL.contains("assets.updated_at"));
    assert!(
        super::infrastructure::ADMIN_ASSET_CONFIGS_SQL
            .contains("COALESCE(configs.updated_at, assets.created_at)")
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
