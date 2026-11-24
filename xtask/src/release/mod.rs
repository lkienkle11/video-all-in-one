//! Release tooling: versioning, update signing and the update manifest.

pub mod changelog;
pub mod check_release;
pub mod keygen;
pub mod latest_json;
pub mod set_version;
pub mod sign;

#[cfg(test)]
mod latest_json_policy_tests;
#[cfg(test)]
mod latest_json_tests;
#[cfg(test)]
mod set_version_tests;
