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
fn isolated_liquidation_registers_negative_equity_as_bad_debt() {
    assert_eq!(
        isolated_liquidation_bad_debt_amount(&decimal("-12.5")),
        decimal("12.500000000000000000")
    );
    assert_eq!(
        isolated_liquidation_bad_debt_amount(&decimal("3.25")),
        decimal("0.000000000000000000")
    );
    assert_eq!(
        isolated_liquidation_bad_debt_amount(&decimal("0")),
        decimal("0.000000000000000000")
    );
}

#[test]
fn isolated_payout_plus_bad_debt_reconstructs_the_position_equity() {
    for equity in ["-12.5", "-0.000000000000000001", "0", "3.25", "100"] {
        let equity = decimal(equity);
        let payout = non_negative_amount(&equity);
        let bad_debt = isolated_liquidation_bad_debt_amount(&equity);

        assert_eq!(
            payout - bad_debt,
            equity.with_scale(18),
            "payout minus bad debt must equal equity so no shortfall disappears"
        );
    }
}

#[test]
fn isolated_bad_debt_is_persisted_and_exposed_to_ops() {
    let worker = include_str!("../../src/workers/margin_liquidation.rs");
    let admin = include_str!("../../src/modules/admin/infrastructure/margin.rs");
    let response = include_str!("../../src/modules/admin/presentation/dashboard_audit.rs");

    assert!(worker.contains("bad_debt_amount"));
    assert!(admin.contains("liquidation.bad_debt_amount"));
    assert!(response.contains("bad_debt_amount"));
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
#[test]
fn margin_liquidation_schedule_rejects_datetime_overflow() {
    assert!(super::checked_schedule_time(chrono::DateTime::<chrono::Utc>::MAX_UTC, 60).is_err());
    let now = chrono::Utc::now();
    assert_eq!(
        super::checked_schedule_time(now, 60).unwrap() - now,
        chrono::TimeDelta::seconds(60),
    );
    assert!(super::checked_schedule_time(now, i64::MAX).is_err());
}
