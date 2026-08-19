import type { MouseEvent, ReactNode } from "react";
import {
  AlertTriangle,
  ClipboardCheck,
  GitBranch,
  Loader2,
  Play,
  ScrollText,
  Square,
} from "lucide-react";

import { SshArtifactLimitedNotice } from "@/components/sessions/SshArtifactLimitedNotice";
import type { ArtifactCaptureMode } from "@/lib/types";
import type { TaskPrimaryCta } from "@/lib/taskPrimaryCta";

export function taskCardPrimaryCtaButtonClass(tone: TaskPrimaryCta["tone"]): string {
  switch (tone) {
    case "danger":
      return "flex items-center gap-1 px-2 py-0.5 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50";
    case "warning":
      return "flex items-center gap-1 px-2 py-0.5 text-xs bg-amber-500 text-black rounded hover:bg-amber-400 transition-colors disabled:opacity-50";
    case "muted":
      return "flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded opacity-50";
    case "primary":
    default:
      return "flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50";
  }
}

interface TaskCardPrimaryCtaIconParams {
  primaryCta: TaskPrimaryCta;
  stopLoading?: boolean;
  reviewLoading?: boolean;
  reviewRunning?: boolean;
  commitLoading?: boolean;
  acceptanceLoading?: boolean;
  runLoading?: boolean;
}

export function renderTaskCardPrimaryCtaIcon({
  primaryCta,
  stopLoading,
  reviewLoading,
  reviewRunning,
  commitLoading,
  acceptanceLoading,
  runLoading,
}: TaskCardPrimaryCtaIconParams): ReactNode {
  switch (primaryCta.kind) {
    case "stop":
      return stopLoading ? (
        <Square className="h-3 w-3" />
      ) : (
        <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-white" />
      );
    case "starting":
    case "running_locked":
    case "queued":
      return <Loader2 className="h-3 w-3 animate-spin" />;
    case "review":
      return reviewLoading || reviewRunning ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : (
        <ScrollText className="h-3 w-3" />
      );
    case "blocked":
      return <AlertTriangle className="h-3 w-3" />;
    case "commit":
      return commitLoading ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : (
        <GitBranch className="h-3 w-3" />
      );
    case "acceptance":
      return acceptanceLoading ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : (
        <ClipboardCheck className="h-3 w-3" />
      );
    case "run":
    default:
      return runLoading ? (
        <Loader2 className="h-3 w-3 animate-spin" />
      ) : (
        <Play className="h-3 w-3" />
      );
  }
}

interface TaskCardPrimaryCtaStripProps {
  primaryCta: TaskPrimaryCta;
  taskStatus: string;
  hasAssignee: boolean;
  hasReviewer: boolean;
  reviewerName?: string | null;
  backgroundRunLabel?: string | null;
  isActionLoading: boolean;
  environmentMode: "local" | "ssh";
  sshReviewEvidenceLimited: boolean;
  lastArtifactCaptureMode: ArtifactCaptureMode | null;
  icon: ReactNode;
  buttonClassName: string;
  onPrimaryClick: (event?: MouseEvent) => void;
}

export function TaskCardPrimaryCtaStrip({
  primaryCta,
  taskStatus,
  hasAssignee,
  hasReviewer,
  reviewerName,
  backgroundRunLabel,
  isActionLoading,
  environmentMode,
  sshReviewEvidenceLimited,
  lastArtifactCaptureMode,
  icon,
  buttonClassName,
  onPrimaryClick,
}: TaskCardPrimaryCtaStripProps) {
  return (
    <div className="mt-2 space-y-1.5 border-t border-border/50 pt-2">
      {environmentMode === "ssh" &&
        (sshReviewEvidenceLimited || lastArtifactCaptureMode) &&
        (primaryCta.kind === "review" || taskStatus === "review") && (
          <SshArtifactLimitedNotice
            artifactCaptureMode={lastArtifactCaptureMode}
            force={sshReviewEvidenceLimited && !lastArtifactCaptureMode}
            compact
          />
        )}
      <div className="flex items-center gap-1">
        {primaryCta.kind === "run" && !hasAssignee ? (
          <span
            className="text-xs text-muted-foreground/50"
            title={primaryCta.reason ?? "请先指派员工"}
          >
            <Play className="mr-0.5 inline h-3 w-3" />
            未指派
          </span>
        ) : primaryCta.kind === "review" && !hasReviewer ? (
          <span
            className="text-xs text-muted-foreground/50"
            title={primaryCta.reason ?? "请先指定审查员"}
          >
            <ScrollText className="mr-0.5 inline h-3 w-3" />
            未指定审查员
          </span>
        ) : primaryCta.kind === "starting" ? (
          <button
            type="button"
            disabled
            className="flex items-center gap-1 rounded bg-violet-600 px-2 py-0.5 text-xs text-white opacity-90"
            title={backgroundRunLabel ?? primaryCta.reason ?? "后台启动中"}
          >
            <Loader2 className="h-3 w-3 animate-spin" />
            {primaryCta.label}
          </button>
        ) : (
          <button
            type="button"
            onClick={(event) => onPrimaryClick(event)}
            disabled={primaryCta.disabled || isActionLoading}
            title={
              primaryCta.kind === "review" && hasReviewer
                ? `由 ${reviewerName ?? "审查员"} 发起代码审核`
                : (primaryCta.reason ?? primaryCta.label)
            }
            className={buttonClassName}
          >
            {icon}
            {primaryCta.label}
          </button>
        )}
      </div>
    </div>
  );
}
