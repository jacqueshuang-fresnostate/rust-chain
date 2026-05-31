use chrono::{DateTime, TimeDelta, Utc};
use thiserror::Error;

pub mod market_feed_config;
pub mod routes;

#[derive(Debug, Clone)]
pub struct AdminScope {
    pub admin_id: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitiveOperationConfirmation {
    admin_id: String,
    operation: String,
    target_type: String,
    target_id: String,
    reason: String,
    requested_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl SensitiveOperationConfirmation {
    pub fn new(
        admin_id: impl Into<String>,
        operation: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        reason: impl Into<String>,
        requested_at: DateTime<Utc>,
        ttl: TimeDelta,
    ) -> Result<Self, SensitiveConfirmationError> {
        if ttl <= TimeDelta::zero() {
            return Err(SensitiveConfirmationError::InvalidTtl);
        }

        Ok(Self {
            admin_id: admin_id.into(),
            operation: operation.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            reason: reason.into(),
            requested_at,
            expires_at: requested_at + ttl,
        })
    }

    pub fn admin_id(&self) -> &str {
        &self.admin_id
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn target_type(&self) -> &str {
        &self.target_type
    }

    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn requested_at(&self) -> DateTime<Utc> {
        self.requested_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    pub fn audit_metadata_key(&self) -> String {
        format!(
            "admin-sensitive:{}:{}:{}:{}:{}",
            self.admin_id,
            self.operation,
            self.target_type,
            self.target_id,
            self.requested_at.timestamp()
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SensitiveConfirmationError {
    #[error("sensitive confirmation ttl must be positive")]
    InvalidTtl,
}

#[cfg(test)]
mod tests {
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
}
