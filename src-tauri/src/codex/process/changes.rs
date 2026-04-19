use super::*;

pub(super) fn run_git_bytes(repo_path: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = std::process::Command::new("git");
    configure_std_command(&mut command);
    let output = command
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("执行 git {:?} 失败: {}", args, error))?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub(super) fn hash_worktree_path(
    repo_path: &str,
    relative_path: &str,
) -> Result<Option<String>, String> {
    let target = Path::new(repo_path).join(relative_path);
    if !target.exists() {
        return Ok(None);
    }

    let mut command = std::process::Command::new("git");
    configure_std_command(&mut command);
    let output = command
        .arg("-C")
        .arg(repo_path)
        .arg("hash-object")
        .arg("--no-filters")
        .arg("--")
        .arg(relative_path)
        .output()
        .map_err(|error| format!("计算文件哈希失败: path={}, error={}", relative_path, error))?;

    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    } else {
        Err(format!(
            "计算文件哈希失败: path={}, error={}",
            relative_path,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn capture_text_snapshot_from_bytes(bytes: &[u8], truncated_hint: bool) -> TextSnapshot {
    if bytes.contains(&0) {
        return TextSnapshot {
            status: TextSnapshotStatus::Binary,
            text: None,
            truncated: false,
        };
    }

    let mut end = bytes.len();
    while end > 0 && std::str::from_utf8(&bytes[..end]).is_err() {
        end -= 1;
    }

    if end == 0 && !bytes.is_empty() {
        return TextSnapshot {
            status: TextSnapshotStatus::Binary,
            text: None,
            truncated: false,
        };
    }

    match std::str::from_utf8(&bytes[..end]) {
        Ok(text) => TextSnapshot {
            status: TextSnapshotStatus::Text,
            text: Some(text.to_string()),
            truncated: truncated_hint || end < bytes.len(),
        },
        Err(_) => TextSnapshot {
            status: TextSnapshotStatus::Binary,
            text: None,
            truncated: false,
        },
    }
}

fn capture_worktree_text_snapshot(repo_path: &str, relative_path: &str) -> TextSnapshot {
    let target = Path::new(repo_path).join(relative_path);
    let metadata = match fs::metadata(&target) {
        Ok(metadata) => metadata,
        Err(_) => return TextSnapshot::missing(),
    };

    if !metadata.is_file() {
        return TextSnapshot {
            status: TextSnapshotStatus::Unavailable,
            text: None,
            truncated: false,
        };
    }

    let max_read = metadata
        .len()
        .min(FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT.saturating_add(4)) as usize;
    let file = match fs::File::open(&target) {
        Ok(file) => file,
        Err(_) => {
            return TextSnapshot {
                status: TextSnapshotStatus::Unavailable,
                text: None,
                truncated: false,
            };
        }
    };
    let mut buffer = Vec::with_capacity(max_read);
    if file
        .take(FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT.saturating_add(4))
        .read_to_end(&mut buffer)
        .is_err()
    {
        return TextSnapshot {
            status: TextSnapshotStatus::Unavailable,
            text: None,
            truncated: false,
        };
    }

    capture_text_snapshot_from_bytes(
        &buffer,
        metadata.len() > FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT,
    )
}

fn capture_git_head_text_snapshot(repo_path: &str, relative_path: &str) -> TextSnapshot {
    match run_git_bytes(repo_path, &["show", &format!("HEAD:{relative_path}")]) {
        Ok(bytes) => capture_text_snapshot_from_bytes(
            &bytes[..bytes
                .len()
                .min(FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT.saturating_add(4) as usize)],
            bytes.len() as u64 > FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT,
        ),
        Err(_) => TextSnapshot::missing(),
    }
}

fn should_read_previous_path(status_x: char, status_y: char) -> bool {
    matches!(status_x, 'R' | 'C') || matches!(status_y, 'R' | 'C')
}

fn entry_is_renamed(entry: &WorkingTreeSnapshotEntry) -> bool {
    matches!(entry.status_x, 'R') || matches!(entry.status_y, 'R')
}

fn entry_is_deleted(entry: &WorkingTreeSnapshotEntry) -> bool {
    matches!(entry.status_x, 'D') || matches!(entry.status_y, 'D')
}

fn entry_is_added(entry: &WorkingTreeSnapshotEntry) -> bool {
    matches!(entry.status_x, 'A' | '?') || matches!(entry.status_y, 'A' | '?')
}

fn entries_have_same_change_identity(
    left: &WorkingTreeSnapshotEntry,
    right: &WorkingTreeSnapshotEntry,
) -> bool {
    left.path == right.path
        && left.previous_path == right.previous_path
        && left.status_x == right.status_x
        && left.status_y == right.status_y
        && left.content_hash == right.content_hash
}

pub(super) fn capture_execution_change_baseline(
    repo_path: &str,
) -> Result<ExecutionChangeBaseline, String> {
    Ok(ExecutionChangeBaseline {
        repo_path: repo_path.to_string(),
        execution_target: EXECUTION_TARGET_LOCAL.to_string(),
        ssh_config_id: None,
        entries: collect_working_tree_snapshot_entries(repo_path, true)?,
    })
}

pub(super) fn should_capture_execution_change_baseline(
    session_kind: CodexSessionKind,
    _execution_target: &str,
) -> bool {
    session_kind == CodexSessionKind::Execution
}

fn collect_working_tree_snapshot_entries(
    repo_path: &str,
    capture_text_snapshots: bool,
) -> Result<HashMap<String, WorkingTreeSnapshotEntry>, String> {
    let output = run_git_bytes(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let parts = output.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut entries = HashMap::new();
    let mut index = 0usize;

    while index < parts.len() {
        let segment = parts[index];
        index += 1;

        if segment.is_empty() {
            continue;
        }

        if segment.len() < 4 {
            return Err(format!(
                "无法解析 git status 输出片段: {:?}",
                String::from_utf8_lossy(segment)
            ));
        }

        let status_x = segment[0] as char;
        let status_y = segment[1] as char;
        let path = String::from_utf8_lossy(&segment[3..]).to_string();
        let previous_path = if should_read_previous_path(status_x, status_y) {
            let original_segment = parts
                .get(index)
                .ok_or_else(|| format!("git status 缺少重命名原路径: {}", path))?;
            index += 1;
            Some(String::from_utf8_lossy(original_segment).to_string())
        } else {
            None
        };
        let content_hash = hash_worktree_path(repo_path, &path)?;
        let text_snapshot = if capture_text_snapshots {
            capture_worktree_text_snapshot(repo_path, &path)
        } else {
            TextSnapshot::missing()
        };

        entries.insert(
            path.clone(),
            WorkingTreeSnapshotEntry {
                path,
                previous_path,
                status_x,
                status_y,
                content_hash,
                text_snapshot,
            },
        );
    }

    Ok(entries)
}

fn classify_new_entry_change_kind(entry: &WorkingTreeSnapshotEntry) -> SessionFileChangeKind {
    if entry_is_renamed(entry) {
        SessionFileChangeKind::Renamed
    } else if entry_is_deleted(entry) {
        SessionFileChangeKind::Deleted
    } else if entry_is_added(entry) {
        SessionFileChangeKind::Added
    } else {
        SessionFileChangeKind::Modified
    }
}

fn build_session_file_change(
    path: String,
    change_kind: SessionFileChangeKind,
    capture_mode: &str,
    previous_path: Option<String>,
) -> CodexSessionFileChangeInput {
    CodexSessionFileChangeInput {
        path,
        change_type: change_kind.as_str().to_string(),
        capture_mode: capture_mode.to_string(),
        previous_path,
        detail: None,
    }
}

fn normalize_repo_relative_path_string(value: &str) -> String {
    normalize_runtime_path_string(value)
        .trim()
        .trim_start_matches("./")
        .replace('\\', "/")
}

fn normalize_session_change_path(repo_path: &str, value: &str) -> String {
    let normalized = normalize_runtime_path_string(value).trim().to_string();
    if normalized.is_empty() {
        return normalized;
    }

    let repo_root = Path::new(repo_path);
    let candidate = Path::new(&normalized);
    if candidate.is_absolute() {
        if let Ok(relative) = candidate.strip_prefix(repo_root) {
            return normalize_repo_relative_path_string(relative.to_string_lossy().as_ref());
        }
    }

    normalize_repo_relative_path_string(&normalized)
}

pub(super) fn normalize_session_file_change_paths(
    repo_path: &str,
    mut change: CodexSessionFileChangeInput,
) -> CodexSessionFileChangeInput {
    change.path = normalize_session_change_path(repo_path, &change.path);
    change.previous_path = change
        .previous_path
        .as_deref()
        .map(|value| normalize_session_change_path(repo_path, value))
        .filter(|value| !value.is_empty());
    change
}

fn parse_git_status_stdout_to_session_changes(
    repo_path: &str,
    stdout: &[u8],
    capture_mode: &str,
) -> Result<Vec<CodexSessionFileChangeInput>, String> {
    let parts = stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut index = 0usize;
    let mut changes = Vec::new();

    while index < parts.len() {
        let segment = parts[index];
        index += 1;

        if segment.is_empty() {
            continue;
        }
        if segment.len() < 4 {
            return Err(format!(
                "无法解析 git status 输出片段: {:?}",
                String::from_utf8_lossy(segment)
            ));
        }

        let status_x = segment[0] as char;
        let status_y = segment[1] as char;
        let path = String::from_utf8_lossy(&segment[3..]).to_string();
        let previous_path = if should_read_previous_path(status_x, status_y) {
            let original_segment = parts
                .get(index)
                .ok_or_else(|| format!("git status 缺少重命名原路径: {}", path))?;
            index += 1;
            Some(String::from_utf8_lossy(original_segment).to_string())
        } else {
            None
        };

        let entry = WorkingTreeSnapshotEntry {
            path: path.clone(),
            previous_path: previous_path.clone(),
            status_x,
            status_y,
            content_hash: None,
            text_snapshot: TextSnapshot::missing(),
        };
        changes.push(normalize_session_file_change_paths(
            repo_path,
            build_session_file_change(
                path,
                classify_new_entry_change_kind(&entry),
                capture_mode,
                previous_path.filter(|_| entry_is_renamed(&entry)),
            ),
        ));
    }

    Ok(changes)
}

fn is_remote_absolute_path(value: &str) -> bool {
    let normalized = normalize_runtime_path_string(value);
    let trimmed = normalized.trim();
    Path::new(trimmed).is_absolute()
        || trimmed == "~"
        || trimmed.starts_with("~/")
        || trimmed.starts_with("$HOME/")
        || trimmed.starts_with("${HOME}/")
}

async fn run_remote_shell_output<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    script: &str,
) -> Result<std::process::Output, String> {
    execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command(script, None),
        true,
    )
    .await
}

fn remote_snapshot_head_limit() -> u64 {
    FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT.saturating_add(4)
}

pub(super) async fn resolve_remote_working_dir<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
) -> Result<String, String> {
    let output = run_remote_shell_output(
        app,
        ssh_config,
        &format!("cd {} && pwd", remote_shell_path_expression(working_dir)),
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("解析远程工作目录失败：{working_dir}")
        } else {
            format!("解析远程工作目录失败：{stderr}")
        });
    }

    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if resolved.is_empty() {
        Err("解析远程工作目录失败：命令未返回路径".to_string())
    } else {
        Ok(normalize_runtime_path_string(&resolved))
    }
}

async fn hash_remote_worktree_path<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
    path: &str,
) -> Result<Option<String>, String> {
    let normalized_path = normalize_runtime_path_string(path);
    let is_absolute = is_remote_absolute_path(&normalized_path);
    let path_expr = if is_absolute {
        remote_shell_path_expression(&normalized_path)
    } else {
        shell_escape_arg(&normalized_path)
    };
    let command = if is_absolute {
        format!(
            "if [ ! -e {path_expr} ]; then exit {missing}; fi; git hash-object --no-filters -- {path_expr}",
            missing = REMOTE_SNAPSHOT_MISSING_EXIT_CODE,
        )
    } else {
        format!(
            "cd {working_dir} && if [ ! -e {path_expr} ]; then exit {missing}; fi; git hash-object --no-filters -- {path_expr}",
            working_dir = remote_shell_path_expression(working_dir),
            missing = REMOTE_SNAPSHOT_MISSING_EXIT_CODE,
        )
    };

    let output = run_remote_shell_output(app, ssh_config, &command).await?;
    if output.status.success() {
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.is_empty() {
            Err(format!("远程文件哈希为空：{path}"))
        } else {
            Ok(Some(hash))
        }
    } else if output.status.code() == Some(REMOTE_SNAPSHOT_MISSING_EXIT_CODE) {
        Ok(None)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

async fn capture_remote_worktree_text_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
    path: &str,
) -> TextSnapshot {
    let normalized_path = normalize_runtime_path_string(path);
    let is_absolute = is_remote_absolute_path(&normalized_path);
    let path_expr = if is_absolute {
        remote_shell_path_expression(&normalized_path)
    } else {
        shell_escape_arg(&normalized_path)
    };
    let command = if is_absolute {
        format!(
            "if [ ! -e {path_expr} ]; then exit {missing}; fi; if [ ! -f {path_expr} ]; then exit {unavailable}; fi; head -c {limit} < {path_expr}",
            missing = REMOTE_SNAPSHOT_MISSING_EXIT_CODE,
            unavailable = REMOTE_SNAPSHOT_UNAVAILABLE_EXIT_CODE,
            limit = remote_snapshot_head_limit(),
        )
    } else {
        format!(
            "cd {working_dir} && if [ ! -e {path_expr} ]; then exit {missing}; fi; if [ ! -f {path_expr} ]; then exit {unavailable}; fi; head -c {limit} < {path_expr}",
            working_dir = remote_shell_path_expression(working_dir),
            missing = REMOTE_SNAPSHOT_MISSING_EXIT_CODE,
            unavailable = REMOTE_SNAPSHOT_UNAVAILABLE_EXIT_CODE,
            limit = remote_snapshot_head_limit(),
        )
    };

    let output = match run_remote_shell_output(app, ssh_config, &command).await {
        Ok(output) => output,
        Err(_) => return TextSnapshot::unavailable(),
    };
    if output.status.success() {
        capture_text_snapshot_from_bytes(
            &output.stdout,
            output.stdout.len() as u64 > FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT,
        )
    } else if output.status.code() == Some(REMOTE_SNAPSHOT_MISSING_EXIT_CODE) {
        TextSnapshot::missing()
    } else {
        TextSnapshot::unavailable()
    }
}

async fn capture_remote_git_head_text_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
    path: &str,
) -> TextSnapshot {
    let normalized_path = normalize_runtime_path_string(path);
    if is_remote_absolute_path(&normalized_path) {
        return TextSnapshot::missing();
    }

    let object_expr = shell_escape_arg(&format!("HEAD:{normalized_path}"));
    let command = format!(
        "cd {working_dir} && git cat-file -e {object_expr} >/dev/null 2>&1 || exit {missing}; git cat-file -p {object_expr} 2>/dev/null | head -c {limit}",
        working_dir = remote_shell_path_expression(working_dir),
        missing = REMOTE_SNAPSHOT_MISSING_EXIT_CODE,
        limit = remote_snapshot_head_limit(),
    );
    let output = match run_remote_shell_output(app, ssh_config, &command).await {
        Ok(output) => output,
        Err(_) => return TextSnapshot::missing(),
    };
    if output.status.success() {
        capture_text_snapshot_from_bytes(
            &output.stdout,
            output.stdout.len() as u64 > FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT,
        )
    } else {
        TextSnapshot::missing()
    }
}

async fn collect_remote_working_tree_snapshot_entries<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
    capture_text_snapshots: bool,
) -> Result<HashMap<String, WorkingTreeSnapshotEntry>, String> {
    let output = run_remote_shell_output(
        app,
        ssh_config,
        &format!(
            "git -C {} status --porcelain=v1 -z --untracked-files=all",
            remote_shell_path_expression(working_dir)
        ),
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "远程 git status 采集失败".to_string()
        } else {
            format!("远程 git status 采集失败：{stderr}")
        });
    }

    let parts = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut entries = HashMap::new();
    let mut index = 0usize;

    while index < parts.len() {
        let segment = parts[index];
        index += 1;

        if segment.is_empty() {
            continue;
        }

        if segment.len() < 4 {
            return Err(format!(
                "无法解析远程 git status 输出片段: {:?}",
                String::from_utf8_lossy(segment)
            ));
        }

        let status_x = segment[0] as char;
        let status_y = segment[1] as char;
        let path = String::from_utf8_lossy(&segment[3..]).to_string();
        let previous_path = if should_read_previous_path(status_x, status_y) {
            let original_segment = parts
                .get(index)
                .ok_or_else(|| format!("远程 git status 缺少重命名原路径: {}", path))?;
            index += 1;
            Some(String::from_utf8_lossy(original_segment).to_string())
        } else {
            None
        };
        let content_hash = hash_remote_worktree_path(app, ssh_config, working_dir, &path).await?;
        let text_snapshot = if capture_text_snapshots {
            capture_remote_worktree_text_snapshot(app, ssh_config, working_dir, &path).await
        } else {
            TextSnapshot::missing()
        };

        entries.insert(
            path.clone(),
            WorkingTreeSnapshotEntry {
                path,
                previous_path,
                status_x,
                status_y,
                content_hash,
                text_snapshot,
            },
        );
    }

    Ok(entries)
}

pub(super) async fn capture_remote_execution_change_baseline<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
) -> Result<ExecutionChangeBaseline, String> {
    let resolved_working_dir = resolve_remote_working_dir(app, ssh_config, working_dir).await?;
    Ok(ExecutionChangeBaseline {
        repo_path: resolved_working_dir.clone(),
        execution_target: EXECUTION_TARGET_SSH.to_string(),
        ssh_config_id: Some(ssh_config.id.clone()),
        entries: collect_remote_working_tree_snapshot_entries(
            app,
            ssh_config,
            &resolved_working_dir,
            true,
        )
        .await?,
    })
}

pub(super) async fn capture_remote_git_status_changes<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
) -> Result<Vec<CodexSessionFileChangeInput>, String> {
    let output = run_remote_shell_output(
        app,
        ssh_config,
        &format!(
            "git -C {} status --porcelain=v1 -z --untracked-files=all",
            remote_shell_path_expression(working_dir)
        ),
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "远程 git status 采集失败".to_string()
        } else {
            format!("远程 git status 采集失败: {stderr}")
        });
    }
    parse_git_status_stdout_to_session_changes(
        working_dir,
        &output.stdout,
        ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS,
    )
}

async fn build_remote_session_file_change_detail<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
    baseline_entries: &HashMap<String, WorkingTreeSnapshotEntry>,
    change_kind: SessionFileChangeKind,
    path: &str,
    previous_path: Option<&str>,
) -> CodexSessionFileChangeDetailInput {
    let before_path = previous_path.unwrap_or(path);
    let before_snapshot = if change_kind == SessionFileChangeKind::Added {
        TextSnapshot::missing()
    } else if let Some(entry) = baseline_entries.get(before_path) {
        entry.text_snapshot.clone()
    } else {
        capture_remote_git_head_text_snapshot(app, ssh_config, working_dir, before_path).await
    };

    let after_snapshot = if change_kind == SessionFileChangeKind::Deleted {
        TextSnapshot::missing()
    } else {
        capture_remote_worktree_text_snapshot(app, ssh_config, working_dir, path).await
    };

    CodexSessionFileChangeDetailInput {
        absolute_path: Some(path_to_runtime_string(&Path::new(working_dir).join(path))),
        previous_absolute_path: previous_path
            .map(|value| path_to_runtime_string(&Path::new(working_dir).join(value))),
        before_status: before_snapshot.status.as_str().to_string(),
        before_text: before_snapshot.text,
        before_truncated: before_snapshot.truncated,
        after_status: after_snapshot.status.as_str().to_string(),
        after_text: after_snapshot.text,
        after_truncated: after_snapshot.truncated,
    }
}

pub(super) async fn attach_remote_session_file_change_details<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    working_dir: &str,
    baseline_entries: &HashMap<String, WorkingTreeSnapshotEntry>,
    changes: Vec<CodexSessionFileChangeInput>,
) -> Vec<CodexSessionFileChangeInput> {
    let mut detailed_changes = Vec::with_capacity(changes.len());

    for change in changes {
        let mut change = normalize_session_file_change_paths(working_dir, change);
        let change_kind = normalize_session_file_change_kind(Some(change.change_type.as_str()))
            .unwrap_or(SessionFileChangeKind::Modified);
        change.detail = Some(
            build_remote_session_file_change_detail(
                app,
                ssh_config,
                working_dir,
                baseline_entries,
                change_kind,
                &change.path,
                change.previous_path.as_deref(),
            )
            .await,
        );
        detailed_changes.push(change);
    }

    detailed_changes
}

pub(super) async fn compute_remote_execution_session_file_changes<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    baseline: &ExecutionChangeBaseline,
) -> Result<Vec<CodexSessionFileChangeInput>, String> {
    let end_entries =
        collect_remote_working_tree_snapshot_entries(app, ssh_config, &baseline.repo_path, false)
            .await?;
    let rename_sources = end_entries
        .values()
        .filter(|entry| entry_is_renamed(entry))
        .filter_map(|entry| entry.previous_path.clone())
        .collect::<HashSet<_>>();
    let mut consumed_baseline = HashSet::new();
    let mut changes = Vec::new();

    let mut end_paths = end_entries.keys().cloned().collect::<Vec<_>>();
    end_paths.sort();

    for path in end_paths {
        let entry = end_entries
            .get(&path)
            .expect("end entry should exist for collected key");

        match baseline.entries.get(&path) {
            None => {
                if let Some(previous_path) = entry.previous_path.as_ref() {
                    consumed_baseline.insert(previous_path.clone());
                }
                changes.push(build_session_file_change(
                    path,
                    classify_new_entry_change_kind(entry),
                    CodexExecutionProvider::Cli.capture_mode(),
                    entry
                        .previous_path
                        .clone()
                        .filter(|_| entry_is_renamed(entry)),
                ));
            }
            Some(baseline_entry) => {
                consumed_baseline.insert(path.clone());
                if entries_have_same_change_identity(baseline_entry, entry) {
                    continue;
                }

                let change_kind = if entry_is_renamed(entry)
                    && baseline_entry.previous_path != entry.previous_path
                {
                    SessionFileChangeKind::Renamed
                } else if entry_is_deleted(entry) {
                    SessionFileChangeKind::Deleted
                } else {
                    SessionFileChangeKind::Modified
                };

                changes.push(build_session_file_change(
                    path,
                    change_kind,
                    CodexExecutionProvider::Cli.capture_mode(),
                    entry
                        .previous_path
                        .clone()
                        .filter(|_| change_kind == SessionFileChangeKind::Renamed),
                ));
            }
        }
    }

    let mut baseline_paths = baseline.entries.keys().cloned().collect::<Vec<_>>();
    baseline_paths.sort();

    for path in baseline_paths {
        if consumed_baseline.contains(&path) || rename_sources.contains(&path) {
            continue;
        }

        let baseline_entry = baseline
            .entries
            .get(&path)
            .expect("baseline entry should exist for collected key");
        let current_hash =
            hash_remote_worktree_path(app, ssh_config, &baseline.repo_path, &path).await?;
        if current_hash == baseline_entry.content_hash {
            continue;
        }

        let change_kind = if current_hash.is_none() {
            SessionFileChangeKind::Deleted
        } else {
            SessionFileChangeKind::Modified
        };
        changes.push(build_session_file_change(
            path,
            change_kind,
            CodexExecutionProvider::Cli.capture_mode(),
            None,
        ));
    }

    Ok(attach_remote_session_file_change_details(
        app,
        ssh_config,
        &baseline.repo_path,
        &baseline.entries,
        changes,
    )
    .await)
}

fn build_session_file_change_detail(
    repo_path: &str,
    baseline_entries: &HashMap<String, WorkingTreeSnapshotEntry>,
    change_kind: SessionFileChangeKind,
    path: &str,
    previous_path: Option<&str>,
) -> CodexSessionFileChangeDetailInput {
    let before_path = previous_path.unwrap_or(path);
    let before_snapshot = if change_kind == SessionFileChangeKind::Added {
        TextSnapshot::missing()
    } else if let Some(entry) = baseline_entries.get(before_path) {
        entry.text_snapshot.clone()
    } else {
        capture_git_head_text_snapshot(repo_path, before_path)
    };

    let after_snapshot = if change_kind == SessionFileChangeKind::Deleted {
        TextSnapshot::missing()
    } else {
        capture_worktree_text_snapshot(repo_path, path)
    };

    CodexSessionFileChangeDetailInput {
        absolute_path: Some(path_to_runtime_string(&Path::new(repo_path).join(path))),
        previous_absolute_path: previous_path
            .map(|value| path_to_runtime_string(&Path::new(repo_path).join(value))),
        before_status: before_snapshot.status.as_str().to_string(),
        before_text: before_snapshot.text,
        before_truncated: before_snapshot.truncated,
        after_status: after_snapshot.status.as_str().to_string(),
        after_text: after_snapshot.text,
        after_truncated: after_snapshot.truncated,
    }
}

pub(super) fn attach_session_file_change_details(
    repo_path: &str,
    baseline_entries: &HashMap<String, WorkingTreeSnapshotEntry>,
    changes: Vec<CodexSessionFileChangeInput>,
) -> Vec<CodexSessionFileChangeInput> {
    changes
        .into_iter()
        .map(|change| {
            let mut change = normalize_session_file_change_paths(repo_path, change);
            let change_kind = normalize_session_file_change_kind(Some(change.change_type.as_str()))
                .unwrap_or(SessionFileChangeKind::Modified);
            change.detail = Some(build_session_file_change_detail(
                repo_path,
                baseline_entries,
                change_kind,
                &change.path,
                change.previous_path.as_deref(),
            ));
            change
        })
        .collect()
}

pub(super) fn compute_execution_session_file_changes(
    baseline: &ExecutionChangeBaseline,
) -> Result<Vec<CodexSessionFileChangeInput>, String> {
    let end_entries = collect_working_tree_snapshot_entries(&baseline.repo_path, false)?;
    compute_execution_session_file_changes_from_entries(
        &baseline.repo_path,
        &baseline.entries,
        &end_entries,
    )
}

pub(super) fn compute_execution_session_file_changes_from_entries(
    repo_path: &str,
    baseline_entries: &HashMap<String, WorkingTreeSnapshotEntry>,
    end_entries: &HashMap<String, WorkingTreeSnapshotEntry>,
) -> Result<Vec<CodexSessionFileChangeInput>, String> {
    let rename_sources = end_entries
        .values()
        .filter(|entry| entry_is_renamed(entry))
        .filter_map(|entry| entry.previous_path.clone())
        .collect::<HashSet<_>>();
    let mut consumed_baseline = HashSet::new();
    let mut changes = Vec::new();

    let mut end_paths = end_entries.keys().cloned().collect::<Vec<_>>();
    end_paths.sort();

    for path in end_paths {
        let entry = end_entries
            .get(&path)
            .expect("end entry should exist for collected key");

        match baseline_entries.get(&path) {
            None => {
                if let Some(previous_path) = entry.previous_path.as_ref() {
                    consumed_baseline.insert(previous_path.clone());
                }
                changes.push(build_session_file_change(
                    path,
                    classify_new_entry_change_kind(entry),
                    CodexExecutionProvider::Cli.capture_mode(),
                    entry
                        .previous_path
                        .clone()
                        .filter(|_| entry_is_renamed(entry)),
                ));
            }
            Some(baseline_entry) => {
                consumed_baseline.insert(path.clone());
                if entries_have_same_change_identity(baseline_entry, entry) {
                    continue;
                }

                let change_kind = if entry_is_renamed(entry)
                    && baseline_entry.previous_path != entry.previous_path
                {
                    SessionFileChangeKind::Renamed
                } else if entry_is_deleted(entry) {
                    SessionFileChangeKind::Deleted
                } else {
                    SessionFileChangeKind::Modified
                };

                changes.push(build_session_file_change(
                    path,
                    change_kind,
                    CodexExecutionProvider::Cli.capture_mode(),
                    entry
                        .previous_path
                        .clone()
                        .filter(|_| change_kind == SessionFileChangeKind::Renamed),
                ));
            }
        }
    }

    let mut baseline_paths = baseline_entries.keys().cloned().collect::<Vec<_>>();
    baseline_paths.sort();

    for path in baseline_paths {
        if consumed_baseline.contains(&path) || rename_sources.contains(&path) {
            continue;
        }

        let baseline_entry = baseline_entries
            .get(&path)
            .expect("baseline entry should exist for collected key");
        let current_hash = hash_worktree_path(repo_path, &path)?;
        if current_hash == baseline_entry.content_hash {
            continue;
        }

        let change_kind = if current_hash.is_none() {
            SessionFileChangeKind::Deleted
        } else {
            SessionFileChangeKind::Modified
        };
        changes.push(build_session_file_change(
            path,
            change_kind,
            CodexExecutionProvider::Cli.capture_mode(),
            None,
        ));
    }

    Ok(attach_session_file_change_details(
        repo_path,
        baseline_entries,
        changes,
    ))
}
