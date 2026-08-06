import { restartClaude } from "@/lib/claude";
import { restartCodex } from "@/lib/codex";
import { restartGrok } from "@/lib/grok";
import { restartOpenCode } from "@/lib/opencode";
import type { AiProvider, CodexSessionKind } from "@/lib/types";

export interface RestartEngineOptions {
  model?: string;
  reasoningEffort?: string;
  systemPrompt?: string | null;
  workingDir?: string;
  taskId?: string;
  taskGitContextId?: string;
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
    default:
      throw new Error(`未知 AI 引擎：${provider}`);
  }
}
