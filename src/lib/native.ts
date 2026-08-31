import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CodexSessionKind, StartSessionOutcome } from "./types";
import type { NativeSubagentScope } from "./nativeSubagentScope";

export type { NativeSubagentScope } from "./nativeSubagentScope";

/** K-token settings use decimal thousands at the UI boundary; persisted
 * settings and the native runner continue to use individual tokens. */
export const NATIVE_TOKEN_UNIT = 1_000;

export function nativeTokensToK(tokens: number): number {
  return Number.isFinite(tokens) ? tokens / NATIVE_TOKEN_UNIT : 0;
}

export function nativeKToTokens(kTokens: number): number {
  return Number.isFinite(kTokens) ? Math.round(kTokens * NATIVE_TOKEN_UNIT) : 0;
}

/** Normalize a user-entered K value to a valid integer-token setting. */
export function normalizeNativeTokenK(
  kTokens: number,
  minTokens: number,
  maxTokens: number,
  defaultTokens: number,
): number {
  const tokens = Number.isFinite(kTokens) ? nativeKToTokens(kTokens) : defaultTokens;
  const bounded = Math.min(maxTokens, Math.max(minTokens, tokens));
  return nativeTokensToK(bounded);
}

export interface NativeOutput {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  session_event_id: string | null;
  line: string;
}

/** Live fragment of the answer being generated. Never persisted: the matching
 * `native-stdout` line arrives right after `clear` and replaces it. */
export interface NativeTextDelta {
  employee_id: string;
  task_id: string | null;
  session_kind: CodexSessionKind;
  session_record_id: string;
  segment: "text" | "reasoning";
  delta: string;
  clear: boolean;
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
  planMode?: boolean;
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
    planMode: options.planMode === true,
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

export function onNativeTextDelta(callback: (delta: NativeTextDelta) => void): Promise<() => void> {
  return listen<NativeTextDelta>("native-text-delta", (event) => {
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

export type NativePermissionDecision = "allow_session" | "allow_once" | "allow_server" | "deny";

export type NativeToolRiskKind = "overwrite" | "delete" | "push" | "force_git" | "mcp" | "opaque";

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
  mcpServerId?: string | null;
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

export interface NativePlanQuestionItem {
  prompt: string;
  options: string[];
}

export interface NativePlanQuestionRequest {
  sessionRecordId: string;
  requestId: string;
  employeeId: string;
  taskId: string | null;
  sessionKind: string;
  questions: NativePlanQuestionItem[];
}

export function onNativePlanQuestion(
  callback: (request: NativePlanQuestionRequest) => void,
): Promise<() => void> {
  return listen<NativePlanQuestionRequest>("native-plan-question", (event) => {
    callback(event.payload);
  });
}

export async function answerNativePlanQuestion(
  sessionRecordId: string,
  requestId: string,
  skipped: boolean,
  answers: string[],
): Promise<void> {
  await invoke("answer_native_plan_question", {
    sessionRecordId,
    requestId,
    skipped,
    answers,
  });
}

export type NativeSubagentPolicy = "conservative" | "balanced" | "aggressive";

export interface NativeSettings {
  max_turns: number;
  confirm_high_risk: boolean;
  max_concurrent_subagents: number;
  subagent_policy: NativeSubagentPolicy | string;
  context_window_tokens: number;
  rollout_token_budget: number;
  max_tool_output_tokens: number;
  permission_timeout_secs: number;
  subagent_budget_share_percent: number;
}

export interface UpdateNativeSettings {
  max_turns?: number;
  confirm_high_risk?: boolean;
  max_concurrent_subagents?: number;
  subagent_policy?: NativeSubagentPolicy | string;
  context_window_tokens?: number;
  rollout_token_budget?: number;
  max_tool_output_tokens?: number;
  permission_timeout_secs?: number;
  subagent_budget_share_percent?: number;
}

export async function getNativeSettings(): Promise<NativeSettings> {
  return invoke<NativeSettings>("get_native_settings");
}

export async function updateNativeSettings(updates: UpdateNativeSettings): Promise<NativeSettings> {
  return invoke<NativeSettings>("update_native_settings", { updates });
}

export const NATIVE_SUBAGENT_CUSTOM_TOOLS = [
  "Read",
  "Grep",
  "Glob",
  "Bash",
  "Edit",
  "Write",
  "WebFetch",
  "WebSearch",
  "TodoWrite",
] as const;

export type NativeSubagentCustomTool = (typeof NATIVE_SUBAGENT_CUSTOM_TOOLS)[number];
export type NativeSubagentModelMode = "inherit" | "channel";
export type NativeSubagentToolMode = "all" | "custom";

export interface NativeSubagent {
  id: string;
  name: string;
  description: string;
  model_mode: NativeSubagentModelMode | string;
  channel_id: string | null;
  model: string | null;
  tool_mode: NativeSubagentToolMode | string;
  tools: string[];
  system_prompt: string;
  inject_agents_md: boolean;
  scope: NativeSubagentScope | string;
  project_ids: string[];
}

export interface CreateNativeSubagent {
  name: string;
  description: string;
  model_mode?: NativeSubagentModelMode | string;
  channel_id?: string | null;
  model?: string | null;
  tool_mode?: NativeSubagentToolMode | string;
  tools?: string[];
  system_prompt?: string;
  inject_agents_md?: boolean;
  scope?: NativeSubagentScope | string;
  project_ids?: string[];
}

export interface UpdateNativeSubagent {
  name?: string;
  description?: string;
  model_mode?: NativeSubagentModelMode | string;
  channel_id?: string | null;
  model?: string | null;
  tool_mode?: NativeSubagentToolMode | string;
  tools?: string[];
  system_prompt?: string;
  inject_agents_md?: boolean;
  scope?: NativeSubagentScope | string;
  project_ids?: string[];
}

export async function listNativeSubagents(): Promise<NativeSubagent[]> {
  return invoke<NativeSubagent[]>("list_native_subagents");
}

export async function createNativeSubagent(payload: CreateNativeSubagent): Promise<NativeSubagent> {
  return invoke<NativeSubagent>("create_native_subagent", { payload });
}

export async function updateNativeSubagent(
  id: string,
  payload: UpdateNativeSubagent,
): Promise<NativeSubagent> {
  return invoke<NativeSubagent>("update_native_subagent", { id, payload });
}

export async function deleteNativeSubagent(id: string): Promise<void> {
  await invoke("delete_native_subagent", { id });
}
