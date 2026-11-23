//! Ports: the traits the core needs from adapters, declared in the core's own language.

pub mod update_backend;

pub use update_backend::{ArtifactVerifier, UpdateBackend};
