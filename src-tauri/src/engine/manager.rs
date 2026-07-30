use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::child::EngineChild;

/// In-memory record for one running engine session process.
#[derive(Clone)]
pub struct ManagedProcess<SessionKind, Extra = ()> {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: SessionKind,
    pub child: Arc<Mutex<EngineChild>>,
    pub session_record_id: String,
    pub cleanup_paths: Vec<PathBuf>,
    pub extra: Extra,
}

/// Process registry keyed by `session_record_id`.
pub struct ProcessManager<SessionKind, Extra = ()> {
    processes: HashMap<String, ManagedProcess<SessionKind, Extra>>,
}

/// Minimal registry contract shared by engine managers.
pub trait EngineProcessRegistry {
    fn has_employee_processes(&self, employee_id: &str) -> bool;
}

impl<SessionKind, Extra> ProcessManager<SessionKind, Extra>
where
    SessionKind: Copy + Eq,
    Extra: Clone,
{
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    pub fn add_process(
        &mut self,
        employee_id: String,
        task_id: Option<String>,
        session_kind: SessionKind,
        child: Arc<Mutex<EngineChild>>,
        session_record_id: String,
        cleanup_paths: Vec<PathBuf>,
        extra: Extra,
    ) {
        self.processes.insert(
            session_record_id.clone(),
            ManagedProcess {
                employee_id,
                task_id,
                session_kind,
                child,
                session_record_id,
                cleanup_paths,
                extra,
            },
        );
    }

    pub fn remove_process(
        &mut self,
        session_record_id: &str,
    ) -> Option<ManagedProcess<SessionKind, Extra>> {
        self.processes.remove(session_record_id)
    }

    pub fn get_process(
        &self,
        session_record_id: &str,
    ) -> Option<ManagedProcess<SessionKind, Extra>> {
        self.processes.get(session_record_id).cloned()
    }

    pub fn get_employee_processes(
        &self,
        employee_id: &str,
    ) -> Vec<ManagedProcess<SessionKind, Extra>> {
        self.processes
            .values()
            .filter(|process| process.employee_id == employee_id)
            .cloned()
            .collect()
    }

    pub fn get_processes(&self) -> Vec<ManagedProcess<SessionKind, Extra>> {
        self.processes.values().cloned().collect()
    }

    pub fn has_employee_processes(&self, employee_id: &str) -> bool {
        self.processes
            .values()
            .any(|process| process.employee_id == employee_id)
    }

    pub fn has_unbound_employee_process(
        &self,
        employee_id: &str,
        session_kind: SessionKind,
    ) -> bool {
        self.processes.values().any(|process| {
            process.employee_id == employee_id
                && process.task_id.is_none()
                && process.session_kind == session_kind
        })
    }

    pub fn get_task_process_any(
        &self,
        task_id: &str,
        session_kind: SessionKind,
    ) -> Option<ManagedProcess<SessionKind, Extra>> {
        self.processes
            .values()
            .find(|process| {
                process.task_id.as_deref() == Some(task_id) && process.session_kind == session_kind
            })
            .cloned()
    }

    pub fn get_task_process(
        &self,
        employee_id: &str,
        task_id: &str,
        session_kind: SessionKind,
    ) -> Option<ManagedProcess<SessionKind, Extra>> {
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

impl<SessionKind, Extra> Default for ProcessManager<SessionKind, Extra>
where
    SessionKind: Copy + Eq,
    Extra: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<SessionKind, Extra> EngineProcessRegistry for ProcessManager<SessionKind, Extra>
where
    SessionKind: Copy + Eq,
    Extra: Clone,
{
    fn has_employee_processes(&self, employee_id: &str) -> bool {
        ProcessManager::has_employee_processes(self, employee_id)
    }
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use tokio::process::Command;

    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TestSessionKind {
        Execution,
        Review,
    }

    fn spawn_test_child() -> Arc<Mutex<EngineChild>> {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 10")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Arc::new(Mutex::new(EngineChild::new(
            command.spawn().expect("spawn test child"),
        )))
    }

    #[test]
    fn supports_multiple_task_sessions_for_same_employee() {
        tauri::async_runtime::block_on(async {
            let child_one = spawn_test_child();
            let child_two = spawn_test_child();

            let mut manager = ProcessManager::<TestSessionKind>::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                TestSessionKind::Execution,
                child_one.clone(),
                "session-1".to_string(),
                Vec::new(),
                (),
            );
            manager.add_process(
                "emp-1".to_string(),
                Some("task-2".to_string()),
                TestSessionKind::Review,
                child_two.clone(),
                "session-2".to_string(),
                Vec::new(),
                (),
            );

            assert!(manager.has_employee_processes("emp-1"));
            assert_eq!(manager.get_employee_processes("emp-1").len(), 2);
            assert!(manager
                .get_task_process_any("task-1", TestSessionKind::Execution)
                .is_some());
            assert!(manager
                .get_task_process("emp-1", "task-2", TestSessionKind::Review)
                .is_some());
            assert!(!manager.has_unbound_employee_process("emp-1", TestSessionKind::Execution));

            for child in [child_one, child_two] {
                let mut child = child.lock().await;
                let _ = child.kill_process_group();
                let _ = child.kill().await;
            }
        });
    }

    #[test]
    fn registry_trait_reports_employee_processes() {
        tauri::async_runtime::block_on(async {
            let child = spawn_test_child();
            let mut manager = ProcessManager::<TestSessionKind>::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                TestSessionKind::Execution,
                child.clone(),
                "session-1".to_string(),
                Vec::new(),
                (),
            );

            let registry: &dyn EngineProcessRegistry = &manager;
            assert!(registry.has_employee_processes("emp-1"));
            assert!(!registry.has_employee_processes("emp-missing"));

            let mut child = child.lock().await;
            let _ = child.kill_process_group();
            let _ = child.kill().await;
        });
    }

    #[test]
    fn detects_only_unbound_employee_sessions() {
        tauri::async_runtime::block_on(async {
            let task_child = spawn_test_child();
            let unbound_child = spawn_test_child();

            let mut manager = ProcessManager::<TestSessionKind>::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                TestSessionKind::Execution,
                task_child.clone(),
                "session-1".to_string(),
                Vec::new(),
                (),
            );

            assert!(!manager.has_unbound_employee_process("emp-1", TestSessionKind::Execution));

            manager.add_process(
                "emp-1".to_string(),
                None,
                TestSessionKind::Execution,
                unbound_child.clone(),
                "session-2".to_string(),
                Vec::new(),
                (),
            );

            assert!(manager.has_unbound_employee_process("emp-1", TestSessionKind::Execution));

            for child in [task_child, unbound_child] {
                let mut child = child.lock().await;
                let _ = child.kill_process_group();
                let _ = child.kill().await;
            }
        });
    }
}
