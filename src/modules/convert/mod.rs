//! convert bounded context.
//!
//! 按 DDD 分层组织，入口模块只做分层声明和公开导出，不承载业务逻辑。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod service;

pub mod routes;

pub use domain::{
    ConfirmConvertQuoteCommand, ConvertBalanceSnapshot, ConvertConfirmationInsert,
    ConvertConfirmationResult, ConvertQuote, ConvertQuoteCacheEntry, ConvertQuoteCommand,
    ConvertQuoteConfirmationRecord, ConvertQuoteCreated, ConvertQuoteError, ConvertRepositoryError,
    ConvertServiceError, DomainLayerMarker, QuoteId, QuoteTtl,
};
pub use infrastructure::{MySqlConvertRepository, RedisConvertQuoteCache};
pub use repository::{
    ConvertOrderRepository, ConvertQuoteInsert, ConvertQuoteInsertResult, ConvertQuoteRepository,
};
pub use service::{ConvertService, ServiceLayerMarker};

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_convert_mod_tests.rs"]
mod tests;
