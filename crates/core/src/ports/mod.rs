//! Ports: the traits the core needs from adapters, declared in the core's own language.

pub mod events;
pub mod export_runner;
pub mod player;
#[cfg(test)]
mod player_tests;
pub mod storage;
pub mod update_backend;

pub use player::{Frame, PlayerError, VideoPlayer};
pub use storage::{JobStore, StoreError, StoredJob};
pub use update_backend::{ArtifactVerifier, UpdateBackend};
