use std::sync::Arc;

use tokio::sync::Mutex;

use super::super::manager::ManagedOpenCodeProcess;
use super::super::OpenCodeManager;

pub struct OpenCodeChild {
    child: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
}

impl OpenCodeChild {
    pub fn new(
        child: tokio::process::Child,
        stdin: Option<tokio::process::ChildStdin>,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
    ) -> Self {
        Self {
            child,
            stdin,
            stdout,
            stderr,
        }
    }

    pub fn child(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }

    pub fn stdin(&mut self) -> Option<&mut tokio::process::ChildStdin> {
        self.stdin.as_mut()
    }

    pub fn stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.stdout.take()
    }

    pub fn stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.stderr.take()
    }

    pub async fn kill_process_group(&mut self) -> Result<(), String> {
        kill_process_group_inner(&mut self.child).await
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|error| format!("终止 OpenCode 进程失败: {error}"))
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("检查 OpenCode 进程状态失败: {error}"))
    }
}

#[cfg(unix)]
async fn kill_process_group_inner(child: &mut tokio::process::Child) -> Result<(), String> {
    if let Some(pid) = child.id() {
        let pid = pid as i32;
        let result = unsafe { libc::kill(-pid, libc::SIGTERM) };
        if result != 0 {
            child
                .kill()
                .await
                .map_err(|error| format!("终止 OpenCode 进程组失败: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
async fn kill_process_group_inner(child: &mut tokio::process::Child) -> Result<(), String> {
    child
        .kill()
        .await
        .map_err(|error| format!("终止 OpenCode 进程失败: {error}"))
}

pub async fn get_live_opencode_process(
    state: &Arc<Mutex<OpenCodeManager>>,
    session_record_id: &str,
) -> Result<Option<ManagedOpenCodeProcess>, String> {
    let manager = state.lock().await;
    Ok(manager.get_process(session_record_id))
}
