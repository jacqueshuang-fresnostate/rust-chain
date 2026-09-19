use super::*;
use crate::modules::margin::infrastructure::interest::{billed_window_end, interest_increment};
use chrono::{TimeDelta, TimeZone};

fn full_elapsed_hours(from: DateTime<Utc>, now: DateTime<Utc>) -> u64 {
    u64::try_from((now - from).num_hours().max(0)).unwrap()
}

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
    let first_end = billed_window_end(start, first_hours).unwrap();
    assert_eq!(first_end, start + TimeDelta::hours(1));
    assert!(
        first_end < first_now,
        "the 30 minute remainder must stay unbilled"
    );

    // 第二轮：到第 3 小时，从上一轮的真实结点起算能收满 2 小时。
    let second_now = start + TimeDelta::hours(3);
    let second_hours = full_elapsed_hours(first_end, second_now);
    assert_eq!(second_hours, 2);
    assert_eq!(
        billed_window_end(first_end, second_hours).unwrap(),
        second_now
    );

    // 两轮合计正好覆盖 3 小时；旧实现把时间戳写成 now，只能收到 2 小时。
    assert_eq!(first_hours + second_hours, 3);
}

#[test]
fn billed_window_end_never_moves_past_the_current_moment() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let now = start + TimeDelta::minutes(59);

    assert_eq!(
        billed_window_end(start, full_elapsed_hours(start, now)).unwrap(),
        start
    );
}

#[test]
fn margin_interest_delta_charges_only_the_billed_full_hours() {
    let (delta, _) =
        interest_increment(&decimal("100"), &decimal("0.0001"), &decimal("0"), 3, 18).unwrap();

    assert_eq!(delta, decimal("0.030000000000000000"));
}

/// 计提只读持仓上固化的利率快照；一旦联产品表取实时值，管理员改配就会追溯未计费窗口。
#[test]
fn accrual_reads_the_position_snapshot_rate_and_ignores_live_product_changes() {
    let worker = include_str!("../../src/workers/margin_interest.rs");

    assert!(worker.contains("positions.hourly_interest_rate"));
    assert!(
        !worker.contains("products.hourly_interest_rate"),
        "accrual must not read the live product rate"
    );
    assert!(
        !worker.contains("INNER JOIN margin_products"),
        "accrual must not join the product table for the rate"
    );
}

#[tokio::test]
async fn margin_interest_timer_overflow_fails_before_database_access() {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .connect_lazy("mysql://root@127.0.0.1/margin_unused_test")
        .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        run_loop(pool, u64::MAX, 1),
    )
    .await
    .unwrap();
    assert!(matches!(result, Err(AppError::Validation(_))));
}
