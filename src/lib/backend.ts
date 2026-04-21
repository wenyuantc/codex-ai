import { invoke } from "@tauri-apps/api/core";
import { normalizeProject } from "./projects";
import type {
  AppNotification,
  ArtifactCaptureMode,
  CodexHealthCheck,
  CodexSdkInstallResult,
  CodexSettings,
  EmployeeRuntimeStatus,
  CodexRuntimeStatus,
  CodexSessionLogLine,
  CodexSessionListItem,
  CodexSessionResumePreview,
  CodexSessionFileChangeDetail,
  DatabaseBackupResult,
  DatabaseRestoreResult,
  Comment,
  ConfirmGitActionResult,
  Employee,
  EnvironmentMode,
  GitActionRequestResult,
  GlobalSearchItemType,
  GlobalSearchResponse,
  GitActionType,
  PreparedTaskGitExecution,
  Project,
  ProjectGitCommitDetail,
  ProjectGitCommitHistory,
  ProjectGitFilePreview,
  ProjectGitOverview,
  TaskGitCommitOverview,
  RemoteCodexHealthCheck,
  RemoteCodexSdkInstallResult,
  RemoteCodexSettings,
  SshConfig,
  SshPasswordProbeResult,
  Subtask,
  Task,
  TaskAutomationMode,
  TaskAutomationState,
  TaskAttachment,
  TaskGitContext,
  TaskExecutionChangeHistoryItem,
  TaskLatestReview,
} from "./types";

type RawHealthCheck = CodexHealthCheck & {
  password_auth_available?: boolean | null;
  execution_target?: string | null;
};

type RawSshConfig = SshConfig & {
  password_auth_available?: boolean | null;
};

type RawSshPasswordProbeResult = Omit<SshPasswordProbeResult, "status" | "execution_allowed"> & {
  auth_type?: "key" | "password" | null;
  status?: string | null;
  execution_allowed?: boolean | null;
  supported?: boolean | null;
  target_host_label?: string | null;
};

function normalizeExecutionTarget(value: string | null | undefined): EnvironmentMode {
  return value === "ssh" ? "ssh" : "local";
}

function normalizeArtifactCaptureMode(value: string | null | undefined): ArtifactCaptureMode {
  switch (value) {
    case "ssh_full":
    case "ssh_git_status":
    case "ssh_none":
      return value;
    default:
      return "local_full";
  }
}

function normalizeSessionListItem(session: CodexSessionListItem): CodexSessionListItem {
  return {
    ...session,
    execution_target: normalizeExecutionTarget(session.execution_target),
    ssh_config_id: session.ssh_config_id ?? null,
    target_host_label: session.target_host_label ?? null,
    artifact_capture_mode: normalizeArtifactCaptureMode(session.artifact_capture_mode),
  };
}

function normalizeHealthCheck(health: RawHealthCheck): CodexHealthCheck {
  const passwordExecutionAllowed =
    health.password_execution_allowed
    ?? health.password_auth_available
    ?? false;
  return {
    ...health,
    execution_target: normalizeExecutionTarget(health.execution_target),
    ssh_config_id: health.ssh_config_id ?? null,
    target_host_label: health.target_host_label ?? null,
    password_probe_status: health.password_probe_status ?? null,
    password_probe_message: health.password_probe_message ?? null,
    password_execution_allowed: passwordExecutionAllowed,
  };
}

function normalizeSshConfig(config: RawSshConfig): SshConfig {
  const passwordExecutionAllowed =
    config.password_execution_allowed
    ?? config.password_auth_available
    ?? false;
  return {
    ...config,
    port: Number(config.port ?? 22) || 22,
    private_key_path: config.private_key_path ?? null,
    known_hosts_mode: config.known_hosts_mode ?? "accept-new",
    password_configured: config.password_configured ?? false,
    passphrase_configured: config.passphrase_configured ?? false,
    password_probe_status: config.password_probe_status ?? null,
    password_probe_message: config.password_probe_message ?? null,
    password_execution_allowed: passwordExecutionAllowed,
    last_checked_at: config.last_checked_at ?? null,
    last_check_status: config.last_check_status ?? null,
    last_check_message: config.last_check_message ?? null,
  };
}

function normalizePasswordProbeResult(result: RawSshPasswordProbeResult): SshPasswordProbeResult {
  const executionAllowed = result.execution_allowed ?? result.supported ?? false;
  const status =
    result.status === "passed"
      ? "supported"
      : result.status === "supported" || result.status === "unsupported" || result.status === "failed"
        ? result.status
        : "failed";
  return {
    ...result,
    auth_type: result.auth_type === "key" ? "key" : "password",
    execution_allowed: executionAllowed,
    target_host_label: result.target_host_label ?? null,
    status,
  };
}

export interface UpdateCodexSettingsInput {
  task_sdk_enabled?: boolean;
  one_shot_sdk_enabled?: boolean;
  one_shot_model?: string;
  one_shot_reasoning_effort?: string;
  task_automation_default_enabled?: boolean;
  task_automation_max_fix_rounds?: number;
  task_automation_failure_strategy?: "blocked" | "manual_control";
  git_preferences?: Partial<CodexSettings["git_preferences"]>;
  node_path_override?: string | null;
}

export interface CreateProjectInput {
  name: string;
  description?: string | null;
  project_type?: EnvironmentMode;
  repo_path?: string | null;
  ssh_config_id?: string | null;
  remote_repo_path?: string | null;
}

export interface UpdateProjectInput {
  name?: string;
  description?: string | null;
  status?: string;
  project_type?: EnvironmentMode;
  repo_path?: string | null;
  ssh_config_id?: string | null;
  remote_repo_path?: string | null;
}

export interface CreateSshConfigInput {
  name: string;
  host: string;
  port?: number;
  username: string;
  auth_type: "key" | "password";
  private_key_path?: string | null;
  password?: string | null;
  passphrase?: string | null;
  known_hosts_mode?: string;
}

export interface UpdateSshConfigInput {
  name?: string;
  host?: string;
  port?: number;
  username?: string;
  auth_type?: "key" | "password";
  private_key_path?: string | null;
  password?: string | null;
  passphrase?: string | null;
  known_hosts_mode?: string;
}

export interface CreateEmployeeInput {
  name: string;
  role: string;
  model?: string;
  reasoning_effort?: string;
  specialization?: string | null;
  system_prompt?: string | null;
  project_id?: string | null;
}

export interface UpdateEmployeeInput {
  name?: string;
  role?: string;
  model?: string;
  reasoning_effort?: string;
  specialization?: string | null;
  system_prompt?: string | null;
  project_id?: string | null;
  status?: string;
}

export interface CreateTaskInput {
  title: string;
  description?: string | null;
  priority?: string;
  project_id: string;
  use_worktree?: boolean;
  assignee_id?: string | null;
  reviewer_id?: string | null;
  attachment_source_paths?: string[];
}

export interface UpdateTaskInput {
  title?: string;
  description?: string | null;
  status?: string;
  priority?: string;
  assignee_id?: string | null;
  reviewer_id?: string | null;
  complexity?: number | null;
  ai_suggestion?: string | null;
  last_codex_session_id?: string | null;
  last_review_session_id?: string | null;
}

export interface SetTaskAutomationModeInput {
  task_id: string;
  automation_mode?: TaskAutomationMode | null;
}

export interface SearchGlobalInput {
  query: string;
  types?: GlobalSearchItemType[];
  limit?: number;
  offset?: number;
  environment_mode?: EnvironmentMode;
}

export async function healthCheck(): Promise<CodexHealthCheck> {
  return normalizeHealthCheck(await invoke<CodexHealthCheck>("health_check"));
}

export async function getRemoteHealthCheck(sshConfigId: string): Promise<RemoteCodexHealthCheck> {
  const normalized = normalizeHealthCheck(
    await invoke<RemoteCodexHealthCheck>("validate_remote_codex_health", { sshConfigId }),
  );
  return {
    ...normalized,
    execution_target: "ssh",
    ssh_config_id: normalized.ssh_config_id ?? sshConfigId,
    target_host_label: normalized.target_host_label ?? null,
    password_probe_status: normalized.password_probe_status ?? null,
    password_probe_message: normalized.password_probe_message ?? null,
    password_execution_allowed: normalized.password_execution_allowed ?? false,
  } as RemoteCodexHealthCheck;
}

export async function backupDatabase(destinationPath: string): Promise<DatabaseBackupResult> {
  return invoke("backup_database", { destinationPath });
}

export async function restoreDatabase(sourcePath: string): Promise<DatabaseRestoreResult> {
  return invoke("restore_database", { sourcePath });
}

export async function openDatabaseFolder(): Promise<void> {
  return invoke("open_database_folder");
}

export async function showMainWindow(): Promise<void> {
  return invoke("show_main_window");
}

export async function readImageFile(path: string): Promise<number[]> {
  return invoke("read_image_file", { path });
}

export async function openTaskAttachment(path: string): Promise<void> {
  return invoke("open_task_attachment", { path });
}

export async function getCodexSessionStatus(employeeId: string): Promise<CodexRuntimeStatus> {
  return invoke("get_codex_session_status", { employeeId });
}

export async function getEmployeeRuntimeStatus(employeeId: string): Promise<EmployeeRuntimeStatus> {
  return invoke("get_employee_runtime_status", { employeeId });
}

export async function searchGlobal(input: SearchGlobalInput): Promise<GlobalSearchResponse> {
  return invoke("search_global", { payload: input });
}

export async function syncSystemNotifications(
  environmentMode?: EnvironmentMode,
  sshConfigId?: string | null,
): Promise<void> {
  return invoke("sync_system_notifications", {
    environmentMode: environmentMode ?? null,
    sshConfigId: sshConfigId ?? null,
  });
}

export async function listNotifications(limit?: number): Promise<AppNotification[]> {
  return invoke("list_notifications", { limit: limit ?? null });
}

export async function markNotificationRead(id: string): Promise<AppNotification> {
  return invoke("mark_notification_read", { id });
}

export async function markAllNotificationsRead(): Promise<number> {
  return invoke("mark_all_notifications_read");
}

export async function listCodexSessions(): Promise<CodexSessionListItem[]> {
  const sessions = await invoke<CodexSessionListItem[]>("list_codex_sessions");
  return sessions.map(normalizeSessionListItem);
}

export async function prepareCodexSessionResume(
  sessionId: string,
): Promise<CodexSessionResumePreview> {
  return invoke("prepare_codex_session_resume", { sessionId });
}

export async function getCodexSessionLogLines(sessionId: string): Promise<CodexSessionLogLine[]> {
  return invoke("get_codex_session_log_lines", { sessionId });
}

export async function getProjectGitOverview(projectId: string): Promise<ProjectGitOverview> {
  return invoke("get_project_git_overview", { projectId });
}

export async function listProjectGitCommits(
  projectId: string,
  offset?: number | null,
  limit?: number | null,
): Promise<ProjectGitCommitHistory> {
  return invoke("list_project_git_commits", {
    projectId,
    offset: offset ?? null,
    limit: limit ?? null,
  });
}

export async function getProjectGitCommitDetail(
  projectId: string,
  commitSha: string,
): Promise<ProjectGitCommitDetail> {
  return invoke("get_project_git_commit_detail", { projectId, commitSha });
}

export async function getProjectGitFilePreview(
  projectId: string,
  relativePath: string,
  previousPath?: string | null,
  changeType?: string | null,
): Promise<ProjectGitFilePreview> {
  return invoke("get_project_git_file_preview", {
    projectId,
    relativePath,
    previousPath: previousPath ?? null,
    changeType: changeType ?? null,
  });
}

export async function getProjectGitCommitFilePreview(
  projectId: string,
  commitSha: string,
  relativePath: string,
  previousPath?: string | null,
  changeType?: string | null,
): Promise<ProjectGitFilePreview> {
  return invoke("get_project_git_commit_file_preview", {
    projectId,
    commitSha,
    relativePath,
    previousPath: previousPath ?? null,
    changeType: changeType ?? null,
  });
}

export async function stageProjectGitFile(projectId: string, relativePath: string): Promise<string> {
  return invoke("stage_project_git_file", { projectId, relativePath });
}

export async function unstageProjectGitFile(projectId: string, relativePath: string): Promise<string> {
  return invoke("unstage_project_git_file", { projectId, relativePath });
}

export async function stageAllProjectGitFiles(projectId: string): Promise<string> {
  return invoke("stage_all_project_git_files", { projectId });
}

export async function unstageAllProjectGitFiles(projectId: string): Promise<string> {
  return invoke("unstage_all_project_git_files", { projectId });
}

export async function commitProjectGitChanges(projectId: string, message: string): Promise<string> {
  return invoke("commit_project_git_changes", { projectId, message });
}

export async function pushProjectGitBranch(
  projectId: string,
  remoteName?: string | null,
  branchName?: string | null,
  forceMode?: "none" | "force" | "force_with_lease" | null,
): Promise<string> {
  return invoke("push_project_git_branch", {
    projectId,
    remoteName: remoteName ?? null,
    branchName: branchName ?? null,
    forceMode: forceMode ?? null,
  });
}

export async function pullProjectGitBranch(
  projectId: string,
  remoteName?: string | null,
  branchName?: string | null,
  mode?: "ff_only" | "rebase" | null,
  autoStash?: boolean | null,
): Promise<string> {
  return invoke("pull_project_git_branch", {
    projectId,
    remoteName: remoteName ?? null,
    branchName: branchName ?? null,
    mode: mode ?? null,
    autoStash: autoStash ?? null,
  });
}

export async function listTaskGitContexts(projectId: string): Promise<TaskGitContext[]> {
  return invoke("list_task_git_contexts", { projectId });
}

export async function prepareTaskGitExecution(
  taskId: string,
  preferredTargetBranch?: string,
): Promise<PreparedTaskGitExecution> {
  return invoke("prepare_task_git_execution", {
    taskId,
    preferredTargetBranch: preferredTargetBranch ?? null,
  });
}

export async function requestGitAction(
  taskGitContextId: string,
  actionType: GitActionType,
  payload: Record<string, unknown>,
): Promise<GitActionRequestResult> {
  return invoke("request_git_action", {
    input: {
      task_git_context_id: taskGitContextId,
      action_type: actionType,
      payload,
    },
  });
}

export async function confirmGitAction(
  taskGitContextId: string,
  token: string,
): Promise<ConfirmGitActionResult> {
  return invoke("confirm_git_action", { taskGitContextId, token });
}

export async function cancelGitAction(
  taskGitContextId: string,
  token?: string,
): Promise<TaskGitContext> {
  return invoke("cancel_git_action", {
    taskGitContextId,
    token: token ?? null,
  });
}

export async function refreshTaskGitContext(taskGitContextId: string): Promise<TaskGitContext> {
  return invoke("refresh_task_git_context", { taskGitContextId });
}

export async function getTaskGitCommitOverview(taskGitContextId: string): Promise<TaskGitCommitOverview> {
  return invoke("get_task_git_commit_overview", { taskGitContextId });
}

export async function stageAllTaskGitFiles(taskGitContextId: string): Promise<string> {
  return invoke("stage_all_task_git_files", { taskGitContextId });
}

export async function commitTaskGitChanges(taskGitContextId: string, message: string): Promise<string> {
  return invoke("commit_task_git_changes", { taskGitContextId, message });
}

export async function reconcileTaskGitContext(taskGitContextId: string): Promise<TaskGitContext> {
  return invoke("reconcile_task_git_context", { taskGitContextId });
}

export async function deleteTaskGitContextRecord(taskGitContextId: string): Promise<string> {
  return invoke("delete_task_git_context_record", { taskGitContextId });
}

export async function getTaskLatestReview(taskId: string): Promise<TaskLatestReview | null> {
  return invoke("get_task_latest_review", { taskId });
}

export async function getTaskExecutionChangeHistory(
  taskId: string,
): Promise<TaskExecutionChangeHistoryItem[]> {
  return invoke("get_task_execution_change_history", { taskId });
}

export async function getCodexSessionExecutionChangeHistory(
  sessionId: string,
): Promise<TaskExecutionChangeHistoryItem> {
  return invoke("get_codex_session_execution_change_history", { sessionId });
}

export async function getCodexSessionFileChangeDetail(
  changeId: string,
): Promise<CodexSessionFileChangeDetail> {
  return invoke("get_codex_session_file_change_detail", { changeId });
}

export async function startTaskCodeReview(taskId: string): Promise<void> {
  return invoke("start_task_code_review", { taskId });
}

export async function setTaskAutomationMode(input: SetTaskAutomationModeInput): Promise<Task> {
  return invoke("set_task_automation_mode", { payload: input });
}

export async function getTaskAutomationState(taskId: string): Promise<TaskAutomationState | null> {
  return invoke("get_task_automation_state", { taskId });
}

export async function restartTaskAutomation(taskId: string): Promise<void> {
  return invoke("restart_task_automation", { taskId });
}

export async function getCodexSettings(): Promise<CodexSettings> {
  return invoke("get_codex_settings");
}

export async function getRemoteCodexSettings(sshConfigId: string): Promise<RemoteCodexSettings> {
  return invoke("get_remote_codex_settings", { sshConfigId });
}

export async function updateCodexSettings(
  updates: UpdateCodexSettingsInput,
): Promise<CodexSettings> {
  return invoke("update_codex_settings", { updates });
}

export async function updateRemoteCodexSettings(
  sshConfigId: string,
  updates: UpdateCodexSettingsInput,
): Promise<RemoteCodexSettings> {
  return invoke("update_remote_codex_settings", {
    payload: {
      ssh_config_id: sshConfigId,
      updates,
    },
  });
}

export async function installCodexSdk(): Promise<CodexSdkInstallResult> {
  return invoke("install_codex_sdk");
}

export async function installRemoteCodexSdk(
  sshConfigId: string,
): Promise<RemoteCodexSdkInstallResult> {
  return invoke("install_remote_codex_sdk", { sshConfigId });
}

export async function createProject(input: CreateProjectInput): Promise<Project> {
  const project = await invoke<Project>("create_project", { payload: input });
  return normalizeProject(project);
}

export async function updateProject(id: string, updates: UpdateProjectInput): Promise<Project> {
  const project = await invoke<Project>("update_project", { id, updates });
  return normalizeProject(project);
}

export async function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
}

export async function listSshConfigs(): Promise<SshConfig[]> {
  const configs = await invoke<SshConfig[]>("list_ssh_configs");
  return configs.map(normalizeSshConfig);
}

export async function createSshConfig(input: CreateSshConfigInput): Promise<SshConfig> {
  return normalizeSshConfig(await invoke<SshConfig>("create_ssh_config", { payload: input }));
}

export async function updateSshConfig(id: string, updates: UpdateSshConfigInput): Promise<SshConfig> {
  return normalizeSshConfig(await invoke<SshConfig>("update_ssh_config", { id, updates }));
}

export async function deleteSshConfig(id: string): Promise<void> {
  return invoke("delete_ssh_config", { id });
}

export async function runSshPasswordProbe(id: string): Promise<SshPasswordProbeResult> {
  return normalizePasswordProbeResult(
    await invoke<SshPasswordProbeResult>("probe_ssh_password_auth", { sshConfigId: id }),
  );
}

export async function createEmployee(input: CreateEmployeeInput): Promise<Employee> {
  return invoke("create_employee", { payload: input });
}

export async function updateEmployee(id: string, updates: UpdateEmployeeInput): Promise<Employee> {
  return invoke("update_employee", { id, updates });
}

export async function deleteEmployee(id: string): Promise<void> {
  return invoke("delete_employee", { id });
}

export async function updateEmployeeStatus(id: string, status: string): Promise<Employee> {
  return invoke("update_employee_status", { id, status });
}

export async function createTask(input: CreateTaskInput): Promise<Task> {
  return invoke("create_task", { payload: input });
}

export async function addTaskAttachments(taskId: string, sourcePaths: string[]): Promise<TaskAttachment[]> {
  return invoke("add_task_attachments", { taskId, sourcePaths });
}

export async function deleteTaskAttachment(id: string): Promise<void> {
  return invoke("delete_task_attachment", { id });
}

export async function updateTask(id: string, updates: UpdateTaskInput): Promise<Task> {
  return invoke("update_task", { id, updates });
}

export async function updateTaskStatus(id: string, status: string): Promise<Task> {
  return invoke("update_task_status", { id, status });
}

export async function deleteTask(id: string): Promise<void> {
  return invoke("delete_task", { id });
}

export async function createSubtask(taskId: string, title: string): Promise<Subtask> {
  return invoke("create_subtask", { payload: { task_id: taskId, title } });
}

export async function updateSubtaskStatus(id: string, status: string): Promise<Subtask> {
  return invoke("update_subtask_status", { id, status });
}

export async function deleteSubtask(id: string): Promise<void> {
  return invoke("delete_subtask", { id });
}

export async function createComment(
  taskId: string,
  content: string,
  employeeId?: string | null,
  isAiGenerated?: boolean,
): Promise<Comment> {
  return invoke("create_comment", {
    payload: {
      task_id: taskId,
      employee_id: employeeId ?? null,
      content,
      is_ai_generated: isAiGenerated ?? false,
    },
  });
}
