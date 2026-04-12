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

export type TaskStatus = "todo" | "in_progress" | "review" | "completed" | "blocked";
export type EmployeeStatus = "online" | "busy" | "offline" | "error";
export type Priority = "low" | "medium" | "high" | "urgent";

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
