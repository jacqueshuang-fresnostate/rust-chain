//! Margin bounded context infrastructure compatibility façade.
//!
//! SQL、Redis、钱包流水和结算写入按真实适配器职责拆分；本文件只保留既有稳定路径。

mod ledger;
mod market_data;
mod position_queries;
mod positions;
mod product_config;
mod query_support;
mod settlement;
mod transfers;

pub(crate) use market_data::{
    cached_margin_entry_price, cached_margin_mark_price, cached_margin_risk_ticker,
};
pub(crate) use position_queries::{
    list_admin_interest_summary, list_admin_margin_positions, list_margin_wallet_accounts,
    list_user_cross_margin_accounts, list_user_margin_positions, load_admin_margin_position_by_id,
    load_user_position_by_id, load_user_risk_position_by_id,
};
pub(crate) use positions::{
    LockedMarginPositionRow, MarginOpenProductRule, ensure_cross_margin_account,
    existing_position_for_idempotency_key, existing_position_for_idempotency_key_readonly,
    insert_margin_position, load_cancelable_position_ids, load_open_position_ids,
    lock_active_open_product, lock_user_position_by_id, set_margin_position_wallet_scope,
};
pub(crate) use product_config::{
    MarginProductSettingRule, MarginProductUpsertValues, ensure_asset_exists, ensure_pair_exists,
    insert_admin_audit_log, insert_margin_product, list_admin_margin_products,
    list_margin_products, load_product_by_id, load_user_margin_setting,
    load_user_margin_setting_from_pool, lock_active_product_setting_rule, lock_product_by_id,
    update_margin_product, update_margin_product_status, upsert_user_margin_setting,
};
pub(crate) use settlement::{
    apply_cross_margin_account_settlement, apply_cross_margin_position_settlement,
    credit_margin_position_amount, debit_margin_position_open_collateral, load_position_by_id,
    mark_position_canceled, mark_position_closed,
};
pub(crate) use transfers::{
    insert_margin_transfer, load_margin_transfer_by_idempotency_key,
    load_margin_transfer_wallet_snapshots, resolve_active_transfer_asset,
    resolve_transfer_asset_id_for_replay, transfer_margin_to_spot_wallets,
    transfer_spot_to_margin_wallets,
};
