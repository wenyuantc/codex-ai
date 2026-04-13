import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface CodexOutput {
  employee_id: string;
  line: string;
}

export interface CodexExit {
  employee_id: string;
  code: number | null;
}

export interface CodexSession {
  employee_id: string;
  task_id: string | null;
  session_id: string;
}

interface StartCodexOptions {
  model?: string;
  reasoningEffort?: string;
  systemPrompt?: string | null;
  workingDir?: string;
  taskId?: string;
  resumeSessionId?: string;
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
export async function aiSuggestAssignee(taskDescription: string, employeeList: string): Promise<string> {
  return invoke<string>("ai_suggest_assignee", { taskDescription, employeeList });
}

export async function aiAnalyzeComplexity(taskDescription: string): Promise<string> {
  return invoke<string>("ai_analyze_complexity", { taskDescription });
}

export async function aiGenerateComment(taskTitle: string, taskDescription: string, context: string): Promise<string> {
  return invoke<string>("ai_generate_comment", { taskTitle, taskDescription, context });
}

export async function aiSplitSubtasks(taskTitle: string, taskDescription: string): Promise<string[]> {
  return invoke<string[]>("ai_split_subtasks", { taskTitle, taskDescription });
}
