//! Tool runtime is consumed by the agent loop / engine child tasks.
#![allow(dead_code)]

pub mod cancel;
pub mod catalog;
pub mod dispatch;
pub mod glob;
pub mod local;
pub mod mcp;
pub mod paths;
pub mod permission;
pub mod ssh;
pub mod web;

pub use cancel::CancelFlag;
pub use catalog::{is_read_only_native_tool, tool_specs, READ_ONLY_NATIVE_TOOL_NAMES};
pub use dispatch::{execute_tool, ToolCtx};
pub use local::LocalWorkspace;
pub use mcp::{connect_mcp_servers, McpSession};
