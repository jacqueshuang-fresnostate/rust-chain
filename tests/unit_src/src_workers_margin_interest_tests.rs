use super::*;
use chrono::{TimeDelta, TimeZone};

fn decimal(value: &str) -> BigDecimal {
    value.parse::<BigDecimal>().unwrap()
}

#[test]
fn interest_windows_carry_the_partial_hour_instead_of_dropping_it() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    // 第一轮：过了 90 分钟，只能收满 1 小时，时间戳推进到「起点 + 1 小时」而不是当前时刻。
    let first_now = start + TimeDelta::minutes(90);
    let first_hours = full_elapsed_hours(start, first_now);
    assert_eq!(first_hours, 1);
    let first_end = billed_window_end(start, first_hours);
    assert_eq!(first_end, start + TimeDelta::hours(1));
    assert!(
        first_end < first_now,
        "the 30 minute remainder must stay unbilled"
    );

    // 第二轮：到第 3 小时，从上一轮的真实结点起算能收满 2 小时。
    let second_now = start + TimeDelta::hours(3);
    let second_hours = full_elapsed_hours(first_end, second_now);
    assert_eq!(second_hours, 2);
    assert_eq!(billed_window_end(first_end, second_hours), second_now);

    // 两轮合计正好覆盖 3 小时；旧实现把时间戳写成 now，只能收到 2 小时。
    assert_eq!(first_hours + second_hours, 3);
}

#[test]
fn billed_window_end_never_moves_past_the_current_moment() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let now = start + TimeDelta::minutes(59);

    assert_eq!(
        billed_window_end(start, full_elapsed_hours(start, now)),
        start
    );
}

#[test]
fn margin_interest_delta_charges_only_the_billed_full_hours() {
    let delta = margin_interest_delta(&decimal("100"), &decimal("0.0001"), 3);

    assert_eq!(delta, decimal("0.030000000000000000"));
}
