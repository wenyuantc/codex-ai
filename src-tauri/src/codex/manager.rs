use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::process::CodexChild;

#[derive(Clone)]
pub struct ManagedCodexProcess {
    pub child: Arc<Mutex<CodexChild>>,
    pub session_record_id: String,
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
    ) {
        self.processes.insert(
            employee_id,
            ManagedCodexProcess {
                child,
                session_record_id,
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
