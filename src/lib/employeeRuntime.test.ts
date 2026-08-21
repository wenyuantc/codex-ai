import { describe, expect, it } from "vitest";

import { formatEmployeeRuntimeLabel, formatPlanUsageLogLine } from "./types";

describe("formatEmployeeRuntimeLabel", () => {
  it("uses Chinese provider labels and employee model/effort", () => {
    expect(
      formatEmployeeRuntimeLabel({
        ai_provider: "claude",
        model: "opus[1m]",
        reasoning_effort: "high",
      }),
    ).toBe("Claude / opus[1m] / high");
    expect(
      formatEmployeeRuntimeLabel({
        ai_provider: "native",
        model: "gpt-5.6-luna",
        reasoning_effort: "high",
      }),
    ).toBe("内置 Agent / gpt-5.6-luna / high");
  });

  it("falls back when employee or fields are missing", () => {
    expect(formatEmployeeRuntimeLabel(null)).toBe("Codex / 默认模型 / 默认推理等级");
    expect(
      formatEmployeeRuntimeLabel({
        ai_provider: "native",
        model: "  ",
        reasoning_effort: "",
      }),
    ).toBe("内置 Agent / 默认模型 / 默认推理等级");
  });
});

describe("formatPlanUsageLogLine", () => {
  it("rewrites the usage terminal line for the plan log", () => {
    expect(formatPlanUsageLogLine("[用量] in=10 out=4 total=14")).toBe(
      "[计划] 用量：in=10 out=4 total=14",
    );
    expect(formatPlanUsageLogLine("  ")).toBeNull();
    expect(formatPlanUsageLogLine(null)).toBeNull();
  });
});
