import type { DashboardTrendRange } from "@/lib/backend";
import i18n from "@/lib/i18n";

export const DASHBOARD_TREND_RANGE_VALUES: DashboardTrendRange[] = ["7d", "30d", "8w"];

export function getDashboardTrendRangeOptions(): {
  value: DashboardTrendRange;
  label: string;
}[] {
  return [
    { value: "7d", label: i18n.t("dashboard:report.range7d") },
    { value: "30d", label: i18n.t("dashboard:report.range30d") },
    { value: "8w", label: i18n.t("dashboard:report.range8w") },
  ];
}

export function normalizeTrendRange(value: string | null | undefined): DashboardTrendRange {
  if (value === "30d" || value === "8w") {
    return value;
  }
  return "7d";
}

export function trendRangeChartTitle(range: DashboardTrendRange): string {
  switch (range) {
    case "30d":
      return i18n.t("dashboard:report.trend30d");
    case "8w":
      return i18n.t("dashboard:report.trend8w");
    default:
      return i18n.t("dashboard:report.trend7d");
  }
}

/** Short axis label for weekly buckets (`2026-W31` → `W31`). */
export function shortTrendPointLabel(label: string, range: DashboardTrendRange): string {
  if (range === "8w" && label.includes("-W")) {
    return `W${label.split("-W")[1] ?? label}`;
  }
  return label;
}

/** Compact token count for chart labels: 950 → "950", 12_340 → "12.3K", 4_560_000 → "4.6M". */
export function formatTokenCount(value: number): string {
  if (!Number.isFinite(value)) {
    return "0";
  }
  const abs = Math.abs(value);
  if (abs >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(abs >= 10_000_000 ? 0 : 1)}M`;
  }
  if (abs >= 1_000) {
    return `${(value / 1_000).toFixed(abs >= 10_000 ? 0 : 1)}K`;
  }
  return `${value}`;
}

export type CacheRateDisplay =
  { kind: "unknown" } | { kind: "empty" } | { kind: "rate"; text: string };

/** Cache hit rate from task/session aggregates. Unknown stays unknown; never fake 0%. */
export function resolveCacheRateDisplay(
  input: {
    cached_tokens: number;
    input_tokens: number;
    sessions_with_cache: number;
  } | null,
): CacheRateDisplay {
  if (!input || input.sessions_with_cache <= 0) {
    return { kind: "unknown" };
  }
  const cached = Math.max(0, input.cached_tokens);
  const prompt = Math.max(0, input.input_tokens);
  const denominator = cached > prompt ? prompt + cached : prompt;
  if (denominator <= 0) {
    return { kind: "empty" };
  }
  return { kind: "rate", text: `${((cached / denominator) * 100).toFixed(1)}%` };
}
