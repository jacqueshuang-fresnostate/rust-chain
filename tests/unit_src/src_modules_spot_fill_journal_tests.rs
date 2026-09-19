use super::*;
use std::str::FromStr;

#[test]
fn spot_fill_journal_distinguishes_actual_inventory_from_user_transfers() {
    let amount = BigDecimal::from_str("1.234567890123456789").unwrap();
    for (source_platform, target_platform, expected) in [
        (
            false,
            false,
            vec![
                ("user_spot_source_liability", amount.clone()),
                ("user_spot_target_liability", -amount.clone()),
            ],
        ),
        (
            false,
            true,
            vec![
                ("user_spot_source_liability", amount.clone()),
                ("platform_spot_inventory", -amount.clone()),
            ],
        ),
        (
            true,
            false,
            vec![
                ("user_spot_target_liability", -amount.clone()),
                ("platform_spot_inventory", amount.clone()),
            ],
        ),
        (true, true, vec![]),
    ] {
        let legs = asset_transfer_legs(&amount, source_platform, target_platform);
        assert_eq!(
            legs.iter()
                .fold(BigDecimal::from(0), |sum, leg| sum + &leg.amount),
            0
        );
        assert_eq!(
            legs.into_iter()
                .map(|leg| (leg.account_code, leg.amount))
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert!(asset_transfer_legs(&BigDecimal::from(0), false, false).is_empty());
}
