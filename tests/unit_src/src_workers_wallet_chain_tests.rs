use super::{
    WithdrawalReceiptStatus, normalize_gateway_identifier, normalize_withdrawal_receipt_status,
    retry_backoff_seconds,
};

#[test]
fn wallet_chain_retry_backoff_is_bounded() {
    assert_eq!(retry_backoff_seconds(1), 5);
    assert_eq!(retry_backoff_seconds(2), 10);
    assert_eq!(retry_backoff_seconds(7), 320);
    assert_eq!(retry_backoff_seconds(50), 320);
}

#[test]
fn wallet_chain_receipt_status_is_strict_and_case_insensitive() {
    assert_eq!(
        normalize_withdrawal_receipt_status(" Confirmed ").unwrap(),
        WithdrawalReceiptStatus::Confirmed
    );
    assert_eq!(
        normalize_withdrawal_receipt_status("FAILED").unwrap(),
        WithdrawalReceiptStatus::Failed
    );
    assert!(normalize_withdrawal_receipt_status("pending").is_err());
}

#[test]
fn wallet_chain_identifiers_reject_whitespace_and_empty_values() {
    assert_eq!(
        normalize_gateway_identifier(" 0xabc ", "tx_hash", 255).unwrap(),
        "0xabc"
    );
    assert!(normalize_gateway_identifier("0x abc", "tx_hash", 255).is_err());
    assert!(normalize_gateway_identifier("", "tx_hash", 255).is_err());
}
