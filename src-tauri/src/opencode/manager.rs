use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::process::{OpenCodeChild, OpenCodeSessionKind};

#[derive(Clone)]
pub struct ManagedOpenCodeProcess {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: OpenCodeSessionKind,
    pub child: Arc<Mutex<OpenCodeChild>>,
    pub session_record_id: String,
    pub cleanup_paths: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct ManagedOpenCodeSdkServer {
    pub child: Arc<Mutex<OpenCodeChild>>,
    pub host: String,
    pub port: u16,
}

pub struct OpenCodeManager {
    processes: HashMap<String, ManagedOpenCodeProcess>,
    sdk_server: Option<ManagedOpenCodeSdkServer>,
}

impl OpenCodeManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            sdk_server: None,
        }
    }

    pub fn add_process(
        &mut self,
        employee_id: String,
        task_id: Option<String>,
        session_kind: OpenCodeSessionKind,
        child: Arc<Mutex<OpenCodeChild>>,
        session_record_id: String,
        cleanup_paths: Vec<PathBuf>,
    ) {
        self.processes.insert(
            session_record_id.clone(),
            ManagedOpenCodeProcess {
                employee_id,
                task_id,
                session_kind,
                child,
                session_record_id,
                cleanup_paths,
            },
        );
    }

    pub fn remove_process(&mut self, session_record_id: &str) -> Option<ManagedOpenCodeProcess> {
        self.processes.remove(session_record_id)
    }

    pub fn get_process(&self, session_record_id: &str) -> Option<ManagedOpenCodeProcess> {
        self.processes.get(session_record_id).cloned()
    }

    pub fn get_employee_processes(&self, employee_id: &str) -> Vec<ManagedOpenCodeProcess> {
        self.processes
            .values()
            .filter(|process| process.employee_id == employee_id)
            .cloned()
            .collect()
    }

    pub fn set_sdk_server(&mut self, host: String, port: u16, child: Arc<Mutex<OpenCodeChild>>) {
        self.sdk_server = Some(ManagedOpenCodeSdkServer { child, host, port });
    }

    pub fn get_sdk_server(&self) -> Option<ManagedOpenCodeSdkServer> {
        self.sdk_server.clone()
    }

    pub fn remove_sdk_server(&mut self) -> Option<ManagedOpenCodeSdkServer> {
        self.sdk_server.take()
    }

    pub fn remove_sdk_server_if_child(
        &mut self,
        child: &Arc<Mutex<OpenCodeChild>>,
    ) -> Option<ManagedOpenCodeSdkServer> {
        match self.sdk_server.as_ref() {
            Some(server) if Arc::ptr_eq(&server.child, child) => self.sdk_server.take(),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn has_employee_processes(&self, employee_id: &str) -> bool {
        self.processes
            .values()
            .any(|process| process.employee_id == employee_id)
    }

    pub fn has_unbound_employee_process(
        &self,
        employee_id: &str,
        session_kind: OpenCodeSessionKind,
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
        session_kind: OpenCodeSessionKind,
    ) -> Option<ManagedOpenCodeProcess> {
        self.processes
            .values()
            .find(|process| {
                process.task_id.as_deref() == Some(task_id) && process.session_kind == session_kind
            })
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use tokio::process::Command;

    use super::*;

    fn spawn_test_child() -> Arc<Mutex<OpenCodeChild>> {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 10")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Arc::new(Mutex::new(OpenCodeChild::new(
            command.spawn().expect("spawn test child"),
            None,
            None,
        )))
    }

    #[test]
    fn supports_multiple_task_sessions_for_same_employee() {
        tauri::async_runtime::block_on(async {
            let child_one = spawn_test_child();
            let child_two = spawn_test_child();

            let mut manager = OpenCodeManager::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                OpenCodeSessionKind::Execution,
                child_one.clone(),
                "session-1".to_string(),
                Vec::new(),
            );
            manager.add_process(
                "emp-1".to_string(),
                Some("task-2".to_string()),
                OpenCodeSessionKind::Execution,
                child_two.clone(),
                "session-2".to_string(),
                Vec::new(),
            );

            assert!(manager.has_employee_processes("emp-1"));
            assert_eq!(manager.get_employee_processes("emp-1").len(), 2);
            assert!(manager
                .get_task_process_any("task-1", OpenCodeSessionKind::Execution)
                .is_some());
            assert!(manager
                .get_task_process_any("task-2", OpenCodeSessionKind::Execution)
                .is_some());
            assert!(!manager.has_unbound_employee_process("emp-1", OpenCodeSessionKind::Execution));

            for child in [child_one, child_two] {
                let mut child = child.lock().await;
                let _ = child.kill_process_group().await;
                let _ = child.kill().await;
            }
        });
    }

    #[test]
    fn detects_only_unbound_employee_sessions() {
        tauri::async_runtime::block_on(async {
            let task_child = spawn_test_child();
            let unbound_child = spawn_test_child();

            let mut manager = OpenCodeManager::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                OpenCodeSessionKind::Execution,
                task_child.clone(),
                "session-1".to_string(),
                Vec::new(),
            );

            assert!(!manager.has_unbound_employee_process("emp-1", OpenCodeSessionKind::Execution));

            manager.add_process(
                "emp-1".to_string(),
                None,
                OpenCodeSessionKind::Execution,
                unbound_child.clone(),
                "session-2".to_string(),
                Vec::new(),
            );

            assert!(manager.has_unbound_employee_process("emp-1", OpenCodeSessionKind::Execution));

            for child in [task_child, unbound_child] {
                let mut child = child.lock().await;
                let _ = child.kill_process_group().await;
                let _ = child.kill().await;
            }
        });
    }
}
