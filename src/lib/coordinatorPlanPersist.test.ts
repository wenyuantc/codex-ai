import { afterEach, describe, expect, it } from "vitest";

import { generateAndPersistCoordinatorPlan } from "@/lib/coordinatorPlanPersist";
import {
  prepareCoordinatorPlanOpen,
  resetCoordinatorPlanInFlightForTests,
} from "@/lib/coordinatorPlanSession";
import type { CoordinatorPlanGenerateAdapters } from "@/lib/coordinatorPlanSession";
import type { Task } from "@/lib/types";
import { useCoordinatorPlanStore } from "@/stores/coordinatorPlanStore";

afterEach(() => {
  useCoordinatorPlanStore.getState().resetForTests();
  resetCoordinatorPlanInFlightForTests();
});

function task(partial: Partial<Task> & Pick<Task, "id" | "title">): Task {
  return {
    description: null,
    status: "todo",
    priority: "medium",
    project_id: "p1",
    assignee_id: "a1",
    reviewer_id: null,
    coordinator_id: "c1",
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
    native_subagent_id: null,
    last_acceptance_status: null,
    completed_at: null,
    deleted_at: null,
    created_at: "2026-09-01",
    updated_at: "2026-09-01",
    ...partial,
  };
}

function adapters(
  generatePlan: CoordinatorPlanGenerateAdapters["generatePlan"],
  extra?: Partial<Pick<CoordinatorPlanGenerateAdapters, "withLogStream">>,
): CoordinatorPlanGenerateAdapters {
  return {
    generatePlan,
    withLogStream: extra?.withLogStream
      ? extra.withLogStream
      : async <T>(onLine: (line: string) => void, run: (requestId: string) => Promise<T>) => {
          onLine("[工具] glob");
          return run("req-1");
        },
    refreshTasks: async () => {},
  };
}

describe("generateAndPersistCoordinatorPlan", () => {
  it("streams plan logs so opening the coordinator dialog during create-and-run is not empty", async () => {
    let release: (value: { markdown: string; usage_line: string | null }) => void = () => {};
    const pending = new Promise<{ markdown: string; usage_line: string | null }>((resolve) => {
      release = resolve;
    });
    const resultPromise = generateAndPersistCoordinatorPlan(
      {
        task: task({ id: "task-1", title: "任务" }),
        coordinatorId: "c1",
        workingDir: "/tmp",
        coordinatorName: "协调员",
        runtimeLabel: "内置 Agent / m / high",
      },
      adapters(async () => pending),
    );

    await Promise.resolve();
    await Promise.resolve();

    const mid = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(mid?.loading).toBe(true);
    expect(mid?.terminalVisible).toBe(true);
    expect(mid?.logs.some((line) => line.includes("[计划]"))).toBe(true);
    expect(mid?.logs.some((line) => line.includes("[工具] glob"))).toBe(true);

    const opened = prepareCoordinatorPlanOpen("task-1", "", false);
    expect(opened.shouldGenerate).toBe(false);
    const afterOpen = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(afterOpen?.logs).toEqual(mid?.logs);
    expect(afterOpen?.terminalVisible).toBe(true);

    release({ markdown: "# 计划", usage_line: null });
    await expect(resultPromise).resolves.toBe("# 计划");
    expect(useCoordinatorPlanStore.getState().byTaskId["task-1"]?.draft).toBe("# 计划");
  });

  it("keeps streamed logs when the dialog opens after background generate finishes", async () => {
    await generateAndPersistCoordinatorPlan(
      {
        task: task({ id: "task-1", title: "任务" }),
        coordinatorId: "c1",
        workingDir: null,
        coordinatorName: "协调员",
        runtimeLabel: "runtime",
      },
      adapters(async () => ({ markdown: "# 计划", usage_line: null })),
    );

    const before = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(before?.logs.some((line) => line.includes("[工具] glob"))).toBe(true);

    const reopened = prepareCoordinatorPlanOpen("task-1", "# 计划", false);
    expect(reopened.shouldGenerate).toBe(false);
    const after = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(after?.logs).toEqual(before?.logs);
    expect(after?.terminalVisible).toBe(true);
  });

  it("throws when the coordinator returns an empty plan", async () => {
    await expect(
      generateAndPersistCoordinatorPlan(
        {
          task: task({ id: "task-1", title: "任务" }),
          coordinatorId: "c1",
          workingDir: null,
        },
        adapters(async () => ({ markdown: "  ", usage_line: null })),
      ),
    ).rejects.toThrow("协调员未返回可用计划。");
  });
});
