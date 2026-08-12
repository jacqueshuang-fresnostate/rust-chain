//! wallet bounded context infrastructure compatibility façade.
//!
//! 具体持久化职责按账户流水、充币链事件、提现状态机与收益查询拆分；本文件仅保留既有路径的兼容导出。
//! 资金不变量仍由各子模块维护：余额三桶与流水同事务一致，链事件重放不得重复入账，提现冻结与释放必须守恒。

mod accounts_ledger;
mod deposits;
mod returns;
mod shared;
mod withdrawals;

pub use accounts_ledger::{
    MySqlWalletRepository, NewAssetLockPosition, NewAssetLockPositionSource,
};
pub use deposits::{
    NewWalletChainEventDeadLetter, WalletChainEventDeadLetterRecord,
    insert_wallet_chain_event_dead_letter, list_wallet_chain_event_dead_letters,
};
pub use withdrawals::HttpWalletChainGateway;

pub(crate) use accounts_ledger::{
    WalletLedgerCategory, WalletLedgerFilter, list_wallet_accounts, list_wallet_ledger,
};
pub(crate) use deposits::{
    assign_deposit_address_in_tx, ensure_deposit_enabled_asset, list_active_deposit_networks,
    list_deposit_assets, list_deposit_events, list_withdraw_assets,
    load_active_deposit_network_config, load_deposit_address_in_tx, load_user_deposit_address,
    load_user_email_in_tx, lock_available_deposit_address, observe_deposit_event,
    reverse_deposit_event,
};
pub(crate) use returns::{
    ReturnHistoryAssetActivityRow, TodayReturnAssetActivityRow, load_current_usdt_prices,
    load_historical_usdt_daily_closes, load_return_history_asset_activity,
    load_today_return_asset_activity,
};
pub(crate) use withdrawals::{
    approve_withdrawal_in_tx, confirm_withdrawal_in_tx, list_admin_wallet_withdrawals_page,
    list_wallet_withdrawals, load_withdrawal_asset_rule,
    load_withdrawal_by_gateway_request_for_update, load_withdrawal_by_user_key,
    mark_withdrawal_broadcasted_in_tx, mark_withdrawal_manual_review_in_tx,
    release_withdrawal_in_tx, reserve_withdrawal_request, update_withdrawal_chain_progress_in_tx,
};

#[cfg(test)]
use accounts_ledger::{
    WalletLedgerEntryRow, classify_wallet_ledger_change_type, push_wallet_ledger_filters,
    wallet_ledger_entry_response,
};

#[cfg(test)]
use returns::{
    return_history_historical_close_if_valid, return_history_kline_document_close_if_valid,
    today_return_ticker_price_if_current,
};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_infrastructure_tests.rs"]
mod tests;
