/**
 * Pure resolver for the single primary CTA on TaskCard / TaskDetailDialog.
 * Priority is fixed; first match wins. Components only render + bind handlers.
 */

export type TaskPrimaryCtaKind =
  | "stop"
  | "running_locked"
  | "starting"
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

function lockedExecutionLabel(
  automationStatus: string | null | undefined,
  pipelineActive: boolean | undefined,
): string {
  if (pipelineActive || (automationStatus && PIPELINE_STATUSES.has(automationStatus))) {
    return "编排中";
  }
  if (automationStatus && FIX_AUTOMATION_STATUSES.has(automationStatus)) {
    return "修复中…";
  }
  return "运行中";
}

/**
 * Resolve the unique primary CTA for a task.
 * Priority (first match):
 * stop → running_locked → starting → review → blocked → commit → acceptance → run → none
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
  } = input;

  const backgroundBusy = backgroundPlanning || backgroundStarting;

  // 1. Process running or mid-pipeline (caller sets canStopProcess for pipeline too) → stop
  if (canStopProcess) {
    return {
      kind: "stop",
      label: "停止",
      disabled: false,
      reason: pipelineActive ? "停止当前编排步骤并转人工" : "停止当前运行会话",
      tone: "danger",
    };
  }

  // Background start is checked before generic locked execution so labels match UX.
  // Design table lists starting after running_locked, but background is a distinct
  // busy mode that historically took precedence over the "运行中" lock label.
  if (backgroundBusy) {
    return {
      kind: "starting",
      label: backgroundPlanning ? "生成计划中" : "启动中",
      disabled: true,
      reason: backgroundPlanning ? "正在生成协调员/执行计划" : "正在后台启动会话",
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
          ? "编排流水线执行中"
          : "自动修复正在启动或运行中",
      tone: "muted",
    };
  }

  // 3. Background (if not folded into executionActive) — already handled above

  // 4. Review
  if (reviewActive || status === "review") {
    if (reviewActive) {
      return {
        kind: "review",
        label: "审核中",
        disabled: true,
        reason: hasReviewer ? "代码审核进行中" : "请先指定审查员",
        tone: "warning",
      };
    }
    if (sshReviewEvidenceLimited) {
      return {
        kind: "review",
        label: "审核",
        disabled: true,
        reason:
          "SSH 产物捕获受限，本地 diff/快照可能不完整；请在远程主机核对变更后再审核，或改用本地完整产物模式",
        tone: "warning",
      };
    }
    return {
      kind: "review",
      label: "审核",
      disabled: !hasReviewer,
      reason: hasReviewer ? "发起代码审核" : "请先指定审查员",
      tone: "warning",
    };
  }

  // 5. Blocked
  if (status === "blocked") {
    return {
      kind: "blocked",
      label: "查看阻塞原因",
      disabled: false,
      reason: "查看或编辑任务阻塞原因",
      tone: "warning",
    };
  }

  // 6. Completed + commit
  if (status === "completed" && canCommit) {
    return {
      kind: "commit",
      label: "提交代码",
      disabled: false,
      tone: "primary",
    };
  }

  // 7. Completed + acceptance
  if (status === "completed" && canGenerateAcceptance) {
    return {
      kind: "acceptance",
      label: "生成验收清单",
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
      reason: "任务已归档",
      tone: "muted",
    };
  }

  // 9. Default run (todo / in_progress / …)
  if (!hasAssignee) {
    return {
      kind: "run",
      label: "运行",
      disabled: true,
      reason: "请先指派员工",
      tone: "primary",
    };
  }

  if (hasIncompleteDependencies) {
    return {
      kind: "run",
      label: "运行",
      disabled: true,
      reason: incompleteDependencySummary
        ? `依赖未完成：${incompleteDependencySummary}`
        : "依赖任务尚未完成，无法运行",
      tone: "primary",
    };
  }

  if (assigneeBusyOnOtherTask) {
    return {
      kind: "run",
      label: "并行运行",
      disabled: false,
      reason: "同员工另有任务运行中，仍可并行启动本任务",
      tone: "primary",
    };
  }

  return {
    kind: "run",
    label: "运行",
    disabled: false,
    reason: "运行任务",
    tone: "primary",
  };
}
