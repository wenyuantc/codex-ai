use tokio::process::Child;

pub struct ClaudeChild {
    child: Child,
}

impl ClaudeChild {
    pub fn new(child: Child) -> Self {
        Self { child }
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
                _ => Err(format!("发送 SIGTERM 到 Claude 进程组失败: {error}")),
            }
        }
    }

    #[cfg(not(unix))]
    pub fn kill_process_group(&mut self) -> Result<(), String> {
        match self.child.start_kill() {
            Ok(()) => Ok(()),
            Err(error) => match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(format!("终止 Claude 进程失败: {error}")),
            },
        }
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        match self.child.kill().await {
            Ok(()) => Ok(()),
            Err(error) => match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(format!("终止 Claude 进程失败: {error}")),
            },
        }
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("检查 Claude 进程状态失败: {error}"))
    }

    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.child.stderr.take()
    }
}
