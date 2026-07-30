//! OpenCode process child handle — shared [`EngineChild`] re-export.
//!
//! Prefer [`OpenCodeChild::with_stdio`] when stdio is taken at spawn time.

pub use crate::engine::EngineChild as OpenCodeChild;
