import { useState } from "react";

import { stopSessionByProvider } from "@/lib/aiEngine";
import { isImageSkipCancelled } from "@/lib/imageAttachmentSkip";
import { reportTaskRunSessionError, startTaskRunSession } from "@/lib/taskRunSession";
import type { Employee, ProjectType, Task } from "@/lib/types";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";

export type TaskExecutionAction = "run" | "stop" | "continue";

interface PreparedExecutionInput {
  prompt: string;
  imagePaths: string[];
  resumeSessionId?: string;
}

interface PrepareExecutionInputOptions {
  planContent?: string;
}

interface UseTaskExecutionActionsOptions {
  task: Task;
  assigneeId?: string | null;
  assignee?: Employee;
  projectRepoPath?: string | null;
  projectType?: ProjectType;
  prepareExecutionInput: (
    followUpPrompt?: string,
    options?: PrepareExecutionInputOptions,
  ) => Promise<PreparedExecutionInput>;
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
  const employeeRuntime = useEmployeeStore((state) =>
    assigneeId ? state.employeeRuntime[assigneeId] : undefined,
  );
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const refreshEmployeeRuntimeStatus = useEmployeeStore(
    (state) => state.refreshEmployeeRuntimeStatus,
  );
  const fetchTaskAutomationState = useTaskStore((state) => state.fetchTaskAutomationState);
  const fetchTasks = useTaskStore((state) => state.fetchTasks);

  const runningSession =
    employeeRuntime?.sessions.find(
      (session) => session.task_id === task.id && session.session_kind === "execution",
    ) ?? null;
  const isRunning = Boolean(runningSession);
  const output = taskLogs[buildTaskLogKey(task.id, "execution")] ?? [];

  const handleExecutionError = async (error: unknown, action: TaskExecutionAction) => {
    const message = await reportTaskRunSessionError(error, assigneeId, task.id);
    onError?.(message, action);
  };

  const startExecution = async (
    action: "run" | "continue",
    followUpPrompt?: string,
    options?: PrepareExecutionInputOptions,
  ) => {
    if (!assigneeId) {
      await handleExecutionError(new Error("请先指定执行员工，再执行任务。"), action);
      return;
    }

    setLoading(action);
    try {
      const executionInput = await prepareExecutionInput(followUpPrompt, options);
      await startTaskRunSession({
        task,
        assigneeId,
        assignee,
        projectRepoPath,
        executionInput,
        clearTaskOutput:
          (action === "run" && clearTaskOutputOnRun) ||
          (action === "continue" && clearTaskOutputOnContinue),
      });
      onStarted?.(action);
    } catch (error) {
      if (isImageSkipCancelled(error)) {
        return;
      }
      await handleExecutionError(error, action);
    } finally {
      setLoading(null);
    }
  };

  const runTask = async (planContent?: string) => {
    await startExecution("run", undefined, { planContent });
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
      await stopSessionByProvider(runningSession.ai_provider, runningSession.session_record_id);
      const runtime = await refreshEmployeeRuntimeStatus(assigneeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(assigneeId, "offline");
      }
      // Automation may flip to manual_control after the process exits; refresh so the
      // card does not keep treating stale waiting_execution phases as "运行中".
      // Exit handlers can finish after stop returns — do an immediate refresh plus a
      // short deferred one to cover that race (backend also emits when ready).
      const refreshAfterStop = async () => {
        if (task.automation_mode === "review_fix_loop_v1") {
          await fetchTaskAutomationState(task.id);
        }
        await fetchTasks(useTaskStore.getState().activeProjectId);
      };
      await refreshAfterStop();
      window.setTimeout(() => {
        void refreshAfterStop();
      }, 400);
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
