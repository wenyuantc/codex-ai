import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2, RefreshCw } from "lucide-react";

import { SessionContinueDialog } from "@/components/sessions/SessionContinueDialog";
import { SessionLogDialog, type SessionLogTarget } from "@/components/sessions/SessionLogDialog";
import { SshArtifactLimitedNotice } from "@/components/sessions/SshArtifactLimitedNotice";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { listActivityLogs, listCodexSessions, prepareCodexSessionResume } from "@/lib/backend";
import { startByProvider, stopSessionByProvider } from "@/lib/aiEngine";
import type {
  ActivityLog,
  AiProvider,
  CodexSessionListItem,
  CodexSessionResumeStatus,
} from "@/lib/types";
import { AI_PROVIDER_OPTIONS, normalizeAiProvider } from "@/lib/types";
import { formatDate, isArtifactCaptureLimited } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { getSessionStatusLabel } from "./taskDetailViewHelpers";
import { TokenUsageBreakdown } from "./TokenUsageBreakdown";

type ChainRole = "execute" | "review" | "fix";

interface ChainItem {
  session: CodexSessionListItem;
  role: ChainRole;
}

interface TaskSessionChainPanelProps {
  taskId: string;
  active?: boolean;
}

function formatAiProviderLabel(provider: AiProvider) {
  const option = AI_PROVIDER_OPTIONS.find((item) => item.value === provider);
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
    case "native":
      return "secondary";
    default:
      return "outline";
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

function formatResumeStatus(status: CodexSessionResumeStatus, translate: (key: string) => string) {
  switch (status) {
    case "ready":
      return translate("detail.chain.resume.ready");
    case "running":
      return translate("detail.chain.resume.running");
    case "missing_employee":
      return translate("detail.chain.resume.missingEmployee");
    case "missing_cli_session":
      return translate("detail.chain.resume.missingCliSession");
    case "stopping":
      return translate("detail.chain.resume.stopping");
    case "invalid":
      return translate("detail.chain.resume.invalid");
    default:
      return status;
  }
}

function roleBadgeClassName(role: ChainRole) {
  switch (role) {
    case "review":
      return "border-blue-500/30 bg-blue-500/10 text-blue-700";
    case "fix":
      return "border-amber-500/30 bg-amber-500/10 text-amber-800";
    case "execute":
    default:
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
  }
}

function formatChainRole(role: ChainRole, translate: (key: string) => string) {
  switch (role) {
    case "review":
      return translate("detail.chain.role.review");
    case "fix":
      return translate("detail.chain.role.fix");
    case "execute":
    default:
      return translate("detail.chain.role.execute");
  }
}

function sessionSortKey(session: CodexSessionListItem) {
  return session.last_updated_at || session.session_record_id;
}

function parseTime(value: string | null | undefined) {
  if (!value) {
    return Number.NaN;
  }
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : Number.NaN;
}

/**
 * Build execution → review → fix timeline labels.
 * Review sessions → review. Execution sessions → execute, unless they follow a
 * failed review or sit near task_automation_fix_started activity → fix.
 */
export function buildTaskSessionChain(
  sessions: CodexSessionListItem[],
  fixLogs: ActivityLog[],
): ChainItem[] {
  const sorted = [...sessions].sort((left, right) =>
    sessionSortKey(left).localeCompare(sessionSortKey(right)),
  );

  const fixTimes = fixLogs
    .map((log) => parseTime(log.created_at))
    .filter((time) => Number.isFinite(time));

  return sorted.map((session, index) => {
    if (session.session_kind === "review") {
      return { session, role: "review" as const };
    }

    const previous = index > 0 ? sorted[index - 1] : null;
    const sessionTime = parseTime(session.last_updated_at);
    const previousTime = previous ? parseTime(previous.last_updated_at) : Number.NaN;

    const followsFailedReview = previous?.session_kind === "review" && previous.status === "failed";

    const hasFixActivityNearSession = fixTimes.some((fixTime) => {
      if (!Number.isFinite(sessionTime)) {
        return false;
      }
      const betweenPreviousAndCurrent =
        previous?.session_kind === "review" &&
        Number.isFinite(previousTime) &&
        fixTime >= previousTime - 60_000 &&
        fixTime <= sessionTime + 120_000;
      const nearCurrent = Math.abs(fixTime - sessionTime) <= 15 * 60_000;
      return betweenPreviousAndCurrent || nearCurrent;
    });

    if (followsFailedReview || hasFixActivityNearSession) {
      return { session, role: "fix" as const };
    }

    return { session, role: "execute" as const };
  });
}

function buildLogTarget(session: CodexSessionListItem): SessionLogTarget {
  return {
    sessionRecordId: session.session_record_id,
    sessionId: session.session_id,
    displayName: session.display_name,
    employeeId: session.employee_id,
    employeeName: session.employee_name,
    taskId: session.task_id,
    taskTitle: session.task_title,
    sessionKind: session.session_kind,
    aiProvider: session.ai_provider,
  };
}

export function TaskSessionChainPanel({ taskId, active = true }: TaskSessionChainPanelProps) {
  const { t } = useTranslation("tasks");
  const employees = useEmployeeStore((state) => state.employees);
  const fetchEmployees = useEmployeeStore((state) => state.fetchEmployees);
  const updateEmployeeStatus = useEmployeeStore((state) => state.updateEmployeeStatus);
  const refreshEmployeeRuntimeStatus = useEmployeeStore(
    (state) => state.refreshEmployeeRuntimeStatus,
  );

  const [sessions, setSessions] = useState<CodexSessionListItem[]>([]);
  const [fixLogs, setFixLogs] = useState<ActivityLog[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [infoMessage, setInfoMessage] = useState<string | null>(null);
  const [stoppingSessionId, setStoppingSessionId] = useState<string | null>(null);
  const [continueSession, setContinueSession] = useState<CodexSessionListItem | null>(null);
  const [continueDialogOpen, setContinueDialogOpen] = useState(false);
  const [continueSubmitting, setContinueSubmitting] = useState(false);
  const [logTarget, setLogTarget] = useState<SessionLogTarget | null>(null);
  const [logDialogOpen, setLogDialogOpen] = useState(false);
  const requestIdRef = useRef(0);

  const loadChain = async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setLoading(true);
    setError(null);

    try {
      const [allSessions, activityPage] = await Promise.all([
        listCodexSessions(),
        listActivityLogs({
          taskId,
          action: "task_automation_fix_started",
          limit: 500,
          offset: 0,
        }).catch(() => ({
          items: [] as ActivityLog[],
          total: 0,
          available_actions: [] as string[],
        })),
      ]);

      if (requestIdRef.current !== requestId) {
        return;
      }

      setSessions(allSessions.filter((session) => session.task_id === taskId));
      // Backend returns DESC; chain UI expects chronological ASC.
      setFixLogs([...activityPage.items].reverse());
    } catch (loadError) {
      if (requestIdRef.current !== requestId) {
        return;
      }
      setSessions([]);
      setFixLogs([]);
      setError(loadError instanceof Error ? loadError.message : t("detail.chain.loadFailed"));
    } finally {
      if (requestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  };

  useEffect(() => {
    if (!active) {
      return;
    }
    void loadChain();
    void fetchEmployees();
  }, [active, taskId, fetchEmployees]);

  const chain = useMemo(() => buildTaskSessionChain(sessions, fixLogs), [sessions, fixLogs]);

  const handleContinueConversation = async (prompt: string) => {
    if (!continueSession) {
      return;
    }

    setContinueSubmitting(true);
    setError(null);
    setInfoMessage(null);

    try {
      const preview = await prepareCodexSessionResume(continueSession.session_record_id);
      if (!preview.can_resume || !preview.resolved_session_id || !preview.employee_id) {
        setError(preview.resume_message ?? t("detail.chain.cannotContinue"));
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
      setContinueDialogOpen(false);
      setContinueSession(null);
      setInfoMessage(t("detail.chain.continued"));
      await loadChain();
    } catch (continueError) {
      setError(
        continueError instanceof Error ? continueError.message : t("detail.chain.continueFailed"),
      );
    } finally {
      setContinueSubmitting(false);
    }
  };

  const handleStopSession = async (session: CodexSessionListItem) => {
    if (stoppingSessionId) {
      return;
    }

    setStoppingSessionId(session.session_record_id);
    setError(null);
    setInfoMessage(null);

    try {
      await stopSessionByProvider(
        normalizeAiProvider(session.ai_provider),
        session.session_record_id,
      );

      if (session.employee_id) {
        await refreshEmployeeRuntimeStatus(session.employee_id);
      }
      setInfoMessage(t("detail.chain.stopRequested", { sessionId: session.session_id }));
      await loadChain();
    } catch (stopError) {
      setError(stopError instanceof Error ? stopError.message : t("detail.chain.stopFailed"));
    } finally {
      setStoppingSessionId(null);
    }
  };

  return (
    <>
      <div className="space-y-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <p className="text-sm font-medium">{t("detail.chain.title")}</p>
            <p className="text-[11px] text-muted-foreground">{t("detail.chain.description")}</p>
          </div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => void loadChain()}
            disabled={loading}
          >
            {loading ? (
              <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="mr-1 h-3.5 w-3.5" />
            )}
            {t("detail.chain.refresh")}
          </Button>
        </div>

        {error && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        {infoMessage && (
          <div className="rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
            {infoMessage}
          </div>
        )}

        {loading && chain.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border/60 px-3 py-8 text-center text-xs text-muted-foreground">
            {t("detail.chain.loading")}
          </div>
        ) : chain.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border/60 px-3 py-8 text-center text-xs text-muted-foreground">
            {t("detail.chain.empty")}
          </div>
        ) : (
          <ol className="relative space-y-0 border-l border-border/80 pl-4">
            {chain.map(({ session, role }, index) => (
              <li key={session.session_record_id} className="relative pb-5 last:pb-0">
                <span
                  className={`absolute -left-[1.29rem] top-1.5 h-2.5 w-2.5 rounded-full border-2 border-background ${
                    role === "review"
                      ? "bg-blue-500"
                      : role === "fix"
                        ? "bg-amber-500"
                        : "bg-emerald-500"
                  }`}
                  aria-hidden
                />
                <div className="rounded-lg border border-border/60 bg-background/70 p-3">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <span
                      className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${roleBadgeClassName(role)}`}
                    >
                      {formatChainRole(role, t)}
                    </span>
                    <Badge variant={aiProviderBadgeVariant(session.ai_provider)}>
                      {formatAiProviderLabel(session.ai_provider)}
                    </Badge>
                    <Badge variant={session.execution_target === "ssh" ? "default" : "outline"}>
                      {session.execution_target === "ssh" ? "SSH" : t("detail.labels.local")}
                    </Badge>
                    <span className="text-[11px] text-muted-foreground">
                      {getSessionStatusLabel(session.status)}
                    </span>
                    <Badge variant={resumeBadgeVariant(session.resume_status)}>
                      {formatResumeStatus(session.resume_status, t)}
                    </Badge>
                    <span className="text-[11px] text-muted-foreground">
                      {formatDate(session.last_updated_at)}
                    </span>
                    {index === 0 && (
                      <span className="text-[11px] text-muted-foreground">
                        {t("detail.chain.origin")}
                      </span>
                    )}
                  </div>

                  <div className="mt-2 flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 flex-1 space-y-1 text-xs">
                      <div className="font-medium text-foreground">{session.display_name}</div>
                      <div className="font-mono text-[11px] text-muted-foreground">
                        {session.session_id}
                      </div>
                      <div className="text-[11px] text-muted-foreground">
                        {t("detail.chain.employee", {
                          name: session.employee_name ?? t("detail.labels.unbound"),
                        })}
                        {session.target_host_label
                          ? t("detail.chain.host", { host: session.target_host_label })
                          : ""}
                      </div>
                      {session.summary && (
                        <p className="text-[11px] text-muted-foreground line-clamp-2">
                          {session.summary}
                        </p>
                      )}
                    </div>
                    <TokenUsageBreakdown
                      layout="inline"
                      source={session}
                      className="w-full min-w-[18rem] sm:w-auto sm:min-w-[22rem] sm:max-w-[26rem]"
                    />
                  </div>

                  {isArtifactCaptureLimited(session.artifact_capture_mode) && (
                    <div className="mt-2">
                      <SshArtifactLimitedNotice
                        artifactCaptureMode={session.artifact_capture_mode}
                        compact
                      />
                    </div>
                  )}

                  <div className="mt-3 flex flex-wrap gap-2">
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        setLogTarget(buildLogTarget(session));
                        setLogDialogOpen(true);
                      }}
                      disabled={!session.task_id && !session.employee_id}
                    >
                      {t("detail.chain.viewLogs")}
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      onClick={() => {
                        setContinueSession(session);
                        setContinueDialogOpen(true);
                        setError(null);
                        setInfoMessage(null);
                      }}
                      disabled={!session.can_resume}
                      title={session.resume_message ?? t("detail.chain.continue")}
                    >
                      {t("detail.chain.continue")}
                    </Button>
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
                            {t("detail.chain.stopping")}
                          </>
                        ) : (
                          t("detail.chain.stop")
                        )}
                      </Button>
                    )}
                  </div>
                </div>
              </li>
            ))}
          </ol>
        )}
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
    </>
  );
}
