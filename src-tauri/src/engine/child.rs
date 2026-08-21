use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};

use super::stdin::{
    encode_await_followups_control, encode_session_followup_input, write_stdin_bytes,
};

/// Shared subprocess handle for AI engine sessions (local or SSH-wrapped).
pub struct EngineChild {
    child: Child,
    /// Optional pre-taken pipes (OpenCode may detach stdio at spawn time).
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    /// Kept open for mid-session follow-up input on interactive SDK bridges.
    stdin: Option<ChildStdin>,
}

/// Minimal process-handle contract shared by engine children.
///
/// Kept for cross-engine polymorphism; concrete call sites still use
/// [`EngineChild`] methods directly until broader trait adoption.
#[allow(dead_code)]
pub trait EngineProcessHandle {
    fn kill_process_group(&mut self) -> Result<(), String>;
    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String>;
    fn take_stdout(&mut self) -> Option<ChildStdout>;
    fn take_stderr(&mut self) -> Option<ChildStderr>;
}

impl EngineChild {
    pub fn new(child: Child) -> Self {
        Self {
            child,
            stdout: None,
            stderr: None,
            stdin: None,
        }
    }

    /// Construct with stdio handles already taken from the child (OpenCode pattern).
    pub fn with_stdio(
        child: Child,
        stdout: Option<ChildStdout>,
        stderr: Option<ChildStderr>,
    ) -> Self {
        Self {
            child,
            stdout,
            stderr,
            stdin: None,
        }
    }

    /// Construct with stdout/stderr/stdin already taken from the child.
    pub fn with_stdio_and_stdin(
        child: Child,
        stdout: Option<ChildStdout>,
        stderr: Option<ChildStderr>,
        stdin: Option<ChildStdin>,
    ) -> Self {
        Self {
            child,
            stdout,
            stderr,
            stdin,
        }
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.stdin.take().or_else(|| self.child.stdin.take())
    }

    pub fn has_stdin(&self) -> bool {
        self.stdin.is_some() || self.child.stdin.is_some()
    }

    /// Write a framed follow-up input line without closing stdin.
    pub async fn write_followup_input(&mut self, input: &str) -> Result<(), String> {
        let bytes = encode_session_followup_input(input)?;
        let stdin = self
            .stdin
            .as_mut()
            .or(self.child.stdin.as_mut())
            .ok_or_else(|| "当前会话没有可写的 stdin（非交互通道）".to_string())?;
        write_stdin_bytes(stdin, &bytes).await
    }

    /// Toggle post-turn wait-for-input on the live SDK bridge (terminal log open/close).
    pub async fn write_await_followups_control(&mut self, enabled: bool) -> Result<(), String> {
        let bytes = encode_await_followups_control(enabled)?;
        let stdin = self
            .stdin
            .as_mut()
            .or(self.child.stdin.as_mut())
            .ok_or_else(|| "当前会话没有可写的 stdin（非交互通道）".to_string())?;
        write_stdin_bytes(stdin, &bytes).await
    }

    /// Close the retained stdin pipe (signals EOF to interactive bridges).
    pub async fn close_stdin(&mut self) -> Result<(), String> {
        if let Some(mut stdin) = self.take_stdin() {
            stdin
                .shutdown()
                .await
                .map_err(|error| format!("关闭会话 stdin 失败: {error}"))?;
        }
        Ok(())
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
        let _ = self.close_stdin().await;
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

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take().or_else(|| self.child.stdout.take())
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.stderr.take().or_else(|| self.child.stderr.take())
    }

    /// OpenCode historical alias for [`Self::take_stdout`].
    pub fn stdout(&mut self) -> Option<ChildStdout> {
        self.take_stdout()
    }

    /// OpenCode historical alias for [`Self::take_stderr`].
    pub fn stderr(&mut self) -> Option<ChildStderr> {
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

    fn take_stdout(&mut self) -> Option<ChildStdout> {
        EngineChild::take_stdout(self)
    }

    fn take_stderr(&mut self) -> Option<ChildStderr> {
        EngineChild::take_stderr(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
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

    #[test]
    fn retained_stdin_accepts_followup_writes() {
        tauri::async_runtime::block_on(async {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg("IFS= read -r line; printf '%s\\n' \"$line\"")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = command.spawn().expect("spawn");
            let stdin = child.stdin.take().expect("stdin");
            let stdout = child.stdout.take().expect("stdout");
            let mut engine = EngineChild::with_stdio_and_stdin(child, None, None, Some(stdin));
            assert!(engine.has_stdin());
            engine
                .write_followup_input("ping from kernel")
                .await
                .expect("write");
            engine.close_stdin().await.expect("close");

            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("read");
            assert!(line.contains("ping from kernel"), "got {line:?}");

            for _ in 0..50 {
                if engine.try_wait().expect("try_wait").is_some() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        });
    }

    #[test]
    fn write_followup_without_stdin_fails_clearly() {
        tauri::async_runtime::block_on(async {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg("true")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut engine = EngineChild::new(command.spawn().expect("spawn"));
            let error = engine
                .write_followup_input("nope")
                .await
                .expect_err("should fail");
            assert!(error.contains("stdin") || error.contains("非交互"));
            let _ = engine.kill().await;
        });
    }
}
