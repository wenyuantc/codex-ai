import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CodexSessionKind } from "./types";

export interface CodexOutput {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string;
}

export interface CodexExit {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string | null;
  code: number | null;
}

export interface CodexSession {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_id: string;
}

interface StartCodexOptions {
  model?: string;
  reasoningEffort?: string;
  systemPrompt?: string | null;
  workingDir?: string;
  taskId?: string;
  resumeSessionId?: string;
  sessionKind?: CodexSessionKind;
  imagePaths?: string[];
}

interface AiExecutionContext {
  taskId?: string;
  workingDir?: string;
}

export type AiOptimizePromptScene =
  | "task_create"
  | "task_continue"
  | "session_continue";

export interface AiOptimizePromptInput {
  scene: AiOptimizePromptScene;
  projectName: string;
  projectDescription?: string | null;
  projectRepoPath?: string | null;
  title?: string | null;
  description?: string | null;
  currentPrompt?: string | null;
  taskTitle?: string | null;
  sessionSummary?: string | null;
  taskId?: string | null;
  workingDir?: string | null;
}

export async function startCodex(employeeId: string, taskDescription: string, options: StartCodexOptions = {}): Promise<void> {
  await invoke("start_codex", {
    employeeId,
    taskDescription,
    model: options.model ?? null,
    reasoningEffort: options.reasoningEffort ?? null,
    systemPrompt: options.systemPrompt ?? null,
    workingDir: options.workingDir ?? null,
    taskId: options.taskId ?? null,
    resumeSessionId: options.resumeSessionId ?? null,
    sessionKind: options.sessionKind ?? null,
    imagePaths: options.imagePaths ?? null,
  });
}

export async function stopCodex(employeeId: string): Promise<void> {
  await invoke("stop_codex", { employeeId });
}

export async function restartCodex(employeeId: string, taskDescription: string, options: StartCodexOptions = {}): Promise<void> {
  await invoke("restart_codex", {
    employeeId,
    taskDescription,
    model: options.model ?? null,
    reasoningEffort: options.reasoningEffort ?? null,
    systemPrompt: options.systemPrompt ?? null,
    workingDir: options.workingDir ?? null,
  });
}

export async function sendCodexInput(employeeId: string, input: string): Promise<void> {
  await invoke("send_codex_input", { employeeId, input });
}

export function onCodexOutput(callback: (output: CodexOutput) => void) {
  return listen<CodexOutput>("codex-stdout", (event) => callback(event.payload));
}

export function onCodexError(callback: (output: CodexOutput) => void) {
  return listen<CodexOutput>("codex-stderr", (event) => callback(event.payload));
}

export function onCodexExit(callback: (exit: CodexExit) => void) {
  return listen<CodexExit>("codex-exit", (event) => callback(event.payload));
}

export function onCodexSession(callback: (session: CodexSession) => void) {
  return listen<CodexSession>("codex-session", (event) => callback(event.payload));
}

// AI-powered features
export async function aiSuggestAssignee(
  taskDescription: string,
  employeeList: string,
  imagePaths?: string[],
  context: AiExecutionContext = {},
): Promise<string> {
  return invoke<string>("ai_suggest_assignee", {
    taskDescription,
    employeeList,
    imagePaths: imagePaths ?? null,
    taskId: context.taskId ?? null,
    workingDir: context.workingDir ?? null,
  });
}

export async function aiAnalyzeComplexity(
  taskDescription: string,
  imagePaths?: string[],
  context: AiExecutionContext = {},
): Promise<string> {
  return invoke<string>("ai_analyze_complexity", {
    taskDescription,
    imagePaths: imagePaths ?? null,
    taskId: context.taskId ?? null,
    workingDir: context.workingDir ?? null,
  });
}

export async function aiGenerateComment(
  taskTitle: string,
  taskDescription: string,
  context: string,
  imagePaths?: string[],
  executionContext: AiExecutionContext = {},
): Promise<string> {
  return invoke<string>("ai_generate_comment", {
    taskTitle,
    taskDescription,
    context,
    imagePaths: imagePaths ?? null,
    taskId: executionContext.taskId ?? null,
    workingDir: executionContext.workingDir ?? null,
  });
}

export async function aiSplitSubtasks(
  taskTitle: string,
  taskDescription: string,
  imagePaths?: string[],
  context: AiExecutionContext = {},
): Promise<string[]> {
  return invoke<string[]>("ai_split_subtasks", {
    taskTitle,
    taskDescription,
    imagePaths: imagePaths ?? null,
    taskId: context.taskId ?? null,
    workingDir: context.workingDir ?? null,
  });
}

export async function aiGeneratePlan(
  taskTitle: string,
  taskDescription: string,
  taskStatus: string,
  taskPriority: string,
  subtasks: string[],
  imagePaths?: string[],
  context: AiExecutionContext = {},
): Promise<string> {
  return invoke<string>("ai_generate_plan", {
    taskTitle,
    taskDescription,
    taskStatus,
    taskPriority,
    subtasks,
    imagePaths: imagePaths ?? null,
    taskId: context.taskId ?? null,
    workingDir: context.workingDir ?? null,
  });
}

export async function aiOptimizePrompt(input: AiOptimizePromptInput): Promise<string> {
  return invoke<string>("ai_optimize_prompt", {
    scene: input.scene,
    projectName: input.projectName,
    projectDescription: input.projectDescription ?? null,
    projectRepoPath: input.projectRepoPath ?? null,
    title: input.title ?? null,
    description: input.description ?? null,
    currentPrompt: input.currentPrompt ?? null,
    taskTitle: input.taskTitle ?? null,
    sessionSummary: input.sessionSummary ?? null,
    taskId: input.taskId ?? null,
    workingDir: input.workingDir ?? null,
  });
}
