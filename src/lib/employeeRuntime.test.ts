import { describe, expect, it } from "vitest";

import {
  formatEmployeeRuntimeLabel,
  formatPlanUsageLogLine,
  normalizeReasoningEffortForProvider,
} from "./types";

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

describe("normalizeReasoningEffortForProvider", () => {
  it("keeps native catalog thinking levels including xhigh and max", () => {
    expect(normalizeReasoningEffortForProvider("native", "xhigh")).toBe("xhigh");
    expect(normalizeReasoningEffortForProvider("native", "max")).toBe("max");
    expect(normalizeReasoningEffortForProvider("native", "minimal")).toBe("minimal");
    expect(normalizeReasoningEffortForProvider("native", "none")).toBe("none");
    expect(normalizeReasoningEffortForProvider("native", "no_think")).toBe("no_think");
    expect(normalizeReasoningEffortForProvider("native", "high")).toBe("high");
  });

  it("falls native unknown effort back to high", () => {
    expect(normalizeReasoningEffortForProvider("native", "")).toBe("high");
    expect(normalizeReasoningEffortForProvider("native", "auto")).toBe("high");
    expect(normalizeReasoningEffortForProvider("native", null)).toBe("high");
  });

  it("still clamps grok to low/medium/high", () => {
    expect(normalizeReasoningEffortForProvider("grok", "xhigh")).toBe("high");
    expect(normalizeReasoningEffortForProvider("grok", "max")).toBe("high");
    expect(normalizeReasoningEffortForProvider("grok", "medium")).toBe("medium");
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
