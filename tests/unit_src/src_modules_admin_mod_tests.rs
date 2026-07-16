use super::*;
use chrono::{TimeDelta, TimeZone, Utc};

#[test]
fn sensitive_confirmation_metadata_expires_at_boundary() {
    let requested_at = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();
    let confirmation = SensitiveOperationConfirmation::new(
        "admin-1",
        "market_strategy.stop",
        "market_strategy",
        "strategy-1",
        "emergency stop",
        requested_at,
        TimeDelta::minutes(5),
    )
    .unwrap();

    assert_eq!(confirmation.admin_id(), "admin-1");
    assert_eq!(confirmation.operation(), "market_strategy.stop");
    assert_eq!(confirmation.target_type(), "market_strategy");
    assert_eq!(confirmation.target_id(), "strategy-1");
    assert_eq!(confirmation.reason(), "emergency stop");
    assert_eq!(
        confirmation.expires_at(),
        requested_at + TimeDelta::minutes(5)
    );
    assert!(!confirmation.is_expired(requested_at + TimeDelta::seconds(299)));
    assert!(confirmation.is_expired(requested_at + TimeDelta::minutes(5)));
    assert_eq!(
        confirmation.audit_metadata_key(),
        format!(
            "admin-sensitive:admin-1:market_strategy.stop:market_strategy:strategy-1:{}",
            requested_at.timestamp()
        )
    );
}

#[test]
fn sensitive_confirmation_requires_positive_ttl() {
    let requested_at = Utc.with_ymd_and_hms(2026, 5, 26, 10, 0, 0).unwrap();

    assert_eq!(
        SensitiveOperationConfirmation::new(
            "admin-1",
            "asset.adjust",
            "wallet_account",
            "account-1",
            "manual correction",
            requested_at,
            TimeDelta::zero(),
        )
        .unwrap_err(),
        SensitiveConfirmationError::InvalidTtl
    );
}
