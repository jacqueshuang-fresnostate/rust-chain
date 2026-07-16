use super::*;
use chrono::{TimeDelta, TimeZone, Utc};
use uuid::Uuid;

#[test]
fn quote_ttl_accepts_before_expiry_and_rejects_at_expiry() {
    let quote_id = QuoteId(Uuid::nil());
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();
    let quote = ConvertQuote::new(quote_id.clone(), now, 10).unwrap();

    assert_eq!(quote.quote_id(), &quote_id);
    assert_eq!(
        quote.idempotency_key(),
        "convert:quote:00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(quote.ttl().expires_at, now + TimeDelta::seconds(10));
    assert_eq!(
        quote.ensure_not_expired(now + TimeDelta::seconds(9)),
        Ok(())
    );
    assert_eq!(
        quote.ensure_not_expired(now + TimeDelta::seconds(10)),
        Err(ConvertQuoteError::Expired)
    );
}

#[test]
fn quote_ttl_requires_positive_ttl() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();

    assert_eq!(
        ConvertQuote::new(QuoteId(Uuid::nil()), now, 0).unwrap_err(),
        ConvertQuoteError::InvalidTtl
    );
}
