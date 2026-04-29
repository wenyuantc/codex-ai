pub struct OpenCodeChild {
    child: tokio::process::Child,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
}

impl OpenCodeChild {
    pub fn new(
        child: tokio::process::Child,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
    ) -> Self {
        Self {
            child,
            stdout,
            stderr,
        }
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
        match self.child.kill().await {
            Ok(()) => Ok(()),
            Err(error) => match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(format!("终止 OpenCode 进程失败: {error}")),
            },
        }
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
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
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
