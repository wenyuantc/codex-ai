use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::process::{
    CodexChild, CodexExecutionProvider, ExecutionChangeBaseline, SdkFileChangeStore,
};

#[derive(Clone)]
pub struct ManagedCodexProcess {
    pub child: Arc<Mutex<CodexChild>>,
    pub session_record_id: String,
    pub provider: CodexExecutionProvider,
    pub execution_change_baseline: Option<ExecutionChangeBaseline>,
    pub sdk_file_change_store: Option<SdkFileChangeStore>,
}

/// Manages running Codex CLI subprocess instances, keyed by employee_id.
pub struct CodexManager {
    processes: HashMap<String, ManagedCodexProcess>,
}

impl CodexManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    pub fn add_process(
        &mut self,
        employee_id: String,
        child: Arc<Mutex<CodexChild>>,
        session_record_id: String,
        provider: CodexExecutionProvider,
        execution_change_baseline: Option<ExecutionChangeBaseline>,
        sdk_file_change_store: Option<SdkFileChangeStore>,
    ) {
        self.processes.insert(
            employee_id,
            ManagedCodexProcess {
                child,
                session_record_id,
                provider,
                execution_change_baseline,
                sdk_file_change_store,
            },
        );
    }

    pub fn remove_process(&mut self, employee_id: &str) -> Option<ManagedCodexProcess> {
        self.processes.remove(employee_id)
    }

    pub fn is_running(&self, employee_id: &str) -> bool {
        self.processes.contains_key(employee_id)
    }

    pub fn get_process(&self, employee_id: &str) -> Option<ManagedCodexProcess> {
        self.processes.get(employee_id).cloned()
    }
}
