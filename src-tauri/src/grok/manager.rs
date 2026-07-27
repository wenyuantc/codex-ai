use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::process::{GrokChild, GrokSessionKind};

#[derive(Clone)]
pub struct ManagedGrokProcess {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: GrokSessionKind,
    pub child: Arc<Mutex<GrokChild>>,
    pub session_record_id: String,
    pub cleanup_paths: Vec<PathBuf>,
}

pub struct GrokManager {
    processes: HashMap<String, ManagedGrokProcess>,
}

impl GrokManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    pub fn add_process(
        &mut self,
        employee_id: String,
        task_id: Option<String>,
        session_kind: GrokSessionKind,
        child: Arc<Mutex<GrokChild>>,
        session_record_id: String,
        cleanup_paths: Vec<PathBuf>,
    ) {
        self.processes.insert(
            session_record_id.clone(),
            ManagedGrokProcess {
                employee_id,
                task_id,
                session_kind,
                child,
                session_record_id,
                cleanup_paths,
            },
        );
    }

    pub fn remove_process(&mut self, session_record_id: &str) -> Option<ManagedGrokProcess> {
        self.processes.remove(session_record_id)
    }

    pub fn get_process(&self, session_record_id: &str) -> Option<ManagedGrokProcess> {
        self.processes.get(session_record_id).cloned()
    }

    pub fn get_employee_processes(&self, employee_id: &str) -> Vec<ManagedGrokProcess> {
        self.processes
            .values()
            .filter(|process| process.employee_id == employee_id)
            .cloned()
            .collect()
    }

    pub fn has_employee_processes(&self, employee_id: &str) -> bool {
        self.processes
            .values()
            .any(|process| process.employee_id == employee_id)
    }

    pub fn get_task_process_any(
        &self,
        task_id: &str,
        session_kind: GrokSessionKind,
    ) -> Option<ManagedGrokProcess> {
        self.processes
            .values()
            .find(|process| {
                process.task_id.as_deref() == Some(task_id) && process.session_kind == session_kind
            })
            .cloned()
    }
}
