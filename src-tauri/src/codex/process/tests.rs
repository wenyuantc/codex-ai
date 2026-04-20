use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    attach_session_file_change_details, build_ai_generate_commit_message_prompt,
    build_ai_generate_plan_prompt, build_ai_optimize_prompt_prompt, build_one_shot_exec_args,
    build_remote_codex_session_command, build_remote_sdk_bridge_command, build_session_exec_args,
    commit_message_uses_process_language, compose_codex_prompt,
    compute_execution_session_file_changes_from_entries, detect_exec_json_output_flag,
    extract_review_report, extract_review_verdict, extract_session_id_from_output,
    format_session_prompt_log, hash_worktree_path, normalize_model,
    normalize_session_file_change_paths, parse_ai_subtasks_response, parse_cli_json_event_line,
    parse_sdk_bridge_output, parse_sdk_file_change_event, sdk_codex_path_override_allowed_for_os,
    should_capture_execution_change_baseline, validate_generated_commit_message, CliJsonOutputFlag,
    CliJsonStreamState, CodexExecutionProvider, CodexSessionKind, TextSnapshot,
    WorkingTreeSnapshotEntry, EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH,
};
use crate::db::models::CodexSessionFileChangeInput;

fn snapshot_entry(
    path: &str,
    status_x: char,
    status_y: char,
    previous_path: Option<&str>,
    content_hash: Option<&str>,
) -> WorkingTreeSnapshotEntry {
    WorkingTreeSnapshotEntry {
        path: path.to_string(),
        previous_path: previous_path.map(ToOwned::to_owned),
        status_x,
        status_y,
        content_hash: content_hash.map(ToOwned::to_owned),
        text_snapshot: TextSnapshot::missing(),
    }
}

#[test]
fn extracts_session_id_from_stdout_line() {
    assert_eq!(
        extract_session_id_from_output("session id: 019d8726-4730-7d71-b00c-aeade2188cb1"),
        Some("019d8726-4730-7d71-b00c-aeade2188cb1".to_string())
    );
}

#[test]
fn ignores_non_session_lines() {
    assert_eq!(extract_session_id_from_output("codex"), None);
    assert_eq!(extract_session_id_from_output("hook: SessionStart"), None);
}

#[test]
fn session_exec_args_pipe_prompt_via_stdin() {
    let args = build_session_exec_args(
        "gpt-5.4",
        "high",
        r"D:\repo\demo",
        &["D:\\repo\\demo\\ui.png".to_string()],
        Some("session-123"),
        Some(CliJsonOutputFlag::Json),
    );

    assert_eq!(
        args,
        vec![
            "exec".to_string(),
            "--model".to_string(),
            "gpt-5.4".to_string(),
            "-c".to_string(),
            "model_reasoning_effort=\"high\"".to_string(),
            "-C".to_string(),
            r"D:\repo\demo".to_string(),
            "--json".to_string(),
            "resume".to_string(),
            "session-123".to_string(),
            "--image".to_string(),
            r"D:\repo\demo\ui.png".to_string(),
            "-".to_string(),
        ]
    );
}

#[test]
fn detects_exec_json_output_flag_by_supported_option() {
    assert_eq!(
        detect_exec_json_output_flag("  --json  Print events to stdout as JSONL"),
        Some(CliJsonOutputFlag::Json)
    );
    assert_eq!(
        detect_exec_json_output_flag("  --experimental-json  Print events"),
        Some(CliJsonOutputFlag::ExperimentalJson)
    );
    assert_eq!(detect_exec_json_output_flag("  --help  Print help"), None);
}

#[test]
fn cli_json_parser_extracts_session_id_and_command_output_delta() {
    let mut state = CliJsonStreamState::default();

    let session_started = parse_cli_json_event_line(
        r#"{"type":"thread.started","thread_id":"session-xyz"}"#,
        &mut state,
    )
    .expect("parse thread.started");
    assert_eq!(session_started.session_id.as_deref(), Some("session-xyz"));
    assert!(session_started.lines.is_empty());

    let command_started = parse_cli_json_event_line(
        r#"{"type":"item.started","item":{"id":"cmd-1","type":"command_execution","command":"bash -lc ls","status":"in_progress"}}"#,
        &mut state,
    )
    .expect("parse item.started");
    assert_eq!(command_started.lines, vec!["[命令] bash -lc ls"]);

    let command_updated = parse_cli_json_event_line(
        r#"{"type":"item.updated","item":{"id":"cmd-1","type":"command_execution","aggregated_output":"line1\nline2\n","status":"in_progress"}}"#,
        &mut state,
    )
    .expect("parse item.updated");
    assert_eq!(command_updated.lines, vec!["line1", "line2"]);

    let command_updated_again = parse_cli_json_event_line(
        r#"{"type":"item.updated","item":{"id":"cmd-1","type":"command_execution","aggregated_output":"line1\nline2\nline3\n","status":"in_progress"}}"#,
        &mut state,
    )
    .expect("parse item.updated delta");
    assert_eq!(command_updated_again.lines, vec!["line3"]);
}

#[test]
fn cli_json_parser_streams_agent_message_and_plan_summary() {
    let mut state = CliJsonStreamState::default();

    let message = parse_cli_json_event_line(
        r#"{"type":"item.completed","item":{"id":"msg-1","type":"agent_message","text":"第一行\n第二行"}}"#,
        &mut state,
    )
    .expect("parse agent message");
    assert_eq!(message.lines, vec!["第一行", "第二行"]);

    let plan = parse_cli_json_event_line(
        r#"{"type":"item.updated","item":{"id":"plan-1","type":"todo_list","items":[{"text":"检查 SSH 日志链路","completed":true},{"text":"切换 exec JSON 事件流","completed":false}]}}"#,
        &mut state,
    )
    .expect("parse todo list");
    assert_eq!(
        plan.lines,
        vec!["[计划] [x] 检查 SSH 日志链路 | [ ] 切换 exec JSON 事件流"]
    );
}

#[test]
fn sdk_path_override_uses_native_binary_only_on_windows() {
    assert!(sdk_codex_path_override_allowed_for_os(
        Path::new(r"C:\Users\demo\AppData\Roaming\npm\codex.exe"),
        "windows"
    ));
    assert!(!sdk_codex_path_override_allowed_for_os(
        Path::new(r"C:\Users\demo\AppData\Roaming\npm\codex.cmd"),
        "windows"
    ));
    assert!(!sdk_codex_path_override_allowed_for_os(
        Path::new(r"C:\Users\demo\AppData\Roaming\npm\codex"),
        "windows"
    ));
}

#[test]
fn sdk_path_override_keeps_non_windows_platforms_compatible() {
    assert!(sdk_codex_path_override_allowed_for_os(
        Path::new("/usr/local/bin/codex"),
        "macos"
    ));
    assert!(sdk_codex_path_override_allowed_for_os(
        Path::new("/home/demo/.local/bin/codex"),
        "linux"
    ));
}

#[test]
fn composes_prompt_with_employee_system_prompt() {
    let prompt = compose_codex_prompt("修复看板状态问题", Some("你是资深前端工程师"));

    assert!(prompt.contains("你是资深前端工程师"));
    assert!(prompt.contains("修复看板状态问题"));
    assert!(prompt.contains("<employee_system_prompt>"));
}

#[test]
fn leaves_prompt_unchanged_without_employee_system_prompt() {
    assert_eq!(compose_codex_prompt("只执行任务", None), "只执行任务");
    assert_eq!(
        compose_codex_prompt("只执行任务", Some("   ")),
        "只执行任务"
    );
}

#[test]
fn review_tag_extraction_ignores_source_code_tag_literals() {
    let raw = r#"const REVIEW_VERDICT_START_TAG: &str = "<review_verdict>";
const REVIEW_VERDICT_END_TAG: &str = "</review_verdict>";
const REVIEW_REPORT_START_TAG: &str = "<review_report>";
const REVIEW_REPORT_END_TAG: &str = "</review_report>";
<review_verdict>
{"passed":false,"needs_human":false,"blocking_issue_count":1,"summary":"发现 1 个阻断问题。"}
</review_verdict>
<review_report>
## 结论
未通过。
</review_report>"#;

    assert_eq!(
        extract_review_verdict(raw).as_deref(),
        Some(
            r#"{"passed":false,"needs_human":false,"blocking_issue_count":1,"summary":"发现 1 个阻断问题。"}"#
        )
    );
    assert_eq!(
        extract_review_report(raw).as_deref(),
        Some("## 结论\n未通过。")
    );
}

#[test]
fn review_tag_extraction_supports_single_line_blocks() {
    let raw = "<review_verdict>{\"passed\":true,\"needs_human\":false,\"blocking_issue_count\":0,\"summary\":\"通过\"}</review_verdict>\n<review_report>## 结论\n通过。</review_report>";

    assert_eq!(
        extract_review_verdict(raw).as_deref(),
        Some("{\"passed\":true,\"needs_human\":false,\"blocking_issue_count\":0,\"summary\":\"通过\"}")
    );
    assert_eq!(
        extract_review_report(raw).as_deref(),
        Some("## 结论\n通过。")
    );
}

#[test]
fn parses_subtasks_from_json_object() {
    let subtasks = parse_ai_subtasks_response(
        r#"{"subtasks":["整理需求说明","拆分前端交互","补充后端接口"]}"#,
    )
    .expect("should parse subtasks");

    assert_eq!(
        subtasks,
        vec!["整理需求说明", "拆分前端交互", "补充后端接口"]
    );
}

#[test]
fn parses_subtasks_from_markdown_code_block() {
    let subtasks = parse_ai_subtasks_response(
        "下面是结果：\n```json\n{\"subtasks\":[\"梳理现状\",\"实现按钮\"]}\n```",
    )
    .expect("should parse fenced json");

    assert_eq!(subtasks, vec!["梳理现状", "实现按钮"]);
}

#[test]
fn parses_subtasks_from_json_array() {
    let subtasks =
        parse_ai_subtasks_response("[\"任务一\", \"任务二\"]").expect("should parse array");

    assert_eq!(subtasks, vec!["任务一", "任务二"]);
}

#[test]
fn one_shot_exec_args_skip_git_repo_check() {
    let args = build_one_shot_exec_args("gpt-5.4", "high", None, &[]);

    assert_eq!(
        args,
        vec![
            "exec".to_string(),
            "--skip-git-repo-check".to_string(),
            "--model".to_string(),
            "gpt-5.4".to_string(),
            "-c".to_string(),
            "model_reasoning_effort=\"high\"".to_string(),
        ]
    );
}

#[test]
fn one_shot_exec_args_include_images_before_prompt() {
    let args = build_one_shot_exec_args(
        "gpt-5.4-mini",
        "medium",
        None,
        &["/tmp/demo/a.png".to_string(), "/tmp/demo/b.jpg".to_string()],
    );

    assert_eq!(
        args,
        vec![
            "exec".to_string(),
            "--skip-git-repo-check".to_string(),
            "--model".to_string(),
            "gpt-5.4-mini".to_string(),
            "-c".to_string(),
            "model_reasoning_effort=\"medium\"".to_string(),
            "--image".to_string(),
            "/tmp/demo/a.png".to_string(),
            "--image".to_string(),
            "/tmp/demo/b.jpg".to_string(),
        ]
    );
}

#[test]
fn normalize_model_accepts_new_codex_variants() {
    assert_eq!(normalize_model(Some("gpt-5.2-codex")), "gpt-5.2-codex");
    assert_eq!(
        normalize_model(Some("gpt-5.1-codex-max")),
        "gpt-5.1-codex-max"
    );
    assert_eq!(
        normalize_model(Some("gpt-5.3-codex-spark")),
        "gpt-5.3-codex-spark"
    );
    assert_eq!(
        normalize_model(Some("gpt-5.1-codex-mini")),
        "gpt-5.1-codex-mini"
    );
}

#[test]
fn one_shot_exec_args_include_working_dir_when_provided() {
    let args = build_one_shot_exec_args("gpt-5.4", "high", Some("/tmp/worktree"), &[]);

    assert_eq!(
        args,
        vec![
            "exec".to_string(),
            "--skip-git-repo-check".to_string(),
            "--model".to_string(),
            "gpt-5.4".to_string(),
            "-c".to_string(),
            "model_reasoning_effort=\"high\"".to_string(),
            "-C".to_string(),
            "/tmp/worktree".to_string(),
        ]
    );
}

#[test]
fn remote_codex_session_command_includes_image_args() {
    let command = build_remote_codex_session_command(
        "gpt-5.4",
        "high",
        "/srv/repo",
        &["/home/demo/.codex-ai/img/task-1/att-1.png".to_string()],
        Some("session-123"),
        Some(CliJsonOutputFlag::Json),
        None,
    );

    assert!(command.contains("exec codex"));
    assert!(command.contains("'--json'"));
    assert!(command.contains("'--image'"));
    assert!(command.contains("'/home/demo/.codex-ai/img/task-1/att-1.png'"));
}

#[test]
fn execution_change_baseline_captures_for_all_execution_sessions() {
    assert!(should_capture_execution_change_baseline(
        CodexSessionKind::Execution,
        EXECUTION_TARGET_LOCAL
    ));
    assert!(should_capture_execution_change_baseline(
        CodexSessionKind::Execution,
        EXECUTION_TARGET_SSH
    ));
    assert!(!should_capture_execution_change_baseline(
        CodexSessionKind::Review,
        EXECUTION_TARGET_LOCAL
    ));
}

#[test]
fn parses_sdk_bridge_success_output() {
    let output = parse_sdk_bridge_output(br#"{"ok":true,"text":"sdk output"}"#, &[])
        .expect("parse sdk bridge success");

    assert_eq!(output, "sdk output");
}

#[test]
fn formats_prompt_log_with_runtime_context() {
    let log = format_session_prompt_log(
        CodexExecutionProvider::Sdk,
        "gpt-5.4",
        "high",
        EXECUTION_TARGET_LOCAL,
        None,
        None,
        None,
        "/tmp/demo",
        "任务标题:\n修复问题",
        &[
            "/tmp/demo/ui.png".to_string(),
            "/tmp/demo/flow.jpg".to_string(),
        ],
    );

    assert!(log.contains("[PROMPT]"));
    assert!(log.contains("运行通道: SDK"));
    assert!(log.contains("模型: gpt-5.4"));
    assert!(log.contains("推理强度: high"));
    assert!(log.contains("执行环境: 本地运行"));
    assert!(log.contains("工作目录: /tmp/demo"));
    assert!(log.contains("附带图片: 2 张"));
    assert!(log.contains("1. ui.png"));
    assert!(log.contains("任务标题:\n修复问题"));
}

#[test]
fn formats_prompt_log_with_ssh_runtime_context() {
    let log = format_session_prompt_log(
        CodexExecutionProvider::Cli,
        "gpt-5.4",
        "medium",
        EXECUTION_TARGET_SSH,
        Some("生产 SSH"),
        Some("10.0.0.8:22"),
        Some("root@10.0.0.8:22"),
        "/root/code/codex-ai",
        "任务标题:\n分析项目",
        &[],
    );

    assert!(log.contains("执行环境: SSH 远程运行"));
    assert!(log.contains("SSH 名称: 生产 SSH"));
    assert!(log.contains("SSH 主机/IP: 10.0.0.8:22"));
    assert!(log.contains("SSH 登录: root@10.0.0.8:22"));
}

#[test]
fn remote_sdk_bridge_command_expands_home_install_dir() {
    let command = build_remote_sdk_bridge_command(
        "~/.codex-ai/codex-sdk-runtime/ssh-1",
        Some("~/.nvm/versions/node/v22.0.0/bin/node"),
    );

    assert!(command.contains("install_dir=\"$HOME/.codex-ai/codex-sdk-runtime/ssh-1\""));
    assert!(
        command.contains("bridge_path=\"$HOME/.codex-ai/codex-sdk-runtime/ssh-1/sdk-bridge.mjs\"")
    );
    assert!(command.contains("cd \"$install_dir\" && exec node \"$bridge_path\""));
}

#[test]
fn builds_plan_prompt_with_required_sections_and_context() {
    let prompt = build_ai_generate_plan_prompt(
        "看板任务详情增加 AI 生成计划",
        "在任务详情里新增 AI 生成计划，并支持插入详情。",
        "todo",
        "high",
        &[
            "补后端命令".to_string(),
            "补前端预览".to_string(),
            "补插入确认弹框".to_string(),
        ],
    );

    assert!(prompt.contains("# 标题"));
    assert!(prompt.contains("## 目标与范围"));
    assert!(prompt.contains("## 实施步骤"));
    assert!(prompt.contains("## 验收与验证"));
    assert!(prompt.contains("## 风险与依赖"));
    assert!(prompt.contains("## 假设"));
    assert!(prompt.contains("任务标题：看板任务详情增加 AI 生成计划"));
    assert!(prompt.contains("当前状态：todo"));
    assert!(prompt.contains("当前优先级：high"));
    assert!(prompt.contains("1. 补后端命令"));
    assert!(prompt.contains("2. 补前端预览"));
    assert!(prompt.contains("不要假装你已经读取仓库"));
    assert!(prompt.contains("如果本次输入附带任务图片"));
}

#[test]
fn builds_commit_message_prompt_with_staged_changes() {
    let prompt = build_ai_generate_commit_message_prompt(
        "看板系统",
        Some("main"),
        Some("共 3 项变更（修改 2，新增 1）"),
        &[
            "修改 src/pages/ProjectDetailPage.tsx".to_string(),
            "新增 src/components/projects/ProjectGitRepoActionDialog.tsx".to_string(),
        ],
        "title_with_body",
    );

    assert!(prompt.contains("你是 Git commit message 助手"));
    assert!(prompt.contains("项目名称：看板系统"));
    assert!(prompt.contains("当前分支：main"));
    assert!(prompt.contains("工作区摘要：共 3 项变更（修改 2，新增 1）"));
    assert!(prompt.contains("- 修改 src/pages/ProjectDetailPage.tsx"));
    assert!(prompt.contains("- 新增 src/components/projects/ProjectGitRepoActionDialog.tsx"));
    assert!(prompt.contains("只返回最终 commit message"));
    assert!(prompt.contains("Conventional Commits 风格"));
    assert!(prompt.contains("第一行是 Conventional Commits 标题"));
    assert!(prompt.contains("补充 2 到 4 行正文"));
    assert!(prompt.contains("不要在标题或正文里出现“暂存”"));
    assert!(prompt.contains("不要因为输入来自暂存区就默认使用 chore"));
}

#[test]
fn builds_title_only_commit_message_prompt() {
    let prompt = build_ai_generate_commit_message_prompt(
        "设置中心",
        Some("feat/git-settings"),
        Some("共 2 项变更（修改 2）"),
        &["修改 src/pages/SettingsPage.tsx".to_string()],
        "title_only",
    );

    assert!(prompt.contains("输出必须是单行 Conventional Commits 标题"));
    assert!(prompt.contains("本次长度配置为“仅标题”"));
    assert!(prompt.contains("只输出单行标题，不要返回项目符号或多段内容"));
    assert!(!prompt.contains("补充 2 到 4 行正文"));
}

#[test]
fn title_only_commit_message_rejects_multiline_output() {
    let error =
        validate_generated_commit_message("feat: 调整设置入口\n\n补充提交详情", "title_only")
            .expect_err("title_only should reject multi-line output");

    assert!(error.contains("仅标题"));
}

#[test]
fn title_with_body_commit_message_accepts_multiline_output() {
    validate_generated_commit_message("feat: 调整设置入口\n\n补充提交详情", "title_with_body")
        .expect("title_with_body should allow body lines");
}

#[test]
fn commit_related_subject_is_not_treated_as_process_language_by_itself() {
    validate_generated_commit_message(
        "fix: 收紧提交信息生成校验\n\n避免仅标题模式误收多行正文",
        "title_with_body",
    )
    .expect("commit-related product change should be allowed");
}

#[test]
fn detects_process_language_in_commit_message_subject() {
    assert!(commit_message_uses_process_language(
        "chore: 核对首页页面暂存内容\n\n- 调整了页面文案"
    ));
    assert!(commit_message_uses_process_language(
        "fix: 更新工作区提交信息\n\n- 优化提交文案"
    ));
    assert!(!commit_message_uses_process_language(
        "fix: 调整首页说明文案\n\n- 将社区文案改为更准确的描述"
    ));
}

#[test]
fn builds_task_create_optimized_prompt_with_project_context() {
    let prompt = build_ai_optimize_prompt_prompt(
        "task_create",
        "看板系统",
        Some("桌面端任务协作应用"),
        Some("/tmp/kanban"),
        Some("新增 AI 优化提示词按钮"),
        Some("在新建任务里生成更准确的详情提示词"),
        None,
        None,
        None,
    )
    .expect("should build task_create prompt");

    assert!(prompt.contains("场景：新建任务"));
    assert!(prompt.contains("适合作为任务详情的中文正文"));
    assert!(prompt.contains("项目名称：看板系统"));
    assert!(prompt.contains("项目描述：桌面端任务协作应用"));
    assert!(prompt.contains("仓库路径：/tmp/kanban"));
    assert!(prompt.contains("标题：新增 AI 优化提示词按钮"));
    assert!(prompt.contains("描述：在新建任务里生成更准确的详情提示词"));
    assert!(prompt.contains("只返回可直接使用的中文正文"));
    assert!(prompt.contains("不要 Markdown 代码块"));
}

#[test]
fn builds_task_continue_optimized_prompt_with_follow_up_context() {
    let prompt = build_ai_optimize_prompt_prompt(
        "task_continue",
        "看板系统",
        None,
        None,
        None,
        Some("当前任务需要补充前端交互"),
        Some("继续完成 AI 优化提示词能力，并补上错误提示"),
        Some("看板新建任务支持 AI 优化提示词"),
        None,
    )
    .expect("should build task_continue prompt");

    assert!(prompt.contains("场景：任务继续对话"));
    assert!(prompt.contains("适合作为续聊输入的中文正文"));
    assert!(prompt.contains("项目描述：未填写"));
    assert!(prompt.contains("仓库路径：未填写"));
    assert!(prompt.contains("描述：当前任务需要补充前端交互"));
    assert!(prompt.contains("当前续聊输入：继续完成 AI 优化提示词能力，并补上错误提示"));
    assert!(prompt.contains("任务标题：看板新建任务支持 AI 优化提示词"));
}

#[test]
fn builds_session_continue_optimized_prompt_with_empty_placeholders() {
    let prompt = build_ai_optimize_prompt_prompt(
        "session_continue",
        "看板系统",
        None,
        None,
        None,
        None,
        None,
        Some("继续对话优化"),
        Some("最近一次处理了任务继续对话的续聊逻辑"),
    )
    .expect("should build session_continue prompt");

    assert!(prompt.contains("场景：Session 继续对话"));
    assert!(prompt.contains("适合作为续聊输入的中文正文"));
    assert!(prompt.contains("标题：未填写"));
    assert!(prompt.contains("描述：未填写"));
    assert!(prompt.contains("当前续聊输入：未填写"));
    assert!(prompt.contains("任务标题：继续对话优化"));
    assert!(prompt.contains("Session 摘要：最近一次处理了任务继续对话的续聊逻辑"));
    assert!(prompt.contains("如果当前输入为空或信息不足"));
}

#[test]
fn computes_added_modified_deleted_and_renamed_changes() {
    let baseline = HashMap::from([
        (
            "src/existing.ts".to_string(),
            snapshot_entry("src/existing.ts", ' ', 'M', None, Some("hash-old")),
        ),
        (
            "src/rename-old.ts".to_string(),
            snapshot_entry("src/rename-old.ts", ' ', 'M', None, Some("rename-hash")),
        ),
    ]);
    let end = HashMap::from([
        (
            "src/existing.ts".to_string(),
            snapshot_entry("src/existing.ts", ' ', 'M', None, Some("hash-new")),
        ),
        (
            "src/new-file.ts".to_string(),
            snapshot_entry("src/new-file.ts", '?', '?', None, Some("new-hash")),
        ),
        (
            "src/removed.ts".to_string(),
            snapshot_entry("src/removed.ts", ' ', 'D', None, None),
        ),
        (
            "src/rename-new.ts".to_string(),
            snapshot_entry(
                "src/rename-new.ts",
                'R',
                ' ',
                Some("src/rename-old.ts"),
                Some("rename-hash"),
            ),
        ),
    ]);

    let changes = compute_execution_session_file_changes_from_entries("/tmp", &baseline, &end)
        .expect("compute session file changes");

    assert_eq!(changes.len(), 4);
    assert_eq!(changes[0].path, "src/existing.ts");
    assert_eq!(changes[0].change_type, "modified");
    assert_eq!(changes[0].capture_mode, "git_fallback");
    assert_eq!(changes[1].path, "src/new-file.ts");
    assert_eq!(changes[1].change_type, "added");
    assert_eq!(changes[1].capture_mode, "git_fallback");
    assert_eq!(changes[2].path, "src/removed.ts");
    assert_eq!(changes[2].change_type, "deleted");
    assert_eq!(changes[2].capture_mode, "git_fallback");
    assert_eq!(changes[3].path, "src/rename-new.ts");
    assert_eq!(changes[3].change_type, "renamed");
    assert_eq!(changes[3].capture_mode, "git_fallback");
    assert_eq!(
        changes[3].previous_path.as_deref(),
        Some("src/rename-old.ts")
    );
}

#[test]
fn parses_sdk_file_change_event_lines() {
    let event = parse_sdk_file_change_event(
        "[CODEX_FILE_CHANGE] {\"changes\":[{\"kind\":\"modified\",\"path\":\"src/app.tsx\",\"previous_path\":\"src/old.tsx\"}]}",
    )
    .expect("parse sdk file change line");

    assert_eq!(event.changes.len(), 1);
    assert_eq!(event.changes[0].kind.as_deref(), Some("modified"));
    assert_eq!(event.changes[0].path.as_deref(), Some("src/app.tsx"));
    assert_eq!(
        event.changes[0].previous_path.as_deref(),
        Some("src/old.tsx")
    );
}

#[test]
fn skips_unchanged_renames_and_baseline_files() {
    let baseline = HashMap::from([(
        "src/renamed.ts".to_string(),
        snapshot_entry(
            "src/renamed.ts",
            'R',
            ' ',
            Some("src/original.ts"),
            Some("same-hash"),
        ),
    )]);
    let end = HashMap::from([(
        "src/renamed.ts".to_string(),
        snapshot_entry(
            "src/renamed.ts",
            'R',
            ' ',
            Some("src/original.ts"),
            Some("same-hash"),
        ),
    )]);

    let changes = compute_execution_session_file_changes_from_entries("/tmp", &baseline, &end)
        .expect("compute session file changes");

    assert!(changes.is_empty());
}

#[test]
fn ignores_baseline_only_files_when_hash_does_not_change() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let repo_root = std::env::temp_dir().join(format!(
        "codex-session-change-test-{}-{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(repo_root.join("src")).expect("create temp repo dir");
    fs::write(repo_root.join("src/stable.ts"), "const value = 1;\n").expect("write temp file");
    let baseline_hash = hash_worktree_path(repo_root.to_string_lossy().as_ref(), "src/stable.ts")
        .expect("hash temp file");

    let baseline = HashMap::from([(
        "src/stable.ts".to_string(),
        snapshot_entry("src/stable.ts", ' ', 'M', None, baseline_hash.as_deref()),
    )]);
    let end = HashMap::new();

    let changes = compute_execution_session_file_changes_from_entries(
        repo_root.to_string_lossy().as_ref(),
        &baseline,
        &end,
    )
    .expect("compute session file changes");

    assert!(changes.is_empty());
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn attaches_before_snapshot_for_newly_modified_tracked_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let repo_root = std::env::temp_dir().join(format!(
        "codex-session-detail-test-{}-{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(repo_root.join("src")).expect("create temp repo dir");
    fs::write(repo_root.join("src/changed.ts"), "const value = 1;\n").expect("write initial file");

    let run_git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .args(args)
            .status()
            .expect("run git command");
        assert!(status.success(), "git {:?} should succeed", args);
    };

    run_git(&["init", "-q"]);
    run_git(&["config", "user.email", "codex@example.com"]);
    run_git(&["config", "user.name", "Codex"]);
    run_git(&["add", "src/changed.ts"]);
    run_git(&["commit", "-q", "-m", "init"]);

    fs::write(repo_root.join("src/changed.ts"), "const value = 2;\n").expect("write updated file");

    let changes = attach_session_file_change_details(
        repo_root.to_string_lossy().as_ref(),
        &HashMap::new(),
        vec![CodexSessionFileChangeInput {
            path: "src/changed.ts".to_string(),
            change_type: "modified".to_string(),
            capture_mode: CodexExecutionProvider::Sdk.capture_mode().to_string(),
            previous_path: None,
            detail: None,
        }],
    );

    let detail = changes[0]
        .detail
        .as_ref()
        .expect("detail should be attached");
    assert_eq!(detail.before_status, "text");
    assert_eq!(detail.before_text.as_deref(), Some("const value = 1;\n"));
    assert_eq!(detail.after_status, "text");
    assert_eq!(detail.after_text.as_deref(), Some("const value = 2;\n"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn normalizes_sdk_absolute_paths_to_repo_relative_paths() {
    let repo_root = std::env::temp_dir().join(format!(
        "codex-session-normalize-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(repo_root.join("src")).expect("create temp repo dir");

    let change = normalize_session_file_change_paths(
        repo_root.to_string_lossy().as_ref(),
        CodexSessionFileChangeInput {
            path: repo_root
                .join("src/changed.ts")
                .to_string_lossy()
                .to_string(),
            change_type: "modified".to_string(),
            capture_mode: CodexExecutionProvider::Sdk.capture_mode().to_string(),
            previous_path: Some(repo_root.join("src/old.ts").to_string_lossy().to_string()),
            detail: None,
        },
    );

    assert_eq!(change.path, "src/changed.ts");
    assert_eq!(change.previous_path.as_deref(), Some("src/old.ts"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn attaches_before_snapshot_for_sdk_absolute_paths_inside_repo() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let repo_root = std::env::temp_dir().join(format!(
        "codex-session-absolute-detail-test-{}-{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(repo_root.join("src")).expect("create temp repo dir");
    fs::write(repo_root.join("src/changed.ts"), "const value = 1;\n").expect("write initial file");

    let run_git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .args(args)
            .status()
            .expect("run git command");
        assert!(status.success(), "git {:?} should succeed", args);
    };

    run_git(&["init", "-q"]);
    run_git(&["config", "user.email", "codex@example.com"]);
    run_git(&["config", "user.name", "Codex"]);
    run_git(&["add", "src/changed.ts"]);
    run_git(&["commit", "-q", "-m", "init"]);

    fs::write(repo_root.join("src/changed.ts"), "const value = 2;\n").expect("write updated file");

    let absolute_path = repo_root
        .join("src/changed.ts")
        .to_string_lossy()
        .to_string();
    let changes = attach_session_file_change_details(
        repo_root.to_string_lossy().as_ref(),
        &HashMap::new(),
        vec![CodexSessionFileChangeInput {
            path: absolute_path,
            change_type: "modified".to_string(),
            capture_mode: CodexExecutionProvider::Sdk.capture_mode().to_string(),
            previous_path: None,
            detail: None,
        }],
    );

    assert_eq!(changes[0].path, "src/changed.ts");
    let detail = changes[0]
        .detail
        .as_ref()
        .expect("detail should be attached");
    assert_eq!(detail.before_status, "text");
    assert_eq!(detail.before_text.as_deref(), Some("const value = 1;\n"));
    assert_eq!(detail.after_status, "text");
    assert_eq!(detail.after_text.as_deref(), Some("const value = 2;\n"));

    let _ = fs::remove_dir_all(&repo_root);
}
