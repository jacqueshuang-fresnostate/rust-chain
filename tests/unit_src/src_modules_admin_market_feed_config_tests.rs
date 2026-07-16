use super::*;

#[test]
fn validates_market_feed_config_values() {
    assert_eq!(
        validate_symbols(&["BTC-USDT".to_owned()], true).unwrap(),
        ["BTCUSDT"]
    );
    assert!(validate_symbols(&[], true).is_err());
    assert!(validate_symbols(&[], false).unwrap().is_empty());
    assert_eq!(
        validate_intervals(&["1m".to_owned(), "1h".to_owned()]).unwrap(),
        ["1m", "1h"]
    );
    assert!(validate_intervals(&["2m".to_owned()]).is_err());
    assert_eq!(
        validate_providers(&["htx".to_owned(), "huobi".to_owned()]).unwrap(),
        ["htx"]
    );
    assert!(
        validate_providers(&["htx".to_owned(), "bitget".to_owned()])
            .unwrap_err()
            .to_string()
            .contains("only supports one enabled provider")
    );
}
