import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  LayoutGrid,
  Loader2,
  MoreHorizontal,
  RefreshCw,
  SlidersHorizontal,
  Table,
} from "lucide-react";
import { useLocation, useSearchParams } from "react-router-dom";

import { SessionCard } from "@/components/sessions/SessionCard";
import { SessionContinueDialog } from "@/components/sessions/SessionContinueDialog";
import { SessionExecutionChangesDialog } from "@/components/sessions/SessionExecutionChangesDialog";
import { SessionLogDialog, type SessionLogTarget } from "@/components/sessions/SessionLogDialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { listCodexSessions, prepareCodexSessionResume } from "@/lib/backend";
import { startByProvider, stopSessionByProvider } from "@/lib/aiEngine";
import {
  aiProviderBadgeVariant,
  formatAiProviderLabel,
  formatResumeStatus,
  formatSessionKind,
  formatSessionStatus,
  formatSessionTokenUsage,
  getStoredSessionsViewMode,
  matchesSessionIdentifier,
  normalizeSearchText,
  sessionStatusBadgeVariant,
  storeSessionsViewMode,
  type SessionsViewMode,
} from "@/lib/sessions";
import type { CodexSessionListItem } from "@/lib/types";
import { AI_PROVIDER_OPTIONS, normalizeAiProvider } from "@/lib/types";
import i18n from "@/lib/i18n";
import { cn, formatDate, isArtifactCaptureLimited } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { SshArtifactLimitedNotice } from "@/components/sessions/SshArtifactLimitedNotice";

const PAGE_SIZE = 10;

function buildLogTarget(session: {
  session_record_id?: string | null;
  session_id?: string | null;
  resolved_session_id?: string | null;
  display_name?: string | null;
  employee_id?: string | null;
  employee_name?: string | null;
  task_id?: string | null;
  task_title?: string | null;
  session_kind?: CodexSessionListItem["session_kind"] | null;
  ai_provider?: string | null;
}): SessionLogTarget {
  return {
    sessionRecordId: session.session_record_id ?? null,
    sessionId:
      session.resolved_session_id ?? session.session_id ?? i18n.t("sessions:unknownSession"),
    displayName: session.display_name ?? i18n.t("sessions:unnamedSession"),
    employeeId: session.employee_id ?? null,
    employeeName: session.employee_name ?? null,
    taskId: session.task_id ?? null,
    taskTitle: session.task_title ?? null,
    sessionKind: session.session_kind ?? null,
    aiProvider: session.ai_provider ?? null,
  };
}

export function SessionsPage() {
  const { t } = useTranslation(["sessions", "common"]);
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const employees = useEmployeeStore((state) => state.employees);
  const fetchEmployees = useEmployeeStore((state) => state.fetchEmployees);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const refreshEmployeeRuntimeStatus = useEmployeeStore(
    (state) => state.refreshEmployeeRuntimeStatus,
  );

  const environmentMode = useProjectStore((state) => state.environmentMode);
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const currentProjectName = useProjectStore((state) => state.currentProject?.name);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const [sessions, setSessions] = useState<CodexSessionListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [page, setPage] = useState(1);
  const [continueDialogOpen, setContinueDialogOpen] = useState(false);
  const [continueSession, setContinueSession] = useState<CodexSessionListItem | null>(null);
  const [continueSubmitting, setContinueSubmitting] = useState(false);
  const [stoppingSessionId, setStoppingSessionId] = useState<string | null>(null);
  const [logDialogOpen, setLogDialogOpen] = useState(false);
  const [logTarget, setLogTarget] = useState<SessionLogTarget | null>(null);
  const [changeDialogOpen, setChangeDialogOpen] = useState(false);
  const [changeTarget, setChangeTarget] = useState<CodexSessionListItem | null>(null);
  const [activeSession, setActiveSession] = useState<SessionLogTarget | null>(null);
  const [sessionIdQuery, setSessionIdQuery] = useState("");
  const [taskIdQuery, setTaskIdQuery] = useState("");
  const [contentQuery, setContentQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [kindFilter, setKindFilter] = useState<string>("all");
  const [providerFilter, setProviderFilter] = useState<string>("all");
  const [employeeFilter, setEmployeeFilter] = useState<string>("all");
  const [failedOnly, setFailedOnly] = useState(false);
  const [moreFiltersOpen, setMoreFiltersOpen] = useState(false);
  const moreFiltersButtonRef = useRef<HTMLButtonElement>(null);
  const wasMoreFiltersOpen = useRef(false);
  const [viewMode, setViewMode] = useState<SessionsViewMode>(() => getStoredSessionsViewMode());
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [infoMessage, setInfoMessage] = useState<string | null>(null);
  const highlightedSessionId = searchParams.get("sessionId");
  const highlightedSessionNonce =
    (location.state as { globalSearchNonce?: number } | null)?.globalSearchNonce ?? null;

  useHotkeys("r", (e) => {
    e.preventDefault();
    if (!loading && !refreshing) {
      void loadSessions(true);
    }
  });

  const filteredSessions = useMemo(() => {
    const normalizedSessionIdQuery = normalizeSearchText(sessionIdQuery);
    const normalizedTaskIdQuery = normalizeSearchText(taskIdQuery);
    const normalizedContentQuery = normalizeSearchText(contentQuery);

    return sessions.filter((session) => {
      if (currentProjectId && session.project_id !== currentProjectId) {
        return false;
      }

      if (environmentMode === "ssh" && session.execution_target !== "ssh") {
        return false;
      }

      if (environmentMode === "local" && session.execution_target === "ssh") {
        return false;
      }

      if (
        environmentMode === "ssh" &&
        selectedSshConfigId &&
        session.ssh_config_id !== selectedSshConfigId
      ) {
        return false;
      }

      if (statusFilter !== "all" && session.status !== statusFilter) {
        return false;
      }

      if (kindFilter !== "all" && session.session_kind !== kindFilter) {
        return false;
      }

      if (providerFilter !== "all" && normalizeAiProvider(session.ai_provider) !== providerFilter) {
        return false;
      }

      if (employeeFilter !== "all" && session.employee_id !== employeeFilter) {
        return false;
      }

      if (failedOnly && session.status !== "failed") {
        return false;
      }

      const matchesSessionId =
        !normalizedSessionIdQuery ||
        normalizeSearchText(session.session_id).includes(normalizedSessionIdQuery) ||
        normalizeSearchText(session.session_record_id).includes(normalizedSessionIdQuery) ||
        normalizeSearchText(session.cli_session_id).includes(normalizedSessionIdQuery);

      if (!matchesSessionId) {
        return false;
      }

      const matchesTaskId =
        !normalizedTaskIdQuery ||
        normalizeSearchText(session.task_id).includes(normalizedTaskIdQuery);

      if (!matchesTaskId) {
        return false;
      }

      if (!normalizedContentQuery) {
        return true;
      }

      const contentHaystack = [
        session.display_name,
        session.summary,
        session.content_preview,
        session.task_title,
        session.project_name,
        session.employee_name,
        session.working_dir,
      ]
        .map((value) => normalizeSearchText(value))
        .join("\n");

      return contentHaystack.includes(normalizedContentQuery);
    });
  }, [
    contentQuery,
    currentProjectId,
    employeeFilter,
    environmentMode,
    failedOnly,
    kindFilter,
    providerFilter,
    selectedSshConfigId,
    sessionIdQuery,
    sessions,
    statusFilter,
    taskIdQuery,
  ]);

  const totalPages =
    filteredSessions.length > 0 ? Math.ceil(filteredSessions.length / PAGE_SIZE) : 0;
  const pageSessions = useMemo(
    () => filteredSessions.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE),
    [filteredSessions, page],
  );
  const rangeStart = filteredSessions.length === 0 ? 0 : (page - 1) * PAGE_SIZE + 1;
  const rangeEnd =
    filteredSessions.length === 0 ? 0 : Math.min(page * PAGE_SIZE, filteredSessions.length);

  const activeMoreFilterCount = [
    normalizeSearchText(sessionIdQuery),
    normalizeSearchText(taskIdQuery),
    kindFilter !== "all",
    providerFilter !== "all",
    employeeFilter !== "all",
    failedOnly,
  ].filter(Boolean).length;

  useEffect(() => {
    setPage(1);
  }, [contentQuery, currentProjectId, sessionIdQuery, taskIdQuery]);

  useEffect(() => {
    setContinueDialogOpen(false);
    setContinueSession(null);
    setContinueSubmitting(false);
    setStoppingSessionId(null);
    setLogDialogOpen(false);
    setLogTarget(null);
    setChangeDialogOpen(false);
    setChangeTarget(null);
    setActiveSession(null);
    setErrorMessage(null);
    setInfoMessage(null);
  }, [currentProjectId, environmentMode, selectedSshConfigId]);

  useEffect(() => {
    if (!highlightedSessionId) {
      return;
    }

    setSessionIdQuery(highlightedSessionId);
    setTaskIdQuery("");
    setContentQuery("");
  }, [highlightedSessionId, highlightedSessionNonce]);

  useEffect(() => {
    if (totalPages > 0 && page > totalPages) {
      setPage(totalPages);
    }
    if (totalPages === 0 && page !== 1) {
      setPage(1);
    }
  }, [page, totalPages]);

  useEffect(() => {
    if (!highlightedSessionId || filteredSessions.length === 0) {
      return;
    }

    const targetIndex = filteredSessions.findIndex((session) =>
      matchesSessionIdentifier(session, highlightedSessionId),
    );
    if (targetIndex < 0) {
      return;
    }

    const targetPage = Math.floor(targetIndex / PAGE_SIZE) + 1;
    if (targetPage !== page) {
      setPage(targetPage);
    }
  }, [filteredSessions, highlightedSessionId, highlightedSessionNonce, page]);

  useEffect(() => {
    if (!highlightedSessionId) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      const matchedSession = pageSessions.find((session) =>
        matchesSessionIdentifier(session, highlightedSessionId),
      );
      if (!matchedSession) {
        return;
      }

      document
        .getElementById(`session-row-${matchedSession.session_record_id}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 80);

    return () => window.clearTimeout(timeoutId);
  }, [highlightedSessionId, highlightedSessionNonce, pageSessions]);

  const loadSessions = async (silent = false): Promise<CodexSessionListItem[]> => {
    if (silent) {
      setRefreshing(true);
    } else {
      setLoading(true);
    }
    setErrorMessage(null);

    try {
      const [sessionItems] = await Promise.all([listCodexSessions(), fetchEmployees()]);
      setSessions(sessionItems);
      return sessionItems;
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : t("loadFailed"));
      return [];
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    void loadSessions();
  }, []);

  const handleViewModeChange = (mode: SessionsViewMode) => {
    setViewMode(mode);
    storeSessionsViewMode(mode);
  };

  useEffect(() => {
    // Restore focus to the toggle when the panel collapses, so keyboard
    // users are not left without a focus target.
    if (wasMoreFiltersOpen.current && !moreFiltersOpen) {
      moreFiltersButtonRef.current?.focus();
    }
    wasMoreFiltersOpen.current = moreFiltersOpen;
  }, [moreFiltersOpen]);

  const openContinueDialog = (session: CodexSessionListItem) => {
    setContinueSession(session);
    setContinueDialogOpen(true);
    setErrorMessage(null);
    setInfoMessage(null);
  };

  const openLogDialog = (session: CodexSessionListItem) => {
    setLogTarget(buildLogTarget(session));
    setLogDialogOpen(true);
  };

  const openChangeDialog = (session: CodexSessionListItem) => {
    setChangeTarget(session);
    setChangeDialogOpen(true);
  };

  const handleStopSession = async (session: CodexSessionListItem) => {
    if (stoppingSessionId) {
      return;
    }

    setStoppingSessionId(session.session_record_id);
    setErrorMessage(null);
    setInfoMessage(null);

    try {
      await stopSessionByProvider(
        normalizeAiProvider(session.ai_provider),
        session.session_record_id,
      );

      if (session.employee_id) {
        await refreshEmployeeRuntimeStatus(session.employee_id);
      }
      await loadSessions(true);
      setInfoMessage(t("stopRequested", { sessionId: session.session_id }));
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : t("stopFailed"));
    } finally {
      setStoppingSessionId(null);
    }
  };

  const handleContinueConversation = async (prompt: string) => {
    if (!continueSession) {
      return;
    }

    setContinueSubmitting(true);
    setErrorMessage(null);
    setInfoMessage(null);

    try {
      const preview = await prepareCodexSessionResume(continueSession.session_record_id);
      if (!preview.can_resume || !preview.resolved_session_id || !preview.employee_id) {
        setErrorMessage(preview.resume_message ?? t("continueUnavailable"));
        return;
      }

      const employee = employees.find((item) => item.id === preview.employee_id);
      await updateEmployeeStatus(preview.employee_id, "busy");

      const startOptions = {
        model: employee?.model,
        reasoningEffort: employee?.reasoning_effort,
        systemPrompt: employee?.system_prompt,
        workingDir: preview.working_dir ?? undefined,
        taskId: preview.task_id ?? undefined,
        taskGitContextId: preview.task_git_context_id ?? undefined,
        resumeSessionId: preview.resolved_session_id,
        sessionKind: preview.session_kind ?? undefined,
      };

      await startByProvider(
        preview.ai_provider || "codex",
        preview.employee_id,
        prompt,
        startOptions,
      );
      await refreshEmployeeRuntimeStatus(preview.employee_id);

      const sessionItems = await loadSessions(true);
      const resumedSession = sessionItems.find(
        (item) =>
          item.employee_id === preview.employee_id &&
          item.cli_session_id === preview.resolved_session_id &&
          item.session_kind === (preview.session_kind ?? "execution") &&
          item.status === "running",
      );
      const nextLogTarget = resumedSession
        ? buildLogTarget(resumedSession)
        : {
            ...buildLogTarget(preview),
            sessionRecordId: null,
          };
      setActiveSession(nextLogTarget);
      setInfoMessage(t("messageSent", { sessionId: preview.resolved_session_id }));
      setContinueDialogOpen(false);
      setContinueSession(null);
      setLogTarget(nextLogTarget);
      setLogDialogOpen(true);
    } catch (error) {
      if (continueSession.employee_id) {
        const runtime = await refreshEmployeeRuntimeStatus(continueSession.employee_id);
        if (!runtime?.running) {
          await updateEmployeeStatus(continueSession.employee_id, "error");
        }
      }
      setErrorMessage(error instanceof Error ? error.message : t("continueFailed"));
    } finally {
      setContinueSubmitting(false);
    }
  };

  return (
    <>
      <div className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold">{t("listTitle")}</h2>
            <p className="text-sm text-muted-foreground">
              {t(environmentMode === "ssh" ? "scopeSsh" : "scopeLocal", {
                scope: currentProjectName
                  ? t("scopeProject", { name: currentProjectName })
                  : t("scopeAllProjects"),
              })}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <div className="flex items-center rounded-lg border border-border p-0.5">
              <Button
                type="button"
                size="icon-sm"
                variant={viewMode === "card" ? "secondary" : "ghost"}
                onClick={() => handleViewModeChange("card")}
                title={t("viewCard")}
                aria-label={t("viewCard")}
                aria-pressed={viewMode === "card"}
              >
                <LayoutGrid />
              </Button>
              <Button
                type="button"
                size="icon-sm"
                variant={viewMode === "table" ? "secondary" : "ghost"}
                onClick={() => handleViewModeChange("table")}
                title={t("viewTable")}
                aria-label={t("viewTable")}
                aria-pressed={viewMode === "table"}
              >
                <Table />
              </Button>
            </div>
            <Button
              type="button"
              variant="outline"
              onClick={() => void loadSessions(true)}
              disabled={loading || refreshing}
            >
              {refreshing ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="mr-2 h-4 w-4" />
              )}
              {t("refresh")}
              <Kbd variant="subtle" size="xs" className="ml-1.5">
                R
              </Kbd>
            </Button>
          </div>
        </div>

        {activeSession && (
          <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-sm text-primary">
            {t("activeContinueHint", { sessionId: activeSession.sessionId })}
          </div>
        )}

        {errorMessage && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            {errorMessage}
          </div>
        )}

        {infoMessage && (
          <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-sm text-primary">
            {infoMessage}
          </div>
        )}

        <Card>
          <CardContent className="space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <Input
                id="session-content-search"
                value={contentQuery}
                onChange={(event) => setContentQuery(event.target.value)}
                placeholder={t("contentPlaceholder")}
                aria-label={t("contentSearch")}
                className="min-w-56 flex-1"
              />
              <select
                className="h-9 rounded-md border border-input bg-background px-2 text-sm"
                value={statusFilter}
                onChange={(e) => setStatusFilter(e.target.value)}
                aria-label={t("status")}
              >
                <option value="all">{t("allStatuses")}</option>
                <option value="pending">{t("statusPending")}</option>
                <option value="running">{t("statusRunning")}</option>
                <option value="stopping">{t("statusStopping")}</option>
                <option value="exited">{t("statusExited")}</option>
                <option value="failed">{t("statusFailed")}</option>
              </select>
              <Button
                type="button"
                variant="outline"
                ref={moreFiltersButtonRef}
                onClick={() => setMoreFiltersOpen((open) => !open)}
                aria-expanded={moreFiltersOpen}
                aria-controls={moreFiltersOpen ? "sessions-more-filters" : undefined}
              >
                <SlidersHorizontal className="mr-1.5 h-4 w-4" />
                {t("moreFilters")}
                {activeMoreFilterCount > 0 && (
                  <Badge variant="secondary" className="ml-1.5">
                    {activeMoreFilterCount}
                  </Badge>
                )}
                <ChevronDown
                  className={cn(
                    "ml-1.5 h-4 w-4 transition-transform",
                    moreFiltersOpen && "rotate-180",
                  )}
                />
              </Button>
            </div>

            {moreFiltersOpen && (
              <div
                id="sessions-more-filters"
                className="space-y-3 rounded-lg border border-border/60 bg-muted/30 p-3"
              >
                <div className="grid gap-3 md:grid-cols-3">
                  <div className="space-y-2">
                    <label
                      className="text-sm font-medium text-foreground"
                      htmlFor="session-id-search"
                    >
                      {t("sessionIdSearch")}
                    </label>
                    <Input
                      id="session-id-search"
                      value={sessionIdQuery}
                      onChange={(event) => setSessionIdQuery(event.target.value)}
                      placeholder={t("sessionIdPlaceholder")}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium text-foreground" htmlFor="task-id-search">
                      {t("taskIdSearch")}
                    </label>
                    <Input
                      id="task-id-search"
                      value={taskIdQuery}
                      onChange={(event) => setTaskIdQuery(event.target.value)}
                      placeholder={t("taskIdPlaceholder")}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("kind")}</label>
                    <select
                      className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm"
                      value={kindFilter}
                      onChange={(e) => setKindFilter(e.target.value)}
                    >
                      <option value="all">{t("allKinds")}</option>
                      <option value="execution">{t("kindExecution")}</option>
                      <option value="review">{t("kindReview")}</option>
                    </select>
                  </div>
                </div>
                <div className="grid gap-3 md:grid-cols-3">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("provider")}</label>
                    <select
                      className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm"
                      value={providerFilter}
                      onChange={(e) => setProviderFilter(e.target.value)}
                    >
                      <option value="all">{t("allProviders")}</option>
                      {AI_PROVIDER_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("employee")}</label>
                    <select
                      className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm"
                      value={employeeFilter}
                      onChange={(e) => setEmployeeFilter(e.target.value)}
                    >
                      <option value="all">{t("allEmployees")}</option>
                      {employees.map((employee) => (
                        <option key={employee.id} value={employee.id}>
                          {employee.name}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="flex items-end pb-1">
                    <label className="flex items-center gap-2 text-sm">
                      <input
                        type="checkbox"
                        checked={failedOnly}
                        onChange={(e) => setFailedOnly(e.target.checked)}
                      />
                      {t("failedOnly")}
                    </label>
                  </div>
                </div>
              </div>
            )}

            {loading ? (
              <div className="flex h-[28rem] items-center justify-center rounded-xl border border-border/70 text-sm text-muted-foreground">
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                {t("loading")}
              </div>
            ) : filteredSessions.length === 0 ? (
              <div className="flex h-[28rem] items-center justify-center rounded-xl border border-border/70 text-sm text-muted-foreground">
                {t("emptyFiltered")}
              </div>
            ) : viewMode === "card" ? (
              <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                {pageSessions.map((session) => (
                  <SessionCard
                    key={session.session_record_id}
                    session={session}
                    highlighted={matchesSessionIdentifier(session, highlightedSessionId)}
                    stopping={stoppingSessionId === session.session_record_id}
                    onContinue={openContinueDialog}
                    onStop={(target) => void handleStopSession(target)}
                    onViewLog={openLogDialog}
                    onViewChanges={openChangeDialog}
                  />
                ))}
              </div>
            ) : (
              <div className="overflow-hidden rounded-xl border border-border/70">
                <div className="overflow-x-auto">
                  <table className="min-w-full text-sm">
                    <thead className="bg-muted/40 text-left">
                      <tr className="border-b border-border">
                        <th className="px-4 py-3 font-medium">{t("colSession")}</th>
                        <th className="px-4 py-3 font-medium">{t("colStatus")}</th>
                        <th className="px-4 py-3 font-medium">{t("colProvider")}</th>
                        <th className="px-4 py-3 font-medium">{t("colUpdated")}</th>
                        <th className="px-4 py-3 font-medium">{t("colTokens")}</th>
                        <th className="px-4 py-3 font-medium">{t("colTask")}</th>
                        <th className="px-4 py-3 font-medium">{t("colEmployee")}</th>
                        <th className="px-4 py-3 font-medium">{t("colActions")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {pageSessions.map((session) => (
                        <tr
                          id={`session-row-${session.session_record_id}`}
                          key={session.session_record_id}
                          className={`border-b border-border/60 align-top last:border-b-0 ${
                            matchesSessionIdentifier(session, highlightedSessionId)
                              ? "bg-primary/5"
                              : ""
                          }`}
                        >
                          <td className="px-4 py-3">
                            <div className="space-y-1">
                              <div
                                className="max-w-md truncate font-medium"
                                title={session.display_name}
                              >
                                {session.display_name}
                              </div>
                              <div
                                className="max-w-md truncate font-mono text-xs text-muted-foreground"
                                title={session.session_id}
                              >
                                {session.session_id}
                              </div>
                              {session.summary && (
                                <div
                                  className="max-w-md truncate text-xs text-muted-foreground"
                                  title={session.summary}
                                >
                                  {session.summary}
                                </div>
                              )}
                              {session.content_preview && (
                                <div
                                  className="max-w-md truncate text-xs text-muted-foreground/80"
                                  title={session.content_preview}
                                >
                                  {t("contentPrefix", { preview: session.content_preview })}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <Badge variant={sessionStatusBadgeVariant(session.status)}>
                              {formatSessionStatus(session.status)}
                            </Badge>
                            <div className="mt-1 whitespace-nowrap text-[11px] text-muted-foreground">
                              {formatSessionKind(session.session_kind)}
                              {" · "}
                              {session.execution_target === "ssh"
                                ? t("common:sshShort")
                                : t("common:localShort")}
                              {" · "}
                              {formatResumeStatus(session.resume_status)}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <Badge
                              variant={aiProviderBadgeVariant(
                                normalizeAiProvider(session.ai_provider),
                              )}
                            >
                              {formatAiProviderLabel(normalizeAiProvider(session.ai_provider))}
                            </Badge>
                          </td>
                          <td className="whitespace-nowrap px-4 py-3 text-xs text-muted-foreground">
                            {formatDate(session.last_updated_at)}
                          </td>
                          <td className="whitespace-nowrap px-4 py-3 text-xs text-muted-foreground">
                            {formatSessionTokenUsage(session)}
                          </td>
                          <td className="px-4 py-3">
                            <div className="space-y-1 text-xs">
                              <div
                                className="max-w-56 truncate"
                                title={session.task_title ?? undefined}
                              >
                                {session.task_title ?? t("noLinkedTask")}
                              </div>
                              <div className="text-muted-foreground">
                                {t("taskIdPrefix")}
                                <span className="ml-1 font-mono">{session.task_id ?? "-"}</span>
                              </div>
                              <div
                                className="max-w-56 truncate text-muted-foreground"
                                title={session.project_name ?? undefined}
                              >
                                {session.project_name ?? t("noLinkedProject")}
                              </div>
                              {session.target_host_label && (
                                <div
                                  className="max-w-56 truncate text-muted-foreground"
                                  title={session.target_host_label}
                                >
                                  {t("hostPrefix", { host: session.target_host_label })}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="space-y-1 text-xs">
                              <div>{session.employee_name ?? t("unboundEmployee")}</div>
                              {session.working_dir && (
                                <div
                                  className="max-w-56 truncate text-muted-foreground"
                                  title={session.working_dir}
                                >
                                  {session.working_dir}
                                </div>
                              )}
                              {isArtifactCaptureLimited(session.artifact_capture_mode) && (
                                <SshArtifactLimitedNotice
                                  artifactCaptureMode={session.artifact_capture_mode}
                                  compact
                                />
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="flex items-center gap-1.5">
                              <Button
                                type="button"
                                size="sm"
                                onClick={() => openContinueDialog(session)}
                                disabled={!session.can_resume}
                                title={session.resume_message ?? t("continue")}
                              >
                                {t("continue")}
                              </Button>
                              <Button
                                type="button"
                                size="sm"
                                variant="outline"
                                onClick={() => openLogDialog(session)}
                                disabled={!session.task_id && !session.employee_id}
                              >
                                {t("viewLog")}
                              </Button>
                              {(session.session_kind === "execution" ||
                                session.status === "running" ||
                                session.status === "stopping") && (
                                <DropdownMenu>
                                  <DropdownMenuTrigger
                                    className="inline-flex size-7 items-center justify-center rounded-lg border border-border bg-background text-foreground outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
                                    aria-label={t("moreActions")}
                                  >
                                    <MoreHorizontal className="h-4 w-4" />
                                  </DropdownMenuTrigger>
                                  <DropdownMenuContent align="end" className="min-w-40">
                                    {session.session_kind === "execution" && (
                                      <DropdownMenuItem onClick={() => openChangeDialog(session)}>
                                        {t("changes")}
                                      </DropdownMenuItem>
                                    )}
                                    {(session.status === "running" ||
                                      session.status === "stopping") && (
                                      <DropdownMenuItem
                                        variant="destructive"
                                        disabled={
                                          session.status === "stopping" ||
                                          stoppingSessionId === session.session_record_id
                                        }
                                        onClick={() => void handleStopSession(session)}
                                      >
                                        {stoppingSessionId === session.session_record_id ? (
                                          <>
                                            <Loader2 className="animate-spin" />
                                            {t("stopping")}
                                          </>
                                        ) : (
                                          t("stop")
                                        )}
                                      </DropdownMenuItem>
                                    )}
                                  </DropdownMenuContent>
                                </DropdownMenu>
                              )}
                            </div>
                            {session.resume_message && (
                              <div
                                className="mt-1 max-w-56 truncate text-xs text-muted-foreground"
                                title={session.resume_message}
                              >
                                {session.resume_message}
                              </div>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-muted-foreground">
                {filteredSessions.length === 0
                  ? t("paginationEmpty")
                  : t("paginationRange", {
                      start: rangeStart,
                      end: rangeEnd,
                      total: filteredSessions.length,
                    })}
              </span>
              <div className="flex items-center gap-2">
                <span className="text-xs text-muted-foreground">
                  {filteredSessions.length === 0
                    ? t("paginationPageEmpty")
                    : t("paginationPage", { page, totalPages })}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setPage((current) => Math.max(1, current - 1))}
                  disabled={loading || page <= 1}
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                  {t("prevPage")}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setPage((current) => current + 1)}
                  disabled={loading || filteredSessions.length === 0 || page >= totalPages}
                >
                  {t("nextPage")}
                  <ChevronRight className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <SessionContinueDialog
        open={continueDialogOpen}
        session={continueSession}
        submitting={continueSubmitting}
        onOpenChange={(open) => {
          if (!continueSubmitting) {
            setContinueDialogOpen(open);
            if (!open) {
              setContinueSession(null);
            }
          }
        }}
        onConfirm={handleContinueConversation}
      />

      <SessionLogDialog open={logDialogOpen} session={logTarget} onOpenChange={setLogDialogOpen} />

      <SessionExecutionChangesDialog
        open={changeDialogOpen}
        session={changeTarget}
        onOpenChange={(open) => {
          setChangeDialogOpen(open);
          if (!open) {
            setChangeTarget(null);
          }
        }}
      />
    </>
  );
}
