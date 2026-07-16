pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub mod routes;

pub use domain::{
    LifecycleStatus, NewCoinDomainError, NewCoinOrderKind, PostListingPurchasePlan,
    UnlockApplication, UnlockFeeBasis, UnlockFeeInput, UnlockFeeQuote, UnlockFeeRule, UnlockRule,
    UnlockSource, UnlockType, apply_unlock_rule, calculate_unlock_fee, ensure_subscription_allowed,
    ensure_unlock_release_allowed, plan_post_listing_purchase,
};

pub use repository::{
    MySqlNewCoinRepository, NewCoinPurchaseOrderInsert, NewCoinPurchaseOrderInsertResult,
    NewCoinPurchaseRepository, NewCoinRepositoryError, PostListingPurchaseRecord,
    UnlockFeePaidStatus, UnlockFeePaymentRecord, UnlockFeePaymentUpdate, UnlockFeeRepository,
    UnlockReleaseRecord, WalletLockCommandOutput,
};

pub use service::{
    NewCoinService, NewCoinServiceError, PayUnlockFeeCommand, PostListingPurchaseCommand,
    PostListingPurchaseResult, ReleaseUnlockCommand, UnlockFeeQuoteCommand, UnlockFeeQuoteResult,
    UnlockReleaseResult,
};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_new_coin_mod_tests.rs"]
mod tests;
