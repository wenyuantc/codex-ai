import { restartClaude, startClaude, stopClaudeSession } from "@/lib/claude";
import { restartCodex, startCodex, stopCodexSession } from "@/lib/codex";
import { restartGrok, startGrok, stopGrokSession } from "@/lib/grok";
import { restartNative, startNative, stopNativeSession } from "@/lib/native";
import { restartOpenCode, startOpenCode, stopOpenCodeSession } from "@/lib/opencode";
import type { AiProvider, CodexSessionKind, StartSessionOutcome } from "@/lib/types";

export interface RestartEngineOptions {
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

/** Thin unified restart entry; fails closed for unknown providers. */
export async function restartByProvider(
  provider: AiProvider | string,
  employeeId: string,
  taskDescription: string,
  options: RestartEngineOptions = {},
): Promise<void> {
  switch (provider) {
    case "codex":
      await restartCodex(employeeId, taskDescription, options);
      return;
    case "claude":
      await restartClaude(employeeId, taskDescription, options);
      return;
    case "grok":
      await restartGrok(employeeId, taskDescription, options);
      return;
    case "opencode":
      await restartOpenCode({
        employeeId,
        taskDescription,
        model: options.model,
        workingDir: options.workingDir,
        taskId: options.taskId,
        taskGitContextId: options.taskGitContextId,
        imagePaths: options.imagePaths,
      });
      return;
    case "native":
      await restartNative(employeeId, taskDescription, options);
      return;
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}

export async function startByProvider(
  provider: AiProvider | string,
  employeeId: string,
  taskDescription: string,
  options: RestartEngineOptions = {},
): Promise<StartSessionOutcome> {
  switch (provider) {
    case "claude":
      return startClaude(employeeId, taskDescription, options);
    case "grok":
      return startGrok(employeeId, taskDescription, options);
    case "opencode":
      return startOpenCode({
        employeeId,
        taskDescription,
        model: options.model,
        workingDir: options.workingDir,
        taskId: options.taskId,
        taskGitContextId: options.taskGitContextId,
        resumeSessionId: options.resumeSessionId,
        imagePaths: options.imagePaths,
      });
    case "native":
      return startNative(employeeId, taskDescription, options);
    case "codex":
      return startCodex(employeeId, taskDescription, options);
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}

export async function stopSessionByProvider(
  provider: AiProvider | string,
  sessionRecordId: string,
): Promise<void> {
  switch (provider) {
    case "claude":
      await stopClaudeSession(sessionRecordId);
      return;
    case "opencode":
      await stopOpenCodeSession(sessionRecordId);
      return;
    case "grok":
      await stopGrokSession(sessionRecordId);
      return;
    case "native":
      await stopNativeSession(sessionRecordId);
      return;
    case "codex":
      await stopCodexSession(sessionRecordId);
      return;
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}
