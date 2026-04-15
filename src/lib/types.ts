export interface Project {
  id: string;
  name: string;
  description: string | null;
  status: string;
  repo_path: string | null;
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
  assignee_id: string | null;
  reviewer_id: string | null;
  complexity: number | null;
  ai_suggestion: string | null;
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
  cli_session_id: string | null;
  working_dir: string | null;
  session_kind: CodexSessionKind;
  status: string;
  started_at: string;
  ended_at: string | null;
  exit_code: number | null;
  resume_session_id: string | null;
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
  task_title: string | null;
  task_status: string | null;
  project_id: string | null;
  project_name: string | null;
  working_dir: string | null;
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
  task_title: string | null;
  project_id: string | null;
  project_name: string | null;
  working_dir: string | null;
  resume_status: CodexSessionResumeStatus;
  resume_message: string | null;
  can_resume: boolean;
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
  checked_at: string;
}

export interface CodexRuntimeStatus {
  running: boolean;
  session: CodexSessionRecord | null;
}

export interface CodexSettings {
  task_sdk_enabled: boolean;
  one_shot_sdk_enabled: boolean;
  one_shot_model: string;
  one_shot_reasoning_effort: string;
  node_path_override: string | null;
  sdk_install_dir: string;
  one_shot_preferred_provider: string;
}

export interface CodexSdkInstallResult {
  sdk_installed: boolean;
  sdk_version: string | null;
  install_dir: string;
  node_version: string | null;
  message: string;
}

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
export type CodexModelId = "gpt-5.4" | "gpt-5.4-mini" | "gpt-5.3-codex" | "gpt-5.2";
export type ReasoningEffort = "low" | "medium" | "high" | "xhigh";
export type TaskStatus = "todo" | "in_progress" | "review" | "completed" | "blocked";
export type EmployeeStatus = "online" | "busy" | "offline" | "error";
export type Priority = "low" | "medium" | "high" | "urgent";

export const CODEX_MODEL_OPTIONS: { value: CodexModelId; label: string }[] = [
  { value: "gpt-5.4", label: "GPT-5.4" },
  { value: "gpt-5.4-mini", label: "GPT-5.4-Mini" },
  { value: "gpt-5.3-codex", label: "GPT-5.3-Codex" },
  { value: "gpt-5.2", label: "GPT-5.2" },
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
