import { memo } from "react";
import { useTranslation } from "react-i18next";
import { Clock } from "lucide-react";

import type { Task } from "@/lib/types";
import { formatDate, formatDuration, getTaskElapsedSeconds } from "@/lib/utils";
import { useSharedNow } from "@/hooks/useSharedNow";

type TaskElapsedFields = Pick<
  Task,
  "status" | "time_started_at" | "time_spent_seconds" | "completed_at" | "updated_at" | "created_at"
>;

interface TaskElapsedSummaryProps {
  task: TaskElapsedFields;
  className?: string;
}

/**
 * Isolated leaf for live elapsed time so parent TaskCard does not re-render
 * every second when a task is running.
 */
export const TaskElapsedSummary = memo(function TaskElapsedSummary({
  task,
  className,
}: TaskElapsedSummaryProps) {
  const { t } = useTranslation("tasks");
  const running = Boolean(task.time_started_at);
  const nowMs = useSharedNow(running);
  const elapsedSeconds = getTaskElapsedSeconds(task, nowMs);
  const completedAtLabel = task.completed_at
    ? formatDate(task.completed_at)
    : formatDate(task.updated_at);

  const summary =
    task.status === "completed"
      ? t("elapsed.completed", {
          date: completedAtLabel,
          duration: formatDuration(elapsedSeconds),
        })
      : task.time_started_at
        ? t("elapsed.timing", { duration: formatDuration(elapsedSeconds) })
        : task.time_spent_seconds > 0
          ? t("elapsed.accumulated", { duration: formatDuration(elapsedSeconds) })
          : t("elapsed.created", { date: formatDate(task.created_at) });

  return (
    <span className={className ?? "flex items-center gap-0.5"}>
      <Clock className="h-3 w-3" />
      {summary}
    </span>
  );
});
