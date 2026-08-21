//! Native model clients. Chat/stream parsers are consumed by the agent loop
//! in a follow-up task; keep them compiled even before that wiring lands.
#![allow(dead_code)]

pub mod anthropic;
pub mod client;
pub mod openai;
pub mod responses;
pub mod retry;
pub mod sse;
pub mod types;
pub mod usage;

pub use client::{ModelClient, ModelClientConfig};
pub use retry::RetryConfig;
pub use usage::usage_to_delta;
