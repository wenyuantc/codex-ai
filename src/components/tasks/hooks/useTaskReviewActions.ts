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
  const codexProcesses = useEmployeeStore((state) => state.codexProcesses);
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const setCodexRunning = useEmployeeStore((state) => state.setCodexRunning);
  const addCodexOutput = useEmployeeStore((state) => state.addCodexOutput);
  const clearTaskCodexOutput = useEmployeeStore((state) => state.clearTaskCodexOutput);
  const refreshCodexRuntimeStatus = useEmployeeStore((state) => state.refreshCodexRuntimeStatus);

  const isRunning = reviewerId
    ? (codexProcesses[reviewerId]?.running ?? false) && codexProcesses[reviewerId]?.activeTaskId === task.id
    : false;
  const output = taskLogs[buildTaskLogKey(task.id, "review")] ?? [];

  const startReview = async () => {
    if (status !== "review" || !reviewerId) {
      return;
    }

    setLoading(true);
    try {
      await updateEmployeeStatus(reviewerId, "busy");
      setCodexRunning(reviewerId, true, task.id);
      clearTaskCodexOutput(task.id, "review");
      await startTaskCodeReview(task.id);
      onStarted?.();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      addCodexOutput(reviewerId, `[ERROR] ${message}`, task.id, "review");
      setCodexRunning(reviewerId, false, null);
      await refreshCodexRuntimeStatus(reviewerId);
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
