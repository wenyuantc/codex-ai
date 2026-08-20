import { describe, expect, it } from "vitest";

import { getBatchRunSkipReason } from "@/lib/batchRunSkip";

const task = { id: "t1", status: "todo", assignee_id: "e1" };

describe("getBatchRunSkipReason", () => {
  it("returns null when the task can run", () => {
    expect(
      getBatchRunSkipReason({
        task,
        queuedTaskIds: new Set(),
        runningTaskIds: new Set(),
        incompleteDependencyIds: new Set(),
      }),
    ).toBeNull();
  });

  it("explains missing assignee, archive, queue, running, and dependencies", () => {
    expect(
      getBatchRunSkipReason({
        task: { ...task, assignee_id: null },
        queuedTaskIds: new Set(),
        runningTaskIds: new Set(),
        incompleteDependencyIds: new Set(),
      }),
    ).toBe("no_assignee");
    expect(
      getBatchRunSkipReason({
        task: { ...task, status: "archived" },
        queuedTaskIds: new Set(),
        runningTaskIds: new Set(),
        incompleteDependencyIds: new Set(),
      }),
    ).toBe("archived");
    expect(
      getBatchRunSkipReason({
        task,
        queuedTaskIds: new Set(["t1"]),
        runningTaskIds: new Set(),
        incompleteDependencyIds: new Set(),
      }),
    ).toBe("queued");
    expect(
      getBatchRunSkipReason({
        task,
        queuedTaskIds: new Set(),
        runningTaskIds: new Set(["t1"]),
        incompleteDependencyIds: new Set(),
      }),
    ).toBe("running");
    expect(
      getBatchRunSkipReason({
        task,
        queuedTaskIds: new Set(),
        runningTaskIds: new Set(),
        incompleteDependencyIds: new Set(["t1"]),
      }),
    ).toBe("dependency");
  });
});
