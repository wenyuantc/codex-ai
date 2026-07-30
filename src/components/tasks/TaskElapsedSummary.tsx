import { memo } from "react";
import { Clock } from "lucide-react";

import type { Task } from "@/lib/types";
import { formatDate, formatDuration, getTaskElapsedSeconds } from "@/lib/utils";
import { useSharedNow } from "@/hooks/useSharedNow";

type TaskElapsedFields = Pick<
  Task,
  | "status"
  | "time_started_at"
  | "time_spent_seconds"
  | "completed_at"
  | "updated_at"
  | "created_at"
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
  const running = Boolean(task.time_started_at);
  const nowMs = useSharedNow(running);
  const elapsedSeconds = getTaskElapsedSeconds(task, nowMs);
  const completedAtLabel = task.completed_at
    ? formatDate(task.completed_at)
    : formatDate(task.updated_at);

  const summary = task.status === "completed"
    ? `完成：${completedAtLabel} · 耗时：${formatDuration(elapsedSeconds)}`
    : task.time_started_at
      ? `计时中：${formatDuration(elapsedSeconds)}`
      : task.time_spent_seconds > 0
        ? `累计：${formatDuration(elapsedSeconds)}`
        : `创建：${formatDate(task.created_at)}`;

  return (
    <span className={className ?? "flex items-center gap-0.5"}>
      <Clock className="h-3 w-3" />
      {summary}
    </span>
  );
});
