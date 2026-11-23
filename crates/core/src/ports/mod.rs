//! Ports: the traits the core needs from adapters, declared in the core's own language.

pub mod storage;
pub mod update_backend;

pub use storage::{JobStore, StoreError, StoredJob};
pub use update_backend::{ArtifactVerifier, UpdateBackend};
