import { useState } from "react";

import { startTaskCodeReview } from "@/lib/backend";
import type { Task } from "@/lib/types";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";

interface UseTaskReviewActionsOptions {
  task: Task;
  reviewerId?: string | null;
  status: string;
  onStarted?: () => void;
  onError?: (message: string) => void;
}

export function useTaskReviewActions({
  task,
  reviewerId,
  status,
  onStarted,
  onError,
}: UseTaskReviewActionsOptions) {
  const [loading, setLoading] = useState(false);
  const employeeRuntime = useEmployeeStore((state) => (
    reviewerId ? state.employeeRuntime[reviewerId] : undefined
  ));
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const addCodexOutput = useEmployeeStore((state) => state.addCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((state) => state.clearTaskCodexOutput);
  const refreshEmployeeRuntimeStatus = useEmployeeStore((state) => state.refreshEmployeeRuntimeStatus);

  const isRunning = Boolean(employeeRuntime?.sessions.find((session) => (
    session.task_id === task.id && session.session_kind === "review"
  )));
  const output = taskLogs[buildTaskLogKey(task.id, "review")] ?? [];

  const startReview = async () => {
    if (status !== "review" || !reviewerId) {
      return;
    }

    setLoading(true);
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
      setLoading(false);
    }
  };

  return {
    isRunning,
    output,
    loading,
    startReview,
  };
}
