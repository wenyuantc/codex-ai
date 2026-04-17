import { useState } from "react";

import { startCodex, stopCodex } from "@/lib/codex";
import { prepareTaskGitExecution } from "@/lib/backend";
import type { Employee, ProjectType, Task } from "@/lib/types";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";

export type TaskExecutionAction = "run" | "stop" | "continue";

interface PreparedExecutionInput {
  prompt: string;
  imagePaths: string[];
  resumeSessionId?: string;
}

interface UseTaskExecutionActionsOptions {
  task: Task;
  assigneeId?: string | null;
  assignee?: Employee;
  projectRepoPath?: string | null;
  projectType?: ProjectType;
  prepareExecutionInput: (followUpPrompt?: string) => Promise<PreparedExecutionInput>;
  clearTaskOutputOnRun?: boolean;
  clearTaskOutputOnContinue?: boolean;
  clearEmployeeOutputOnRun?: boolean;
  onStarted?: (action: Exclude<TaskExecutionAction, "stop">) => void;
  onStopped?: () => void;
  onError?: (message: string, action: TaskExecutionAction) => void;
}

export function useTaskExecutionActions({
  task,
  assigneeId,
  assignee,
  projectRepoPath,
  projectType = "local",
  prepareExecutionInput,
  clearTaskOutputOnRun = false,
  clearTaskOutputOnContinue = false,
  clearEmployeeOutputOnRun = false,
  onStarted,
  onStopped,
  onError,
}: UseTaskExecutionActionsOptions) {
  const [loading, setLoading] = useState<TaskExecutionAction | null>(null);
  const codexProcesses = useEmployeeStore((state) => state.codexProcesses);
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const setCodexRunning = useEmployeeStore((state) => state.setCodexRunning);
  const addCodexOutput = useEmployeeStore((state) => state.addCodexOutput);
  const clearCodexOutput = useEmployeeStore((state) => state.clearCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((state) => state.clearTaskCodexOutput);
  const refreshCodexRuntimeStatus = useEmployeeStore((state) => state.refreshCodexRuntimeStatus);
  const updateTaskStatus = useTaskStore((state) => state.updateTaskStatus);

  const isRunning = assigneeId
    ? (codexProcesses[assigneeId]?.running ?? false) && codexProcesses[assigneeId]?.activeTaskId === task.id
    : false;
  const output = taskLogs[buildTaskLogKey(task.id, "execution")] ?? [];

  const handleExecutionError = async (error: unknown, action: TaskExecutionAction) => {
    const message = error instanceof Error ? error.message : String(error);
    if (assigneeId) {
      addCodexOutput(assigneeId, `[ERROR] ${message}`, task.id);
      setCodexRunning(assigneeId, false, null);
      await refreshCodexRuntimeStatus(assigneeId);
    }
    onError?.(message, action);
  };

  const startExecution = async (action: "run" | "continue", followUpPrompt?: string) => {
    if (!assigneeId) {
      return;
    }

    setLoading(action);
    try {
      if (action === "run") {
        if (clearEmployeeOutputOnRun) {
          clearCodexOutput(assigneeId);
        }
        if (clearTaskOutputOnRun) {
          clearTaskCodexOutput(task.id);
        }
      } else if (clearTaskOutputOnContinue) {
        clearTaskCodexOutput(task.id);
      }

      const executionInput = await prepareExecutionInput(followUpPrompt);
      let workingDir = projectRepoPath ?? undefined;
      let taskGitContextId: string | undefined;

      if (projectType === "local") {
        const prepared = await prepareTaskGitExecution(task.id);
        workingDir = prepared.working_dir;
        taskGitContextId = prepared.task_git_context_id;
      }

      if (!workingDir) {
        throw new Error("当前项目缺少可用工作目录，无法启动任务执行。");
      }

      await updateEmployeeStatus(assigneeId, "busy");
      await updateTaskStatus(task.id, "in_progress");
      setCodexRunning(assigneeId, true, task.id);
      await startCodex(assigneeId, executionInput.prompt, {
        model: assignee?.model,
        reasoningEffort: assignee?.reasoning_effort,
        systemPrompt: assignee?.system_prompt,
        workingDir,
        taskId: task.id,
        taskGitContextId,
        resumeSessionId: executionInput.resumeSessionId,
        imagePaths: executionInput.imagePaths,
      });
      onStarted?.(action);
    } catch (error) {
      await handleExecutionError(error, action);
    } finally {
      setLoading(null);
    }
  };

  const runTask = async () => {
    await startExecution("run");
  };

  const continueTask = async (followUpPrompt: string) => {
    await startExecution("continue", followUpPrompt);
  };

  const stopTask = async () => {
    if (!assigneeId) {
      return;
    }

    setLoading("stop");
    try {
      await stopCodex(assigneeId);
      setCodexRunning(assigneeId, false, null);
      await updateEmployeeStatus(assigneeId, "offline");
      await refreshCodexRuntimeStatus(assigneeId);
      onStopped?.();
    } catch (error) {
      await handleExecutionError(error, "stop");
    } finally {
      setLoading(null);
    }
  };

  return {
    isRunning,
    output,
    loading,
    runTask,
    continueTask,
    stopTask,
  };
}
