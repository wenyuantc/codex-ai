use super::*;

pub(crate) fn truncate_review_text(value: &str, limit: usize) -> (String, bool) {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        return (trimmed.to_string(), false);
    }

    let truncated = trimmed.chars().take(limit).collect::<String>();
    (truncated, true)
}

fn is_supported_review_text_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some(
            "ts" | "tsx"
                | "js"
                | "jsx"
                | "json"
                | "rs"
                | "md"
                | "css"
                | "scss"
                | "html"
                | "yml"
                | "yaml"
                | "toml"
                | "sql"
                | "sh"
                | "txt"
        )
    )
}

fn run_git_text(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    configure_std_command(&mut command);
    let output = command
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("执行 git {:?} 失败: {}", args, error))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn read_untracked_review_snippets(repo_path: &str, untracked_files: &[String]) -> String {
    let mut snippets = Vec::new();
    let mut consumed_chars = 0usize;

    for relative_path in untracked_files.iter().take(REVIEW_UNTRACKED_FILE_LIMIT) {
        if consumed_chars >= REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT {
            break;
        }

        let full_path = Path::new(repo_path).join(relative_path);
        let metadata = match fs::metadata(&full_path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if !metadata.is_file()
            || metadata.len() > REVIEW_UNTRACKED_FILE_SIZE_LIMIT
            || !is_supported_review_text_extension(&full_path)
        {
            continue;
        }

        let content = match fs::read_to_string(&full_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let remaining = REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT.saturating_sub(consumed_chars);
        if remaining == 0 {
            break;
        }

        let (snippet, truncated) = truncate_review_text(&content, remaining.min(12_000));
        if snippet.is_empty() {
            continue;
        }

        consumed_chars += snippet.chars().count();
        snippets.push(format!(
            "### {}\n```text\n{}\n```\n{}",
            relative_path,
            snippet,
            if truncated {
                "（内容已截断）"
            } else {
                ""
            }
        ));
    }

    if snippets.is_empty() {
        "（无可直接读取的未跟踪文本文件内容）".to_string()
    } else {
        snippets.join("\n\n")
    }
}

fn build_untracked_review_section(untracked_files: &[String], snippets: &str) -> String {
    if untracked_files.is_empty() {
        "（无未跟踪文件）".to_string()
    } else {
        format!(
            "未跟踪文件列表：\n{}\n\n未跟踪文本文件摘录：\n{}",
            untracked_files
                .iter()
                .map(|path| format!("- {}", path))
                .collect::<Vec<_>>()
                .join("\n"),
            snippets,
        )
    }
}

pub(crate) fn build_task_review_context_from_git_outputs(
    status_output: &str,
    unstaged_stat: &str,
    unstaged_diff: &str,
    staged_stat: &str,
    staged_diff: &str,
    untracked_files: &[String],
    untracked_section: &str,
) -> Result<String, String> {
    let status_trimmed = status_output.trim();
    if status_trimmed.is_empty() {
        return Err("当前工作区没有可审核的代码改动".to_string());
    }

    let combined_diff = [staged_diff.trim(), unstaged_diff.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if combined_diff.trim().is_empty() && untracked_files.is_empty() {
        return Err("当前工作区没有可审核的代码 diff".to_string());
    }

    let combined_stat = [staged_stat.trim(), unstaged_stat.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let (diff_body, diff_truncated) = truncate_review_text(&combined_diff, REVIEW_DIFF_CHAR_LIMIT);

    Ok(format!(
        "## Git 状态\n{}\n\n## Diff 概览\n{}\n\n## 完整 Diff\n{}\n{}\n\n## 未跟踪文件\n{}",
        status_trimmed,
        if combined_stat.trim().is_empty() {
            "（无 diff 统计）"
        } else {
            combined_stat.trim()
        },
        if diff_body.trim().is_empty() {
            "（无已跟踪文件 diff）"
        } else {
            diff_body.trim()
        },
        if diff_truncated {
            "\n（完整 diff 已截断）"
        } else {
            ""
        },
        untracked_section
    ))
}

#[cfg(test)]
pub(crate) fn collect_task_review_context(repo_path: &str) -> Result<String, String> {
    let status_output = run_git_text(repo_path, &["status", "--short"])?;
    let unstaged_stat = run_git_text(repo_path, &["diff", "--no-ext-diff", "--stat"])?;
    let unstaged_diff = run_git_text(repo_path, &["diff", "--no-ext-diff"])?;
    let staged_stat = run_git_text(repo_path, &["diff", "--no-ext-diff", "--stat", "--cached"])?;
    let staged_diff = run_git_text(repo_path, &["diff", "--no-ext-diff", "--cached"])?;
    let untracked_output =
        run_git_text(repo_path, &["ls-files", "--others", "--exclude-standard"])?;
    let untracked_files = untracked_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let untracked_section = build_untracked_review_section(
        &untracked_files,
        &read_untracked_review_snippets(repo_path, &untracked_files),
    );

    build_task_review_context_from_git_outputs(
        &status_output,
        &unstaged_stat,
        &unstaged_diff,
        &staged_stat,
        &staged_diff,
        &untracked_files,
        &untracked_section,
    )
}

async fn collect_project_task_review_context_for_task<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
) -> Result<(String, String), String> {
    let repo_path = if project.project_type == PROJECT_TYPE_SSH {
        project
            .remote_repo_path
            .clone()
            .ok_or_else(|| "当前 SSH 项目未配置远程仓库目录，无法审核代码".to_string())?
    } else {
        project
            .repo_path
            .clone()
            .ok_or_else(|| "当前项目未配置仓库路径，无法审核代码".to_string())?
    };

    let mut candidate_dirs = Vec::new();
    if let Some(last_session_id) = task.last_codex_session_id.as_deref() {
        let latest_bound_working_dir = sqlx::query_scalar::<_, Option<String>>(
            "SELECT working_dir FROM codex_sessions WHERE id = $1 AND session_kind = 'execution' LIMIT 1",
        )
        .bind(last_session_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("查询最近执行 Session 工作区失败: {}", error))?
        .flatten();
        if let Some(path) = latest_bound_working_dir {
            candidate_dirs.push(path);
        }
    }

    let latest_execution_working_dir = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT working_dir
        FROM codex_sessions
        WHERE task_id = $1
          AND session_kind = 'execution'
          AND working_dir IS NOT NULL
        ORDER BY started_at DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&task.id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("查询任务最近执行工作区失败: {}", error))?
    .flatten();
    if let Some(path) = latest_execution_working_dir {
        candidate_dirs.push(path);
    }
    candidate_dirs.push(repo_path.clone());

    let mut seen_dirs = HashSet::new();
    let mut last_empty_change_error: Option<String> = None;
    let mut last_runtime_error: Option<String> = None;

    for candidate in candidate_dirs {
        let normalized = candidate.trim().to_string();
        if normalized.is_empty() || !seen_dirs.insert(normalized.clone()) {
            continue;
        }

        match crate::git_runtime::collect_review_context(
            app,
            if project.project_type == PROJECT_TYPE_SSH {
                EXECUTION_TARGET_SSH
            } else {
                EXECUTION_TARGET_LOCAL
            },
            project.ssh_config_id.as_deref(),
            &normalized,
        )
        .await
        {
            Ok(context) => return Ok((normalized, context)),
            Err(error)
                if error == "当前工作区没有可审核的代码改动"
                    || error == "当前工作区没有可审核的代码 diff" =>
            {
                last_empty_change_error = Some(error);
            }
            Err(error) => {
                last_runtime_error = Some(error);
            }
        }
    }

    Err(last_empty_change_error
        .or(last_runtime_error)
        .unwrap_or_else(|| "当前工作区没有可审核的代码改动".to_string()))
}

#[cfg(test)]
pub(crate) async fn collect_local_task_review_context_for_task(
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
) -> Result<(String, String), String> {
    let repo_path = project
        .repo_path
        .clone()
        .ok_or_else(|| "当前项目未配置仓库路径，无法审核代码".to_string())?;

    let mut candidate_dirs = Vec::new();
    if let Some(last_session_id) = task.last_codex_session_id.as_deref() {
        let latest_bound_working_dir = sqlx::query_scalar::<_, Option<String>>(
            "SELECT working_dir FROM codex_sessions WHERE id = $1 AND session_kind = 'execution' LIMIT 1",
        )
        .bind(last_session_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("查询最近执行 Session 工作区失败: {}", error))?
        .flatten();
        if let Some(path) = latest_bound_working_dir {
            candidate_dirs.push(path);
        }
    }

    let latest_execution_working_dir = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT working_dir
        FROM codex_sessions
        WHERE task_id = $1
          AND session_kind = 'execution'
          AND working_dir IS NOT NULL
        ORDER BY started_at DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&task.id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("查询任务最近执行工作区失败: {}", error))?
    .flatten();
    if let Some(path) = latest_execution_working_dir {
        candidate_dirs.push(path);
    }
    candidate_dirs.push(repo_path);

    let mut seen_dirs = HashSet::new();
    let mut last_empty_change_error: Option<String> = None;
    let mut last_runtime_error: Option<String> = None;

    for candidate in candidate_dirs {
        let normalized = candidate.trim().to_string();
        if normalized.is_empty() || !seen_dirs.insert(normalized.clone()) {
            continue;
        }

        match collect_task_review_context(&normalized) {
            Ok(context) => return Ok((normalized, context)),
            Err(error)
                if error == "当前工作区没有可审核的代码改动"
                    || error == "当前工作区没有可审核的代码 diff" =>
            {
                last_empty_change_error = Some(error);
            }
            Err(error) => {
                last_runtime_error = Some(error);
            }
        }
    }

    Err(last_empty_change_error
        .or(last_runtime_error)
        .unwrap_or_else(|| "当前工作区没有可审核的代码改动".to_string()))
}

fn shell_join_single_quoted(args: &[&str]) -> String {
    args.iter()
        .map(|arg| shell_escape_single_quoted(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

async fn run_remote_git_text<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    repo_path: &str,
    args: &[&str],
) -> Result<String, String> {
    let remote_command = build_remote_shell_command(
        &format!(
            "git -C {} {}",
            remote_shell_path_expression(repo_path),
            shell_join_single_quoted(args)
        ),
        None,
    );
    let output = execute_ssh_command(app, ssh_config, &remote_command, true).await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("远程执行 git {:?} 失败", args)
        } else {
            format!(
                "远程执行 git {:?} 失败: {}",
                args,
                redact_secret_text(&stderr)
            )
        })
    }
}

pub(crate) async fn collect_remote_task_review_context<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    repo_path: &str,
) -> Result<String, String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let status_output =
        run_remote_git_text(app, &ssh_config, repo_path, &["status", "--short"]).await?;
    let unstaged_stat = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["diff", "--no-ext-diff", "--stat"],
    )
    .await?;
    let unstaged_diff =
        run_remote_git_text(app, &ssh_config, repo_path, &["diff", "--no-ext-diff"]).await?;
    let staged_stat = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["diff", "--no-ext-diff", "--stat", "--cached"],
    )
    .await?;
    let staged_diff = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["diff", "--no-ext-diff", "--cached"],
    )
    .await?;
    let untracked_output = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["ls-files", "--others", "--exclude-standard"],
    )
    .await?;
    let untracked_files = untracked_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let untracked_section = build_untracked_review_section(
        &untracked_files,
        "（SSH 模式暂不采集远程未跟踪文件内容摘录，请结合未跟踪文件列表人工确认）",
    );

    build_task_review_context_from_git_outputs(
        &status_output,
        &unstaged_stat,
        &unstaged_diff,
        &staged_stat,
        &staged_diff,
        &untracked_files,
        &untracked_section,
    )
}

pub(crate) fn build_task_review_prompt(
    task: &Task,
    project: &Project,
    review_working_dir: &str,
    review_context: &str,
) -> String {
    format!(
        "你正在执行一次只读代码审查。\n\
要求：\n\
- 只允许阅读和分析代码，禁止修改任何文件，禁止执行 git commit/reset/checkout/merge/rebase 等写操作\n\
- 审核范围仅限下方提供的任务信息和当前工作区改动\n\
- 最终结构化判定必须且只能输出在 {verdict_start_tag} 和 {verdict_end_tag} 之间，内容必须是 JSON，对应字段：passed(boolean)、needs_human(boolean)、blocking_issue_count(number)、summary(string)\n\
- 最终人类可读报告必须且只能输出在 {start_tag} 和 {end_tag} 之间\n\
- 报告必须使用中文 Markdown，包含以下小节：## 结论、## 阻断问题、## 风险提醒、## 改进建议、## 验证缺口\n\
- 如果没有阻断问题，明确写“无阻断问题”\n\
- 如果 diff 信息被截断，要把这件事写进“验证缺口”\n\n\
任务标题：{title}\n\
任务状态：{status}\n\
任务优先级：{priority}\n\
项目名称：{project_name}\n\
仓库路径：{repo_path}\n\
执行目标：{execution_target}\n\
任务描述：{description}\n\n\
{review_context}",
        verdict_start_tag = REVIEW_VERDICT_START_TAG,
        verdict_end_tag = REVIEW_VERDICT_END_TAG,
        start_tag = REVIEW_REPORT_START_TAG,
        end_tag = REVIEW_REPORT_END_TAG,
        title = task.title.trim(),
        status = task.status.trim(),
        priority = task.priority.trim(),
        project_name = project.name.trim(),
        repo_path = review_working_dir,
        execution_target = if project.project_type == PROJECT_TYPE_SSH {
            "SSH 远程工作区"
        } else {
            "本地工作区"
        },
        description = task.description.as_deref().unwrap_or("（未填写）"),
        review_context = review_context,
    )
}

pub(crate) fn parse_review_verdict_json(value: &str) -> Result<ReviewVerdict, String> {
    let verdict = serde_json::from_str::<ReviewVerdict>(value)
        .map_err(|error| format!("Failed to parse review verdict JSON: {}", error))?;

    if verdict.summary.trim().is_empty() {
        return Err("Review verdict summary cannot be empty".to_string());
    }

    if verdict.blocking_issue_count < 0 {
        return Err("Review verdict blocking_issue_count cannot be negative".to_string());
    }

    Ok(verdict)
}

fn task_attachments_root_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .or_else(|_| app.path().app_config_dir())
        .map(|dir| dir.join("task-attachments"))
        .map_err(|error| format!("无法解析附件存储目录: {}", error))
}

pub(crate) fn task_attachment_dir<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
) -> Result<PathBuf, String> {
    Ok(task_attachments_root_dir(app)?.join(task_id))
}

fn task_attachment_mime_type(path: &Path) -> String {
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("tsv") => "text/tab-separated-values",
        Some("yml") | Some("yaml") => "application/yaml",
        Some("xml") => "application/xml",
        Some("zip") => "application/zip",
        Some("7z") => "application/x-7z-compressed",
        Some("gz") => "application/gzip",
        Some("tar") => "application/x-tar",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn mime_type_is_image(mime_type: &str) -> bool {
    mime_type.trim().to_ascii_lowercase().starts_with("image/")
}

pub(crate) fn task_attachment_is_image(attachment: &TaskAttachment) -> bool {
    mime_type_is_image(&attachment.mime_type)
        || mime_type_is_image(&task_attachment_mime_type(Path::new(
            &attachment.stored_path,
        )))
}

pub(crate) fn filter_image_attachments(attachments: &[TaskAttachment]) -> Vec<TaskAttachment> {
    attachments
        .iter()
        .filter(|attachment| task_attachment_is_image(attachment))
        .cloned()
        .collect()
}

fn validate_task_attachment_source_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("附件路径不能为空".to_string());
    }

    let canonical = Path::new(trimmed)
        .canonicalize()
        .map_err(|error| format!("附件路径不存在或不可访问: {}", error))?;

    if !canonical.is_file() {
        return Err(format!("附件路径 {} 不是文件", canonical.display()));
    }

    Ok(canonical)
}

fn validate_managed_task_attachment_path<R: Runtime>(
    app: &AppHandle<R>,
    path: &str,
) -> Result<PathBuf, String> {
    let canonical = validate_task_attachment_source_path(path)?;
    let root = task_attachments_root_dir(app)?;
    let root = root.canonicalize().unwrap_or(root);

    if !canonical.starts_with(&root) {
        return Err(format!(
            "附件路径不在应用托管目录内: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

pub(crate) fn cleanup_task_attachment_files(paths: &[String]) {
    for path in paths {
        let target = Path::new(path);
        if target.exists() {
            if let Err(error) = fs::remove_file(target) {
                eprintln!(
                    "[task-attachments] 清理附件文件失败: path={}, error={}",
                    target.display(),
                    error
                );
            }
        }
    }
}

pub(crate) fn cleanup_empty_attachment_dir<R: Runtime>(app: &AppHandle<R>, task_id: &str) {
    let Ok(dir) = task_attachment_dir(app, task_id) else {
        return;
    };

    let is_empty = fs::read_dir(&dir)
        .ok()
        .and_then(|mut entries| entries.next().transpose().ok())
        .flatten()
        .is_none();

    if is_empty {
        let _ = fs::remove_dir(&dir);
    }
}

fn build_task_attachment_from_source<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    source_path: &str,
    sort_order: i32,
) -> Result<TaskAttachment, String> {
    let source = validate_task_attachment_source_path(source_path)?;
    let attachment_id = new_id();
    let original_name = source
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("无法解析附件文件名: {}", source.display()))?;
    let mime_type = task_attachment_mime_type(&source);
    let extension = source
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase());
    let target_dir = task_attachment_dir(app, task_id)?;
    fs::create_dir_all(&target_dir).map_err(|error| format!("创建任务附件目录失败: {}", error))?;
    let target_path = match extension {
        Some(extension) if !extension.is_empty() => {
            target_dir.join(format!("{attachment_id}.{extension}"))
        }
        _ => target_dir.join(&attachment_id),
    };
    fs::copy(&source, &target_path).map_err(|error| {
        format!(
            "复制附件失败: {} -> {}: {}",
            source.display(),
            target_path.display(),
            error
        )
    })?;
    let file_size = fs::metadata(&target_path)
        .map_err(|error| format!("读取附件信息失败: {}", error))?
        .len() as i64;

    Ok(TaskAttachment {
        id: attachment_id,
        task_id: task_id.to_string(),
        original_name,
        stored_path: target_path.to_string_lossy().to_string(),
        mime_type,
        file_size,
        sort_order,
        created_at: now_sqlite(),
    })
}

pub(crate) fn build_task_attachments_from_sources<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    source_paths: &[String],
    start_sort_order: i32,
) -> Result<Vec<TaskAttachment>, String> {
    let mut attachments = Vec::new();

    for (index, source_path) in source_paths.iter().enumerate() {
        match build_task_attachment_from_source(
            app,
            task_id,
            source_path,
            start_sort_order + index as i32,
        ) {
            Ok(attachment) => attachments.push(attachment),
            Err(error) => {
                let copied_paths = attachments
                    .iter()
                    .map(|attachment| attachment.stored_path.clone())
                    .collect::<Vec<_>>();
                cleanup_task_attachment_files(&copied_paths);
                cleanup_empty_attachment_dir(app, task_id);
                return Err(error);
            }
        }
    }

    Ok(attachments)
}

fn task_attachment_file_name(attachment: &TaskAttachment) -> Result<String, String> {
    Path::new(&attachment.stored_path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("无法解析附件文件名: {}", attachment.stored_path))
}

pub(crate) fn remote_task_attachment_dir(home_dir: &str, task_id: &str) -> String {
    remote_path_join(
        &remote_path_join(
            home_dir.trim_end_matches('/'),
            REMOTE_TASK_ATTACHMENT_ROOT_DIR,
        ),
        task_id,
    )
}

pub(crate) fn remote_task_attachment_path(
    home_dir: &str,
    attachment: &TaskAttachment,
) -> Result<String, String> {
    Ok(remote_path_join(
        &remote_task_attachment_dir(home_dir, &attachment.task_id),
        &task_attachment_file_name(attachment)?,
    ))
}

async fn resolve_remote_home_dir_with_config<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
) -> Result<String, String> {
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command("printf '%s' \"$HOME\"", None),
        true,
    )
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "无法解析远程 HOME 目录".to_string()
        } else {
            format!("无法解析远程 HOME 目录：{}", redact_secret_text(&stderr))
        });
    }

    let home_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if home_dir.is_empty() {
        return Err("远程 HOME 目录为空".to_string());
    }

    Ok(home_dir)
}

async fn upload_task_attachment_to_remote<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    home_dir: &str,
    attachment: &TaskAttachment,
    skip_missing_local_source: bool,
) -> Result<Option<String>, String> {
    let source = match validate_managed_task_attachment_path(app, &attachment.stored_path) {
        Ok(source) => source,
        Err(error) if skip_missing_local_source => {
            let _ = error;
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let bytes = match fs::read(&source) {
        Ok(bytes) => bytes,
        Err(_) if skip_missing_local_source => return Ok(None),
        Err(error) => {
            return Err(format!("读取本地附件失败: {}: {}", source.display(), error));
        }
    };
    let remote_dir = remote_task_attachment_dir(home_dir, &attachment.task_id);
    let remote_path = remote_task_attachment_path(home_dir, attachment)?;
    let remote_command = build_remote_shell_command(
        &format!(
            "mkdir -p {} && cat > {}",
            remote_shell_path_expression(&remote_dir),
            remote_shell_path_expression(&remote_path),
        ),
        None,
    );
    let output =
        execute_ssh_command_with_input(app, ssh_config, &remote_command, &bytes, true).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("上传附件到远程失败：{}", attachment.original_name)
        } else {
            format!(
                "上传附件到远程失败：{}：{}",
                attachment.original_name,
                redact_secret_text(&stderr)
            )
        });
    }

    Ok(Some(remote_path))
}

async fn remove_remote_task_attachment_by_path<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_path: &str,
) -> Result<(), String> {
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command(
            &format!("rm -f {}", remote_shell_path_expression(remote_path)),
            None,
        ),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("删除远程附件失败：{}", remote_path)
        } else {
            format!(
                "删除远程附件失败：{}：{}",
                remote_path,
                redact_secret_text(&stderr)
            )
        });
    }
    Ok(())
}

pub(crate) async fn sync_task_attachment_records_to_remote<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    attachments: &[TaskAttachment],
    skip_missing_local_source: bool,
) -> Result<RemoteTaskAttachmentSyncResult, String> {
    if attachments.is_empty() {
        return Ok(RemoteTaskAttachmentSyncResult {
            remote_paths: Vec::new(),
            skipped_local_paths: Vec::new(),
        });
    }

    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let home_dir = resolve_remote_home_dir_with_config(app, &ssh_config).await?;
    let mut remote_paths = Vec::with_capacity(attachments.len());
    let mut skipped_local_paths = Vec::new();

    for attachment in attachments {
        match upload_task_attachment_to_remote(
            app,
            &ssh_config,
            &home_dir,
            attachment,
            skip_missing_local_source,
        )
        .await
        {
            Ok(Some(remote_path)) => remote_paths.push(remote_path),
            Ok(None) => skipped_local_paths.push(attachment.stored_path.clone()),
            Err(error) => {
                for remote_path in &remote_paths {
                    if let Err(cleanup_error) =
                        remove_remote_task_attachment_by_path(app, &ssh_config, remote_path).await
                    {
                        eprintln!(
                            "[task-attachments] 清理远程附件失败: path={}, error={}",
                            remote_path, cleanup_error
                        );
                    }
                }
                return Err(error);
            }
        }
    }

    Ok(RemoteTaskAttachmentSyncResult {
        remote_paths,
        skipped_local_paths,
    })
}

pub(crate) async fn sync_task_image_attachments_to_remote<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    task_id: &str,
) -> Result<RemoteTaskAttachmentSyncResult, String> {
    let pool = sqlite_pool(app).await?;
    let attachments = fetch_task_attachments(&pool, task_id).await?;
    let image_attachments = filter_image_attachments(&attachments);
    sync_task_attachment_records_to_remote(app, ssh_config_id, &image_attachments, true).await
}

pub(crate) async fn cleanup_remote_task_attachment_paths<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    remote_paths: &[String],
) {
    if remote_paths.is_empty() {
        return;
    }

    let pool = match sqlite_pool(app).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!(
                "[task-attachments] 获取数据库连接失败，无法清理远程附件: {}",
                error
            );
            return;
        }
    };
    let ssh_config = match fetch_ssh_config_record_by_id(&pool, ssh_config_id).await {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "[task-attachments] 读取 SSH 配置失败，无法清理远程附件: {}",
                error
            );
            return;
        }
    };

    for remote_path in remote_paths {
        if let Err(error) =
            remove_remote_task_attachment_by_path(app, &ssh_config, remote_path).await
        {
            eprintln!(
                "[task-attachments] 清理远程附件失败: path={}, error={}",
                remote_path, error
            );
        }
    }
}

pub(crate) async fn cleanup_remote_task_attachments_for_task<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    task_id: &str,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let home_dir = resolve_remote_home_dir_with_config(app, &ssh_config).await?;
    let remote_dir = remote_task_attachment_dir(&home_dir, task_id);
    let output = execute_ssh_command(
        app,
        &ssh_config,
        &build_remote_shell_command(
            &format!("rm -rf {}", remote_shell_path_expression(&remote_dir)),
            None,
        ),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("删除远程任务附件目录失败：{}", remote_dir)
        } else {
            format!(
                "删除远程任务附件目录失败：{}：{}",
                remote_dir,
                redact_secret_text(&stderr)
            )
        });
    }
    Ok(())
}

pub(crate) async fn cleanup_remote_task_attachment<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    attachment: &TaskAttachment,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let home_dir = resolve_remote_home_dir_with_config(app, &ssh_config).await?;
    let remote_path = remote_task_attachment_path(&home_dir, attachment)?;
    remove_remote_task_attachment_by_path(app, &ssh_config, &remote_path).await
}

#[tauri::command]
pub async fn read_image_file(path: String) -> Result<Vec<u8>, String> {
    let source = validate_task_attachment_source_path(&path)?;
    fs::read(&source).map_err(|error| format!("读取图片文件失败: {}", error))
}

#[tauri::command]
pub async fn open_task_attachment<R: Runtime>(
    app: AppHandle<R>,
    path: String,
) -> Result<(), String> {
    let source = validate_managed_task_attachment_path(&app, &path)?;
    app.opener()
        .open_path(source.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("打开附件失败: {}", error))
}

pub(crate) async fn record_task_review_requested_activity(
    pool: &SqlitePool,
    reviewer_id: &str,
    reviewer_name: &str,
    task_id: &str,
    project_id: &str,
) {
    if let Err(error) = insert_activity_log(
        pool,
        "task_review_requested",
        &format!("{} 发起代码审核", reviewer_name),
        Some(reviewer_id),
        Some(task_id),
        Some(project_id),
    )
    .await
    {
        eprintln!(
            "[task-review] activity log write failed after review session start: {}",
            error
        );
    }
}

fn emit_task_preflight_log<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    task_id: &str,
    session_kind: &str,
    line: impl Into<String>,
) {
    let _ = app.emit(
        "codex-stdout",
        CodexOutput {
            employee_id: employee_id.to_string(),
            task_id: Some(task_id.to_string()),
            session_kind: session_kind.to_string(),
            session_record_id: format!("preflight:{}:{}", session_kind, task_id),
            session_event_id: None,
            line: line.into(),
        },
    );
}

pub(crate) async fn start_task_code_review_internal(
    app: AppHandle,
    manager_state: Arc<Mutex<CodexManager>>,
    task_id: &str,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, task_id).await?;
    if task.status != "review" {
        return Err("只有“审核中”的任务才能发起代码审核".to_string());
    }

    let reviewer_id = task
        .reviewer_id
        .as_deref()
        .ok_or_else(|| "请先为任务指定审查员".to_string())?;
    let reviewer = fetch_employee_by_id(&pool, reviewer_id).await?;
    if reviewer.role != "reviewer" {
        return Err(format!("员工 {} 不是审查员角色", reviewer.name));
    }

    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let (review_working_dir, review_context) = if project.project_type == PROJECT_TYPE_SSH {
        let ssh_config_id = project
            .ssh_config_id
            .as_deref()
            .ok_or_else(|| "当前 SSH 项目未绑定 SSH 配置，无法审核代码".to_string())?;
        let remote_repo_path = project
            .remote_repo_path
            .as_deref()
            .ok_or_else(|| "当前 SSH 项目未配置远程仓库目录，无法审核代码".to_string())?;
        let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            format!(
                "[SSH] 正在连接 {}（{}@{}:{}）...",
                ssh_config.name, ssh_config.username, ssh_config.host, ssh_config.port
            ),
        );
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            format!(
                "[SSH] 正在准备远程审核上下文，仓库目录：{}",
                remote_repo_path
            ),
        );
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            "[SSH] 正在通过 Git bridge 采集远程工作区 diff，用于生成审核上下文...".to_string(),
        );
        let (_, review_context) =
            collect_project_task_review_context_for_task(&app, &pool, &task, &project).await?;
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            "[SSH] 远程审核上下文采集完成，正在启动审核会话...".to_string(),
        );
        (remote_repo_path.to_string(), review_context)
    } else {
        collect_project_task_review_context_for_task(&app, &pool, &task, &project).await?
    };
    let review_prompt =
        build_task_review_prompt(&task, &project, &review_working_dir, &review_context);

    crate::codex::start_codex_with_manager(
        app.clone(),
        manager_state,
        reviewer.id.clone(),
        review_prompt,
        Some(reviewer.model.clone()),
        Some(reviewer.reasoning_effort.clone()),
        reviewer.system_prompt.clone(),
        Some(review_working_dir),
        Some(task.id.clone()),
        None,
        None,
        None,
        Some("review".to_string()),
    )
    .await?;

    record_task_review_requested_activity(
        &pool,
        reviewer.id.as_str(),
        reviewer.name.as_str(),
        task.id.as_str(),
        task.project_id.as_str(),
    )
    .await;

    Ok(())
}

#[tauri::command]
pub async fn start_task_code_review(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    task_id: String,
) -> Result<(), String> {
    start_task_code_review_internal(app, state.inner().clone(), &task_id).await
}
