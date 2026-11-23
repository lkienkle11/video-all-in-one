//! SQLite persistence for Video All-in-One: the database file, its schema migrations and the job store.

mod backup;
mod database;
#[cfg(test)]
mod database_tests;
mod error;
mod job_rows;
mod job_store;
#[cfg(test)]
mod job_store_tests;
mod migrations;
#[cfg(test)]
mod test_support;

pub use database::Database;
pub use error::StorageError;
pub use job_store::SqliteJobStore;
pub use migrations::{Migration, SUPPORTED_SCHEMA};
