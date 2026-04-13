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
  complexity: number | null;
  ai_suggestion: string | null;
  last_codex_session_id: string | null;
  created_at: string;
  updated_at: string;
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
