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
  "merge" | "push" | "rebase" | "cherry_pick" | "stash" | "unstash" | "cleanup_worktree";
export type ProjectGitRepoActionType = "commit" | "push" | "pull";
export type ProjectGitBranchActionType = "switch" | "create" | "delete" | "merge";
export type GitMergeFastForwardMode = "ff" | "no_ff" | "ff_only";
export type GitMergeStrategy = "ort" | "recursive" | "resolve" | "ours" | "subtree";

export interface Project {
  id: string;
  name: string;
  description: string | null;
  status: string;
  repo_path: string | null;
  project_type: ProjectType;
  ssh_config_id: string | null;
  remote_repo_path: string | null;
  test_command: string | null;
  deleted_at: string | null;
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
  ai_provider: AiProvider;
  ai_channel_id: string | null;
  created_at: string;
  updated_at: string;
}

export type AiChannelProtocol = "openai" | "anthropic" | "codex";

export interface AiChannelModel {
  id: string;
  context_tokens: number | null;
  max_output_tokens: number | null;
  thinking_enabled: boolean | null;
  thinking_level: string | null;
  thinking_levels: string[] | null;
}

export interface ModelCatalogEntry {
  id: string;
  aliases: string[];
  vendor: string;
  label: string;
  context_tokens: number;
  max_output_tokens: number;
  thinking: boolean;
  thinking_levels: string[];
}

export interface AiChannel {
  id: string;
  name: string;
  protocol: AiChannelProtocol;
  base_url: string;
  extra_headers_json: string | null;
  models: AiChannelModel[];
  enabled: boolean;
  api_key: string | null;
  api_key_configured: boolean;
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
  coordinator_id: string | null;
  plan_content: string | null;
  complexity: number | null;
  ai_suggestion: string | null;
  automation_mode: TaskAutomationMode | null;
  last_codex_session_id: string | null;
  last_review_session_id: string | null;
  time_started_at: string | null;
  time_spent_seconds: number;
  completed_at: string | null;
  deleted_at: string | null;
  due_date: string | null;
  blocked_reason: string | null;
  milestone_id: string | null;
  /** NULL/missing = inherit global enabled; JSON array string when overridden */
  mcp_server_ids: string | null;
  native_subagent_id: string | null;
  acceptance_checklist: string | null;
  last_acceptance_status: string | null;
  created_at: string;
  updated_at: string;
}

export type AcceptanceRunStatus = "running" | "passed" | "failed" | "skipped";

export interface TaskAcceptanceRun {
  id: string;
  task_id: string;
  status: AcceptanceRunStatus | string;
  trigger: string;
  acceptance_checklist: string | null;
  command: string | null;
  command_exit_code: number | null;
  command_output_excerpt: string | null;
  ai_verdict: string | null;
  summary: string | null;
  created_at: string;
  finished_at: string | null;
}

export interface Milestone {
  id: string;
  project_id: string;
  name: string;
  due_date: string | null;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface Tag {
  id: string;
  project_id: string;
  name: string;
  color: string | null;
  created_at: string;
}

export interface TaskTag {
  task_id: string;
  tag_id: string;
}

export interface TaskDependency {
  id: string;
  task_id: string;
  depends_on_task_id: string;
  created_at: string;
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

export interface TaskFileRef {
  id: string;
  task_id: string;
  relative_path: string;
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

export interface TaskTemplateSubtaskSpec {
  title: string;
  sort_order: number;
}

export interface TaskTemplate {
  id: string;
  name: string;
  description: string | null;
  project_id: string | null;
  title_template: string;
  description_template: string | null;
  priority: string;
  use_worktree: boolean;
  tags: string[];
  subtasks: TaskTemplateSubtaskSpec[];
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
}

export interface CreateTaskTemplateInput {
  name: string;
  description?: string | null;
  project_id?: string | null;
  title_template: string;
  description_template?: string | null;
  priority?: string;
  use_worktree?: boolean;
  tags?: string[];
  subtasks?: TaskTemplateSubtaskSpec[];
}

export interface UpdateTaskTemplateInput {
  name?: string;
  description?: string | null;
  project_id?: string | null;
  title_template?: string;
  description_template?: string | null;
  priority?: string;
  use_worktree?: boolean;
  tags?: string[];
  subtasks?: TaskTemplateSubtaskSpec[];
}

export interface ApplyTaskTemplateInput {
  template_id: string;
  project_id: string;
  variable_sets?: Record<string, string>[];
  assignee_id?: string | null;
  reviewer_id?: string | null;
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
  ai_provider: AiProvider;
  thinking_budget_tokens: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  total_tokens: number | null;
  reasoning_tokens: number | null;
  cached_tokens: number | null;
  execution_target: EnvironmentMode;
  ssh_config_id: string | null;
  target_host_label: string | null;
  artifact_capture_mode: ArtifactCaptureMode;
  session_origin: SessionOrigin;
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

export type ReviewFindingSeverity = "blocker" | "warning" | "info";

export interface ReviewFinding {
  file: string;
  line: number | null;
  severity: ReviewFindingSeverity | string;
  message: string;
}

export interface TaskLatestReview {
  session: CodexSessionRecord;
  report: string | null;
  reviewer_name: string | null;
  findings: ReviewFinding[];
  has_findings_event: boolean;
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
  pipeline_active: boolean;
  pipeline_step_index: number | null;
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
  ai_provider: AiProvider;
  session_kind: CodexSessionKind;
  session_origin: SessionOrigin;
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
  input_tokens: number | null;
  output_tokens: number | null;
  total_tokens: number | null;
  reasoning_tokens: number | null;
  cached_tokens: number | null;
  resume_status: CodexSessionResumeStatus;
  resume_message: string | null;
  can_resume: boolean;
}

export interface CodexSessionResumePreview {
  requested_session_id: string;
  resolved_session_id: string | null;
  session_record_id: string | null;
  ai_provider: AiProvider | null;
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
  | "run_completed"
  | "task_completed"
  | "sdk_unavailable"
  | "database_error"
  | "ssh_config_error";
export type NotificationSeverity = "info" | "success" | "warning" | "error" | "critical";
export type NotificationDeliveryMode = "one_time" | "sticky";
export type NotificationState = "active" | "resolved";
export type DesktopNotificationDeliveryReason = "created" | "reactivated" | "updated" | "transient";

export interface NotificationSoundSettings {
  enabled: boolean;
}

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

export interface DesktopNotificationEvent {
  reason: DesktopNotificationDeliveryReason;
  notification_id: string;
  title: string;
  message: string;
  severity: NotificationSeverity;
  action_route: string | null;
  project_id: string | null;
  task_id: string | null;
  ssh_config_id: string | null;
  is_transient: boolean;
  last_triggered_at: string;
}

export type DesktopNotificationExtra = DesktopNotificationEvent;

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

export interface ProjectGitCommitHistory {
  commits: ProjectGitCommit[];
  has_more: boolean;
}

export interface ProjectGitFileChangeRef {
  path: string;
  previous_path: string | null;
  change_type: "added" | "modified" | "deleted" | "renamed";
}

export interface ProjectGitCommitFileChange extends ProjectGitFileChangeRef {}

export interface ProjectGitCommitDetail {
  project_id: string;
  execution_target: EnvironmentMode;
  sha: string;
  short_sha: string;
  subject: string;
  body: string | null;
  author_name: string;
  author_email: string | null;
  authored_at: string;
  diff_text: string | null;
  diff_truncated: boolean;
  changed_files: ProjectGitCommitFileChange[];
}

export interface ProjectGitWorkingTreeChange extends ProjectGitFileChangeRef {
  stage_status: "staged" | "unstaged" | "partially_staged" | "untracked" | "unmerged";
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
  before_label: string;
  before_status: "text" | "missing" | "binary" | "unavailable";
  before_text: string | null;
  before_truncated: boolean;
  after_label: string;
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
  recent_commits_has_more: boolean;
  active_contexts: TaskGitContext[];
  pending_action_contexts: TaskGitContext[];
}

export interface ProjectGitWorktree {
  path: string;
  branch: string | null;
  head_sha: string | null;
  short_head_sha: string | null;
  is_main: boolean;
  is_bare: boolean;
  is_detached: boolean;
  is_locked: boolean;
  lock_reason: string | null;
  is_prunable: boolean;
  prunable_reason: string | null;
  task_git_context_id: string | null;
  task_id: string | null;
  task_title: string | null;
  working_tree_summary: string | null;
  working_tree_changes: ProjectGitWorkingTreeChange[];
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

export type TaskCommitMode = "worktree" | "project_repo";

export interface TaskCommitActionState {
  task_id: string;
  project_id: string;
  mode: TaskCommitMode | string;
  task_git_context_id: string | null;
  working_dir: string | null;
  execution_target: EnvironmentMode | string | null;
  current_branch: string | null;
  git_context_state: TaskGitContextState | string | null;
  worktree_missing: boolean;
  has_stageable: boolean;
  has_staged: boolean;
  has_unmerged: boolean;
  can_commit: boolean;
  can_ai_commit: boolean;
  can_merge: boolean;
  warnings: string[];
  error: string | null;
}

export interface TaskCommitOverview {
  task_id: string;
  project_id: string;
  mode: TaskCommitMode | string;
  task_git_context_id: string | null;
  working_dir: string;
  execution_target: EnvironmentMode | string;
  current_branch: string | null;
  working_tree_summary: string | null;
  working_tree_changes: ProjectGitWorkingTreeChange[];
  has_unmerged: boolean;
  warning: string | null;
  refreshed_at: string;
}

export interface TaskAiCommitResult {
  task_id: string;
  mode: TaskCommitMode | string;
  message: string;
  detail: string;
  conflict_resolved: boolean;
  merge_ready: boolean;
}

export interface TaskAiConflictResolveResult {
  task_id: string;
  working_dir: string;
  resolved_files: string[];
  detail: string;
  merge_completed: boolean;
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
  one_shot_preferred_provider: AiProvider;
  sdk_installed: boolean;
  sdk_version: string | null;
  sdk_install_dir: string;
  task_execution_effective_provider: string;
  one_shot_effective_provider: string;
  one_shot_effective_channel: string;
  one_shot_status_message: string;
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
  ai_provider: AiProvider;
  session_kind: CodexSessionKind;
  session_origin: SessionOrigin;
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
  ai_commit_preferred_provider: AiProvider;
  ai_commit_model_source: AiCommitModelSource;
  ai_commit_model: string;
  ai_commit_reasoning_effort: string;
}

export interface CodexSettings {
  task_sdk_enabled: boolean;
  one_shot_sdk_enabled: boolean;
  one_shot_preferred_provider: AiProvider;
  one_shot_model: string;
  one_shot_reasoning_effort: string;
  /** 一次性 AI 使用内置 Agent（native）时绑定的 AI 渠道 id。 */
  one_shot_native_channel_id: string | null;
  task_automation_default_enabled: boolean;
  task_automation_max_fix_rounds: number;
  task_automation_failure_strategy: TaskAutomationFailureStrategy;
  tester_automation_enabled: boolean;
  tester_allow_ai_only: boolean;
  default_test_command: string | null;
  git_preferences: GitPreferences;
  node_path_override: string | null;
  sdk_install_dir: string;
  /** 0 = unlimited. Gate reads local settings only. */
  max_concurrent_sessions: number;
}

export type StartSessionOutcome = { status: "started" } | { status: "queued"; position: number };

export interface TaskRunQueueItem {
  id: number;
  task_id: string;
  provider: string;
  employee_id: string;
  enqueued_at: string;
  position: number;
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
export type SessionOrigin = "direct" | "pipeline";
export type CodexSessionResumeStatus =
  "ready" | "running" | "missing_employee" | "missing_cli_session" | "stopping" | "invalid";
export type CodexModelId =
  | "gpt-5.6-sol"
  | "gpt-5.6-terra"
  | "gpt-5.6-luna"
  | "gpt-5.5"
  | "gpt-5.4"
  | "gpt-5.2-codex"
  | "gpt-5.1-codex-max"
  | "gpt-5.4-mini"
  | "gpt-5.3-codex"
  | "gpt-5.3-codex-spark"
  | "gpt-5.2"
  | "gpt-5.1-codex-mini";
export type ReasoningEffort = "low" | "medium" | "high" | "xhigh" | "max";

export type AiProvider = "codex" | "claude" | "opencode" | "grok" | "native";
export type ClaudeModelId = "opus" | "opus[1m]" | "sonnet" | "sonnet[1m]" | "haiku";
export type GrokModelId = "grok-4.5" | string;
export type ModelId = CodexModelId | ClaudeModelId | GrokModelId | string;
export type TaskStatus = "todo" | "in_progress" | "review" | "completed" | "blocked" | "archived";
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
  | "completed"
  | "pipeline_launching_step"
  | "pipeline_waiting_step"
  | "pipeline_manual_launching_step"
  | "pipeline_manual_waiting_step"
  | "pipeline_step_failed";
export type TaskAutomationPendingAction = "start_review" | "start_fix";
export type TaskPipelineStepStatus =
  "pending" | "launching" | "running" | "succeeded" | "failed" | "cancelled" | "skipped";

export interface TaskPipelineStep {
  id: string;
  task_id: string;
  step_index: number;
  title: string;
  goal: string | null;
  success_criteria: string | null;
  employee_id: string | null;
  status: TaskPipelineStepStatus | string;
  session_id: string | null;
  handoff_summary: string | null;
  last_error: string | null;
  started_at: string | null;
  ended_at: string | null;
  created_at: string;
  updated_at: string;
}
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

export function isSupportedWorktreeLocationMode(value: string): value is WorktreeLocationMode {
  return WORKTREE_LOCATION_MODE_OPTIONS.some((option) => option.value === value);
}

export function normalizeWorktreeLocationMode(
  value: string | null | undefined,
): WorktreeLocationMode {
  return value && isSupportedWorktreeLocationMode(value) ? value : "repo_sibling_hidden";
}

export function isSupportedAiCommitMessageLength(value: string): value is AiCommitMessageLength {
  return AI_COMMIT_MESSAGE_LENGTH_OPTIONS.some((option) => option.value === value);
}

export function normalizeAiCommitMessageLength(
  value: string | null | undefined,
): AiCommitMessageLength {
  return value && isSupportedAiCommitMessageLength(value) ? value : "title_with_body";
}

export function isSupportedAiCommitModelSource(value: string): value is AiCommitModelSource {
  return AI_COMMIT_MODEL_SOURCE_OPTIONS.some((option) => option.value === value);
}

export function normalizeAiCommitModelSource(
  value: string | null | undefined,
): AiCommitModelSource {
  return value && isSupportedAiCommitModelSource(value) ? value : "inherit_one_shot";
}

export const CODEX_MODEL_OPTIONS: { value: CodexModelId; label: string }[] = [
  { value: "gpt-5.6-sol", label: "GPT-5.6-Sol" },
  { value: "gpt-5.6-terra", label: "GPT-5.6-Terra" },
  { value: "gpt-5.6-luna", label: "GPT-5.6-Luna" },
  { value: "gpt-5.5", label: "GPT-5.5" },
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
  { value: "xhigh", label: "最高" },
  { value: "max", label: "极高" },
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

/** OpenCode 推理强度仅 low / medium / high */
export const OPENCODE_EFFORT_OPTIONS: { value: string; label: string }[] = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
];

export function normalizeReasoningEffort(value: string | null | undefined): ReasoningEffort {
  return value && isSupportedReasoningEffort(value) ? value : "high";
}

export const AI_PROVIDER_OPTIONS: { value: AiProvider; label: string }[] = [
  { value: "codex", label: "Codex (OpenAI)" },
  { value: "claude", label: "Claude (Anthropic)" },
  { value: "opencode", label: "OpenCode (开源)" },
  { value: "grok", label: "Grok" },
  { value: "native", label: "内置 Agent" },
];

/** Git 一次性提交 / 运行时 one-shot 仍走外部 CLI，不含内置 Agent。 */
export const CLI_AI_PROVIDER_OPTIONS = AI_PROVIDER_OPTIONS.filter(
  (option) => option.value !== "native",
);

export const CLAUDE_MODEL_OPTIONS: { value: ClaudeModelId; label: string }[] = [
  { value: "opus", label: "Claude Opus" },
  { value: "opus[1m]", label: "Claude Opus 1M" },
  { value: "sonnet", label: "Claude Sonnet" },
  { value: "sonnet[1m]", label: "Claude Sonnet 1M" },
  { value: "haiku", label: "Claude Haiku" },
];

export const GROK_MODEL_OPTIONS: { value: string; label: string }[] = [
  { value: "grok-4.5", label: "Grok 4.5" },
];

/** Grok 推理强度仅 low / medium / high */
export const GROK_EFFORT_OPTIONS: { value: string; label: string }[] = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
];

/** 内置 Agent 思考等级与模型目录对齐，不能套用 Grok 三档白名单。 */
export const NATIVE_THINKING_LEVELS = [
  "none",
  "no_think",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
] as const;

export const CLAUDE_THINKING_BUDGET_OPTIONS: {
  value: string;
  label: string;
}[] = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "超高" },
  { value: "max", label: "最大" },
  { value: "auto", label: "自动" },
];

export function getDefaultReasoningEffortForProvider(_provider: AiProvider): string {
  return "high";
}

export function isSupportedClaudeReasoningEffort(value: string): boolean {
  return CLAUDE_THINKING_BUDGET_OPTIONS.some((option) => option.value === value);
}

export function isSupportedGrokReasoningEffort(value: string): boolean {
  return GROK_EFFORT_OPTIONS.some((option) => option.value === value);
}

export function isSupportedNativeReasoningEffort(value: string): boolean {
  return (NATIVE_THINKING_LEVELS as readonly string[]).includes(value);
}

export function normalizeReasoningEffortForProvider(
  provider: AiProvider,
  value: string | null | undefined,
): string {
  if (provider === "claude") {
    return value && isSupportedClaudeReasoningEffort(value)
      ? value
      : getDefaultReasoningEffortForProvider(provider);
  }

  if (provider === "opencode") {
    return value && OPENCODE_EFFORT_OPTIONS.some((option) => option.value === value)
      ? value
      : "high";
  }

  if (provider === "native") {
    return value && isSupportedNativeReasoningEffort(value)
      ? value
      : getDefaultReasoningEffortForProvider(provider);
  }

  if (provider === "grok") {
    return value && isSupportedGrokReasoningEffort(value)
      ? value
      : getDefaultReasoningEffortForProvider(provider);
  }

  return normalizeReasoningEffort(value);
}

export function normalizeModelForProvider(
  provider: AiProvider,
  value: string | null | undefined,
): string {
  if (provider === "claude") {
    return normalizeClaudeModel(value);
  }

  if (provider === "opencode") {
    return value && value.trim().length > 0 ? value.trim() : "openai/gpt-4o";
  }

  if (provider === "grok") {
    return normalizeGrokModel(value);
  }

  if (provider === "native") {
    const normalized = value?.trim();
    return normalized && normalized.length > 0 ? normalized : "default";
  }

  return normalizeCodexModel(value);
}

export function isSupportedClaudeModel(value: string): value is ClaudeModelId {
  return CLAUDE_MODEL_OPTIONS.some((option) => option.value === value);
}

export function isSupportedGrokModel(value: string): boolean {
  return value.trim().length > 0;
}

export function normalizeClaudeModel(value: string | null | undefined): ClaudeModelId {
  const normalized = value?.trim();
  if (!normalized) return "sonnet";
  if (isSupportedClaudeModel(normalized)) return normalized;
  const hasOneMillionContext = normalized.includes("[1m]");
  if (normalized.startsWith("claude-opus-")) return hasOneMillionContext ? "opus[1m]" : "opus";
  if (normalized.startsWith("claude-sonnet-"))
    return hasOneMillionContext ? "sonnet[1m]" : "sonnet";
  if (normalized.startsWith("claude-haiku-")) return "haiku";
  return "sonnet";
}

export function normalizeGrokModel(value: string | null | undefined): string {
  const normalized = value?.trim();
  if (normalized) return normalized;
  return "grok-4.5";
}

export interface GrokModelInfo {
  value: string;
  label: string;
  is_default?: boolean;
}

export function getModelOptionsForProvider(provider: AiProvider) {
  if (provider === "claude") return CLAUDE_MODEL_OPTIONS;
  if (provider === "opencode") return [];
  if (provider === "grok") return GROK_MODEL_OPTIONS;
  if (provider === "native") return [];
  return CODEX_MODEL_OPTIONS;
}

export function getDefaultModelForProvider(provider: AiProvider): ModelId {
  if (provider === "claude") return "sonnet";
  if (provider === "opencode") return "openai/gpt-4o";
  if (provider === "grok") return "grok-4.5";
  if (provider === "native") return "default";
  return "gpt-5.4";
}

export function normalizeAiProvider(value: string | null | undefined): AiProvider {
  if (value === "claude") return "claude";
  if (value === "opencode") return "opencode";
  if (value === "grok") return "grok";
  if (value === "native") return "native";
  return "codex";
}

export function normalizeCliAiProvider(value: string | null | undefined): AiProvider {
  const provider = normalizeAiProvider(value);
  return provider === "native" ? "codex" : provider;
}

export function formatEmployeeAiProviderLabel(
  provider: AiProvider | string | null | undefined,
): string {
  switch (normalizeAiProvider(provider)) {
    case "claude":
      return "Claude";
    case "opencode":
      return "OpenCode";
    case "grok":
      return "Grok";
    case "native":
      return "内置 Agent";
    default:
      return "Codex";
  }
}

export function formatEmployeeRuntimeLabel(
  employee?: Pick<Employee, "ai_provider" | "model" | "reasoning_effort"> | null,
): string {
  const provider = formatEmployeeAiProviderLabel(employee?.ai_provider);
  const model = employee?.model?.trim() || "默认模型";
  const effort = employee?.reasoning_effort?.trim() || "默认推理等级";
  return `${provider} / ${model} / ${effort}`;
}

export function formatPlanUsageLogLine(usageLine: string | null | undefined): string | null {
  const rest = usageLine?.trim().replace(/^\[用量\]\s*/, "");
  if (!rest) return null;
  return `[计划] 用量：${rest}`;
}

export interface ClaudeSettings {
  sdk_enabled: boolean;
  default_model: string;
  default_thinking_budget: number;
  sdk_install_dir: string;
  node_path_override: string | null;
  cli_path_override: string | null;
}

export interface ClaudeHealthCheck {
  cli_available: boolean;
  cli_version: string | null;
  sdk_installed: boolean;
  sdk_version: string | null;
  node_available: boolean;
  node_version: string | null;
  sdk_install_dir: string;
  effective_provider: string;
  sdk_status_message: string;
  checked_at: string;
}

export interface ClaudeSdkInstallResult {
  sdk_installed: boolean;
  sdk_version: string | null;
  install_dir: string;
  node_version: string | null;
  message: string;
}

export interface GrokSettings {
  default_model: string;
  default_reasoning_effort: string;
  cli_path_override: string | null;
}

export interface GrokHealthCheck {
  cli_available: boolean;
  cli_version: string | null;
  cli_path: string | null;
  auth_ok?: boolean | null;
  status_message: string;
  checked_at: string;
}

export interface GrokCliInstallResult {
  execution_target: string;
  ssh_config_id: string | null;
  target_host_label: string | null;
  cli_available: boolean;
  cli_version: string | null;
  cli_path: string | null;
  message: string;
}

export interface RemoteGrokHealthCheck {
  available: boolean;
  version: string | null;
  auth_ok?: boolean | null;
  message: string;
  checked_at: string;
}

export const ACTIVE_TASK_STATUSES: {
  value: Exclude<TaskStatus, "archived">;
  label: string;
  color: string;
}[] = [
  { value: "todo", label: "待办", color: "bg-slate-500" },
  { value: "in_progress", label: "进行中", color: "bg-blue-500" },
  { value: "review", label: "审核中", color: "bg-yellow-500" },
  { value: "completed", label: "已完成", color: "bg-green-500" },
  { value: "blocked", label: "已阻塞", color: "bg-red-500" },
];

export const TASK_STATUSES: { value: TaskStatus; label: string; color: string }[] = [
  ...ACTIVE_TASK_STATUSES,
  { value: "archived", label: "已归档", color: "bg-gray-500" },
];

export const PRIORITIES: { value: Priority; label: string; color: string }[] = [
  { value: "low", label: "低", color: "text-slate-500" },
  { value: "medium", label: "中", color: "text-blue-500" },
  { value: "high", label: "高", color: "text-orange-500" },
  { value: "urgent", label: "紧急", color: "text-red-500" },
];
