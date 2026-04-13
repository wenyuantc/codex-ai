import { invoke } from "@tauri-apps/api/core";
import type {
  CodexHealthCheck,
  CodexRuntimeStatus,
  Comment,
  Employee,
  Project,
  Subtask,
  Task,
} from "./types";

export interface CreateProjectInput {
  name: string;
  description?: string | null;
  repo_path?: string | null;
}

export interface UpdateProjectInput {
  name?: string;
  description?: string | null;
  status?: string;
  repo_path?: string | null;
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
  assignee_id?: string | null;
}

export interface UpdateTaskInput {
  title?: string;
  description?: string | null;
  status?: string;
  priority?: string;
  assignee_id?: string | null;
  complexity?: number | null;
  ai_suggestion?: string | null;
  last_codex_session_id?: string | null;
}

export async function healthCheck(): Promise<CodexHealthCheck> {
  return invoke("health_check");
}

export async function getCodexSessionStatus(employeeId: string): Promise<CodexRuntimeStatus> {
  return invoke("get_codex_session_status", { employeeId });
}

export async function createProject(input: CreateProjectInput): Promise<Project> {
  return invoke("create_project", { payload: input });
}

export async function updateProject(id: string, updates: UpdateProjectInput): Promise<Project> {
  return invoke("update_project", { id, updates });
}

export async function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
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
