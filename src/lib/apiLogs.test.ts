import { describe, expect, it } from "vitest";

import {
  API_CALL_LOG_PAGE_SIZE,
  emptyApiCallLogStats,
  formatApiCallLogDurationMs,
  formatApiCallLogThinking,
  formatApiCallLogTokenCount,
  isApiCallLogStatus,
  isTruncatedFlag,
  nextApiCallLogRequestPage,
  prettyPrintJsonBody,
  resolveApiCallLogListPage,
} from "@/lib/apiLogs";

describe("formatApiCallLogTokenCount", () => {
  it("shows the unknown label instead of 0 when tokens are missing", () => {
    expect(formatApiCallLogTokenCount(null, "未知")).toBe("未知");
    expect(formatApiCallLogTokenCount(undefined, "Unknown")).toBe("Unknown");
  });

  it("keeps an explicit zero", () => {
    expect(formatApiCallLogTokenCount(0, "未知")).toBe("0");
  });
});

describe("formatApiCallLogDurationMs", () => {
  it("renders whole seconds without rounding up", () => {
    expect(formatApiCallLogDurationMs(320, "未知")).toBe("<1s");
    expect(formatApiCallLogDurationMs(999, "未知")).toBe("<1s");
    expect(formatApiCallLogDurationMs(1000, "未知")).toBe("1s");
    expect(formatApiCallLogDurationMs(1200, "未知")).toBe("1s");
    expect(formatApiCallLogDurationMs(1999, "未知")).toBe("1s");
    expect(formatApiCallLogDurationMs(10000, "未知")).toBe("10s");
    expect(formatApiCallLogDurationMs(10500, "未知")).toBe("10s");
  });

  it("shows the unknown label when duration is missing", () => {
    expect(formatApiCallLogDurationMs(null, "未知")).toBe("未知");
    expect(formatApiCallLogDurationMs(Number.NaN, "未知")).toBe("未知");
  });

  it("clamps negative durations to <1s", () => {
    expect(formatApiCallLogDurationMs(-12, "未知")).toBe("<1s");
  });
});

describe("formatApiCallLogThinking", () => {
  it("uses the off label when thinking is disabled", () => {
    expect(
      formatApiCallLogThinking({ thinking_enabled: 0, thinking_level: "high" }, "未知", "关闭"),
    ).toBe("关闭");
  });

  it("falls back to unknown when enabled but the level is empty", () => {
    expect(
      formatApiCallLogThinking({ thinking_enabled: 1, thinking_level: null }, "未知", "关闭"),
    ).toBe("未知");
  });

  it("returns the stored thinking level when enabled", () => {
    expect(
      formatApiCallLogThinking({ thinking_enabled: 1, thinking_level: "high" }, "未知", "关闭"),
    ).toBe("high");
  });
});

describe("prettyPrintJsonBody", () => {
  it("pretty-prints valid JSON and leaves invalid text unchanged", () => {
    expect(prettyPrintJsonBody('{"a":1}')).toBe('{\n  "a": 1\n}');
    expect(prettyPrintJsonBody("not-json")).toBe("not-json");
    expect(prettyPrintJsonBody(null)).toBe("");
  });
});

describe("api log helpers", () => {
  it("recognizes known statuses and truncation flags", () => {
    expect(isApiCallLogStatus("success")).toBe(true);
    expect(isApiCallLogStatus("running")).toBe(false);
    expect(isTruncatedFlag(1)).toBe(true);
    expect(isTruncatedFlag(0)).toBe(false);
    expect(emptyApiCallLogStats().call_count).toBe(0);
  });
});

describe("nextApiCallLogRequestPage", () => {
  it("forces page 1 when filters or scope change, even from later pages", () => {
    expect(nextApiCallLogRequestPage(true, 4)).toBe(1);
    expect(nextApiCallLogRequestPage(true, 1)).toBe(1);
  });

  it("keeps the current page when only paginating", () => {
    expect(nextApiCallLogRequestPage(false, 3)).toBe(3);
    expect(nextApiCallLogRequestPage(false, 0)).toBe(1);
  });
});

describe("resolveApiCallLogListPage", () => {
  it("keeps a valid page and its items", () => {
    const items = [{ id: "p2" }];
    expect(resolveApiCallLogListPage(2, { items, total: API_CALL_LOG_PAGE_SIZE * 2 + 1 })).toEqual({
      page: 2,
      items,
      total: API_CALL_LOG_PAGE_SIZE * 2 + 1,
      needsRefetch: false,
    });
  });

  it("clears items immediately when the query has no rows", () => {
    expect(resolveApiCallLogListPage(3, { items: [{ id: "stale" }], total: 0 })).toEqual({
      page: 1,
      items: [],
      total: 0,
      needsRefetch: false,
    });
  });

  it("clears stale rows and signals a refetch when the page is past the last page", () => {
    expect(
      resolveApiCallLogListPage(5, {
        items: [{ id: "stale-page" }],
        total: API_CALL_LOG_PAGE_SIZE + 2,
      }),
    ).toEqual({
      page: 2,
      items: [],
      total: API_CALL_LOG_PAGE_SIZE + 2,
      needsRefetch: true,
    });
  });
});
