import { describe, expect, it } from "vitest";

import {
  buildTokenUsageMetrics,
  formatTokenCount,
  formatUsageMetricText,
  normalizeTrendRange,
  resolveCacheRateDisplay,
  shortTrendPointLabel,
  trendRangeChartTitle,
} from "@/lib/dashboardReport";

describe("normalizeTrendRange", () => {
  it("accepts known presets and defaults unknown values to 7d", () => {
    expect(normalizeTrendRange("7d")).toBe("7d");
    expect(normalizeTrendRange("30d")).toBe("30d");
    expect(normalizeTrendRange("8w")).toBe("8w");
    expect(normalizeTrendRange(undefined)).toBe("7d");
    expect(normalizeTrendRange("weird")).toBe("7d");
  });
});

describe("trendRangeChartTitle", () => {
  it("returns Chinese titles for each preset", () => {
    expect(trendRangeChartTitle("7d")).toBe("近 7 日完成趋势");
    expect(trendRangeChartTitle("30d")).toBe("近 30 日完成趋势");
    expect(trendRangeChartTitle("8w")).toBe("近 8 周完成趋势");
  });
});

describe("shortTrendPointLabel", () => {
  it("shortens weekly labels and leaves daily labels intact", () => {
    expect(shortTrendPointLabel("2026-W31", "8w")).toBe("W31");
    expect(shortTrendPointLabel("08-10", "7d")).toBe("08-10");
  });
});

describe("formatTokenCount", () => {
  it("keeps small numbers and compacts thousands/millions", () => {
    expect(formatTokenCount(0)).toBe("0");
    expect(formatTokenCount(950)).toBe("950");
    expect(formatTokenCount(12_340)).toBe("12K");
    expect(formatTokenCount(1_250)).toBe("1.3K");
    expect(formatTokenCount(4_560_000)).toBe("4.6M");
    expect(formatTokenCount(12_000_000)).toBe("12M");
    expect(formatTokenCount(Number.NaN)).toBe("0");
  });
});

describe("resolveCacheRateDisplay", () => {
  it("keeps unknown when no session reported cache", () => {
    expect(resolveCacheRateDisplay(null)).toEqual({ kind: "unknown" });
    expect(
      resolveCacheRateDisplay({
        cached_tokens: 0,
        input_tokens: 100,
        sessions_with_cache: 0,
      }),
    ).toEqual({ kind: "unknown" });
  });

  it("formats zero and normal rates, and uses input+cached when cache exceeds input", () => {
    expect(
      resolveCacheRateDisplay({
        cached_tokens: 0,
        input_tokens: 100,
        sessions_with_cache: 1,
      }),
    ).toEqual({ kind: "rate", text: "0.0%" });
    expect(
      resolveCacheRateDisplay({
        cached_tokens: 25,
        input_tokens: 100,
        sessions_with_cache: 1,
      }),
    ).toEqual({ kind: "rate", text: "25.0%" });
    expect(
      resolveCacheRateDisplay({
        cached_tokens: 200,
        input_tokens: 50,
        sessions_with_cache: 1,
      }),
    ).toEqual({ kind: "rate", text: "80.0%" });
  });

  it("shows an empty placeholder when cache is known but input is zero", () => {
    expect(
      resolveCacheRateDisplay({
        cached_tokens: 0,
        input_tokens: 0,
        sessions_with_cache: 1,
      }),
    ).toEqual({ kind: "empty" });
  });
});

describe("buildTokenUsageMetrics", () => {
  it("keeps session fields unknown independently instead of pretending 0", () => {
    expect(buildTokenUsageMetrics({})).toEqual({
      input: { kind: "unknown" },
      output: { kind: "unknown" },
      cached: { kind: "unknown" },
      total: { kind: "unknown" },
      cacheRate: { kind: "unknown" },
    });
    expect(
      buildTokenUsageMetrics({
        total_tokens: 12_340,
        input_tokens: 8000,
        output_tokens: 4340,
      }),
    ).toEqual({
      input: { kind: "value", text: "8.0K" },
      output: { kind: "value", text: "4.3K" },
      cached: { kind: "unknown" },
      total: { kind: "value", text: "12K" },
      cacheRate: { kind: "unknown" },
    });
  });

  it("computes session cache rate only when both input and cache are known", () => {
    expect(
      buildTokenUsageMetrics({
        input_tokens: 100,
        cached_tokens: 25,
        output_tokens: 10,
        total_tokens: 110,
      }).cacheRate,
    ).toEqual({ kind: "rate", text: "25.0%" });
    expect(
      buildTokenUsageMetrics({
        cached_tokens: 25,
        total_tokens: 110,
      }).cacheRate,
    ).toEqual({ kind: "unknown" });
    expect(
      buildTokenUsageMetrics({
        input_tokens: 0,
        cached_tokens: 0,
      }).cacheRate,
    ).toEqual({ kind: "empty" });
  });

  it("uses aggregate flags so missing cache stays unknown even when usage exists", () => {
    expect(
      buildTokenUsageMetrics({
        input_tokens: 100,
        output_tokens: 20,
        total_tokens: 120,
        cached_tokens: 0,
        sessions_with_usage: 2,
        sessions_with_cache: 0,
      }),
    ).toEqual({
      input: { kind: "value", text: "100" },
      output: { kind: "value", text: "20" },
      cached: { kind: "unknown" },
      total: { kind: "value", text: "120" },
      cacheRate: { kind: "unknown" },
    });
    expect(
      buildTokenUsageMetrics({
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
        cached_tokens: 0,
        sessions_with_usage: 0,
        sessions_with_cache: 0,
      }),
    ).toEqual({
      input: { kind: "unknown" },
      output: { kind: "unknown" },
      cached: { kind: "unknown" },
      total: { kind: "unknown" },
      cacheRate: { kind: "unknown" },
    });
  });
});

describe("formatUsageMetricText", () => {
  const labels = { unknown: "未知", empty: "—" };

  it("renders known counts and rates, and keeps unknown/empty distinct", () => {
    expect(formatUsageMetricText({ kind: "value", text: "12K" }, labels)).toBe("12K");
    expect(formatUsageMetricText({ kind: "rate", text: "25.0%" }, labels)).toBe("25.0%");
    expect(formatUsageMetricText({ kind: "unknown" }, labels)).toBe("未知");
    expect(formatUsageMetricText({ kind: "empty" }, labels)).toBe("—");
  });
});
