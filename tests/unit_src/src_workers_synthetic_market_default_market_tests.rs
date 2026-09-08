use super::*;
use chrono::TimeZone;

fn minute() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 7, 1, 0, 0).unwrap()
}

fn snapshot(open: DateTime<Utc>) -> MarketKlineSnapshot {
    MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        "LOCAL-USDT",
        "1m",
        open,
        MarketKlineValues {
            open: BigDecimal::from(100),
            high: BigDecimal::from(102),
            low: BigDecimal::from(99),
            close: BigDecimal::from(101),
            volume: BigDecimal::from(60),
        },
        open + TimeDelta::seconds(59),
    )
    .unwrap()
}

fn fixture(source: &str, open: DateTime<Utc>) -> (PairGenerationRun, StoredMinute) {
    let state = StoredMinute {
        closed: snapshot(open),
        seed: "fixture".into(),
        version: 1,
        anchor: BigDecimal::from(100),
        parameters: (source == "default").then(DefaultMarketParameters::default),
        price_precision: 2,
        qty_precision: 2,
        pending_close: None,
        pending_owner: None,
        follow: None,
        accepted_follow: None,
    };
    let run = PairGenerationRun {
        generation: 1,
        active_source: source.into(),
        strategy_id: (source == "strategy").then_some(7),
        strategy_version: (source == "strategy").then_some(1),
        default_version: (source == "default").then_some(1),
        lease_owner: Some("owner-a".into()),
        last_price: Some(BigDecimal::from(101)),
        last_tick_at: Some(open + TimeDelta::seconds(59)),
        state_json: Some(SqlxJson(serde_json::to_value(&state).unwrap())),
    };
    (run, state)
}

fn save(run: &mut PairGenerationRun, state: &StoredMinute) {
    run.state_json = Some(SqlxJson(serde_json::to_value(state).unwrap()));
}

#[test]
fn reserved_minute_blocks_different_source_before_first_checkpoint() {
    let (mut run, _) = fixture("default", minute());
    run.last_tick_at = None;
    assert!(must_wait_for_boundary(Some(&run), "strategy", Some(7), Some(1), minute()).unwrap());
    assert!(!must_wait_for_boundary(Some(&run), "default", None, None, minute()).unwrap());
    assert!(
        !must_wait_for_boundary(
            Some(&run),
            "strategy",
            Some(7),
            Some(1),
            minute() + TimeDelta::minutes(1)
        )
        .unwrap()
    );
}

#[test]
fn manual_version_and_strategy_identity_both_own_the_minute() {
    let (run, _) = fixture("strategy", minute());
    assert!(!must_wait_for_boundary(Some(&run), "strategy", Some(7), Some(1), minute()).unwrap());
    assert!(must_wait_for_boundary(Some(&run), "strategy", Some(7), Some(2), minute()).unwrap());
    assert!(must_wait_for_boundary(Some(&run), "strategy", Some(8), Some(1), minute()).unwrap());
    assert!(must_wait_for_boundary(Some(&run), "default", None, None, minute()).unwrap());
}

#[test]
fn all_pause_resume_continues_only_the_reserved_default_minute() {
    let (mut run, _) = fixture("default", minute());
    run.active_source = "none".into();
    assert!(!must_wait_for_boundary(Some(&run), "default", None, None, minute()).unwrap());
    assert!(must_wait_for_boundary(Some(&run), "strategy", Some(7), Some(1), minute()).unwrap());
    let (_, mut state) = fixture("strategy", minute());
    state.parameters = None;
    save(&mut run, &state);
    assert!(must_wait_for_boundary(Some(&run), "default", None, None, minute()).unwrap());
}

#[test]
fn online_closure_requires_continuity_but_not_reconstruction_after_restart() {
    let (run, state) = fixture("default", minute());
    let now = minute() + TimeDelta::minutes(1);
    let close = online_previous_close(Some(&run), Some(&state), now, "owner-a")
        .unwrap()
        .unwrap();
    assert_eq!(close.open_time(), minute());
    assert_eq!(close.close(), state.closed.close());
    assert!(
        online_previous_close(Some(&run), Some(&state), now, "owner-b")
            .unwrap()
            .is_none()
    );
    assert!(
        online_previous_close(
            Some(&run),
            Some(&state),
            now + TimeDelta::seconds(5),
            "owner-a"
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn persisted_pending_survives_manual_handoff_failure_and_owner_transfer() {
    let current = minute() + TimeDelta::minutes(1);
    let (mut run, mut state) = fixture("strategy", current);
    state.pending_close = Some(snapshot(minute()));
    state.pending_owner = Some("owner-a".into());
    save(&mut run, &state);
    run.last_tick_at = None; // The first ticker may have committed before its checkpoint failed.
    for owner in ["owner-a", "owner-b"] {
        let close = source_transition_close(Some(&run), current + TimeDelta::seconds(2), owner)
            .unwrap()
            .unwrap();
        assert_eq!(close, snapshot(minute()));
    }
    assert!(
        source_transition_close(Some(&run), current + TimeDelta::minutes(1), "owner-b")
            .unwrap()
            .is_none()
    );
    state.pending_close = Some(snapshot(minute() - TimeDelta::minutes(1)));
    save(&mut run, &state);
    assert!(
        source_transition_close(Some(&run), current, "owner-b")
            .unwrap()
            .is_none()
    );
}

#[test]
fn invalid_persisted_state_fails_closed() {
    let (mut run, _) = fixture("default", minute());
    run.generation = 0;
    assert!(stored_minute(&run).is_err());
    run.generation = 1;
    run.state_json = Some(SqlxJson(serde_json::json!({"closed": "invalid"})));
    assert!(stored_minute(&run).is_err());
}
