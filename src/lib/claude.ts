import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CodexSessionKind } from "./types";

export interface ClaudeOutput {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string;
}

export interface ClaudeExit {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  status: string;
  line: string | null;
  code: number | null;
}

export interface ClaudeSession {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_id: string;
}

interface StartClaudeOptions {
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

export async function startClaude(
  employeeId: string,
  taskDescription: string,
  options: StartClaudeOptions = {},
): Promise<void> {
  await invoke("start_claude", {
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

export async function stopClaude(employeeId: string): Promise<void> {
  await invoke("stop_claude", { employeeId });
}

export async function stopClaudeSession(
  sessionRecordId: string,
): Promise<void> {
  await invoke("stop_claude_session", { sessionRecordId });
}

export function onClaudeOutput(
  callback: (output: ClaudeOutput) => void,
): Promise<() => void> {
  return listen<ClaudeOutput>("claude-stdout", (event) => {
    callback(event.payload);
  });
}

export function onClaudeError(
  callback: (output: ClaudeOutput) => void,
): Promise<() => void> {
  return listen<ClaudeOutput>("claude-stderr", (event) => {
    callback(event.payload);
  });
}

export function onClaudeExit(
  callback: (exit: ClaudeExit) => void,
): Promise<() => void> {
  return listen<ClaudeExit>("claude-exit", (event) => {
    callback(event.payload);
  });
}

export function onClaudeSession(
  callback: (session: ClaudeSession) => void,
): Promise<() => void> {
  return listen<ClaudeSession>("claude-session", (event) => {
    callback(event.payload);
  });
}
