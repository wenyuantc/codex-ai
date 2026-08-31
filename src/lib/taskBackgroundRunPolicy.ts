import { hasSavedTaskPlan } from "@/lib/taskPlanRun";
import type { TaskPrimaryCta } from "@/lib/taskPrimaryCta";

/**
 * Whether the kanban card should offer "Run in background".
 * Aligns with the primary Run CTA: only when the task can actually start.
 */
export function canOfferTaskBackgroundRun(input: {
  primaryCtaKind: TaskPrimaryCta["kind"];
  hideRunAction?: boolean;
}): boolean {
  if (input.hideRunAction) {
    return false;
  }
  return input.primaryCtaKind === "run";
}

export function isTaskBackgroundRunDisabled(input: {
  canOffer: boolean;
  primaryCtaDisabled: boolean;
  isActionLoading: boolean;
  isRunning: boolean;
  isReviewRunning: boolean;
}): boolean {
  return (
    !input.canOffer ||
    input.primaryCtaDisabled ||
    input.isActionLoading ||
    input.isRunning ||
    input.isReviewRunning
  );
}

/**
 * Coordinator tasks reuse a saved plan when present; otherwise generate one
 * silently (same as create-and-run). Empty task descriptions are allowed —
 * the existing prompt builder already falls back to title/subtasks/plan.
 */
export function shouldGenerateCoordinatorPlanForBackgroundRun(input: {
  coordinatorId?: string | null;
  savedPlan?: string | null;
}): boolean {
  return Boolean(input.coordinatorId) && !hasSavedTaskPlan(input.savedPlan);
}

export function resolveTaskBackgroundRunInitialPhase(input: {
  coordinatorId?: string | null;
  savedPlan?: string | null;
}): "planning" | "starting" {
  return shouldGenerateCoordinatorPlanForBackgroundRun(input) ? "planning" : "starting";
}
