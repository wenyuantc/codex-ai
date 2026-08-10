import { describe, expect, it } from "vitest";

import { filterKanbanTasks, filterKanbanTaskIds } from "@/lib/kanbanFilters";
import type { Task } from "@/lib/types";

function task(partial: Partial<Task> & Pick<Task, "id" | "title" | "status">): Task {
  return {
    description: null,
    priority: "medium",
    project_id: "p1",
    assignee_id: null,
    reviewer_id: null,
    coordinator_id: null,
    complexity: null,
    ai_suggestion: null,
    plan_content: null,
    last_codex_session_id: null,
    last_review_session_id: null,
    due_date: null,
    blocked_reason: null,
    milestone_id: null,
    acceptance_checklist: null,
    use_worktree: false,
    automation_mode: null,
    time_started_at: null,
    time_spent_seconds: 0,
    mcp_server_ids: null,
    last_acceptance_status: null,
    completed_at: null,
    deleted_at: null,
    created_at: "2026-08-01",
    updated_at: "2026-08-01",
    ...partial,
  };
}

describe("filterKanbanTasks", () => {
  const tasks = [
    task({ id: "a", title: "Alpha", status: "todo", assignee_id: "e1", priority: "high" }),
    task({ id: "b", title: "Beta", status: "blocked", blocked_reason: "x" }),
    task({ id: "c", title: "Gamma", status: "archived" }),
    task({
      id: "d",
      title: "Delta",
      status: "in_progress",
      due_date: "2020-01-01",
      milestone_id: "m1",
    }),
  ];

  it("excludes archived tasks", () => {
    expect(filterKanbanTasks(tasks, {}).map((item) => item.id)).toEqual(["a", "b", "d"]);
  });

  it("filters blocked and overdue", () => {
    expect(filterKanbanTasks(tasks, { blockedOnly: true }).map((item) => item.id)).toEqual(["b"]);
    expect(filterKanbanTasks(tasks, { overdueOnly: true }).map((item) => item.id)).toEqual(["d"]);
  });

  it("filters by milestone, priority, assignee, and keyword", () => {
    expect(filterKanbanTasks(tasks, { milestoneId: "m1" }).map((item) => item.id)).toEqual(["d"]);
    expect(filterKanbanTasks(tasks, { priority: "high" }).map((item) => item.id)).toEqual(["a"]);
    expect(
      filterKanbanTasks(tasks, { assigneeId: "__unassigned__" }).map((item) => item.id),
    ).toEqual(["b", "d"]);
    expect(filterKanbanTasks(tasks, { keyword: "alp" }).map((item) => item.id)).toEqual(["a"]);
  });

  it("filters by tag map", () => {
    const tagMap = new Map<string, string[]>([
      ["a", ["t1"]],
      ["b", ["t2"]],
    ]);
    expect(filterKanbanTasks(tasks, { tagId: "t1" }, tagMap).map((item) => item.id)).toEqual(["a"]);
  });

  it("filterKanbanTaskIds mirrors task ids", () => {
    expect(filterKanbanTaskIds(tasks, { priority: "high" })).toEqual(["a"]);
  });
});
