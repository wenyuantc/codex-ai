export type ProjectType = "local" | "ssh";
export type EnvironmentMode = ProjectType;
export type SshAuthType = "key" | "password";
export type ArtifactCaptureMode = "local_full" | "ssh_full" | "ssh_git_status" | "ssh_none";
export type SshPasswordProbeStatus = "unknown" | "supported" | "unsupported" | "failed";
export type TaskGitContextState =
  | "provisioning"
  | "ready"
  | "running"
  | "merge_ready"
  | "action_pending"
  | "completed"
  | "failed"
  | "drifted";
export type GitActionType =
  | "merge"
  | "push"
  | "rebase"
  | "cherry_pick"
  | "stash"
  | "unstash"
  | "cleanup_worktree";
export type ProjectGitRepoActionType = "commit" | "push" | "pull";

export interface Project {
  id: string;
  name: string;
  description: string | null;
  status: string;
  repo_path: string | null;
  project_type: ProjectType;
  ssh_config_id: string | null;
  remote_repo_path: string | null;
  created_at: string;
  updated_at: string;
}

export interface SshConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: SshAuthType;
  private_key_path: string | null;
  known_hosts_mode: string;
  password_configured: boolean;
  passphrase_configured: boolean;
  password_probe_status: SshPasswordProbeStatus | null;
  password_probe_message: string | null;
  password_execution_allowed: boolean;
  password_auth_available?: boolean;
  last_checked_at: string | null;
  last_check_status: string | null;
  last_check_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface Employee {
  id: string;
  name: string;
  role: string;
  model: string;
  reasoning_effort: string;
  status: string;
  specialization: string | null;
  system_prompt: string | null;
  project_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface Task {
  id: string;
  title: string;
  description: string | null;
  status: string;
  priority: string;
  project_id: string;
  use_worktree: boolean;
  assignee_id: string | null;
  reviewer_id: string | null;
  complexity: number | null;
  ai_suggestion: string | null;
  automation_mode: TaskAutomationMode | null;
  last_codex_session_id: string | null;
  last_review_session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface TaskAttachment {
  id: string;
  task_id: string;
  original_name: string;
  stored_path: string;
  mime_type: string;
  file_size: number;
  sort_order: number;
  created_at: string;
}

export interface Subtask {
  id: string;
  task_id: string;
  title: string;
  status: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface Comment {
  id: string;
  task_id: string;
  employee_id: string | null;
  content: string;
  is_ai_generated: number;
  created_at: string;
}

export interface ActivityLog {
  id: string;
  employee_id: string | null;
  action: string;
  details: string | null;
  task_id: string | null;
  project_id: string | null;
  created_at: string;
  employee_name?: string;
  project_name?: string;
}

export interface EmployeeMetric {
  id: string;
  employee_id: string;
  tasks_completed: number;
  average_completion_time: number | null;
  success_rate: number | null;
  period_start: string;
  period_end: string;
  created_at: string;
}

export interface ProjectEmployee {
  project_id: string;
  employee_id: string;
  role: string;
  joined_at: string;
}

export interface CodexSessionRecord {
  id: string;
  employee_id: string | null;
  task_id: string | null;
  project_id: string | null;
  task_git_context_id: string | null;
  cli_session_id: string | null;
  working_dir: string | null;
  session_kind: CodexSessionKind;
  status: string;
  started_at: string;
  ended_at: string | null;
  exit_code: number | null;
  resume_session_id: string | null;
  execution_target: EnvironmentMode;
  ssh_config_id: string | null;
  target_host_label: string | null;
  artifact_capture_mode: ArtifactCaptureMode;
  created_at: string;
}

export interface CodexSessionFileChange {
  id: string;
  session_id: string;
  path: string;
  change_type: "added" | "modified" | "deleted" | "renamed";
  capture_mode: "sdk_event" | "git_fallback";
  previous_path: string | null;
  created_at: string;
}

export interface TaskLatestReview {
  session: CodexSessionRecord;
  report: string | null;
  reviewer_name: string | null;
}

export interface TaskExecutionChangeHistoryItem {
  session: CodexSessionRecord;
  capture_mode: "sdk_event" | "git_fallback";
  changes: CodexSessionFileChange[];
}

export interface ReviewVerdict {
  passed: boolean;
  needs_human: boolean;
  blocking_issue_count: number;
  summary: string;
}

export interface TaskAutomationState {
  task_id: string;
  phase: TaskAutomationPhase;
  round_count: number;
  consumed_session_id: string | null;
  last_trigger_session_id: string | null;
  pending_action: TaskAutomationPendingAction | null;
  pending_round_count: number | null;
  last_error: string | null;
  last_verdict: ReviewVerdict | null;
  updated_at: string;
}

export interface CodexSessionFileChangeDetail {
  change: CodexSessionFileChange;
  working_dir: string | null;
  absolute_path: string | null;
  previous_absolute_path: string | null;
  before_status: "text" | "missing" | "binary" | "unavailable";
  before_text: string | null;
  before_truncated: boolean;
  after_status: "text" | "missing" | "binary" | "unavailable";
  after_text: string | null;
  after_truncated: boolean;
  diff_text: string | null;
  diff_truncated: boolean;
  snapshot_status: "ready" | "unavailable";
  snapshot_message: string | null;
}

export interface CodexSessionLogLine {
  event_id: string;
  line: string;
}

export interface CodexSessionListItem {
  session_record_id: string;
  session_id: string;
  cli_session_id: string | null;
  session_kind: CodexSessionKind;
  status: string;
  last_updated_at: string;
  display_name: string;
  summary: string | null;
  content_preview: string | null;
  employee_id: string | null;
  employee_name: string | null;
  task_id: string | null;
  task_git_context_id: string | null;
  task_title: string | null;
  task_status: string | null;
  project_id: string | null;
  project_name: string | null;
  working_dir: string | null;
  execution_target: EnvironmentMode;
  ssh_config_id: string | null;
  target_host_label: string | null;
  artifact_capture_mode: ArtifactCaptureMode;
  resume_status: CodexSessionResumeStatus;
  resume_message: string | null;
  can_resume: boolean;
}

export interface CodexSessionResumePreview {
  requested_session_id: string;
  resolved_session_id: string | null;
  session_record_id: string | null;
  session_kind: CodexSessionKind | null;
  session_status: string | null;
  display_name: string | null;
  summary: string | null;
  employee_id: string | null;
  employee_name: string | null;
  task_id: string | null;
  task_git_context_id: string | null;
  task_title: string | null;
  project_id: string | null;
  project_name: string | null;
  working_dir: string | null;
  resume_status: CodexSessionResumeStatus;
  resume_message: string | null;
  can_resume: boolean;
}

export type GlobalSearchItemType = "project" | "task" | "employee" | "session";
export type GlobalSearchState = "ok" | "empty_query" | "query_too_short";

export interface GlobalSearchItem {
  item_type: GlobalSearchItemType;
  item_id: string;
  title: string;
  subtitle: string | null;
  summary: string | null;
  navigation_path: string;
  score: number;
  updated_at: string | null;
  project_id: string | null;
  task_id: string | null;
  employee_id: string | null;
  session_id: string | null;
}

export interface GlobalSearchResponse {
  query: string;
  normalized_query: string;
  state: GlobalSearchState;
  message: string | null;
  min_query_length: number;
  total: number;
  items: GlobalSearchItem[];
}

export type NotificationType =
  | "review_pending"
  | "run_failed"
  | "task_completed"
  | "sdk_unavailable"
  | "database_error"
  | "ssh_config_error";
export type NotificationSeverity = "info" | "success" | "warning" | "error" | "critical";
export type NotificationDeliveryMode = "one_time" | "sticky";
export type NotificationState = "active" | "resolved";

export interface AppNotification {
  id: string;
  notification_type: NotificationType;
  severity: NotificationSeverity;
  source_module: string;
  title: string;
  message: string;
  recommendation: string | null;
  action_label: string | null;
  action_route: string | null;
  related_object_type: string | null;
  related_object_id: string | null;
  project_id: string | null;
  task_id: string | null;
  ssh_config_id: string | null;
  delivery_mode: NotificationDeliveryMode;
  state: NotificationState;
  is_read: boolean;
  dedupe_key: string | null;
  occurrence_count: number;
  first_triggered_at: string;
  last_triggered_at: string;
  read_at: string | null;
  resolved_at: string | null;
  created_at: string;
  updated_at: string;
  is_transient?: false;
}

export interface NotificationCenterChanged {
  reason: string;
  notification_id: string | null;
}

export interface TransientNotification {
  id: string;
  notification_type: NotificationType;
  severity: NotificationSeverity;
  source_module: string;
  title: string;
  message: string;
  recommendation: string | null;
  action_label: string | null;
  action_route: string | null;
  related_object_type: string | null;
  related_object_id: string | null;
  project_id: string | null;
  task_id: string | null;
  ssh_config_id: string | null;
  delivery_mode: NotificationDeliveryMode;
  occurrence_count: number;
  first_triggered_at: string;
  last_triggered_at: string;
  is_read: boolean;
  is_transient: true;
}

export type NotificationItem = AppNotification | TransientNotification;

export interface TaskGitContext {
  id: string;
  task_id: string;
  project_id: string;
  base_branch: string | null;
  task_branch: string | null;
  target_branch: string | null;
  worktree_path: string | null;
  repo_head_commit_at_prepare: string | null;
  state: TaskGitContextState;
  context_version: number;
  pending_action_type: GitActionType | null;
  pending_action_token_hash: string | null;
  pending_action_payload_json: string | null;
  pending_action_nonce: string | null;
  pending_action_requested_at: string | null;
  pending_action_expires_at: string | null;
  pending_action_repo_revision: string | null;
  pending_action_bound_context_version: number | null;
  last_reconciled_at: string | null;
  last_error: string | null;
  worktree_missing: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProjectGitCommit {
  sha: string;
  short_sha: string | null;
  subject: string;
  author_name: string | null;
  authored_at: string;
}

export interface ProjectGitWorkingTreeChange {
  path: string;
  previous_path: string | null;
  change_type: "added" | "modified" | "deleted" | "renamed";
  stage_status: "staged" | "unstaged" | "partially_staged" | "untracked";
  can_open_file: boolean;
}

export interface ProjectGitFilePreview {
  project_id: string;
  relative_path: string;
  previous_path: string | null;
  absolute_path: string | null;
  previous_absolute_path: string | null;
  execution_target: EnvironmentMode;
  change_type: ProjectGitWorkingTreeChange["change_type"];
  before_status: "text" | "missing" | "binary" | "unavailable";
  before_text: string | null;
  before_truncated: boolean;
  after_status: "text" | "missing" | "binary" | "unavailable";
  after_text: string | null;
  after_truncated: boolean;
  message: string | null;
}

export interface ProjectGitOverview {
  project_id: string;
  repo_path: string | null;
  execution_target: EnvironmentMode;
  git_runtime_provider: "simple_git";
  git_runtime_status: "ready" | "bootstrapping" | "unavailable";
  git_runtime_message: string | null;
  default_branch: string | null;
  current_branch: string | null;
  project_branches: string[];
  head_commit_sha: string | null;
  working_tree_summary: string | null;
  ahead_commits: number | null;
  behind_commits: number | null;
  working_tree_changes: ProjectGitWorkingTreeChange[];
  refreshed_at: string;
  recent_commits: ProjectGitCommit[];
  active_contexts: TaskGitContext[];
  pending_action_contexts: TaskGitContext[];
}

export interface TaskGitCommitOverview {
  task_git_context_id: string;
  project_id: string;
  worktree_path: string;
  execution_target: EnvironmentMode;
  current_branch: string | null;
  working_tree_summary: string | null;
  working_tree_changes: ProjectGitWorkingTreeChange[];
  refreshed_at: string;
}

export interface PreparedTaskGitExecution {
  task_git_context_id: string;
  working_dir: string;
  task_branch: string;
  target_branch: string;
  base_branch: string | null;
  context_version: number;
}

export interface GitActionRequestResult {
  task_git_context_id: string;
  action_type: GitActionType;
  token: string;
  expires_at: string;
  state: TaskGitContextState;
  context_version: number;
}

export interface ConfirmGitActionResult {
  context: TaskGitContext;
  action_type: GitActionType;
  message: string;
}

export interface CodexHealthCheck {
  codex_available: boolean;
  codex_version: string | null;
  node_available: boolean;
  node_version: string | null;
  task_sdk_enabled: boolean;
  one_shot_sdk_enabled: boolean;
  sdk_installed: boolean;
  sdk_version: string | null;
  sdk_install_dir: string;
  task_execution_effective_provider: string;
  one_shot_effective_provider: string;
  sdk_status_message: string;
  database_loaded: boolean;
  database_path: string | null;
  database_current_version: number | null;
  database_current_description: string | null;
  database_latest_version: number;
  shell_available: boolean;
  last_session_error: string | null;
  execution_target?: EnvironmentMode;
  target_host_label?: string | null;
  ssh_config_id?: string | null;
  password_probe_status?: SshPasswordProbeStatus | null;
  password_probe_message?: string | null;
  password_execution_allowed?: boolean;
  password_auth_available?: boolean;
  checked_at: string;
}

export interface CodexRuntimeStatus {
  running: boolean;
  session: CodexSessionRecord | null;
}

export interface EmployeeRunningSession {
  session_record_id: string;
  cli_session_id: string | null;
  task_id: string | null;
  task_title: string | null;
  session_kind: CodexSessionKind;
  started_at: string;
  status: string;
}

export interface EmployeeRuntimeStatus {
  running: boolean;
  sessions: EmployeeRunningSession[];
  latest_session: CodexSessionRecord | null;
}

export interface GitPreferences {
  default_task_use_worktree: boolean;
  worktree_location_mode: WorktreeLocationMode;
  worktree_custom_root: string | null;
  ai_commit_message_length: AiCommitMessageLength;
  ai_commit_model_source: AiCommitModelSource;
  ai_commit_model: CodexModelId;
  ai_commit_reasoning_effort: ReasoningEffort;
}

export interface CodexSettings {
  task_sdk_enabled: boolean;
  one_shot_sdk_enabled: boolean;
  one_shot_model: string;
  one_shot_reasoning_effort: string;
  task_automation_default_enabled: boolean;
  task_automation_max_fix_rounds: number;
  task_automation_failure_strategy: TaskAutomationFailureStrategy;
  git_preferences: GitPreferences;
  node_path_override: string | null;
  sdk_install_dir: string;
  one_shot_preferred_provider: string;
}

export type RemoteCodexSettings = CodexSettings;

export interface SshPasswordProbeResult {
  ssh_config_id: string;
  target_host_label?: string | null;
  auth_type?: SshAuthType;
  status: SshPasswordProbeStatus;
  execution_allowed: boolean;
  supported?: boolean;
  message: string;
  checked_at: string;
}

export type RemoteCodexHealthCheck = CodexHealthCheck;

export interface CodexSdkInstallResult {
  sdk_installed: boolean;
  sdk_version: string | null;
  install_dir: string;
  node_version: string | null;
  message: string;
}

export type RemoteCodexSdkInstallResult = CodexSdkInstallResult;

export interface DatabaseBackupResult {
  source_path: string;
  destination_path: string;
  database_version: number | null;
  created_at: string;
  message: string;
}

export interface DatabaseRestoreResult {
  source_path: string;
  backup_path: string;
  database_version: number | null;
  restored_at: string;
  message: string;
}

export type CodexSessionKind = "execution" | "review";
export type CodexSessionResumeStatus =
  | "ready"
  | "running"
  | "missing_employee"
  | "missing_cli_session"
  | "stopping"
  | "invalid";
export type CodexModelId =
  | "gpt-5.4"
  | "gpt-5.2-codex"
  | "gpt-5.1-codex-max"
  | "gpt-5.4-mini"
  | "gpt-5.3-codex"
  | "gpt-5.3-codex-spark"
  | "gpt-5.2"
  | "gpt-5.1-codex-mini";
export type ReasoningEffort = "low" | "medium" | "high" | "xhigh";
export type TaskStatus = "todo" | "in_progress" | "review" | "completed" | "blocked";
export type TaskAutomationMode = "review_fix_loop_v1";
export type TaskAutomationPhase =
  | "idle"
  | "launching_review"
  | "waiting_review"
  | "launching_fix"
  | "waiting_execution"
  | "committing_code"
  | "review_launch_failed"
  | "fix_launch_failed"
  | "commit_failed"
  | "manual_control"
  | "blocked"
  | "completed";
export type TaskAutomationPendingAction = "start_review" | "start_fix";
export type TaskAutomationFailureStrategy = "blocked" | "manual_control";
export type WorktreeLocationMode = "repo_sibling_hidden" | "repo_child_hidden" | "custom_root";
export type AiCommitMessageLength = "title_only" | "title_with_body";
export type AiCommitModelSource = "inherit_one_shot" | "custom";
export type EmployeeStatus = "online" | "busy" | "offline" | "error";
export type Priority = "low" | "medium" | "high" | "urgent";

export const TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS: {
  value: TaskAutomationFailureStrategy;
  label: string;
}[] = [
  { value: "blocked", label: "转阻塞" },
  { value: "manual_control", label: "转人工" },
];

export const WORKTREE_LOCATION_MODE_OPTIONS: {
  value: WorktreeLocationMode;
  label: string;
  description: string;
}[] = [
  {
    value: "repo_sibling_hidden",
    label: "仓库同级隐藏目录",
    description: "保持当前行为，放在仓库同级的 .codex-ai-worktrees-* 目录中",
  },
  {
    value: "repo_child_hidden",
    label: "仓库 .git 目录",
    description: "放在仓库的 .git/codex-ai-worktrees 目录中，不会污染主工作区",
  },
  {
    value: "custom_root",
    label: "自定义根目录",
    description: "使用你指定的根目录，并自动拼接仓库与任务目录",
  },
];

export const AI_COMMIT_MESSAGE_LENGTH_OPTIONS: {
  value: AiCommitMessageLength;
  label: string;
  description: string;
}[] = [
  {
    value: "title_with_body",
    label: "标题+详情",
    description: "生成 Conventional Commit 标题，并补充正文说明改动",
  },
  {
    value: "title_only",
    label: "仅标题",
    description: "只生成单行 Conventional Commit 标题",
  },
];

export const AI_COMMIT_MODEL_SOURCE_OPTIONS: {
  value: AiCommitModelSource;
  label: string;
  description: string;
}[] = [
  {
    value: "inherit_one_shot",
    label: "跟随一次性 AI",
    description: "复用当前一次性 AI 的模型与推理强度",
  },
  {
    value: "custom",
    label: "单独指定",
    description: "为 Git 提交信息生成单独配置模型与推理强度",
  },
];

export function isSupportedTaskAutomationFailureStrategy(
  value: string,
): value is TaskAutomationFailureStrategy {
  return TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS.some((option) => option.value === value);
}

export function normalizeTaskAutomationFailureStrategy(
  value: string | null | undefined,
): TaskAutomationFailureStrategy {
  return value && isSupportedTaskAutomationFailureStrategy(value) ? value : "blocked";
}

export function isSupportedWorktreeLocationMode(
  value: string,
): value is WorktreeLocationMode {
  return WORKTREE_LOCATION_MODE_OPTIONS.some((option) => option.value === value);
}

export function normalizeWorktreeLocationMode(
  value: string | null | undefined,
): WorktreeLocationMode {
  return value && isSupportedWorktreeLocationMode(value)
    ? value
    : "repo_sibling_hidden";
}

export function isSupportedAiCommitMessageLength(
  value: string,
): value is AiCommitMessageLength {
  return AI_COMMIT_MESSAGE_LENGTH_OPTIONS.some((option) => option.value === value);
}

export function normalizeAiCommitMessageLength(
  value: string | null | undefined,
): AiCommitMessageLength {
  return value && isSupportedAiCommitMessageLength(value)
    ? value
    : "title_with_body";
}

export function isSupportedAiCommitModelSource(
  value: string,
): value is AiCommitModelSource {
  return AI_COMMIT_MODEL_SOURCE_OPTIONS.some((option) => option.value === value);
}

export function normalizeAiCommitModelSource(
  value: string | null | undefined,
): AiCommitModelSource {
  return value && isSupportedAiCommitModelSource(value)
    ? value
    : "inherit_one_shot";
}

export const CODEX_MODEL_OPTIONS: { value: CodexModelId; label: string }[] = [
  { value: "gpt-5.4", label: "GPT-5.4" },
  { value: "gpt-5.2-codex", label: "GPT-5.2-Codex" },
  { value: "gpt-5.1-codex-max", label: "GPT-5.1-Codex-Max" },
  { value: "gpt-5.4-mini", label: "GPT-5.4-Mini" },
  { value: "gpt-5.3-codex", label: "GPT-5.3-Codex" },
  { value: "gpt-5.3-codex-spark", label: "GPT-5.3-Codex-Spark" },
  { value: "gpt-5.2", label: "GPT-5.2" },
  { value: "gpt-5.1-codex-mini", label: "GPT-5.1-Codex-Mini" },
];

export const REASONING_EFFORT_OPTIONS: { value: ReasoningEffort; label: string }[] = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "超高" },
];

export function isSupportedCodexModel(value: string): value is CodexModelId {
  return CODEX_MODEL_OPTIONS.some((option) => option.value === value);
}

export function isSupportedReasoningEffort(value: string): value is ReasoningEffort {
  return REASONING_EFFORT_OPTIONS.some((option) => option.value === value);
}

export function normalizeCodexModel(value: string | null | undefined): CodexModelId {
  return value && isSupportedCodexModel(value) ? value : "gpt-5.4";
}

export function normalizeReasoningEffort(value: string | null | undefined): ReasoningEffort {
  return value && isSupportedReasoningEffort(value) ? value : "high";
}

export const TASK_STATUSES: { value: TaskStatus; label: string; color: string }[] = [
  { value: "todo", label: "待办", color: "bg-slate-500" },
  { value: "in_progress", label: "进行中", color: "bg-blue-500" },
  { value: "review", label: "审核中", color: "bg-yellow-500" },
  { value: "completed", label: "已完成", color: "bg-green-500" },
  { value: "blocked", label: "已阻塞", color: "bg-red-500" },
];

export const PRIORITIES: { value: Priority; label: string; color: string }[] = [
  { value: "low", label: "低", color: "text-slate-500" },
  { value: "medium", label: "中", color: "text-blue-500" },
  { value: "high", label: "高", color: "text-orange-500" },
  { value: "urgent", label: "紧急", color: "text-red-500" },
];
