use crate::modules::wallet::application::normalize_asset_symbol;

#[test]
fn normalize_asset_symbol_to_uppercase() {
    assert_eq!(normalize_asset_symbol(" usdt ").unwrap(), "USDT");
}

#[test]
fn normalize_asset_symbol_rejects_invalid_format() {
    assert!(normalize_asset_symbol("BTC-USDT").is_err());
}
