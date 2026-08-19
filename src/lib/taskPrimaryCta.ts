/**
 * Pure resolver for the single primary CTA on TaskCard / TaskDetailDialog.
 * Priority is fixed; first match wins. Components only render + bind handlers.
 */

import i18n from "@/lib/i18n";

export type TaskPrimaryCtaKind =
  | "stop"
  | "running_locked"
  | "starting"
  | "queued"
  | "review"
  | "blocked"
  | "commit"
  | "acceptance"
  | "run"
  | "none";

export type TaskPrimaryCtaTone = "danger" | "primary" | "warning" | "muted";

export interface TaskPrimaryCta {
  kind: TaskPrimaryCtaKind;
  label: string;
  disabled: boolean;
  /** Tooltip / aria-description */
  reason?: string;
  tone: TaskPrimaryCtaTone;
}

export interface ResolveTaskPrimaryCtaInput {
  status: string;
  /** From getTaskActionRuntimeState — process, automation fix, or pipeline. */
  executionActive: boolean;
  /** From getTaskActionRuntimeState — review session or automation review. */
  reviewActive: boolean;
  /** Local process is actually running and can be stopped by the user. */
  canStopProcess: boolean;
  backgroundPlanning?: boolean;
  backgroundStarting?: boolean;
  hasAssignee: boolean;
  hasReviewer: boolean;
  canCommit: boolean;
  canGenerateAcceptance: boolean;
  /** Automation display status (phase), for locked labels. */
  automationStatus?: string | null;
  /** Raw pipeline_active — distinguishes orchestration labels. */
  pipelineActive?: boolean;
  /**
   * Assignee has another task's execution/review session running.
   * This task itself is idle — still runnable (multi-task), but copy must clarify.
   */
  assigneeBusyOnOtherTask?: boolean;
  /** Incomplete dependency tasks block run (and backend also rejects complete). */
  hasIncompleteDependencies?: boolean;
  incompleteDependencySummary?: string | null;
  /**
   * SSH session artifacts are limited — local-diff-based review is unreliable.
   * When true and status is review (idle), disable starting review from CTA.
   */
  sshReviewEvidenceLimited?: boolean;
  /** Task is waiting in the global run queue. */
  queued?: boolean;
}

const FIX_AUTOMATION_STATUSES = new Set([
  "launching_fix",
  "waiting_execution",
  "committing_code",
  "fix_started",
]);

const PIPELINE_STATUSES = new Set([
  "pipeline_launching_step",
  "pipeline_waiting_step",
  "pipeline_manual_launching_step",
  "pipeline_manual_waiting_step",
]);

function ct(key: string, options?: Record<string, unknown>): string {
  return i18n.t(`tasks:primaryCta.${key}`, options);
}

function lockedExecutionLabel(
  automationStatus: string | null | undefined,
  pipelineActive: boolean | undefined,
): string {
  if (pipelineActive || (automationStatus && PIPELINE_STATUSES.has(automationStatus))) {
    return ct("locked.orchestrating");
  }
  if (automationStatus && FIX_AUTOMATION_STATUSES.has(automationStatus)) {
    return ct("locked.fixing");
  }
  return ct("locked.running");
}

/**
 * Resolve the unique primary CTA for a task.
 * Priority (first match):
 * stop → running_locked → starting → queued → review → blocked → commit → acceptance → run → none
 *
 * Note: when a background plan/start is busy, it is preferred over generic
 * running_locked so labels stay accurate (matches TaskCard historical UX).
 */
export function resolveTaskPrimaryCta(input: ResolveTaskPrimaryCtaInput): TaskPrimaryCta {
  const {
    status,
    executionActive,
    reviewActive,
    canStopProcess,
    backgroundPlanning = false,
    backgroundStarting = false,
    hasAssignee,
    hasReviewer,
    canCommit,
    canGenerateAcceptance,
    automationStatus,
    pipelineActive,
    assigneeBusyOnOtherTask = false,
    hasIncompleteDependencies = false,
    incompleteDependencySummary = null,
    sshReviewEvidenceLimited = false,
    queued = false,
  } = input;

  const backgroundBusy = backgroundPlanning || backgroundStarting;

  // 1. Process running or mid-pipeline (caller sets canStopProcess for pipeline too) → stop
  if (canStopProcess) {
    return {
      kind: "stop",
      label: ct("stop.label"),
      disabled: false,
      reason: pipelineActive ? ct("stop.reasonPipeline") : ct("stop.reasonSession"),
      tone: "danger",
    };
  }

  // Background start is checked before generic locked execution so labels match UX.
  // Design table lists starting after running_locked, but background is a distinct
  // busy mode that historically took precedence over the "运行中" lock label.
  if (backgroundBusy) {
    return {
      kind: "starting",
      label: backgroundPlanning ? ct("starting.planningLabel") : ct("starting.startingLabel"),
      disabled: true,
      reason: backgroundPlanning ? ct("starting.planningReason") : ct("starting.startingReason"),
      tone: "muted",
    };
  }

  // 2. Execution occupied by automation / residual pipeline lock (not stoppable here)
  if (executionActive) {
    const label = lockedExecutionLabel(automationStatus, pipelineActive);
    return {
      kind: "running_locked",
      label,
      disabled: true,
      reason:
        pipelineActive || (automationStatus && PIPELINE_STATUSES.has(automationStatus))
          ? ct("locked.reasonPipeline")
          : ct("locked.reasonAutoFix"),
      tone: "muted",
    };
  }

  // 3. Background (if not folded into executionActive) — already handled above

  if (queued) {
    return {
      kind: "queued",
      label: ct("queued.label"),
      disabled: true,
      reason: ct("queued.reason"),
      tone: "muted",
    };
  }

  // 4. Review
  if (reviewActive || status === "review") {
    if (reviewActive) {
      return {
        kind: "review",
        label: ct("review.reviewingLabel"),
        disabled: true,
        reason: hasReviewer ? ct("review.reasonActive") : ct("review.reasonNeedReviewer"),
        tone: "warning",
      };
    }
    if (sshReviewEvidenceLimited) {
      return {
        kind: "review",
        label: ct("review.label"),
        disabled: true,
        reason: ct("review.reasonSshLimited"),
        tone: "warning",
      };
    }
    return {
      kind: "review",
      label: ct("review.label"),
      disabled: !hasReviewer,
      reason: hasReviewer ? ct("review.reasonStart") : ct("review.reasonNeedReviewer"),
      tone: "warning",
    };
  }

  // 5. Blocked
  if (status === "blocked") {
    return {
      kind: "blocked",
      label: ct("blocked.label"),
      disabled: false,
      reason: ct("blocked.reason"),
      tone: "warning",
    };
  }

  // 6. Completed + commit
  if (status === "completed" && canCommit) {
    return {
      kind: "commit",
      label: ct("commit.label"),
      disabled: false,
      tone: "primary",
    };
  }

  // 7. Completed + acceptance
  if (status === "completed" && canGenerateAcceptance) {
    return {
      kind: "acceptance",
      label: ct("acceptance.label"),
      disabled: false,
      tone: "primary",
    };
  }

  // 8. Archived
  if (status === "archived") {
    return {
      kind: "none",
      label: "",
      disabled: true,
      reason: ct("archived.reason"),
      tone: "muted",
    };
  }

  // 9. Default run (todo / in_progress / …)
  if (!hasAssignee) {
    return {
      kind: "run",
      label: ct("run.label"),
      disabled: true,
      reason: ct("run.reasonNeedAssignee"),
      tone: "primary",
    };
  }

  if (hasIncompleteDependencies) {
    return {
      kind: "run",
      label: ct("run.label"),
      disabled: true,
      reason: incompleteDependencySummary
        ? ct("run.reasonDepsIncompleteWithList", { summary: incompleteDependencySummary })
        : ct("run.reasonDepsIncomplete"),
      tone: "primary",
    };
  }

  if (assigneeBusyOnOtherTask) {
    return {
      kind: "run",
      label: ct("run.parallelLabel"),
      disabled: false,
      reason: ct("run.reasonParallel"),
      tone: "primary",
    };
  }

  return {
    kind: "run",
    label: ct("run.label"),
    disabled: false,
    reason: ct("run.reasonDefault"),
    tone: "primary",
  };
}
