/// 申购与赎回是两条独立路径，都必须按资产精度处理金额并写平台对手腿。
/// 这里用源码级断言锁住接线，避免其中一条路径在后续重构里悄悄退回 18 位账本口径。
#[test]
fn subscribe_and_redeem_both_use_asset_precision_and_platform_legs() {
    let source = include_str!("../../src/modules/earn/application.rs");

    assert!(source.contains("validate_amount_asset_precision(&amount, asset_precision)"));
    assert!(source.contains("earn_subscription_journal_legs(&amount)"));
    assert!(source.contains("earn_redemption_journal_legs("));
    assert_eq!(
        source.matches("load_asset_precision_in_tx").count(),
        5,
        "subscribe, redeem, replay and both product exposure writes must read asset precision"
    );
    assert_eq!(
        source
            .matches("validate_exposure_capacity(write.principal_capacity.as_ref(), precision)")
            .count(),
        2
    );
    assert_eq!(
        source
            .matches("validate_exposure_capacity(write.liability_capacity.as_ref(), precision)")
            .count(),
        2
    );
}
