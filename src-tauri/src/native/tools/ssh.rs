use tauri::AppHandle;

use crate::app::{execute_ssh_command, execute_ssh_command_with_input, shell_escape_single_quoted};
use crate::db::models::SshConfigRecord;

use super::paths::resolve_under_workspace_posix;

#[derive(Clone)]
pub struct SshToolRuntime {
    pub app: AppHandle,
    pub config: SshConfigRecord,
    pub root: String,
}

impl SshToolRuntime {
    pub async fn read(&self, path: &str) -> Result<String, String> {
        let command = ssh_read_command(&self.root, path)?;
        stdout_or_err(execute_ssh_command(&self.app, &self.config, &command, true).await?)
    }

    pub async fn write(&self, path: &str, content: &str) -> Result<String, String> {
        let command = ssh_write_command(&self.root, path)?;
        let output = execute_ssh_command_with_input(
            &self.app,
            &self.config,
            &command,
            content.as_bytes(),
            true,
        )
        .await?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(format!("Wrote {} bytes to {path}", content.len()))
    }

    pub async fn glob(&self) -> Result<String, String> {
        let command = ssh_glob_command(&self.root)?;
        stdout_or_err(execute_ssh_command(&self.app, &self.config, &command, true).await?)
    }

    pub async fn grep(&self, pattern: &str, path: Option<&str>) -> Result<String, String> {
        let command = ssh_grep_command(&self.root, pattern, path)?;
        stdout_or_err(execute_ssh_command(&self.app, &self.config, &command, true).await?)
    }

    pub async fn bash(&self, command: &str) -> Result<String, String> {
        let status = self.bash_with_status(command).await?;
        if status.exit_code != 0 {
            return Err(if status.output.trim().is_empty() {
                format!("command failed: {}", status.exit_code)
            } else {
                status.output
            });
        }
        if status.output.trim().is_empty() {
            Ok("(no output)".to_string())
        } else {
            Ok(status.output)
        }
    }

    pub async fn bash_with_status(
        &self,
        command: &str,
    ) -> Result<super::local::CommandStatus, String> {
        let remote = ssh_bash_command(&self.root, command)?;
        let output = execute_ssh_command(&self.app, &self.config, &remote, true).await?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        Ok(super::local::CommandStatus {
            exit_code: output.status.code().unwrap_or(-1),
            output: text,
            timed_out: false,
        })
    }

    pub async fn delete(&self, path: &str) -> Result<String, String> {
        let command = ssh_delete_command(&self.root, path)?;
        stdout_or_err(execute_ssh_command(&self.app, &self.config, &command, true).await?)?;
        Ok(format!("Deleted {path}"))
    }
}

fn stdout_or_err(output: std::process::Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        if stdout.trim().is_empty() {
            Ok("(no output)".to_string())
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(if stderr.trim().is_empty() {
            stdout
        } else {
            stderr.into_owned()
        })
    }
}

pub fn ssh_read_command(root: &str, path: &str) -> Result<String, String> {
    let resolved = resolve_under_workspace_posix(root, path)?;
    Ok(format!("cat {}", shell_escape_single_quoted(&resolved)))
}

pub fn ssh_write_command(root: &str, path: &str) -> Result<String, String> {
    let resolved = resolve_under_workspace_posix(root, path)?;
    let parent = resolved
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .filter(|dir| !dir.is_empty())
        .unwrap_or("/");
    Ok(format!(
        "mkdir -p {} && cat > {}",
        shell_escape_single_quoted(parent),
        shell_escape_single_quoted(&resolved)
    ))
}

pub fn ssh_glob_command(root: &str) -> Result<String, String> {
    let root = resolve_under_workspace_posix(root, ".")?;
    Ok(format!(
        "cd {} && find . -type f | sed 's|^./||' | head -n 500",
        shell_escape_single_quoted(&root)
    ))
}

pub fn ssh_grep_command(root: &str, pattern: &str, path: Option<&str>) -> Result<String, String> {
    let target = match path {
        Some(value) => resolve_under_workspace_posix(root, value)?,
        None => resolve_under_workspace_posix(root, ".")?,
    };
    Ok(format!(
        "cd {} && (rg -n --no-heading {} . 2>/dev/null || grep -R -n {} .)",
        shell_escape_single_quoted(&target),
        shell_escape_single_quoted(pattern),
        shell_escape_single_quoted(pattern)
    ))
}

pub fn ssh_delete_command(root: &str, path: &str) -> Result<String, String> {
    let resolved = resolve_under_workspace_posix(root, path)?;
    Ok(format!("rm -f {}", shell_escape_single_quoted(&resolved)))
}

pub fn ssh_bash_command(root: &str, command: &str) -> Result<String, String> {
    let root = resolve_under_workspace_posix(root, ".")?;
    if command.trim().is_empty() {
        return Err("command 不能为空".to_string());
    }
    Ok(format!(
        "cd {} && bash -lc {}",
        shell_escape_single_quoted(&root),
        shell_escape_single_quoted(command)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_stay_inside_workspace() {
        let read = ssh_read_command("/proj", "src/a.rs").unwrap();
        assert!(read.contains("/proj/src/a.rs"));
        assert!(ssh_read_command("/proj", "../etc/passwd").is_err());
        let write = ssh_write_command("/proj", "src/a.rs").unwrap();
        assert!(write.contains("mkdir -p"));
        assert!(write.contains("cat >"));
        let bash = ssh_bash_command("/proj", "ls").unwrap();
        assert!(bash.contains("cd '/proj'"));
        assert!(bash.contains("bash -lc"));
        let delete = ssh_delete_command("/proj", "src/a.rs").unwrap();
        assert!(delete.contains("rm -f"));
        assert!(delete.contains("/proj/src/a.rs"));
        assert!(ssh_delete_command("/proj", "../etc/passwd").is_err());
        let glob = ssh_glob_command("/proj").unwrap();
        assert!(glob.contains("find . -type f"));
        let grep = ssh_grep_command("/proj", "TODO", Some("src")).unwrap();
        assert!(grep.contains("TODO"));
    }
}
