use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::process::{
    CodexChild, CodexExecutionProvider, CodexSessionKind, ExecutionChangeBaseline,
    SdkFileChangeStore,
};

#[derive(Clone)]
pub struct ManagedCodexProcess {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: CodexSessionKind,
    pub child: Arc<Mutex<CodexChild>>,
    pub session_record_id: String,
    pub provider: CodexExecutionProvider,
    pub execution_change_baseline: Option<ExecutionChangeBaseline>,
    pub sdk_file_change_store: Option<SdkFileChangeStore>,
    pub cleanup_paths: Vec<PathBuf>,
}

/// Manages running Codex subprocess instances, keyed by session_record_id.
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
        task_id: Option<String>,
        session_kind: CodexSessionKind,
        child: Arc<Mutex<CodexChild>>,
        session_record_id: String,
        provider: CodexExecutionProvider,
        execution_change_baseline: Option<ExecutionChangeBaseline>,
        sdk_file_change_store: Option<SdkFileChangeStore>,
        cleanup_paths: Vec<PathBuf>,
    ) {
        self.processes.insert(
            session_record_id.clone(),
            ManagedCodexProcess {
                employee_id,
                task_id,
                session_kind,
                child,
                session_record_id,
                provider,
                execution_change_baseline,
                sdk_file_change_store,
                cleanup_paths,
            },
        );
    }

    pub fn remove_process(&mut self, session_record_id: &str) -> Option<ManagedCodexProcess> {
        self.processes.remove(session_record_id)
    }

    pub fn get_process(&self, session_record_id: &str) -> Option<ManagedCodexProcess> {
        self.processes.get(session_record_id).cloned()
    }

    pub fn get_employee_processes(&self, employee_id: &str) -> Vec<ManagedCodexProcess> {
        self.processes
            .values()
            .filter(|process| process.employee_id == employee_id)
            .cloned()
            .collect()
    }

    pub fn get_processes(&self) -> Vec<ManagedCodexProcess> {
        self.processes.values().cloned().collect()
    }

    pub fn has_employee_processes(&self, employee_id: &str) -> bool {
        self.processes
            .values()
            .any(|process| process.employee_id == employee_id)
    }

    #[cfg(test)]
    pub fn get_task_process(
        &self,
        employee_id: &str,
        task_id: &str,
        session_kind: CodexSessionKind,
    ) -> Option<ManagedCodexProcess> {
        self.processes
            .values()
            .find(|process| {
                process.employee_id == employee_id
                    && process.task_id.as_deref() == Some(task_id)
                    && process.session_kind == session_kind
            })
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use tokio::process::Command;

    use super::*;

    fn spawn_test_child() -> Arc<Mutex<CodexChild>> {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 10")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Arc::new(Mutex::new(CodexChild::new(
            command.spawn().expect("spawn test child"),
        )))
    }

    #[test]
    fn supports_multiple_sessions_for_same_employee() {
        tauri::async_runtime::block_on(async {
            let child_one = spawn_test_child();
            let child_two = spawn_test_child();

            let mut manager = CodexManager::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                CodexSessionKind::Execution,
                child_one.clone(),
                "session-1".to_string(),
                CodexExecutionProvider::Cli,
                None,
                None,
                Vec::new(),
            );
            manager.add_process(
                "emp-1".to_string(),
                Some("task-2".to_string()),
                CodexSessionKind::Review,
                child_two.clone(),
                "session-2".to_string(),
                CodexExecutionProvider::Cli,
                None,
                None,
                Vec::new(),
            );

            assert!(manager.has_employee_processes("emp-1"));
            assert_eq!(manager.get_employee_processes("emp-1").len(), 2);
            assert!(manager
                .get_task_process("emp-1", "task-1", CodexSessionKind::Execution)
                .is_some());
            assert!(manager
                .get_task_process("emp-1", "task-2", CodexSessionKind::Review)
                .is_some());

            for child in [child_one, child_two] {
                let mut child = child.lock().await;
                let _ = child.kill_process_group();
                let _ = child.kill().await;
            }
        });
    }
}
