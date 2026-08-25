pub mod agent;
pub mod channels;
pub mod images;
pub mod manager;
pub mod model;
pub mod model_catalog;
pub mod prompt;
pub mod protocol;
pub mod secret_store;
pub mod session;
pub mod settings;
pub mod subagents;
pub mod tools;

pub use manager::NativeAgentManager;
pub use session::{
    list_live_native_employee_processes, run_native_one_shot, run_native_read_only_one_shot,
    start_native_with_manager, stop_native_for_automation_restart,
};
