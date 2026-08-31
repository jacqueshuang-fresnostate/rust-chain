//! spot bounded context infrastructure compatibility facade.
//!
//! 真实职责按订单读模型/仓储、成交结算、钱包资金腿和行情价格拆分。既有
//! `spot::infrastructure::*` 路径通过本 façade 保持不变；事务 owner 仍在应用层或取消仓储，
//! 本层不调整 SQL、锁序、流水字段或幂等语义。

mod common;
mod market_prices;
mod order_repository;
mod read_models;
mod trade_settlement;
mod wallet_accounts;

pub(crate) use common::is_duplicate_key_error;
pub(crate) use market_prices::{
    latest_spot_market_price, triggered_limit_buy_order_ids, triggered_limit_sell_order_ids,
    triggered_stop_limit_buy_order_ids, triggered_stop_limit_sell_order_ids,
};
pub(crate) use order_repository::{
    SqlxSpotOrderCancelRepository, insert_spot_liquidity_buy_order_in_tx,
    insert_spot_liquidity_sell_order_in_tx, insert_spot_order_in_tx,
    load_spot_order_by_idempotency_key, lock_spot_fill_orders_in_order, lock_spot_order_by_db_id,
    store_spot_order_idempotency_response_in_tx,
};
pub use read_models::MySqlSpotRepository;
pub(crate) use read_models::{
    SpotOrderListFilter, SpotTradeListFilter, list_admin_spot_orders_page,
    list_admin_spot_trades_page, list_spot_orders, list_spot_trades,
    list_user_cancellable_spot_order_ids, load_spot_order_by_id,
};
pub(crate) use trade_settlement::{
    insert_spot_trade, load_existing_spot_trade_by_idempotency_key, load_spot_pair_db_id,
    pair_assets_in_tx, remaining_spot_fill_reservation_before_trade_in_tx,
    save_spot_order_fill_state, spot_order_reservation_in_tx,
};
pub(crate) use wallet_accounts::{
    SpotLedgerMetadata, ensure_spot_liquidity_inventory_in_tx, ensure_spot_liquidity_user_in_tx,
    ensure_wallet_account_in_tx, freeze_wallet_for_inserted_order_in_tx,
    lock_spot_fill_wallet_rows_in_order, release_buy_order_surplus_reservation_after_fill,
};

/// 在调用方成交事务内执行一条资金腿：借记 frozen 或贷记 available，并追加同桶流水。
/// 应用层是事务 owner，必须先按稳定顺序锁齐双方 base/quote 钱包，再依次完成四条资金腿与订单写回。
/// 本兼容入口只转发，不改变余额不足检查、钱包更新 SQL、流水元数据及提交边界。
pub(crate) async fn apply_spot_wallet_settlement_leg(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &bigdecimal::BigDecimal,
    credit_available: bool,
    ledger: SpotLedgerMetadata<'_>,
) -> crate::error::AppResult<()> {
    wallet_accounts::apply_spot_wallet_settlement_leg(
        tx,
        user_id,
        asset_id,
        amount,
        credit_available,
        ledger,
    )
    .await
}
