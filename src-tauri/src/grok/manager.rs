use super::process::GrokSessionKind;
use crate::engine::manager::{ManagedProcess, ProcessManager};

pub type ManagedGrokProcess = ManagedProcess<GrokSessionKind>;
pub type GrokManager = ProcessManager<GrokSessionKind>;

#[cfg(test)]
mod tests {
    use std::process::Stdio;
    use std::sync::Arc;

    use tokio::process::Command;
    use tokio::sync::Mutex;

    use super::*;
    use crate::engine::EngineChild;

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

            let mut manager = GrokManager::new();
            manager.add_process(
                "emp-1".to_string(),
                Some("task-1".to_string()),
                GrokSessionKind::Execution,
                child_one.clone(),
                "session-1".to_string(),
                Vec::new(),
                (),
            );
            manager.add_process(
                "emp-1".to_string(),
                Some("task-2".to_string()),
                GrokSessionKind::Review,
                child_two.clone(),
                "session-2".to_string(),
                Vec::new(),
                (),
            );

            assert!(manager.has_employee_processes("emp-1"));
            assert_eq!(manager.get_employee_processes("emp-1").len(), 2);
            assert!(manager
                .get_task_process_any("task-1", GrokSessionKind::Execution)
                .is_some());
            assert!(manager
                .get_task_process_any("task-2", GrokSessionKind::Review)
                .is_some());

            for child in [child_one, child_two] {
                let mut child = child.lock().await;
                let _ = child.kill_process_group();
                let _ = child.kill().await;
            }
        });
    }
}
