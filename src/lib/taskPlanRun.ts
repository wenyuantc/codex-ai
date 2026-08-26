export function hasSavedTaskPlan(planContent?: string | null): boolean {
  return Boolean(planContent?.trim());
}

export type TaskPlanRunChoice = "continue_existing" | "regenerate";
