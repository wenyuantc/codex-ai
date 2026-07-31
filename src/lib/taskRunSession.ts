import { startCodex } from "@/lib/codex";
import { startClaude } from "@/lib/claude";
import { startGrok } from "@/lib/grok";
import { startOpenCode } from "@/lib/opencode";
import { prepareTaskGitExecution } from "@/lib/backend";
import type { Employee, Task } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";

export interface PreparedTaskRunInput {
  prompt: string;
  imagePaths: string[];
  resumeSessionId?: string;
}

export interface StartTaskRunSessionParams {
  task: Task;
  assigneeId: string;
  assignee?: Employee;
  projectRepoPath?: string | null;
  executionInput: PreparedTaskRunInput;
  clearTaskOutput?: boolean;
}

/**
 * Shared kernel for starting a task execution session.
 * Used by useTaskExecutionActions and create-and-run flow.
 * Throws on failure so callers can surface errors consistently.
 */
export async function startTaskRunSession({
  task,
  assigneeId,
  assignee,
  projectRepoPath,
  executionInput,
  clearTaskOutput = false,
}: StartTaskRunSessionParams): Promise<void> {
  if (!assigneeId) {
    throw new Error("请先指定执行员工，再执行任务。");
  }
  if (task.status === "archived") {
    throw new Error("已归档任务不可启动执行会话");
  }

  const employeeStore = useEmployeeStore.getState();
  const taskStore = useTaskStore.getState();

  if (clearTaskOutput) {
    employeeStore.clearTaskCodexOutput(task.id);
  }

  let workingDir = projectRepoPath ?? undefined;
  let taskGitContextId: string | undefined;

  if (task.use_worktree) {
    const prepared = await prepareTaskGitExecution(task.id);
    workingDir = prepared.working_dir;
    taskGitContextId = prepared.task_git_context_id;
  }

  if (!workingDir) {
    throw new Error("当前项目缺少可用工作目录，无法启动任务执行。");
  }

  await employeeStore.updateEmployeeStatus(assigneeId, "busy");
  await taskStore.updateTaskStatus(task.id, "in_progress");

  const startOptions = {
    model: assignee?.model,
    reasoningEffort: assignee?.reasoning_effort,
    systemPrompt: assignee?.system_prompt,
    workingDir,
    taskId: task.id,
    taskGitContextId,
    resumeSessionId: executionInput.resumeSessionId,
    imagePaths: executionInput.imagePaths,
  };

  if (assignee?.ai_provider === "claude") {
    await startClaude(assigneeId, executionInput.prompt, startOptions);
  } else if (assignee?.ai_provider === "opencode") {
    await startOpenCode({
      employeeId: assigneeId,
      taskDescription: executionInput.prompt,
      model: assignee.model,
      workingDir,
      taskId: task.id,
      taskGitContextId,
      resumeSessionId: executionInput.resumeSessionId,
      imagePaths: executionInput.imagePaths,
    });
  } else if (assignee?.ai_provider === "grok") {
    await startGrok(assigneeId, executionInput.prompt, startOptions);
  } else {
    await startCodex(assigneeId, executionInput.prompt, startOptions);
  }

  await taskStore.startTaskTimer(task.id);
  await employeeStore.refreshEmployeeRuntimeStatus(assigneeId);
}

export async function reportTaskRunSessionError(
  error: unknown,
  assigneeId?: string | null,
  taskId?: string,
): Promise<string> {
  const message = error instanceof Error ? error.message : String(error);
  if (assigneeId) {
    const employeeStore = useEmployeeStore.getState();
    employeeStore.addCodexOutput(assigneeId, `[ERROR] ${message}`, taskId);
    const runtime = await employeeStore.refreshEmployeeRuntimeStatus(assigneeId);
    if (!runtime?.running) {
      await employeeStore.updateEmployeeStatus(assigneeId, "error");
    }
  }
  return message;
}
