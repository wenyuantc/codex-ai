use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::process::{OpenCodeChild, OpenCodeSessionKind};
use crate::engine::manager::{ManagedProcess, ProcessManager};

pub type ManagedOpenCodeProcess = ManagedProcess<OpenCodeSessionKind>;

#[derive(Clone)]
pub struct ManagedOpenCodeSdkServer {
    pub child: Arc<Mutex<OpenCodeChild>>,
    pub host: String,
    pub port: u16,
}

pub struct OpenCodeManager {
    inner: ProcessManager<OpenCodeSessionKind>,
    sdk_server: Option<ManagedOpenCodeSdkServer>,
}

impl OpenCodeManager {
    pub fn new() -> Self {
        Self {
            inner: ProcessManager::new(),
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
        self.inner.add_process(
            employee_id,
            task_id,
            session_kind,
            child,
            session_record_id,
            cleanup_paths,
            (),
        );
    }

    pub fn remove_process(&mut self, session_record_id: &str) -> Option<ManagedOpenCodeProcess> {
        self.inner.remove_process(session_record_id)
    }

    pub fn get_process(&self, session_record_id: &str) -> Option<ManagedOpenCodeProcess> {
        self.inner.get_process(session_record_id)
    }

    pub fn get_employee_processes(&self, employee_id: &str) -> Vec<ManagedOpenCodeProcess> {
        self.inner.get_employee_processes(employee_id)
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

    pub fn has_employee_processes(&self, employee_id: &str) -> bool {
        self.inner.has_employee_processes(employee_id)
    }

    pub fn has_unbound_employee_process(
        &self,
        employee_id: &str,
        session_kind: OpenCodeSessionKind,
    ) -> bool {
        self.inner
            .has_unbound_employee_process(employee_id, session_kind)
    }

    pub fn get_task_process_any(
        &self,
        task_id: &str,
        session_kind: OpenCodeSessionKind,
    ) -> Option<ManagedOpenCodeProcess> {
        self.inner.get_task_process_any(task_id, session_kind)
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
        Arc::new(Mutex::new(OpenCodeChild::with_stdio(
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
                let _ = child.kill_process_group();
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
                let _ = child.kill_process_group();
                let _ = child.kill().await;
            }
        });
    }
}
