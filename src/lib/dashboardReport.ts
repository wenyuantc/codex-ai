import type { DashboardTrendRange } from "@/lib/backend";

export const DASHBOARD_TREND_RANGE_OPTIONS: {
  value: DashboardTrendRange;
  label: string;
}[] = [
  { value: "7d", label: "近 7 日" },
  { value: "30d", label: "近 30 日" },
  { value: "8w", label: "近 8 周" },
];

export function normalizeTrendRange(value: string | null | undefined): DashboardTrendRange {
  if (value === "30d" || value === "8w") {
    return value;
  }
  return "7d";
}

export function trendRangeChartTitle(range: DashboardTrendRange): string {
  switch (range) {
    case "30d":
      return "近 30 日完成趋势";
    case "8w":
      return "近 8 周完成趋势";
    default:
      return "近 7 日完成趋势";
  }
}

/** Short axis label for weekly buckets (`2026-W31` → `W31`). */
export function shortTrendPointLabel(label: string, range: DashboardTrendRange): string {
  if (range === "8w" && label.includes("-W")) {
    return `W${label.split("-W")[1] ?? label}`;
  }
  return label;
}
