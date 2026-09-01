import { describe, expect, it } from "vitest";

import {
  applyTodoSnapshotToKeys,
  extractLatestSessionTodos,
  parseSessionTodoItemLine,
  parseSessionTodoSnapshot,
} from "@/lib/sessionTodos";

describe("parseSessionTodoItemLine", () => {
  it("parses status, content and known priority", () => {
    expect(parseSessionTodoItemLine("- [in_progress] 定位 TestController (high)")).toEqual({
      content: "定位 TestController",
      status: "in_progress",
      priority: "high",
    });
  });

  it("keeps trailing parens that are not a priority", () => {
    expect(parseSessionTodoItemLine("- [pending] 修复 foo (legacy)")).toEqual({
      content: "修复 foo (legacy)",
      status: "pending",
    });
  });

  it("normalizes completed aliases", () => {
    expect(parseSessionTodoItemLine("- [done] 补测试 (low)")?.status).toBe("completed");
    expect(parseSessionTodoItemLine("- [complete] 补测试")?.status).toBe("completed");
  });
});

describe("extractLatestSessionTodos", () => {
  it("returns undefined when TodoWrite was never called", () => {
    expect(extractLatestSessionTodos(["[读取] src/lib/foo.ts", "[工具结果] ok"])).toBeUndefined();
  });

  it("parses a native multiline snapshot", () => {
    const snapshot = [
      "[待办]",
      "- [in_progress] 定位 TestController (high)",
      "- [pending] 实现 ok 接口 (medium)",
      "- [pending] 补测试 (low)",
    ].join("\n");
    expect(extractLatestSessionTodos([snapshot])).toEqual([
      { content: "定位 TestController", status: "in_progress", priority: "high" },
      { content: "实现 ok 接口", status: "pending", priority: "medium" },
      { content: "补测试", status: "pending", priority: "low" },
    ]);
  });

  it("keeps the latest snapshot after a later TodoWrite", () => {
    const first =
      "[待办]\n- [in_progress] 定位 TestController (high)\n- [pending] 实现 ok 接口 (medium)";
    const second =
      "[待办]\n- [completed] 定位 TestController (high)\n- [in_progress] 实现 ok 接口 (medium)";
    expect(
      extractLatestSessionTodos(["[读取] a.ts", first, "[工具结果] 已更新 2 项", second]),
    ).toEqual([
      { content: "定位 TestController", status: "completed", priority: "high" },
      { content: "实现 ok 接口", status: "in_progress", priority: "medium" },
    ]);
  });

  it("treats an empty snapshot as clearing the list", () => {
    const first = "[待办]\n- [pending] 补测试 (medium)";
    expect(extractLatestSessionTodos([first, "[待办] (空)"])).toEqual([]);
  });

  it("does not clear the list on a lone todo header", () => {
    const first = "[待办]\n- [pending] 补测试 (medium)";
    expect(extractLatestSessionTodos([first, "[待办]", "[读取] a.ts"])).toEqual([
      { content: "补测试", status: "pending", priority: "medium" },
    ]);
  });

  it("ignores non-list todo labels", () => {
    expect(
      extractLatestSessionTodos(["[待办] 读取任务清单", "[待办] 更新任务清单", "[读取] a.ts"]),
    ).toBeUndefined();
  });

  it("does not treat tool results as a todo snapshot", () => {
    expect(
      extractLatestSessionTodos([
        "[工具结果]\n- [completed] 定位 TestController (medium)\n- [pending] 实现 ok 接口 (medium)",
      ]),
    ).toBeUndefined();
  });

  it("ignores sub-agent prefixed snapshots", () => {
    expect(
      extractLatestSessionTodos([
        "[子 Agent 1(general) - 改文件] [待办]\n- [pending] 子任务 (medium)",
      ]),
    ).toBeUndefined();
  });

  it("parses a single native event as a snapshot", () => {
    expect(parseSessionTodoSnapshot("[待办] 读取任务清单")).toBeUndefined();
    expect(parseSessionTodoSnapshot("[待办] (空)")).toEqual([]);
    expect(parseSessionTodoSnapshot("[待办]\n- [completed] 定位 TestController (medium)")).toEqual([
      { content: "定位 TestController", status: "completed", priority: "medium" },
    ]);
  });

  it("parses a snapshot split across consecutive log lines", () => {
    expect(
      extractLatestSessionTodos([
        "[待办]",
        "- [completed] 定位 TestController (medium)",
        "- [in_progress] 实现 ok 接口 (medium)",
        "[读取] src/main.rs",
      ]),
    ).toEqual([
      { content: "定位 TestController", status: "completed", priority: "medium" },
      { content: "实现 ok 接口", status: "in_progress", priority: "medium" },
    ]);
  });
});

describe("applyTodoSnapshotToKeys", () => {
  it("writes the same snapshot to each cache key", () => {
    const snapshot = [{ content: "补测试", status: "pending" as const }];
    expect(applyTodoSnapshotToKeys({}, ["task:1:execution", "sess-1"], snapshot)).toEqual({
      "task:1:execution": snapshot,
      "sess-1": snapshot,
    });
  });

  it("clears cached todos when the snapshot is empty", () => {
    const current = {
      "sess-1": [{ content: "补测试", status: "pending" as const }],
    };
    expect(applyTodoSnapshotToKeys(current, ["sess-1"], [])).toEqual({ "sess-1": [] });
  });

  it("leaves the cache unchanged when the line is not a snapshot", () => {
    const current = {
      "sess-1": [{ content: "补测试", status: "pending" as const }],
    };
    expect(applyTodoSnapshotToKeys(current, ["sess-1"], undefined)).toBe(current);
  });
});
