//! Margin bounded context infrastructure compatibility façade.
//!
//! SQL、Redis、钱包流水和结算写入按真实适配器职责拆分；本文件只保留既有稳定路径。

mod close_executions;
mod cross_accounts;
mod ledger;
mod market_data;
mod position_queries;
mod positions;
mod product_config;
mod query_support;
mod settlement;
mod transfers;

pub(crate) use close_executions::{
    MarginCloseExecutionWrite, insert_margin_close_execution,
    list_user_margin_position_close_executions, load_margin_close_execution_by_id,
    load_margin_close_execution_by_key_readonly, lock_margin_close_execution_by_key,
};
pub(crate) use cross_accounts::{
    activate_cross_margin_account_for_open, bump_cross_margin_account_version,
    discard_new_cross_margin_account_for_pending_order, ensure_and_lock_cross_margin_account,
    ensure_and_lock_cross_margin_account_with_creation, load_cross_margin_account,
    load_margin_open_product_account_scope, load_margin_position_account_scope,
    lock_cross_margin_risk_positions, require_active_cross_margin_account,
    update_locked_cross_margin_risk,
};
pub(crate) use market_data::{
    MarginRiskTicker, cached_margin_entry_price, cached_margin_mark_price,
    cached_margin_risk_ticker,
};
pub(crate) use position_queries::{
    MarginRiskPositionRow, list_admin_interest_summary, list_admin_margin_positions,
    list_margin_wallet_accounts, list_user_cross_margin_accounts,
    list_user_cross_margin_risk_positions, list_user_margin_positions,
    load_admin_margin_position_by_id, load_user_cross_margin_wallet_available,
    load_user_position_by_id, load_user_risk_position_by_id,
};
pub(crate) use positions::{
    LockedMarginPositionRow, MarginOpenProductRule, existing_position_for_idempotency_key,
    existing_position_for_idempotency_key_readonly, insert_margin_position,
    load_cancelable_position_ids, load_open_position_ids, lock_active_open_product,
    lock_pending_margin_limit_position_by_id, lock_user_position_by_id,
    mark_margin_limit_position_filled, set_margin_position_wallet_scope,
    triggered_margin_limit_position_ids,
};
pub(crate) use product_config::{
    MarginProductSettingRule, MarginProductUpsertValues, ensure_asset_exists, ensure_pair_exists,
    insert_admin_audit_log, insert_margin_product, list_admin_margin_products,
    list_margin_products, load_product_by_id, load_user_margin_setting,
    load_user_margin_setting_from_pool, lock_active_product_setting_rule, lock_product_by_id,
    update_margin_product, update_margin_product_status, upsert_user_margin_setting,
};
pub(crate) use settlement::{
    MarginPositionPartialCloseWrite, apply_cross_margin_account_settlement,
    apply_cross_margin_position_settlement, credit_margin_position_amount,
    debit_margin_position_open_collateral, load_position_by_id, mark_position_canceled,
    mark_position_closed, mark_position_partially_closed,
};
pub(crate) use transfers::{
    apply_margin_to_spot_transfer, apply_spot_to_margin_transfer, insert_margin_transfer,
    load_margin_transfer_by_idempotency_key, load_margin_transfer_wallet_snapshots,
    lock_margin_transfer_wallets, resolve_active_transfer_asset,
    resolve_transfer_asset_id_for_replay,
};
