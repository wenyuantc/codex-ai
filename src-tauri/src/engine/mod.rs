//! Shared AI engine process kernel: registry, child lifecycle, and execution context.
//!
//! Protocol-specific stream parsing and CLI/SDK launch stay in each engine module.

pub mod child;
pub mod context;
pub mod manager;
pub mod status;

pub use child::EngineChild;
pub use context::ExecutionContext;
pub use status::resolve_final_session_status;

// Traits and generic manager types are available via submodule paths
// (`engine::child::EngineProcessHandle`, `engine::manager::*`).
