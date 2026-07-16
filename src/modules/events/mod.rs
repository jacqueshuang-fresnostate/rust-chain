pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;
pub use self::domain::{
    INBOX_CONSUMED, INBOX_DEAD_LETTER, INBOX_PROCESSING, INBOX_PROCESSING_LEASE_SECONDS,
    INBOX_PROCESSING_TOKEN_FORMAT, INBOX_RETRY, OUTBOX_DEAD_LETTER, OUTBOX_PENDING,
    OUTBOX_PUBLISHED, OUTBOX_RETRY,
};
pub use infrastructure::{MySqlEventInboxRepository, MySqlEventOutboxRepository};
pub use repository::{EventInboxRepository, EventOutboxRepository};

pub use service::*;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_events_mod_tests.rs"]
mod tests;
