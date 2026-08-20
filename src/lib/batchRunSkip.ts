export type BatchRunSkipReason =
  "no_assignee" | "archived" | "queued" | "running" | "dependency" | "start_failed";

export function getBatchRunSkipReason(input: {
  task: { id: string; status: string; assignee_id?: string | null };
  queuedTaskIds: Set<string>;
  runningTaskIds: Set<string>;
  incompleteDependencyIds: Set<string>;
}): BatchRunSkipReason | null {
  if (!input.task.assignee_id) {
    return "no_assignee";
  }
  if (input.task.status === "archived") {
    return "archived";
  }
  if (input.queuedTaskIds.has(input.task.id)) {
    return "queued";
  }
  if (input.runningTaskIds.has(input.task.id)) {
    return "running";
  }
  if (input.incompleteDependencyIds.has(input.task.id)) {
    return "dependency";
  }
  return null;
}
