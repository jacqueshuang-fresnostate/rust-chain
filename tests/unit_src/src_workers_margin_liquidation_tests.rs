use super::*;
use std::{str::FromStr, time::Instant};

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn margin_liquidation_limit_is_clamped() {
    assert_eq!(margin_liquidation_limit(0), 1);
    assert_eq!(margin_liquidation_limit(50), 50);
    assert_eq!(margin_liquidation_limit(500), 100);
}

#[test]
fn margin_liquidation_scan_limit_scans_past_broken_rows() {
    assert_eq!(margin_liquidation_scan_limit(0), 10);
    assert_eq!(margin_liquidation_scan_limit(1), 10);
    assert_eq!(margin_liquidation_scan_limit(50), 500);
    assert_eq!(margin_liquidation_scan_limit(500), 500);
}

#[test]
fn cross_positions_with_the_same_symbol_share_one_mark_read_plan() {
    let positions = vec![
        CrossMarginPositionCandidate {
            id: 13,
            pair_id: 7,
            symbol: "BTC-USDT".to_owned(),
        },
        CrossMarginPositionCandidate {
            id: 11,
            pair_id: 7,
            symbol: "BTC-USDT".to_owned(),
        },
        CrossMarginPositionCandidate {
            id: 19,
            pair_id: 9,
            symbol: "ETH-USDT".to_owned(),
        },
        CrossMarginPositionCandidate {
            id: 17,
            pair_id: 8,
            symbol: "BTC-USDT".to_owned(),
        },
    ];

    let grouped = cross_position_keys_by_symbol(&positions);

    assert_eq!(grouped.len(), 2);
    assert_eq!(
        grouped.get("BTC-USDT"),
        Some(&vec![(13, 7), (11, 7), (17, 8)])
    );
    assert_eq!(grouped.get("ETH-USDT"), Some(&vec![(19, 9)]));
}

#[test]
fn margin_liquidation_risk_state_rejects_invalid_direction() {
    let error = margin_liquidation_risk_state(
        "sideways",
        &decimal("20"),
        &decimal("100"),
        &decimal("0"),
        &decimal("100"),
        &decimal("90"),
        &decimal("0.05"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("long or short"));
}

#[test]
fn liquidation_mark_is_rechecked_after_waiting_for_database_locks() {
    let logical_now = Utc::now();
    let fresh = MarginLiquidationMark {
        price: decimal("100"),
        observed_at: logical_now,
        validated_logical_at: logical_now,
        validated_at: Instant::now(),
    };
    assert!(ensure_liquidation_mark_fresh(&fresh).is_ok());

    let stale_after_wait = MarginLiquidationMark {
        validated_at: Instant::now() - std::time::Duration::from_secs(61),
        ..fresh.clone()
    };
    assert!(
        ensure_liquidation_mark_fresh(&stale_after_wait)
            .unwrap_err()
            .to_string()
            .contains("stale")
    );

    let future = MarginLiquidationMark {
        observed_at: logical_now + chrono::TimeDelta::seconds(120),
        ..fresh
    };
    assert!(
        ensure_liquidation_mark_fresh(&future)
            .unwrap_err()
            .to_string()
            .contains("future")
    );
}
