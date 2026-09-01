import { describe, expect, it } from "vitest";

import { buildReviewFixCreatePayload, type ReviewFixSourceTask } from "@/lib/taskCreateAndRun";

function sourceTask(overrides: Partial<ReviewFixSourceTask> = {}): ReviewFixSourceTask {
  return {
    project_id: "proj-1",
    use_worktree: true,
    reviewer_id: null,
    coordinator_id: null,
    native_subagent_id: null,
    ...overrides,
  };
}

const baseInput = {
  title: "修复：原任务",
  description: "审核结果：…",
  priority: "high",
  assigneeId: "dev-1",
};

describe("buildReviewFixCreatePayload", () => {
  it("copies the source task reviewer when creating a fix task", () => {
    const payload = buildReviewFixCreatePayload({
      ...baseInput,
      sourceTask: sourceTask({ reviewer_id: "reviewer-1" }),
    });

    expect(payload.reviewer_id).toBe("reviewer-1");
    expect(payload.assignee_id).toBe("dev-1");
    expect(payload.project_id).toBe("proj-1");
    expect(payload.use_worktree).toBe(true);
  });

  it("prefers the sidebar reviewer over the saved source-task reviewer", () => {
    const payload = buildReviewFixCreatePayload({
      ...baseInput,
      sourceTask: sourceTask({ reviewer_id: "reviewer-saved" }),
      reviewerId: "reviewer-sidebar",
    });

    expect(payload.reviewer_id).toBe("reviewer-sidebar");
  });

  it("does not invent a reviewer when neither sidebar nor source task has one", () => {
    const payload = buildReviewFixCreatePayload({
      ...baseInput,
      sourceTask: sourceTask(),
      reviewerId: "  ",
    });

    expect(payload.reviewer_id).toBeUndefined();
    expect(payload.coordinator_id).toBeUndefined();
    expect(payload.native_subagent_id).toBeUndefined();
  });

  it("copies coordinator and native subagent with the same sidebar-first rule", () => {
    const payload = buildReviewFixCreatePayload({
      ...baseInput,
      sourceTask: sourceTask({
        reviewer_id: "reviewer-1",
        coordinator_id: "coord-saved",
        native_subagent_id: "sub-saved",
      }),
      coordinatorId: "coord-sidebar",
      nativeSubagentId: "",
    });

    expect(payload.coordinator_id).toBe("coord-sidebar");
    expect(payload.native_subagent_id).toBe("sub-saved");
  });
});
