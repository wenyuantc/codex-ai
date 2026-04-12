import { invoke } from "@tauri-apps/api/core";
import { listen, type Unlisten } from "@tauri-apps/api/event";

export interface CodexOutput {
  employee_id: string;
  line: string;
}

export interface CodexExit {
  employee_id: string;
  code: number | null;
}

export async function startCodex(employeeId: string, taskDescription: string): Promise<void> {
  await invoke("start_codex", { employeeId, taskDescription });
}

export async function stopCodex(employeeId: string): Promise<void> {
  await invoke("stop_codex", { employeeId });
}

export async function restartCodex(employeeId: string, taskDescription: string): Promise<void> {
  await invoke("restart_codex", { employeeId, taskDescription });
}

export async function sendCodexInput(employeeId: string, input: string): Promise<void> {
  await invoke("send_codex_input", { employeeId, input });
}

export function onCodexOutput(callback: (output: CodexOutput) => void): Promise<Unlisten> {
  return listen<CodexOutput>("codex-stdout", (event) => callback(event.payload));
}

export function onCodexError(callback: (output: CodexOutput) => void): Promise<Unlisten> {
  return listen<CodexOutput>("codex-stderr", (event) => callback(event.payload));
}

export function onCodexExit(callback: (exit: CodexExit) => void): Promise<Unlisten> {
  return listen<CodexExit>("codex-exit", (event) => callback(event.payload));
}
