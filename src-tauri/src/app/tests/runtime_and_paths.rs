use super::*;

fn sample_codex_settings() -> CodexSettings {
    CodexSettings {
        task_sdk_enabled: true,
        one_shot_sdk_enabled: true,
        one_shot_model: "gpt-5.4".to_string(),
        one_shot_reasoning_effort: "high".to_string(),
        task_automation_default_enabled: false,
        task_automation_max_fix_rounds: 3,
        task_automation_failure_strategy: "blocked".to_string(),
        git_preferences: GitPreferences {
            default_task_use_worktree: false,
            worktree_location_mode: "repo_sibling_hidden".to_string(),
            worktree_custom_root: None,
            ai_commit_message_length: "title_with_body".to_string(),
            ai_commit_model_source: "inherit_one_shot".to_string(),
            ai_commit_model: "gpt-5.4".to_string(),
            ai_commit_reasoning_effort: "high".to_string(),
        },
        node_path_override: None,
        sdk_install_dir: "~/.codex-ai/codex-sdk-runtime/ssh-1".to_string(),
        one_shot_preferred_provider: "sdk".to_string(),
    }
}

#[test]
fn rejects_missing_project_repo_path() {
    let path = format!(
        "{}/codex-ai-test-missing-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );

    assert!(validate_project_repo_path(Some(&path)).is_err());
}

#[test]
fn validates_git_runtime_directory() {
    let root = std::env::temp_dir().join(format!(
        "codex-ai-runtime-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let git_dir = root.join(".git");
    fs::create_dir_all(&git_dir).expect("create .git dir");

    let validated = validate_runtime_working_dir(Some(root.to_string_lossy().as_ref()))
        .expect("valid git working dir");
    assert!(validated.contains("codex-ai-runtime"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn strips_windows_verbatim_path_prefixes() {
    assert_eq!(
        normalize_runtime_path_string(r"\\?\D:\repo\demo"),
        r"D:\repo\demo"
    );
    assert_eq!(
        normalize_runtime_path_string(r"\\?\UNC\server\share\demo"),
        r"\\server\share\demo"
    );
    assert_eq!(
        normalize_runtime_path_string(r"/tmp/codex-ai"),
        "/tmp/codex-ai"
    );
}

#[test]
fn remote_shell_path_expression_expands_home_prefix() {
    assert_eq!(
        remote_shell_path_expression("~/codex sdk"),
        "\"$HOME/codex sdk\""
    );
    assert_eq!(
        remote_shell_path_expression("${HOME}/runtime"),
        "\"$HOME/runtime\""
    );
}

#[test]
fn remote_shell_command_bootstraps_common_node_paths() {
    let command = build_remote_shell_command(
        "codex --version",
        Some("~/.nvm/versions/node/v22.0.0/bin/node"),
    );

    assert!(command.starts_with("sh -lc "));
    assert!(command.contains(".nvm/versions/node/*/bin"));
    assert!(command.contains(". \"$HOME/.nvm/nvm.sh\""));
    assert!(command.contains(".local/share/pnpm"));
    assert!(command.contains("$HOME/.nvm/versions/node/v22.0.0/bin"));
}

#[test]
fn remote_shell_bootstrap_prefers_nvm_default_alias_before_fallback_versions() {
    let root = std::env::temp_dir().join(format!("codex-nvm-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join(".nvm/versions/node/v18.20.0/bin")).expect("create v18 dir");
    fs::create_dir_all(root.join(".nvm/versions/node/v9.11.2/bin")).expect("create v9 dir");
    fs::write(
        root.join(".nvm/versions/node/v18.20.0/bin/node"),
        "#!/bin/sh\nexit 0\n",
    )
    .expect("write fake v18 node");
    fs::write(
        root.join(".nvm/versions/node/v9.11.2/bin/node"),
        "#!/bin/sh\nexit 0\n",
    )
    .expect("write fake v9 node");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(
            root.join(".nvm/versions/node/v18.20.0/bin/node"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("chmod fake v18 node");
        fs::set_permissions(
            root.join(".nvm/versions/node/v9.11.2/bin/node"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("chmod fake v9 node");
    }
    fs::write(
        root.join(".nvm/nvm.sh"),
        r#"nvm() {
  if [ "$1" = "which" ] && [ "$2" = "default" ]; then
    printf '%s\n' "$HOME/.nvm/versions/node/v18.20.0/bin/node"
    return 0
  fi
  return 1
}
"#,
    )
    .expect("write fake nvm script");

    let output = Command::new("sh")
        .env("HOME", &root)
        .arg("-lc")
        .arg(build_remote_shell_command("printf '%s' \"$PATH\"", None))
        .output()
        .expect("run bootstrap command");
    assert!(output.status.success(), "bootstrap shell should succeed");

    let path = String::from_utf8_lossy(&output.stdout).to_string();
    let expected_v18 = root
        .join(".nvm/versions/node/v18.20.0/bin")
        .to_string_lossy()
        .to_string();
    let expected_v9 = root
        .join(".nvm/versions/node/v9.11.2/bin")
        .to_string_lossy()
        .to_string();

    assert!(path.starts_with(&format!("{expected_v18}:")));
    assert!(path.contains(&expected_v9));
    assert!(
        path.find(&expected_v18).unwrap_or(usize::MAX)
            < path.find(&expected_v9).unwrap_or(usize::MAX)
    );

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn remote_runtime_health_falls_back_to_exec_when_sdk_is_missing() {
    let settings = sample_codex_settings();
    let values = HashMap::from([
        ("CODEX_STATUS".to_string(), "0".to_string()),
        ("CODEX_VERSION".to_string(), "0.34.0".to_string()),
        ("NODE_STATUS".to_string(), "0".to_string()),
        ("NODE_VERSION".to_string(), "v22.15.0".to_string()),
        ("SDK_INSTALLED".to_string(), "0".to_string()),
        ("SDK_VERSION".to_string(), "".to_string()),
    ]);

    let runtime = build_remote_codex_runtime_health(&settings, &values, "");

    assert_eq!(runtime.task_execution_effective_provider, "exec");
    assert_eq!(runtime.one_shot_effective_provider, "exec");
    assert!(runtime.status_message.contains("未安装"));
}

#[test]
fn sdk_notification_unavailable_follows_effective_provider() {
    assert!(sdk_notification_unavailable(true, false, "exec", "sdk"));
    assert!(sdk_notification_unavailable(false, true, "sdk", "exec"));
    assert!(!sdk_notification_unavailable(true, true, "sdk", "sdk"));
    assert!(!sdk_notification_unavailable(false, false, "exec", "exec"));
}

#[test]
fn remote_runtime_health_rejects_unsupported_node_versions() {
    let settings = sample_codex_settings();
    let values = HashMap::from([
        ("CODEX_STATUS".to_string(), "0".to_string()),
        ("CODEX_VERSION".to_string(), "0.34.0".to_string()),
        ("NODE_STATUS".to_string(), "0".to_string()),
        ("NODE_VERSION".to_string(), "v9.11.2".to_string()),
        ("SDK_INSTALLED".to_string(), "1".to_string()),
        ("SDK_VERSION".to_string(), "0.12.0".to_string()),
    ]);

    let runtime = build_remote_codex_runtime_health(&settings, &values, "");

    assert_eq!(runtime.task_execution_effective_provider, "exec");
    assert_eq!(runtime.one_shot_effective_provider, "exec");
    assert!(runtime.status_message.contains("Node 版本过低"));
    assert!(sdk_notification_unavailable(
        settings.task_sdk_enabled,
        settings.one_shot_sdk_enabled,
        &runtime.task_execution_effective_provider,
        &runtime.one_shot_effective_provider,
    ));
}

#[test]
fn resolve_project_task_default_settings_falls_back_to_local_when_remote_load_fails() {
    let settings = resolve_project_task_default_settings(
        PROJECT_TYPE_SSH,
        Some("ssh-1"),
        || Ok("local".to_string()),
        |_| Err("remote settings broken".to_string()),
    );

    assert_eq!(settings.as_deref(), Some("local"));
}

#[test]
fn resolve_project_task_default_settings_falls_back_to_local_when_ssh_config_missing() {
    let settings = resolve_project_task_default_settings(
        PROJECT_TYPE_SSH,
        None,
        || Ok("local".to_string()),
        |_| Ok("remote".to_string()),
    );

    assert_eq!(settings.as_deref(), Some("local"));
}

#[test]
fn resolve_project_task_default_settings_keeps_local_fallback_behavior() {
    let settings = resolve_project_task_default_settings(
        PROJECT_TYPE_LOCAL,
        None,
        || Err("local settings broken".to_string()),
        |_| Ok("remote".to_string()),
    );

    assert!(settings.is_none());
}
