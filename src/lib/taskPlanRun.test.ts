import { describe, expect, it } from "vitest";

import { hasSavedTaskPlan } from "@/lib/taskPlanRun";

describe("hasSavedTaskPlan", () => {
  it("treats missing or blank plan content as unsaved", () => {
    expect(hasSavedTaskPlan(null)).toBe(false);
    expect(hasSavedTaskPlan(undefined)).toBe(false);
    expect(hasSavedTaskPlan("")).toBe(false);
    expect(hasSavedTaskPlan("   \n\t")).toBe(false);
  });

  it("treats non-empty plan content as saved", () => {
    expect(hasSavedTaskPlan("目标与范围")).toBe(true);
    expect(hasSavedTaskPlan("  已有计划  ")).toBe(true);
  });
});
