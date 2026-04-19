import { useState } from "react";

import { startCodex, stopCodexSession } from "@/lib/codex";
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
  onStarted?: (action: Exclude<TaskExecutionAction, "stop">) => void;
  onStopped?: () => void;
  onError?: (message: string, action: TaskExecutionAction) => void;
}

export function useTaskExecutionActions({
  task,
  assigneeId,
  assignee,
  projectRepoPath,
  projectType: _projectType = "local",
  prepareExecutionInput,
  clearTaskOutputOnRun = false,
  clearTaskOutputOnContinue = false,
  onStarted,
  onStopped,
  onError,
}: UseTaskExecutionActionsOptions) {
  const [loading, setLoading] = useState<TaskExecutionAction | null>(null);
  const employeeRuntime = useEmployeeStore((state) => (
    assigneeId ? state.employeeRuntime[assigneeId] : undefined
  ));
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const addCodexOutput = useEmployeeStore((state) => state.addCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((state) => state.clearTaskCodexOutput);
  const refreshEmployeeRuntimeStatus = useEmployeeStore((state) => state.refreshEmployeeRuntimeStatus);
  const updateTaskStatus = useTaskStore((state) => state.updateTaskStatus);

  const runningSession = employeeRuntime?.sessions.find((session) => (
    session.task_id === task.id && session.session_kind === "execution"
  )) ?? null;
  const isRunning = Boolean(runningSession);
  const output = taskLogs[buildTaskLogKey(task.id, "execution")] ?? [];

  const handleExecutionError = async (error: unknown, action: TaskExecutionAction) => {
    const message = error instanceof Error ? error.message : String(error);
    if (assigneeId) {
      addCodexOutput(assigneeId, `[ERROR] ${message}`, task.id);
      const runtime = await refreshEmployeeRuntimeStatus(assigneeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(assigneeId, "error");
      }
    }
    onError?.(message, action);
  };

  const startExecution = async (action: "run" | "continue", followUpPrompt?: string) => {
    if (!assigneeId) {
      return;
    }

    setLoading(action);
    try {
      if (action === "run" && clearTaskOutputOnRun) {
        clearTaskCodexOutput(task.id);
      } else if (action === "continue" && clearTaskOutputOnContinue) {
        clearTaskCodexOutput(task.id);
      }

      const executionInput = await prepareExecutionInput(followUpPrompt);
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

      await updateEmployeeStatus(assigneeId, "busy");
      await updateTaskStatus(task.id, "in_progress");
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
      await refreshEmployeeRuntimeStatus(assigneeId);
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
    if (!runningSession) {
      return;
    }

    setLoading("stop");
    try {
      await stopCodexSession(runningSession.session_record_id);
      const runtime = await refreshEmployeeRuntimeStatus(assigneeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(assigneeId, "offline");
      }
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
