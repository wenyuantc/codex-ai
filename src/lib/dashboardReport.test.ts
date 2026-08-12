import { describe, expect, it } from "vitest";

import {
  formatTokenCount,
  normalizeTrendRange,
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
