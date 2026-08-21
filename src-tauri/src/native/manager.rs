use std::collections::HashMap;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::native::tools::CancelFlag;

#[derive(Debug, Clone)]
pub struct NativeSessionInfo {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: String,
    pub session_record_id: String,
}

pub enum NativeFollowup {
    Input(String),
    Finish,
}

pub struct NativeLiveSession {
    pub info: NativeSessionInfo,
    pub cancel: CancelFlag,
    pub followup_tx: mpsc::Sender<NativeFollowup>,
    pub join: JoinHandle<()>,
}

#[derive(Default)]
pub struct NativeAgentManager {
    sessions: HashMap<String, NativeLiveSession>,
}

impl NativeAgentManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_session(&mut self, session: NativeLiveSession) {
        self.sessions
            .insert(session.info.session_record_id.clone(), session);
    }

    pub fn remove_session(&mut self, session_record_id: &str) -> Option<NativeLiveSession> {
        self.sessions.remove(session_record_id)
    }

    pub fn get_session(&self, session_record_id: &str) -> Option<&NativeLiveSession> {
        self.sessions.get(session_record_id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn has_employee_processes(&self, employee_id: &str) -> bool {
        self.sessions
            .values()
            .any(|session| session.info.employee_id == employee_id)
    }

    pub fn get_employee_processes(&self, employee_id: &str) -> Vec<NativeSessionInfo> {
        self.sessions
            .values()
            .filter(|session| session.info.employee_id == employee_id)
            .map(|session| session.info.clone())
            .collect()
    }

    pub fn get_task_process_any(
        &self,
        task_id: &str,
        session_kind: &str,
    ) -> Option<NativeSessionInfo> {
        self.sessions
            .values()
            .find(|session| {
                session.info.task_id.as_deref() == Some(task_id)
                    && session.info.session_kind == session_kind
            })
            .map(|session| session.info.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tracks_employee_and_task_sessions() {
        let mut manager = NativeAgentManager::new();
        let (tx, _rx) = mpsc::channel(1);
        manager.add_session(NativeLiveSession {
            info: NativeSessionInfo {
                employee_id: "emp-1".to_string(),
                task_id: Some("task-1".to_string()),
                session_kind: "execution".to_string(),
                session_record_id: "sess-1".to_string(),
            },
            cancel: CancelFlag::new(),
            followup_tx: tx,
            join: tokio::spawn(async {}),
        });
        assert!(manager.has_employee_processes("emp-1"));
        assert!(manager
            .get_task_process_any("task-1", "execution")
            .is_some());
        assert_eq!(manager.len(), 1);
        manager.remove_session("sess-1");
        assert_eq!(manager.len(), 0);
    }
}
