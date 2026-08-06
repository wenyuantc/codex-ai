import { describe, it, expect } from "vitest";

import { getKeywordMatchedActions, isInvalidDateRange } from "@/stores/dashboardStore";

describe("isInvalidDateRange", () => {
  it("rejects a range whose start is after its end", () => {
    expect(isInvalidDateRange({ startDate: "2026-08-07", endDate: "2026-08-06" })).toBe(true);
  });

  it("accepts a well-ordered range", () => {
    expect(isInvalidDateRange({ startDate: "2026-08-06", endDate: "2026-08-07" })).toBe(false);
  });

  it("accepts a single-day range because the end is taken as end-of-day", () => {
    expect(isInvalidDateRange({ startDate: "2026-08-06", endDate: "2026-08-06" })).toBe(false);
  });

  it("accepts a half-open range", () => {
    expect(isInvalidDateRange({ startDate: "2026-08-06" })).toBe(false);
    expect(isInvalidDateRange({ endDate: "2026-08-06" })).toBe(false);
    expect(isInvalidDateRange({})).toBe(false);
  });

  it("does not flag an unparseable date as an invalid range", () => {
    expect(isInvalidDateRange({ startDate: "not-a-date", endDate: "2026-08-06" })).toBe(false);
  });
});

describe("getKeywordMatchedActions", () => {
  const actions = ["task_created", "task_deleted", "ssh_config_created"];

  it("matches actions by their Chinese label, not the raw key", () => {
    // "ssh_config_created" is labelled 新增SSH配置, so it must NOT match 创建 —
    // matching against the raw key would wrongly include it.
    expect(getKeywordMatchedActions("创建", actions)).toEqual(["task_created"]);
  });

  it("expects an already lower-cased keyword because labels are normalized", () => {
    // Labels go through toLocaleLowerCase(); the store normalizes the keyword
    // before calling in, so an upper-case keyword finds nothing here.
    expect(getKeywordMatchedActions("ssh", actions)).toEqual(["ssh_config_created"]);
    expect(getKeywordMatchedActions("SSH", actions)).toEqual([]);
  });

  it("returns nothing when no label contains the keyword", () => {
    expect(getKeywordMatchedActions("不存在的关键词", actions)).toEqual([]);
  });

  it("matches every action on an empty keyword", () => {
    expect(getKeywordMatchedActions("", actions)).toEqual(actions);
  });
});
