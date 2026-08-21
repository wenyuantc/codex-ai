import { buildTokenUsageMetrics, formatUsageMetricText } from "@/lib/dashboardReport";
import i18n from "@/lib/i18n";
import type { AiProvider, CodexSessionListItem, CodexSessionResumeStatus } from "@/lib/types";
import { AI_PROVIDER_OPTIONS } from "@/lib/types";

export type SessionsViewMode = "card" | "table";

export const SESSIONS_VIEW_MODE_STORAGE_KEY = "codex-ai:sessions-view-mode";

export function getStoredSessionsViewMode(): SessionsViewMode {
  if (typeof window === "undefined") {
    return "card";
  }

  return window.localStorage.getItem(SESSIONS_VIEW_MODE_STORAGE_KEY) === "table" ? "table" : "card";
}

export function storeSessionsViewMode(mode: SessionsViewMode) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(SESSIONS_VIEW_MODE_STORAGE_KEY, mode);
}

export function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").toLocaleLowerCase().trim();
}

export function matchesSessionIdentifier(session: CodexSessionListItem, query: string | null) {
  if (!query) {
    return false;
  }

  const normalizedQuery = normalizeSearchText(query);
  return [session.session_id, session.session_record_id, session.cli_session_id].some(
    (value) => normalizeSearchText(value) === normalizedQuery,
  );
}

export function formatSessionTokenUsage(session: {
  total_tokens?: number | null;
  input_tokens?: number | null;
  output_tokens?: number | null;
  cached_tokens?: number | null;
}): string {
  const metrics = buildTokenUsageMetrics(session);
  const labels = {
    unknown: i18n.t("sessions:tokenUnknown"),
    empty: i18n.t("sessions:tokenEmpty"),
  };
  const noneKnown =
    metrics.input.kind === "unknown" &&
    metrics.output.kind === "unknown" &&
    metrics.cached.kind === "unknown" &&
    metrics.total.kind === "unknown" &&
    metrics.cacheRate.kind === "unknown";
  if (noneKnown) {
    return labels.unknown;
  }
  return i18n.t("sessions:tokenBreakdown", {
    total: formatUsageMetricText(metrics.total, labels),
    input: formatUsageMetricText(metrics.input, labels),
    output: formatUsageMetricText(metrics.output, labels),
    cache: formatUsageMetricText(metrics.cached, labels),
    rate: formatUsageMetricText(metrics.cacheRate, labels),
  });
}

export function formatAiProviderLabel(provider: AiProvider) {
  const option = AI_PROVIDER_OPTIONS.find((o) => o.value === provider);
  return option?.label ?? provider;
}

export function aiProviderBadgeVariant(provider: AiProvider): "default" | "secondary" | "outline" {
  switch (provider) {
    case "codex":
      return "default";
    case "claude":
      return "secondary";
    case "opencode":
      return "outline";
    case "grok":
      return "outline";
    case "native":
      return "secondary";
    default:
      return "outline";
  }
}

export type SessionDisplayKind = "execution" | "review" | "pipeline";

export function sessionDisplayKind(session: {
  session_kind?: string | null;
  session_origin?: string | null;
}): SessionDisplayKind {
  if (session.session_kind === "review") {
    return "review";
  }
  if (session.session_origin === "pipeline") {
    return "pipeline";
  }
  return "execution";
}

export function formatSessionKind(session: {
  session_kind?: string | null;
  session_origin?: string | null;
}) {
  switch (sessionDisplayKind(session)) {
    case "review":
      return i18n.t("sessions:kindReview");
    case "pipeline":
      return i18n.t("sessions:kindPipeline");
    default:
      return i18n.t("sessions:kindExecution");
  }
}

export function sessionKindBadgeClassName(kind: SessionDisplayKind): string {
  switch (kind) {
    case "review":
      return "border-blue-500/30 bg-blue-500/10 text-blue-700";
    case "pipeline":
      return "border-violet-500/30 bg-violet-500/10 text-violet-700";
    default:
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
  }
}

export function formatSessionStatus(status: string) {
  switch (status) {
    case "pending":
      return i18n.t("sessions:statusPending");
    case "running":
      return i18n.t("sessions:statusRunning");
    case "stopping":
      return i18n.t("sessions:statusStopping");
    case "exited":
      return i18n.t("sessions:statusExited");
    case "failed":
      return i18n.t("sessions:statusFailed");
    default:
      return status;
  }
}

export function sessionStatusBadgeVariant(
  status: string,
): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "running":
      return "default";
    case "stopping":
    case "pending":
      return "secondary";
    case "failed":
      return "destructive";
    default:
      return "outline";
  }
}

export function formatResumeStatus(status: CodexSessionResumeStatus) {
  switch (status) {
    case "ready":
      return i18n.t("sessions:resumeReady");
    case "running":
      return i18n.t("sessions:resumeRunning");
    case "missing_employee":
      return i18n.t("sessions:resumeMissingEmployee");
    case "missing_cli_session":
      return i18n.t("sessions:resumeMissingCli");
    case "stopping":
      return i18n.t("sessions:resumeStopping");
    case "invalid":
      return i18n.t("sessions:resumeInvalid");
    default:
      return status;
  }
}
