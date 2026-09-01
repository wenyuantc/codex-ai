import { afterEach, describe, expect, it } from "vitest";

import {
  generateCoordinatorPlanForTask,
  prepareCoordinatorPlanOpen,
  resetCoordinatorPlanInFlightForTests,
  runExclusiveCoordinatorPlanGenerate,
  shouldAutoGenerateCoordinatorPlan,
  shouldHydrateCoordinatorPlanOnOpen,
} from "@/lib/coordinatorPlanSession";
import {
  EMPTY_COORDINATOR_PLAN_SESSION,
  useCoordinatorPlanStore,
} from "@/stores/coordinatorPlanStore";

afterEach(() => {
  useCoordinatorPlanStore.getState().resetForTests();
  resetCoordinatorPlanInFlightForTests();
});

describe("shouldAutoGenerateCoordinatorPlan", () => {
  it("only generates from Run when there is no saved plan and nothing in flight", () => {
    expect(
      shouldAutoGenerateCoordinatorPlan({
        autoGenerate: false,
        savedPlan: "",
        loading: false,
        inFlight: false,
      }),
    ).toBe(false);
    expect(
      shouldAutoGenerateCoordinatorPlan({
        autoGenerate: true,
        savedPlan: "已有计划",
        loading: false,
        inFlight: false,
      }),
    ).toBe(false);
    expect(
      shouldAutoGenerateCoordinatorPlan({
        autoGenerate: true,
        savedPlan: "",
        loading: true,
        inFlight: false,
      }),
    ).toBe(false);
    expect(
      shouldAutoGenerateCoordinatorPlan({
        autoGenerate: true,
        savedPlan: "  ",
        loading: false,
        inFlight: false,
      }),
    ).toBe(true);
  });
});

describe("shouldHydrateCoordinatorPlanOnOpen", () => {
  it("does not wipe an in-progress or already-hydrated session", () => {
    expect(shouldHydrateCoordinatorPlanOnOpen(undefined)).toBe(true);
    expect(shouldHydrateCoordinatorPlanOnOpen(EMPTY_COORDINATOR_PLAN_SESSION)).toBe(true);
    expect(
      shouldHydrateCoordinatorPlanOnOpen({
        ...EMPTY_COORDINATOR_PLAN_SESSION,
        loading: true,
      }),
    ).toBe(false);
    expect(
      shouldHydrateCoordinatorPlanOnOpen({
        ...EMPTY_COORDINATOR_PLAN_SESSION,
        draft: "进行中的草稿",
      }),
    ).toBe(false);
    expect(
      shouldHydrateCoordinatorPlanOnOpen({
        ...EMPTY_COORDINATOR_PLAN_SESSION,
        logs: ["[计划] still running"],
      }),
    ).toBe(false);
  });
});

describe("prepareCoordinatorPlanOpen", () => {
  it("does not request generation when opening the plan viewer", () => {
    const result = prepareCoordinatorPlanOpen("task-1", "", false);
    expect(result.shouldGenerate).toBe(false);
    expect(useCoordinatorPlanStore.getState().byTaskId["task-1"]?.loading).toBe(false);
  });

  it("keeps logs when reopening a loading session instead of hydrating", () => {
    useCoordinatorPlanStore.getState().patch("task-1", {
      loading: true,
      logs: ["[计划] 正在生成协调员执行计划，可能需要一点时间..."],
      terminalVisible: true,
    });
    const result = prepareCoordinatorPlanOpen("task-1", "", true);
    expect(result.shouldGenerate).toBe(false);
    const session = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(session?.loading).toBe(true);
    expect(session?.logs).toEqual(["[计划] 正在生成协调员执行计划，可能需要一点时间..."]);
  });

  it("auto-generates from Run only when no saved plan exists", () => {
    expect(prepareCoordinatorPlanOpen("task-1", "", true).shouldGenerate).toBe(true);
    useCoordinatorPlanStore.getState().resetForTests();
    expect(prepareCoordinatorPlanOpen("task-1", "已保存计划", true).shouldGenerate).toBe(false);
  });

  it("lets Run join an in-flight generate but does not start one for the viewer", async () => {
    let release: (value: string) => void = () => {};
    const pending = new Promise<string>((resolve) => {
      release = resolve;
    });
    const first = runExclusiveCoordinatorPlanGenerate("task-1", () => pending);
    expect(prepareCoordinatorPlanOpen("task-1", "", false).shouldGenerate).toBe(false);
    expect(prepareCoordinatorPlanOpen("task-1", "", true).shouldGenerate).toBe(true);
    expect(useCoordinatorPlanStore.getState().byTaskId["task-1"]?.loading).toBeFalsy();
    release("# 计划");
    expect(await first).toBe("# 计划");
  });
});

describe("generateCoordinatorPlanForTask", () => {
  it("does not start a second IPC while one is in flight", async () => {
    let calls = 0;
    let release: (value: { markdown: string; usage_line: string | null }) => void = () => {};
    const pending = new Promise<{ markdown: string; usage_line: string | null }>((resolve) => {
      release = resolve;
    });
    const adapters = {
      generatePlan: async () => {
        calls += 1;
        return pending;
      },
      withLogStream: async <T>(
        _onLine: (line: string) => void,
        run: (requestId: string) => Promise<T>,
      ) => run("req-1"),
      refreshTasks: async () => {},
    };
    const params = {
      taskId: "task-1",
      coordinatorId: "coord-1",
      coordinatorName: "协调员",
      runtimeLabel: "内置 Agent / m / high",
      title: "任务",
      description: null,
      status: "todo",
      priority: "medium",
      workingDir: "/tmp",
    };
    const first = generateCoordinatorPlanForTask(params, adapters);
    await Promise.resolve();
    const second = generateCoordinatorPlanForTask(params, adapters);
    expect(useCoordinatorPlanStore.getState().byTaskId["task-1"]?.loading).toBe(true);
    release({ markdown: "# 计划", usage_line: null });
    expect(await first).toBe("# 计划");
    expect(await second).toBe("# 计划");
    expect(calls).toBe(1);
    expect(useCoordinatorPlanStore.getState().byTaskId["task-1"]?.loading).toBe(false);
    expect(useCoordinatorPlanStore.getState().byTaskId["task-1"]?.draft).toBe("# 计划");
  });

  it("keeps logs after the caller closes the dialog (store is not reset)", async () => {
    const adapters = {
      generatePlan: async () => ({ markdown: "# 计划", usage_line: null }),
      withLogStream: async <T>(
        onLine: (line: string) => void,
        run: (requestId: string) => Promise<T>,
      ) => {
        onLine("[工具] glob");
        return run("req-1");
      },
      refreshTasks: async () => {},
    };
    await generateCoordinatorPlanForTask(
      {
        taskId: "task-1",
        coordinatorId: "coord-1",
        runtimeLabel: "runtime",
        title: "任务",
        status: "todo",
        priority: "medium",
        workingDir: null,
      },
      adapters,
    );
    const before = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(before?.draft).toBe("# 计划");
    expect(before?.logs.some((line) => line.includes("[工具] glob"))).toBe(true);
    const reopened = prepareCoordinatorPlanOpen("task-1", "", false);
    expect(reopened.shouldGenerate).toBe(false);
    const after = useCoordinatorPlanStore.getState().byTaskId["task-1"];
    expect(after?.draft).toBe("# 计划");
    expect(after?.logs).toEqual(before?.logs);
  });

  it("sends revision instruction and current markdown when revising", async () => {
    const captured: {
      revision_instruction?: string | null;
      current_markdown?: string | null;
    } = {};
    const adapters = {
      generatePlan: async (input: {
        revision_instruction?: string | null;
        current_markdown?: string | null;
      }) => {
        captured.revision_instruction = input.revision_instruction;
        captured.current_markdown = input.current_markdown;
        return { markdown: "# 新计划", usage_line: null };
      },
      withLogStream: async <T>(
        onLine: (line: string) => void,
        run: (requestId: string) => Promise<T>,
      ) => {
        onLine("[工具] grep");
        return run("req-2");
      },
      refreshTasks: async () => {},
    };
    const plan = await generateCoordinatorPlanForTask(
      {
        taskId: "task-1",
        coordinatorId: "coord-1",
        runtimeLabel: "runtime",
        title: "任务",
        status: "todo",
        priority: "medium",
        workingDir: null,
        revisionInstruction: " 拆第2步 ",
        currentMarkdown: "# 旧计划",
      },
      adapters,
    );
    expect(plan).toBe("# 新计划");
    expect(captured.revision_instruction).toBe("拆第2步");
    expect(captured.current_markdown).toBe("# 旧计划");
    const logs = useCoordinatorPlanStore.getState().byTaskId["task-1"]?.logs ?? [];
    expect(logs.some((line) => line.includes("按意见修改当前计划"))).toBe(true);
  });
});
