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

/** Compact token count: 950 → "950", 12_340 → "12.34K", 1_000_000 → "1.00M". */
export function formatTokenCount(value: number): string {
  if (!Number.isFinite(value)) {
    return "0";
  }
  const abs = Math.abs(value);
  if (abs >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(2)}M`;
  }
  if (abs >= 1_000) {
    const kText = (value / 1_000).toFixed(2);
    if (Math.abs(Number(kText)) >= 1000) {
      return `${(value / 1_000_000).toFixed(2)}M`;
    }
    return `${kText}K`;
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

export type TokenUsageCountDisplay = { kind: "unknown" } | { kind: "value"; text: string };

export interface TokenUsageMetricSource {
  input_tokens?: number | null;
  output_tokens?: number | null;
  total_tokens?: number | null;
  cached_tokens?: number | null;
  sessions_with_usage?: number | null;
  sessions_with_cache?: number | null;
}

export interface TokenUsageMetrics {
  input: TokenUsageCountDisplay;
  output: TokenUsageCountDisplay;
  cached: TokenUsageCountDisplay;
  total: TokenUsageCountDisplay;
  cacheRate: CacheRateDisplay;
}

export function formatUsageMetricText(
  metric: TokenUsageCountDisplay | CacheRateDisplay,
  labels: { unknown: string; empty: string },
): string {
  if (metric.kind === "value" || metric.kind === "rate") {
    return metric.text;
  }
  if (metric.kind === "empty") {
    return labels.empty;
  }
  return labels.unknown;
}

function isKnownCount(value: number | null | undefined): value is number {
  return value != null && Number.isFinite(value);
}

function countDisplay(value: number | null | undefined, known: boolean): TokenUsageCountDisplay {
  if (!known || !isKnownCount(value)) {
    return { kind: "unknown" };
  }
  return { kind: "value", text: formatTokenCount(value) };
}

/**
 * Session or task-aggregate token metrics for UI.
 * Per-field unknown stays unknown; never coerce missing values to 0.
 */
export function buildTokenUsageMetrics(source: TokenUsageMetricSource): TokenUsageMetrics {
  const isAggregate = source.sessions_with_usage != null || source.sessions_with_cache != null;
  if (isAggregate) {
    const hasUsage = (source.sessions_with_usage ?? 0) > 0;
    const hasCache = (source.sessions_with_cache ?? 0) > 0;
    return {
      input: countDisplay(source.input_tokens ?? 0, hasUsage),
      output: countDisplay(source.output_tokens ?? 0, hasUsage),
      cached: countDisplay(source.cached_tokens ?? 0, hasCache),
      total: countDisplay(source.total_tokens ?? 0, hasUsage),
      cacheRate: resolveCacheRateDisplay({
        cached_tokens: source.cached_tokens ?? 0,
        input_tokens: source.input_tokens ?? 0,
        sessions_with_cache: source.sessions_with_cache ?? 0,
      }),
    };
  }

  const inputKnown = isKnownCount(source.input_tokens);
  const cachedKnown = isKnownCount(source.cached_tokens);
  return {
    input: countDisplay(source.input_tokens, inputKnown),
    output: countDisplay(source.output_tokens, isKnownCount(source.output_tokens)),
    cached: countDisplay(source.cached_tokens, cachedKnown),
    total: countDisplay(source.total_tokens, isKnownCount(source.total_tokens)),
    cacheRate:
      inputKnown && cachedKnown
        ? resolveCacheRateDisplay({
            cached_tokens: source.cached_tokens ?? 0,
            input_tokens: source.input_tokens ?? 0,
            sessions_with_cache: 1,
          })
        : { kind: "unknown" },
  };
}
