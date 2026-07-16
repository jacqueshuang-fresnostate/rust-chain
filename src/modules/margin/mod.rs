pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod repository;
pub mod routes;
pub mod service;

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_margin_application_tests.rs"]
mod tests;
