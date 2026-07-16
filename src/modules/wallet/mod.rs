//! wallet bounded context.
//!
//! 按 DDD 结构划分：domain、repository、service、application、infrastructure、presentation、routes。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub mod routes;

pub use domain::{
    BalanceBucket, BalanceChange, LedgerBatch, LedgerMetadata, LockPosition, LockPositionSource,
    LockSchedule, MAX_ASSET_PRECISION_SCALE, MAX_WITHDRAW_FEE_TIER_COUNT, WalletAccount,
    WalletDomainError, WalletLedgerEntry, WalletServiceError, WithdrawFeeTier,
    amount_fits_asset_precision, asset_amount_fractional_scale, calculate_withdraw_fee,
    create_lock_positions, fixed_time_merge_key, immediate_on_listing_merge_key,
    normalize_withdraw_fee_tiers, truncate_amount_to_asset_precision,
    verify_locked_balance_invariant,
};
pub use infrastructure::{MySqlWalletRepository, NewAssetLockPosition, NewAssetLockPositionSource};
pub use repository::WalletRepository;
pub use service::{
    BalanceUpdateCommand, FreezeBalanceCommand, LockPositionCreationCommand, SettleBalanceCommand,
    UnfreezeBalanceCommand, WalletService,
};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_wallet_mod_tests.rs"]
mod tests;
