import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CodexSessionKind } from "./types";

export interface GrokOutput {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string;
}

export interface GrokExit {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  status: string;
  line: string | null;
  code: number | null;
}

export interface GrokSession {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_id: string;
}

interface StartGrokOptions {
  model?: string;
  reasoningEffort?: string;
  systemPrompt?: string | null;
  workingDir?: string;
  taskId?: string;
  taskGitContextId?: string;
  resumeSessionId?: string;
  sessionKind?: CodexSessionKind;
  imagePaths?: string[];
}

export async function startGrok(
  employeeId: string,
  taskDescription: string,
  options: StartGrokOptions = {},
): Promise<void> {
  await invoke("start_grok", {
    employeeId,
    taskDescription,
    model: options.model ?? null,
    reasoningEffort: options.reasoningEffort ?? null,
    systemPrompt: options.systemPrompt ?? null,
    workingDir: options.workingDir ?? null,
    taskId: options.taskId ?? null,
    taskGitContextId: options.taskGitContextId ?? null,
    resumeSessionId: options.resumeSessionId ?? null,
    imagePaths: options.imagePaths ?? null,
    sessionKind: options.sessionKind ?? null,
  });
}

export async function stopGrok(employeeId: string): Promise<void> {
  await invoke("stop_grok", { employeeId });
}

export async function stopGrokSession(sessionRecordId: string): Promise<void> {
  await invoke("stop_grok_session", { sessionRecordId });
}

export function onGrokOutput(callback: (output: GrokOutput) => void): Promise<() => void> {
  return listen<GrokOutput>("grok-stdout", (event) => {
    callback(event.payload);
  });
}

export function onGrokError(callback: (output: GrokOutput) => void): Promise<() => void> {
  return listen<GrokOutput>("grok-stderr", (event) => {
    callback(event.payload);
  });
}

export function onGrokExit(callback: (exit: GrokExit) => void): Promise<() => void> {
  return listen<GrokExit>("grok-exit", (event) => {
    callback(event.payload);
  });
}

export function onGrokSession(callback: (session: GrokSession) => void): Promise<() => void> {
  return listen<GrokSession>("grok-session", (event) => {
    callback(event.payload);
  });
}
