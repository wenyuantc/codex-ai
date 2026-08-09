import type { TaskAutomationState, TaskPipelineStep } from "@/lib/types";

export function pipelineStatusLabel(status: string): string {
  switch (status) {
    case "pending":
      return "待执行";
    case "launching":
      return "启动中";
    case "running":
      return "执行中";
    case "succeeded":
      return "已完成";
    case "failed":
      return "失败";
    case "cancelled":
      return "已取消";
    case "skipped":
      return "已跳过";
    default:
      return status;
  }
}

export type PipelineOverallStatus =
  "empty" | "pending" | "running" | "failed" | "completed" | "cancelled";

export function getPipelineOverallStatus(
  steps: TaskPipelineStep[],
  automation?: Pick<TaskAutomationState, "pipeline_active" | "phase"> | null,
): PipelineOverallStatus {
  if (steps.length === 0) {
    return "empty";
  }

  if (
    steps.some((step) => step.status === "failed") ||
    automation?.phase === "pipeline_step_failed"
  ) {
    return "failed";
  }

  if (
    automation?.pipeline_active ||
    steps.some((step) => step.status === "launching" || step.status === "running")
  ) {
    return "running";
  }

  if (steps.every((step) => step.status === "succeeded" || step.status === "skipped")) {
    return "completed";
  }

  if (steps.some((step) => step.status === "cancelled")) {
    return "cancelled";
  }

  return "pending";
}

export function getPipelineOverallStatusLabel(status: PipelineOverallStatus): string {
  switch (status) {
    case "empty":
      return "无编排";
    case "pending":
      return "待开始";
    case "running":
      return "编排中";
    case "failed":
      return "编排失败";
    case "completed":
      return "已完成";
    case "cancelled":
      return "已转人工";
    default:
      return status;
  }
}

/** 1-based current step number for display. */
export function getPipelineCurrentStepNumber(
  steps: TaskPipelineStep[],
  automation?: Pick<TaskAutomationState, "pipeline_active" | "pipeline_step_index"> | null,
): number {
  if (automation?.pipeline_step_index != null && automation.pipeline_step_index >= 0) {
    return automation.pipeline_step_index + 1;
  }

  const active = steps.find((step) => step.status === "running" || step.status === "launching");
  if (active) {
    return active.step_index + 1;
  }

  const failed = steps.find((step) => step.status === "failed");
  if (failed) {
    return failed.step_index + 1;
  }

  const firstPending = steps.find((step) => step.status === "pending");
  if (firstPending) {
    return firstPending.step_index + 1;
  }

  return steps.length;
}

export function getPipelineProgressSummary(
  steps: TaskPipelineStep[],
  automation?: Pick<
    TaskAutomationState,
    "pipeline_active" | "pipeline_step_index" | "phase"
  > | null,
): string | null {
  if (steps.length === 0) {
    return null;
  }

  const overall = getPipelineOverallStatus(steps, automation);
  const current = getPipelineCurrentStepNumber(steps, automation);
  const total = steps.length;
  const label = getPipelineOverallStatusLabel(overall);

  if (overall === "completed") {
    return `编排进度 ${total}/${total} · ${label}`;
  }

  return `编排进度 ${current}/${total} · ${label}`;
}

/**
 * Compact kanban badge text from automation cursor only (no steps fetch).
 * Returns null when there is nothing worth showing.
 */
export function getPipelineKanbanBadgeLabel(
  automation?: Pick<
    TaskAutomationState,
    "pipeline_active" | "pipeline_step_index" | "phase"
  > | null,
): string | null {
  if (!automation) {
    return null;
  }

  if (automation.phase === "pipeline_step_failed") {
    const step =
      automation.pipeline_step_index != null && automation.pipeline_step_index >= 0
        ? automation.pipeline_step_index + 1
        : null;
    return step != null ? `编排失败 · 第 ${step} 步` : "编排失败";
  }

  if (automation.pipeline_active) {
    const step =
      automation.pipeline_step_index != null && automation.pipeline_step_index >= 0
        ? automation.pipeline_step_index + 1
        : null;
    return step != null ? `编排中 · 第 ${step} 步` : "编排中";
  }

  return null;
}

export function pipelineStepStatusDotClass(status: string): string {
  switch (status) {
    case "succeeded":
      return "bg-emerald-500";
    case "running":
    case "launching":
      return "bg-primary animate-pulse";
    case "failed":
      return "bg-destructive";
    case "cancelled":
      return "bg-amber-500";
    case "skipped":
      return "bg-muted-foreground/50";
    default:
      return "bg-muted-foreground/30";
  }
}

export function pipelineStepStatusTextClass(status: string): string {
  switch (status) {
    case "succeeded":
      return "text-emerald-700 dark:text-emerald-300";
    case "running":
    case "launching":
      return "text-primary";
    case "failed":
      return "text-destructive";
    case "cancelled":
      return "text-amber-800 dark:text-amber-200";
    default:
      return "text-muted-foreground";
  }
}

/** Steps that can be started with 手动运行 after 转人工. */
export function isPipelineStepManuallyRunnable(status: string): boolean {
  return status === "pending" || status === "failed" || status === "cancelled";
}

/** Whether orchestration is in 转人工 / manual_control mode. */
export function isPipelineManualControl(
  automation?: Pick<TaskAutomationState, "phase"> | null,
): boolean {
  return automation?.phase === "manual_control";
}

/**
 * Show per-step 手动运行 after 转人工, or when orchestration is no longer auto-running
 * but still has incomplete steps (failed/cancelled/pending).
 */
export function shouldShowPipelineManualRun(
  steps: TaskPipelineStep[],
  automation?: Pick<TaskAutomationState, "pipeline_active" | "phase"> | null,
): boolean {
  if (steps.length === 0) {
    return false;
  }

  const midFlight =
    Boolean(automation?.pipeline_active) &&
    (automation?.phase === "pipeline_launching_step" ||
      automation?.phase === "pipeline_waiting_step");
  if (midFlight) {
    return false;
  }

  if (automation?.phase === "manual_control") {
    return steps.some((step) => isPipelineStepManuallyRunnable(step.status));
  }

  // Also expose manual run when pipeline failed/cancelled and user has not started auto-run again.
  const overall = getPipelineOverallStatus(steps, automation);
  return (
    (overall === "failed" || overall === "cancelled" || overall === "pending") &&
    steps.some((step) => isPipelineStepManuallyRunnable(step.status)) &&
    !steps.every((step) => step.status === "pending")
  );
}

/** Whether the top-level 重试失败步骤 control should be available. */
export function shouldShowPipelineRetry(
  steps: TaskPipelineStep[],
  automation?: Pick<TaskAutomationState, "pipeline_active" | "phase"> | null,
): boolean {
  if (automation?.phase === "pipeline_step_failed") {
    return true;
  }
  return steps.some((step) => step.status === "failed");
}
