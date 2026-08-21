import { formatTokenCount } from "@/lib/dashboardReport";
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
}): string {
  if (session.total_tokens == null || !Number.isFinite(session.total_tokens)) {
    return i18n.t("sessions:tokenUnknown");
  }
  const total = formatTokenCount(session.total_tokens);
  if (
    session.input_tokens != null &&
    session.output_tokens != null &&
    Number.isFinite(session.input_tokens) &&
    Number.isFinite(session.output_tokens)
  ) {
    return i18n.t("sessions:tokenBreakdown", {
      total,
      input: formatTokenCount(session.input_tokens),
      output: formatTokenCount(session.output_tokens),
    });
  }
  return i18n.t("sessions:tokenTotal", { total });
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

export function formatSessionKind(kind: CodexSessionListItem["session_kind"]) {
  return kind === "review" ? i18n.t("sessions:kindReview") : i18n.t("sessions:kindExecution");
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
