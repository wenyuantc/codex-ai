import type { Task } from "@/lib/types";
import { isTaskOverdue } from "@/lib/utils";

/** taskId → tag ids for the task */
export type TaskTagMap = Map<string, string[]>;

export interface KanbanFilterState {
  keyword?: string;
  overdueOnly?: boolean;
  blockedOnly?: boolean;
  /** null / "all" / undefined = no filter */
  milestoneId?: string | null;
  /** null / "all" / undefined = no filter */
  tagId?: string | null;
  /** null / "all" / undefined = no filter */
  priority?: string | null;
  /** null / "all" / undefined = no filter; special "__unassigned__" for no assignee */
  assigneeId?: string | null;
}

const ALL = "all";
const UNASSIGNED = "__unassigned__";

function isActiveFilter(value: string | null | undefined): value is string {
  return Boolean(value && value !== ALL);
}

/**
 * Shared kanban filter used by KanbanPage (select-all) and KanbanBoard (columns).
 * Always excludes archived tasks. Overdue uses isTaskOverdue.
 */
export function filterKanbanTasks(
  tasks: Task[],
  filters: KanbanFilterState,
  taskTagMap?: TaskTagMap,
): Task[] {
  const normalized = (filters.keyword ?? "").trim().toLowerCase();
  const overdueOnly = Boolean(filters.overdueOnly);
  const blockedOnly = Boolean(filters.blockedOnly);
  const milestoneId = isActiveFilter(filters.milestoneId) ? filters.milestoneId : null;
  const tagId = isActiveFilter(filters.tagId) ? filters.tagId : null;
  const priority = isActiveFilter(filters.priority) ? filters.priority : null;
  const assigneeId = isActiveFilter(filters.assigneeId) ? filters.assigneeId : null;

  return tasks.filter((task) => {
    if (task.status === "archived") {
      return false;
    }

    if (overdueOnly && !isTaskOverdue(task)) {
      return false;
    }

    if (blockedOnly && task.status !== "blocked") {
      return false;
    }

    if (milestoneId && (task.milestone_id ?? "") !== milestoneId) {
      return false;
    }

    if (tagId) {
      const tagIds = taskTagMap?.get(task.id) ?? [];
      if (!tagIds.includes(tagId)) {
        return false;
      }
    }

    if (priority && task.priority !== priority) {
      return false;
    }

    if (assigneeId) {
      if (assigneeId === UNASSIGNED) {
        if (task.assignee_id) {
          return false;
        }
      } else if ((task.assignee_id ?? "") !== assigneeId) {
        return false;
      }
    }

    if (normalized) {
      const title = task.title.toLowerCase();
      const description = (task.description ?? "").toLowerCase();
      if (!title.includes(normalized) && !description.includes(normalized)) {
        return false;
      }
    }

    return true;
  });
}

export function filterKanbanTaskIds(
  tasks: Task[],
  filters: KanbanFilterState,
  taskTagMap?: TaskTagMap,
): string[] {
  return filterKanbanTasks(tasks, filters, taskTagMap).map((task) => task.id);
}
