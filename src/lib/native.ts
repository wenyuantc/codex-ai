import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CodexSessionKind, StartSessionOutcome } from "./types";

export interface NativeOutput {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string;
}

export interface NativeExit {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string | null;
  code: number | null;
}

export interface NativeSession {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_id: string;
}

interface StartNativeOptions {
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

export async function startNative(
  employeeId: string,
  taskDescription: string,
  options: StartNativeOptions = {},
): Promise<StartSessionOutcome> {
  return invoke<StartSessionOutcome>("start_native_session", {
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

export async function stopNative(employeeId: string): Promise<void> {
  await invoke("stop_native", { employeeId });
}

export async function stopNativeSession(sessionRecordId: string): Promise<void> {
  await invoke("stop_native_session", { sessionRecordId });
}

export async function restartNative(
  employeeId: string,
  taskDescription: string,
  options: StartNativeOptions = {},
): Promise<void> {
  await invoke("restart_native_session", {
    employeeId,
    taskDescription,
    model: options.model ?? null,
    reasoningEffort: options.reasoningEffort ?? null,
    systemPrompt: options.systemPrompt ?? null,
    workingDir: options.workingDir ?? null,
    taskId: options.taskId ?? null,
    taskGitContextId: options.taskGitContextId ?? null,
    imagePaths: options.imagePaths ?? null,
    sessionKind: options.sessionKind ?? null,
  });
}

export async function sendNativeInput(employeeId: string, input: string): Promise<void> {
  await invoke("send_native_input", { employeeId, input });
}

export async function finishNativeInput(employeeId: string): Promise<void> {
  await invoke("finish_native_input", { employeeId });
}

export function onNativeOutput(callback: (output: NativeOutput) => void): Promise<() => void> {
  return listen<NativeOutput>("native-stdout", (event) => {
    callback(event.payload);
  });
}

export function onNativeExit(callback: (exit: NativeExit) => void): Promise<() => void> {
  return listen<NativeExit>("native-exit", (event) => {
    callback(event.payload);
  });
}

export function onNativeSession(callback: (session: NativeSession) => void): Promise<() => void> {
  return listen<NativeSession>("native-session", (event) => {
    callback(event.payload);
  });
}

export type NativePermissionDecision = "allow_session" | "allow_once" | "deny";

export type NativeToolRiskKind = "overwrite" | "delete" | "push" | "force_git" | "mcp";

export interface NativePermissionRequest {
  sessionRecordId: string;
  requestId: string;
  employeeId: string;
  taskId: string | null;
  sessionKind: string;
  toolName: string;
  kind: NativeToolRiskKind;
  summary: string;
  remote: boolean;
}

export function onNativePermissionRequest(
  callback: (request: NativePermissionRequest) => void,
): Promise<() => void> {
  return listen<NativePermissionRequest>("native-permission-request", (event) => {
    callback(event.payload);
  });
}

export async function resolveNativeToolPermission(
  sessionRecordId: string,
  requestId: string,
  decision: NativePermissionDecision,
): Promise<void> {
  await invoke("resolve_native_tool_permission", {
    sessionRecordId,
    requestId,
    decision,
  });
}

export type NativeSubagentPolicy = "conservative" | "balanced" | "aggressive";

export interface NativeSettings {
  max_turns: number;
  confirm_high_risk: boolean;
  max_concurrent_subagents: number;
  subagent_policy: NativeSubagentPolicy | string;
}

export interface UpdateNativeSettings {
  max_turns?: number;
  confirm_high_risk?: boolean;
  max_concurrent_subagents?: number;
  subagent_policy?: NativeSubagentPolicy | string;
}

export async function getNativeSettings(): Promise<NativeSettings> {
  return invoke<NativeSettings>("get_native_settings");
}

export async function updateNativeSettings(updates: UpdateNativeSettings): Promise<NativeSettings> {
  return invoke<NativeSettings>("update_native_settings", { updates });
}
