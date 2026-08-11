import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight, Loader2, RefreshCw } from "lucide-react";
import { useLocation, useSearchParams } from "react-router-dom";

import { SessionContinueDialog } from "@/components/sessions/SessionContinueDialog";
import { SessionExecutionChangesDialog } from "@/components/sessions/SessionExecutionChangesDialog";
import { SessionLogDialog, type SessionLogTarget } from "@/components/sessions/SessionLogDialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { listCodexSessions, prepareCodexSessionResume } from "@/lib/backend";
import { startCodex, stopCodexSession } from "@/lib/codex";
import { startClaude, stopClaudeSession } from "@/lib/claude";
import { startGrok, stopGrokSession } from "@/lib/grok";
import { startOpenCode, stopOpenCodeSession } from "@/lib/opencode";
import type { AiProvider, CodexSessionListItem, CodexSessionResumeStatus } from "@/lib/types";
import { AI_PROVIDER_OPTIONS, normalizeAiProvider } from "@/lib/types";
import i18n from "@/lib/i18n";
import { formatDate, isArtifactCaptureLimited } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { SshArtifactLimitedNotice } from "@/components/sessions/SshArtifactLimitedNotice";

const PAGE_SIZE = 10;

function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").toLocaleLowerCase().trim();
}

function matchesSessionIdentifier(session: CodexSessionListItem, query: string | null) {
  if (!query) {
    return false;
  }

  const normalizedQuery = normalizeSearchText(query);
  return [session.session_id, session.session_record_id, session.cli_session_id].some(
    (value) => normalizeSearchText(value) === normalizedQuery,
  );
}

function formatAiProviderLabel(provider: AiProvider) {
  const option = AI_PROVIDER_OPTIONS.find((o) => o.value === provider);
  return option?.label ?? provider;
}

function aiProviderBadgeVariant(provider: AiProvider): "default" | "secondary" | "outline" {
  switch (provider) {
    case "codex":
      return "default";
    case "claude":
      return "secondary";
    case "opencode":
      return "outline";
    case "grok":
      return "outline";
    default:
      return "outline";
  }
}

function formatSessionKind(kind: CodexSessionListItem["session_kind"]) {
  return kind === "review" ? i18n.t("sessions:kindReview") : i18n.t("sessions:kindExecution");
}

function formatSessionStatus(status: string) {
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

function formatResumeStatus(status: CodexSessionResumeStatus) {
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

function resumeBadgeVariant(
  status: CodexSessionResumeStatus,
): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "ready":
      return "default";
    case "running":
    case "stopping":
      return "secondary";
    case "missing_employee":
    case "missing_cli_session":
    case "invalid":
      return "destructive";
    default:
      return "outline";
  }
}

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
      const provider = normalizeAiProvider(session.ai_provider);
      if (provider === "claude") {
        await stopClaudeSession(session.session_record_id);
      } else if (provider === "grok") {
        await stopGrokSession(session.session_record_id);
      } else if (provider === "opencode") {
        await stopOpenCodeSession(session.session_record_id);
      } else {
        await stopCodexSession(session.session_record_id);
      }

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

      if (preview.ai_provider === "claude") {
        await startClaude(preview.employee_id, prompt, startOptions);
      } else if (preview.ai_provider === "grok") {
        await startGrok(preview.employee_id, prompt, startOptions);
      } else if (preview.ai_provider === "opencode") {
        await startOpenCode({
          employeeId: preview.employee_id,
          taskDescription: prompt,
          model: employee?.model,
          workingDir: preview.working_dir ?? undefined,
          taskId: preview.task_id ?? undefined,
          taskGitContextId: preview.task_git_context_id ?? undefined,
          resumeSessionId: preview.resolved_session_id,
        });
      } else {
        await startCodex(preview.employee_id, prompt, startOptions);
      }
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
            <div className="grid gap-3 md:grid-cols-3">
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground" htmlFor="session-id-search">
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
                <label
                  className="text-sm font-medium text-foreground"
                  htmlFor="session-content-search"
                >
                  {t("contentSearch")}
                </label>
                <Input
                  id="session-content-search"
                  value={contentQuery}
                  onChange={(event) => setContentQuery(event.target.value)}
                  placeholder={t("contentPlaceholder")}
                />
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-5">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("status")}</label>
                <select
                  className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm"
                  value={statusFilter}
                  onChange={(e) => setStatusFilter(e.target.value)}
                >
                  <option value="all">{t("allStatuses")}</option>
                  <option value="pending">{t("statusPending")}</option>
                  <option value="running">{t("statusRunning")}</option>
                  <option value="stopping">{t("statusStopping")}</option>
                  <option value="exited">{t("statusExited")}</option>
                  <option value="failed">{t("statusFailed")}</option>
                </select>
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

            <div className="overflow-hidden rounded-xl border border-border/70">
              {loading ? (
                <div className="flex h-[28rem] items-center justify-center text-sm text-muted-foreground">
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t("loading")}
                </div>
              ) : filteredSessions.length === 0 ? (
                <div className="flex h-[28rem] items-center justify-center text-sm text-muted-foreground">
                  {t("emptyFiltered")}
                </div>
              ) : (
                <div className="overflow-x-auto">
                  <table className="min-w-full text-sm">
                    <thead className="bg-muted/40 text-left">
                      <tr className="border-b border-border">
                        <th className="px-4 py-3 font-medium">{t("colSession")}</th>
                        <th className="px-4 py-3 font-medium">{t("colStatus")}</th>
                        <th className="px-4 py-3 font-medium">{t("colProvider")}</th>
                        <th className="px-4 py-3 font-medium">{t("colUpdated")}</th>
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
                              <div className="font-medium">{session.display_name}</div>
                              <div className="font-mono text-xs text-muted-foreground">
                                {session.session_id}
                              </div>
                              {session.summary && (
                                <div className="max-w-md text-xs text-muted-foreground">
                                  {session.summary}
                                </div>
                              )}
                              {session.content_preview && (
                                <div className="max-w-md text-xs text-muted-foreground/80">
                                  {t("contentPrefix", { preview: session.content_preview })}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="flex flex-col gap-2">
                              <Badge variant="outline">
                                {formatSessionKind(session.session_kind)}
                              </Badge>
                              <Badge variant="secondary">
                                {formatSessionStatus(session.status)}
                              </Badge>
                              <Badge
                                variant={session.execution_target === "ssh" ? "default" : "outline"}
                              >
                                {session.execution_target === "ssh"
                                  ? t("common:sshShort")
                                  : t("common:localShort")}
                              </Badge>
                              <Badge variant={resumeBadgeVariant(session.resume_status)}>
                                {formatResumeStatus(session.resume_status)}
                              </Badge>
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
                          <td className="px-4 py-3 text-xs text-muted-foreground">
                            {formatDate(session.last_updated_at)}
                          </td>
                          <td className="px-4 py-3">
                            <div className="space-y-1 text-xs">
                              <div>{session.task_title ?? t("noLinkedTask")}</div>
                              <div className="text-muted-foreground">
                                {t("taskIdPrefix")}
                                <span className="ml-1 font-mono">{session.task_id ?? "-"}</span>
                              </div>
                              <div className="text-muted-foreground">
                                {session.project_name ?? t("noLinkedProject")}
                              </div>
                              {session.target_host_label && (
                                <div className="text-muted-foreground">
                                  {t("hostPrefix", { host: session.target_host_label })}
                                </div>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3">
                            <div className="space-y-1 text-xs">
                              <div>{session.employee_name ?? t("unboundEmployee")}</div>
                              {session.working_dir && (
                                <div className="max-w-56 break-all text-muted-foreground">
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
                            <div className="flex min-w-40 flex-col gap-2">
                              {session.session_kind === "execution" && (
                                <Button
                                  type="button"
                                  size="sm"
                                  variant="secondary"
                                  onClick={() => openChangeDialog(session)}
                                >
                                  {t("changes")}
                                </Button>
                              )}
                              {(session.status === "running" || session.status === "stopping") && (
                                <Button
                                  type="button"
                                  size="sm"
                                  variant="destructive"
                                  onClick={() => void handleStopSession(session)}
                                  disabled={
                                    session.status === "stopping" ||
                                    stoppingSessionId === session.session_record_id
                                  }
                                >
                                  {stoppingSessionId === session.session_record_id ? (
                                    <>
                                      <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
                                      {t("stopping")}
                                    </>
                                  ) : (
                                    t("stop")
                                  )}
                                </Button>
                              )}
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
                              {session.resume_message && (
                                <div className="text-xs text-muted-foreground">
                                  {session.resume_message}
                                </div>
                              )}
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

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
