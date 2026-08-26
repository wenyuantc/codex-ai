import { useState } from "react";

import { stopSessionByProvider } from "@/lib/aiEngine";
import { startTaskCodeReview } from "@/lib/backend";
import type { Task } from "@/lib/types";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";

export type TaskReviewAction = "start" | "stop";

interface UseTaskReviewActionsOptions {
  task: Task;
  reviewerId?: string | null;
  status: string;
  onStarted?: () => void;
  onStopped?: () => void;
  onError?: (message: string) => void;
}

export function useTaskReviewActions({
  task,
  reviewerId,
  status,
  onStarted,
  onStopped,
  onError,
}: UseTaskReviewActionsOptions) {
  const [loading, setLoading] = useState<TaskReviewAction | null>(null);
  const employeeRuntime = useEmployeeStore((state) =>
    reviewerId ? state.employeeRuntime[reviewerId] : undefined,
  );
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const addCodexOutput = useEmployeeStore((state) => state.addCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((state) => state.clearTaskCodexOutput);
  const refreshEmployeeRuntimeStatus = useEmployeeStore(
    (state) => state.refreshEmployeeRuntimeStatus,
  );
  const fetchTaskAutomationState = useTaskStore((state) => state.fetchTaskAutomationState);
  const fetchTasks = useTaskStore((state) => state.fetchTasks);

  const runningSession =
    employeeRuntime?.sessions.find(
      (session) => session.task_id === task.id && session.session_kind === "review",
    ) ?? null;
  const isRunning = Boolean(runningSession);
  const output = taskLogs[buildTaskLogKey(task.id, "review")] ?? [];

  const startReview = async () => {
    if (status !== "review" || !reviewerId) {
      return;
    }

    setLoading("start");
    try {
      await updateEmployeeStatus(reviewerId, "busy");
      clearTaskCodexOutput(task.id, "review");
      await startTaskCodeReview(task.id);
      await refreshEmployeeRuntimeStatus(reviewerId);
      onStarted?.();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      addCodexOutput(reviewerId, `[ERROR] ${message}`, task.id, "review");
      const runtime = await refreshEmployeeRuntimeStatus(reviewerId);
      if (!runtime?.running) {
        await updateEmployeeStatus(reviewerId, "error");
      }
      onError?.(message);
    } finally {
      setLoading(null);
    }
  };

  const stopReview = async () => {
    if (!reviewerId || !runningSession) {
      return;
    }

    setLoading("stop");
    try {
      await stopSessionByProvider(runningSession.ai_provider, runningSession.session_record_id);
      const runtime = await refreshEmployeeRuntimeStatus(reviewerId);
      if (!runtime?.running) {
        await updateEmployeeStatus(reviewerId, "offline");
      }
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
      const message = error instanceof Error ? error.message : String(error);
      addCodexOutput(reviewerId, `[ERROR] ${message}`, task.id, "review");
      onError?.(message);
    } finally {
      setLoading(null);
    }
  };

  return {
    isRunning,
    output,
    loading,
    startReview,
    stopReview,
  };
}
