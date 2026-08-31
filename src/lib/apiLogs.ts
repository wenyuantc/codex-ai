import type { NativeApiCallLogListItem, NativeApiCallLogStats } from "@/lib/backend";
import { formatTokenCount } from "@/lib/dashboardReport";

export const API_CALL_LOG_PAGE_SIZE = 20;

export type ApiCallLogStatus = "success" | "failed" | "cancelled";

export const API_CALL_LOG_STATUSES: ApiCallLogStatus[] = ["success", "failed", "cancelled"];

export function isApiCallLogStatus(value: string | null | undefined): value is ApiCallLogStatus {
  return value === "success" || value === "failed" || value === "cancelled";
}

export function formatApiCallLogTokenCount(
  value: number | null | undefined,
  unknownLabel: string,
): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return unknownLabel;
  }
  return formatTokenCount(value);
}

export function formatApiCallLogDurationMs(
  value: number | null | undefined,
  unknownLabel: string,
  lessThanOneSecondLabel = "<1s",
): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return unknownLabel;
  }

  const safeMs = Math.max(0, value);
  const seconds = Math.floor(safeMs / 1000);
  if (seconds < 1) {
    return lessThanOneSecondLabel;
  }
  return `${seconds}s`;
}

export function formatApiCallLogThinking(
  item: Pick<NativeApiCallLogListItem, "thinking_enabled" | "thinking_level">,
  unknownLabel: string,
  offLabel: string,
): string {
  if (!item.thinking_enabled) {
    return offLabel;
  }
  const level = item.thinking_level?.trim();
  return level ? level : unknownLabel;
}

export function isTruncatedFlag(value: number | null | undefined): boolean {
  return (value ?? 0) > 0;
}

export function prettyPrintJsonBody(value: string | null | undefined): string {
  const trimmed = value?.trim() ?? "";
  if (!trimmed) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2);
  } catch {
    return value ?? "";
  }
}

export function emptyApiCallLogStats(): NativeApiCallLogStats {
  return {
    call_count: 0,
    input_tokens_sum: null,
    output_tokens_sum: null,
    cached_tokens_sum: null,
    total_tokens_sum: null,
    avg_first_token_ms: null,
    avg_duration_ms: null,
  };
}

export function nextApiCallLogRequestPage(queryChanged: boolean, currentPage: number): number {
  if (queryChanged) {
    return 1;
  }
  return Number.isFinite(currentPage) && currentPage > 1 ? Math.floor(currentPage) : 1;
}

export function resolveApiCallLogListPage<T>(
  requestedPage: number,
  result: { items: T[]; total: number },
  pageSize: number = API_CALL_LOG_PAGE_SIZE,
): {
  page: number;
  items: T[];
  total: number;
  needsRefetch: boolean;
} {
  const total = Number.isFinite(result.total) ? Math.max(0, result.total) : 0;
  const totalPages = total > 0 ? Math.ceil(total / pageSize) : 0;
  const safePage = Number.isFinite(requestedPage) ? Math.floor(requestedPage) : 1;

  if (totalPages === 0) {
    return { page: 1, items: [], total: 0, needsRefetch: false };
  }

  if (safePage < 1) {
    return { page: 1, items: [], total, needsRefetch: true };
  }

  if (safePage > totalPages) {
    return { page: totalPages, items: [], total, needsRefetch: true };
  }

  return {
    page: safePage,
    items: result.items,
    total,
    needsRefetch: false,
  };
}
