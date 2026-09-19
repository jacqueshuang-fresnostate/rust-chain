use super::retry_delay;

#[test]
fn failure_backoff_is_bounded_and_never_permanent() {
    assert_eq!(retry_delay(0), 60);
    assert_eq!(retry_delay(1), 60);
    assert_eq!(retry_delay(2), 120);
    assert_eq!(retry_delay(7), 3600);
    assert_eq!(retry_delay(u64::MAX), 3600);
}
