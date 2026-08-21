import { FileText, Loader2, MoreHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";

import { SshArtifactLimitedNotice } from "@/components/sessions/SshArtifactLimitedNotice";
import { TokenUsageBreakdown } from "@/components/tasks/detail/TokenUsageBreakdown";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  aiProviderBadgeVariant,
  formatAiProviderLabel,
  formatResumeStatus,
  formatSessionKind,
  formatSessionStatus,
  formatSessionTokenUsage,
  sessionStatusBadgeVariant,
} from "@/lib/sessions";
import type { CodexSessionListItem } from "@/lib/types";
import { normalizeAiProvider } from "@/lib/types";
import { cn, formatDate, isArtifactCaptureLimited } from "@/lib/utils";

interface SessionCardProps {
  session: CodexSessionListItem;
  highlighted: boolean;
  stopping: boolean;
  onContinue: (session: CodexSessionListItem) => void;
  onStop: (session: CodexSessionListItem) => void;
  onViewLog: (session: CodexSessionListItem) => void;
  onViewChanges: (session: CodexSessionListItem) => void;
}

export function SessionCard({
  session,
  highlighted,
  stopping,
  onContinue,
  onStop,
  onViewLog,
  onViewChanges,
}: SessionCardProps) {
  const { t } = useTranslation(["sessions", "common"]);
  const isSsh = session.execution_target === "ssh";
  const showChanges = session.session_kind === "execution";
  const showStop = session.status === "running" || session.status === "stopping";
  const hasMoreActions = showChanges || showStop;

  return (
    <div
      id={`session-row-${session.session_record_id}`}
      className={cn(
        "flex flex-col gap-1.5 rounded-xl border border-border/70 bg-card p-3",
        highlighted && "border-primary/40 bg-primary/5",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium" title={session.display_name}>
            {session.display_name}
          </div>
          <div
            className="truncate font-mono text-[11px] text-muted-foreground"
            title={session.session_id}
          >
            {session.session_id}
          </div>
        </div>
        <Badge variant={sessionStatusBadgeVariant(session.status)} className="shrink-0">
          {session.status === "running" && (
            <span className="size-1.5 animate-pulse rounded-full bg-current" />
          )}
          {formatSessionStatus(session.status)}
        </Badge>
      </div>

      <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[11px] text-muted-foreground">
        <Badge variant={aiProviderBadgeVariant(normalizeAiProvider(session.ai_provider))}>
          {formatAiProviderLabel(normalizeAiProvider(session.ai_provider))}
        </Badge>
        <span>{formatSessionKind(session.session_kind)}</span>
        <span aria-hidden>·</span>
        <span>{isSsh ? t("common:sshShort") : t("common:localShort")}</span>
        <span aria-hidden>·</span>
        <span>{formatResumeStatus(session.resume_status)}</span>
        <span aria-hidden>·</span>
        <span>{formatDate(session.last_updated_at)}</span>
      </div>

      <TokenUsageBreakdown
        layout="inline"
        source={session}
        className="w-full text-left"
        title={formatSessionTokenUsage(session)}
      />

      <div className="truncate text-xs text-muted-foreground">
        <span title={session.task_title ?? undefined}>
          {session.task_title ?? t("noLinkedTask")}
        </span>
        <span aria-hidden> · </span>
        <span>{session.employee_name ?? t("unboundEmployee")}</span>
      </div>

      {session.target_host_label && (
        <div
          className="truncate text-[11px] text-muted-foreground"
          title={session.target_host_label}
        >
          {t("hostPrefix", { host: session.target_host_label })}
        </div>
      )}

      {session.resume_message && (
        <div className="truncate text-[11px] text-muted-foreground" title={session.resume_message}>
          {session.resume_message}
        </div>
      )}

      {isArtifactCaptureLimited(session.artifact_capture_mode) && (
        <SshArtifactLimitedNotice artifactCaptureMode={session.artifact_capture_mode} compact />
      )}

      <div className="mt-auto flex items-center gap-1.5 pt-1">
        <Button
          type="button"
          size="sm"
          onClick={() => onContinue(session)}
          disabled={!session.can_resume}
          title={session.resume_message ?? t("continue")}
        >
          {t("continue")}
        </Button>
        <Button
          type="button"
          size="icon-sm"
          variant="outline"
          onClick={() => onViewLog(session)}
          disabled={!session.task_id && !session.employee_id}
          title={t("viewLog")}
          aria-label={t("viewLog")}
        >
          <FileText />
        </Button>
        {hasMoreActions && (
          <DropdownMenu>
            <DropdownMenuTrigger
              className="inline-flex size-7 items-center justify-center rounded-lg border border-border bg-background text-foreground outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
              aria-label={t("moreActions")}
            >
              <MoreHorizontal className="h-4 w-4" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="min-w-40">
              {showChanges && (
                <DropdownMenuItem onClick={() => onViewChanges(session)}>
                  {t("changes")}
                </DropdownMenuItem>
              )}
              {showStop && (
                <DropdownMenuItem
                  variant="destructive"
                  disabled={session.status === "stopping" || stopping}
                  onClick={() => onStop(session)}
                >
                  {stopping ? (
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
    </div>
  );
}
