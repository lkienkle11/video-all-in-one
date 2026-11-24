//! The schema versions the app knows. Each migration is an SQL script applied once, in version order.

/// One schema step. Applying it moves the database to `version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: i64,
    pub sql: &'static str,
}

/// The newest schema this build opens. A file with a higher version is refused.
pub const SUPPORTED_SCHEMA: i64 = 1;

/// The migrations shipped with the app.
pub(crate) const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: include_str!("../migrations/0001_init.sql"),
}];
