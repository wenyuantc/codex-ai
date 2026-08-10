import type {
  ArtifactCaptureMode,
  Task,
  TaskAutomationState as PersistedTaskAutomationState,
} from "./types";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

import i18n from "@/lib/i18n";
import { getDateLocale, getLocalePreference } from "@/lib/i18n/locale";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function parseDateValue(dateStr: string): Date | null {
  const trimmed = dateStr.trim();
  if (!trimmed) {
    return null;
  }

  const normalized = trimmed.includes("T") ? trimmed : trimmed.replace(" ", "T");
  const withTimezone = /(?:Z|[+-]\d{2}:\d{2})$/i.test(normalized) ? normalized : `${normalized}Z`;
  const parsed = new Date(withTimezone);

  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

export function formatDate(dateStr: string): string {
  const parsed = parseDateValue(dateStr);
  return parsed ? parsed.toLocaleString(getDateLocale(getLocalePreference())) : dateStr;
}

/** YYYY-MM-DD prefix from a stored due_date (date-only or datetime). */
export function getDateOnly(dateStr: string | null | undefined): string | null {
  if (!dateStr) {
    return null;
  }
  const match = dateStr.trim().match(/^(\d{4}-\d{2}-\d{2})/);
  return match?.[1] ?? null;
}

export function todayDateOnly(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function isTaskOverdue(
  task: Pick<Task, "due_date" | "status">,
  today = todayDateOnly(),
): boolean {
  if (!task.due_date || task.status === "completed" || task.status === "archived") {
    return false;
  }
  const due = getDateOnly(task.due_date);
  return Boolean(due && due < today);
}

export function formatDuration(totalSeconds: number | null | undefined): string {
  const safeSeconds = Math.max(0, Math.floor(Number(totalSeconds ?? 0)));
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  const seconds = safeSeconds % 60;

  if (hours > 0) {
    return minutes > 0
      ? i18n.t("common:hoursMinutes", { hours, minutes })
      : i18n.t("common:hours", { count: hours });
  }
  if (minutes > 0) {
    return seconds > 0
      ? i18n.t("common:minutesSeconds", { minutes, seconds })
      : i18n.t("common:minutes", { count: minutes });
  }
  return i18n.t("common:seconds", { count: seconds });
}

export function getTaskElapsedSeconds(
  task: Pick<Task, "time_started_at" | "time_spent_seconds">,
  nowMs = Date.now(),
): number {
  const trackedSeconds = Math.max(0, Math.floor(Number(task.time_spent_seconds ?? 0)));
  const startedAt = task.time_started_at ? parseDateValue(task.time_started_at) : null;

  if (!startedAt) {
    return trackedSeconds;
  }

  return trackedSeconds + Math.max(0, Math.floor((nowMs - startedAt.getTime()) / 1000));
}

export function getStatusColor(status: string): string {
  const colors: Record<string, string> = {
    todo: "bg-slate-500",
    in_progress: "bg-blue-500",
    review: "bg-yellow-500",
    completed: "bg-green-500",
    blocked: "bg-red-500",
    online: "bg-green-500",
    busy: "bg-yellow-500",
    offline: "bg-gray-500",
    error: "bg-red-500",
    active: "bg-green-500",
    archived: "bg-gray-500",
  };
  return colors[status] || "bg-gray-500";
}

export function getStatusLabel(status: string): string {
  return i18n.t(`common:status.${status}`, { defaultValue: status });
}

export function getActivityActionLabel(action: string): string {
  if (!action) {
    return action;
  }
  return i18n.t(`activity:actions.${action}`, { defaultValue: action });
}

export function isArtifactCaptureLimited(mode: ArtifactCaptureMode): boolean {
  return mode === "ssh_git_status" || mode === "ssh_none";
}

export function getActivityDetailsLabel(
  action: string,
  details: string | null | undefined,
): string | null {
  if (!details) {
    return null;
  }

  if (action === "task_status_changed") {
    const separator = " -> ";
    const separatorIndex = details.lastIndexOf(separator);

    if (separatorIndex > 0) {
      const subject = details.slice(0, separatorIndex).trim();
      const nextStatus = details.slice(separatorIndex + separator.length).trim();

      if (subject && nextStatus) {
        return `${subject} -> ${getStatusLabel(nextStatus)}`;
      }
    }
  }

  return details;
}

export function getPriorityColor(priority: string): string {
  const colors: Record<string, string> = {
    low: "text-slate-500",
    medium: "text-blue-500",
    high: "text-orange-500",
    urgent: "text-red-500",
  };
  return colors[priority] || "text-gray-500";
}

export function getPriorityLabel(priority: string): string {
  return i18n.t(`common:priority.${priority}`, { defaultValue: priority });
}

export function getEmployeeRoleLabel(role: string): string {
  return i18n.t(`common:role.${role}`, { defaultValue: role });
}

export interface TaskAutomationDisplayState {
  enabled: boolean;
  status: string;
  updatedAt: string | null;
  note: string | null;
  source: "task" | "automation_state";
  roundCount: number | null;
}

export function getAcceptanceStatusLabel(status: string | null | undefined): string {
  if (!status) return i18n.t("common:acceptanceIdle");
  return i18n.t(`common:acceptance.${status}`, { defaultValue: status });
}

export function getAcceptanceStatusClassName(status: string | null | undefined): string {
  switch (status) {
    case "passed":
      return "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300";
    case "failed":
      return "bg-destructive/15 text-destructive";
    case "running":
      return "bg-amber-500/15 text-amber-700 dark:text-amber-300";
    case "skipped":
      return "bg-muted text-muted-foreground";
    default:
      return "bg-muted text-muted-foreground";
  }
}

export function getTaskAutomationStatusLabel(status: string): string {
  return i18n.t(`common:automation.${status}`, { defaultValue: status });
}

const ACTIVE_REVIEW_AUTOMATION_PHASES = new Set(["launching_review", "waiting_review"]);

const ACTIVE_EXECUTION_AUTOMATION_PHASES = new Set([
  "launching_fix",
  "waiting_execution",
  "committing_code",
]);

const ACTIVE_PIPELINE_PHASES = new Set([
  "pipeline_launching_step",
  "pipeline_waiting_step",
  "pipeline_manual_launching_step",
  "pipeline_manual_waiting_step",
]);

export function isTaskAutomationReviewActive(
  automationState?: Pick<TaskAutomationDisplayState, "enabled" | "status"> | null,
): boolean {
  return Boolean(
    automationState?.enabled && ACTIVE_REVIEW_AUTOMATION_PHASES.has(automationState.status),
  );
}

export function isTaskAutomationExecutionActive(
  automationState?: Pick<TaskAutomationDisplayState, "enabled" | "status"> | null,
): boolean {
  return Boolean(
    automationState?.enabled && ACTIVE_EXECUTION_AUTOMATION_PHASES.has(automationState.status),
  );
}

/** Coordinator pipeline is mid-flight (launching or waiting on a step). */
export function isTaskPipelineRunning(
  automationState?: Pick<PersistedTaskAutomationState, "pipeline_active" | "phase"> | null,
): boolean {
  if (!automationState?.pipeline_active) {
    return false;
  }
  return ACTIVE_PIPELINE_PHASES.has(automationState.phase);
}

export interface TaskActionRuntimeState {
  reviewActive: boolean;
  executionActive: boolean;
}

export function getTaskActionRuntimeState(params: {
  automationState?: Pick<TaskAutomationDisplayState, "enabled" | "status"> | null;
  isReviewRunning: boolean;
  isExecutionRunning: boolean;
  /** Raw automation row — used for pipeline_active even when auto-QC is off. */
  pipelineState?: Pick<PersistedTaskAutomationState, "pipeline_active" | "phase"> | null;
}): TaskActionRuntimeState {
  return {
    reviewActive: params.isReviewRunning || isTaskAutomationReviewActive(params.automationState),
    executionActive:
      params.isExecutionRunning ||
      isTaskAutomationExecutionActive(params.automationState) ||
      isTaskPipelineRunning(params.pipelineState),
  };
}

export function getTaskAutomationDisplayState(
  task: Task,
  automationState?: PersistedTaskAutomationState | null,
): TaskAutomationDisplayState {
  const enabled = task.automation_mode === "review_fix_loop_v1";

  if (!enabled) {
    return {
      enabled: false,
      status: "disabled",
      updatedAt: task.updated_at ?? null,
      note: null,
      source: "task",
      roundCount: null,
    };
  }

  if (!automationState) {
    return {
      enabled: true,
      status: "idle",
      updatedAt: task.updated_at ?? null,
      note: null,
      source: "task",
      roundCount: 0,
    };
  }

  return {
    enabled: true,
    status: automationState.phase,
    updatedAt: automationState.updated_at ?? task.updated_at ?? null,
    note: automationState.last_error ?? automationState.last_verdict?.summary ?? null,
    source: "automation_state",
    roundCount: automationState.round_count,
  };
}

export function timeAgo(dateStr: string | null): string {
  if (!dateStr) return i18n.t("common:unknown");
  const diff = Date.now() - new Date(dateStr + "Z").getTime();
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return i18n.t("common:justNow");
  if (minutes < 60) return i18n.t("common:minutesAgo", { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return i18n.t("common:hoursAgo", { count: hours });
  const days = Math.floor(hours / 24);
  return i18n.t("common:daysAgo", { count: days });
}
