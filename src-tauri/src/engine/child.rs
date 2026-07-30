use tokio::process::Child;

/// Shared subprocess handle for AI engine sessions (local or SSH-wrapped).
pub struct EngineChild {
    child: Child,
    /// Optional pre-taken pipes (OpenCode may detach stdio at spawn time).
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
}

/// Minimal process-handle contract shared by engine children.
pub trait EngineProcessHandle {
    fn kill_process_group(&mut self) -> Result<(), String>;
    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String>;
    fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout>;
    fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr>;
}

impl EngineChild {
    pub fn new(child: Child) -> Self {
        Self {
            child,
            stdout: None,
            stderr: None,
        }
    }

    /// Construct with stdio handles already taken from the child (OpenCode pattern).
    pub fn with_stdio(
        child: Child,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
    ) -> Self {
        Self {
            child,
            stdout,
            stderr,
        }
    }

    #[cfg(unix)]
    pub fn kill_process_group(&mut self) -> Result<(), String> {
        let Some(pid) = self.child.id() else {
            return Ok(());
        };

        let result = unsafe { libc::killpg(pid as i32, libc::SIGTERM) };
        if result == 0 {
            Ok(())
        } else {
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(format!("发送 SIGTERM 到 AI 引擎进程组失败: {error}")),
            }
        }
    }

    #[cfg(not(unix))]
    pub fn kill_process_group(&mut self) -> Result<(), String> {
        match self.child.start_kill() {
            Ok(()) => Ok(()),
            Err(error) => match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(format!("终止 AI 引擎进程失败: {error}")),
            },
        }
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        match self.child.kill().await {
            Ok(()) => Ok(()),
            Err(error) => match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(format!("终止 AI 引擎进程失败: {error}")),
            },
        }
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("检查 AI 引擎进程状态失败: {error}"))
    }

    /// Convenience for callers that previously returned `Option<i32>` exit codes.
    pub fn try_wait_code(&mut self) -> Result<Option<i32>, String> {
        Ok(self.try_wait()?.and_then(|status| status.code()))
    }

    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.stdout.take().or_else(|| self.child.stdout.take())
    }

    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.stderr.take().or_else(|| self.child.stderr.take())
    }

    /// OpenCode historical alias for [`Self::take_stdout`].
    pub fn stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.take_stdout()
    }

    /// OpenCode historical alias for [`Self::take_stderr`].
    pub fn stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.take_stderr()
    }
}

impl EngineProcessHandle for EngineChild {
    fn kill_process_group(&mut self) -> Result<(), String> {
        EngineChild::kill_process_group(self)
    }

    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        EngineChild::try_wait(self)
    }

    fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        EngineChild::take_stdout(self)
    }

    fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        EngineChild::take_stderr(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tokio::process::Command;

    #[test]
    fn process_handle_trait_covers_engine_child() {
        tauri::async_runtime::block_on(async {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg("true")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = EngineChild::new(command.spawn().expect("spawn"));
            let handle: &mut dyn EngineProcessHandle = &mut child;
            let _ = handle.take_stdout();
            let _ = handle.take_stderr();
            // Wait for exit so try_wait can observe completion.
            for _ in 0..50 {
                if handle.try_wait().expect("try_wait").is_some() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            let _ = handle.kill_process_group();
            let _ = child.kill().await;
        });
    }
}
