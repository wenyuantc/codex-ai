import { Copy, FileText, Loader2, Play, ScrollText, Wrench } from "lucide-react";
import { useTranslation } from "react-i18next";

import type {
  CodexSessionFileChange,
  TaskExecutionChangeHistoryItem,
  TaskLatestReview,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import { CodexTerminal } from "@/components/codex/CodexTerminal";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useSessionLogAwaitFollowups } from "@/hooks/useSessionLogAwaitFollowups";
import { formatDate } from "@/lib/utils";
import { useEmployeeStore } from "@/stores/employeeStore";
import { DetailSection, DetailStat } from "./DetailSection";
import { TaskFileChangeHistoryPanel } from "./TaskFileChangeHistoryPanel";
import { getSessionStatusLabel } from "./taskDetailViewHelpers";

interface TaskReviewPanelProps {
  taskId: string;
  status: string;
  reviewerId: string;
  reviewerName?: string;
  isReviewActive: boolean;
  reviewLoading: boolean;
  reviewError: string | null;
  reviewNotice: string | null;
  latestReview: TaskLatestReview | null;
  latestReviewLoading: boolean;
  hasReviewOutput: boolean;
  assigneeId: string;
  reviewFixSubmitting: boolean;
  onStartReview: () => void;
  onRefreshReview: () => void;
  onCopyReview: () => void;
  onOpenReviewFix: () => void;
  executionChangeHistory: TaskExecutionChangeHistoryItem[];
  executionChangeHistoryLoading: boolean;
  executionChangeHistoryError: string | null;
  onRefreshHistory: () => void;
  onOpenChangeDetail: (change: CodexSessionFileChange) => void;
}

export function TaskReviewPanel({
  taskId,
  status,
  reviewerId,
  reviewerName,
  isReviewActive,
  reviewLoading,
  reviewError,
  reviewNotice,
  latestReview,
  latestReviewLoading,
  hasReviewOutput,
  assigneeId,
  reviewFixSubmitting,
  onStartReview,
  onRefreshReview,
  onCopyReview,
  onOpenReviewFix,
  executionChangeHistory,
  executionChangeHistoryLoading,
  executionChangeHistoryError,
  onRefreshHistory,
  onOpenChangeDetail,
}: TaskReviewPanelProps) {
  const { t } = useTranslation("tasks");
  const employees = useEmployeeStore((s) => s.employees);
  const employeeRuntime = useEmployeeStore((s) => s.employeeRuntime);
  const reviewer = employees.find((item) => item.id === reviewerId);
  const reviewLive = Boolean(
    employeeRuntime[reviewerId]?.sessions?.some(
      (session) => session.task_id === taskId && session.session_kind === "review",
    ),
  );
  const reviewProvider =
    employeeRuntime[reviewerId]?.sessions?.find(
      (session) => session.task_id === taskId && session.session_kind === "review",
    )?.ai_provider ??
    reviewer?.ai_provider ??
    null;
  const reviewTerminalLive = reviewLive || isReviewActive;
  useSessionLogAwaitFollowups(
    Boolean(reviewerId) && (isReviewActive || hasReviewOutput),
    reviewerId,
    reviewProvider,
    reviewTerminalLive,
  );
  return (
    <div className="space-y-4">
      <DetailSection
        icon={ScrollText}
        title={t("detail.review.title")}
        description={t("detail.review.description")}
        actions={
          <Button
            type="button"
            size="sm"
            onClick={onStartReview}
            disabled={reviewLoading || isReviewActive || status !== "review" || !reviewerId}
            className="bg-amber-500 text-black hover:bg-amber-400"
            title={
              isReviewActive
                ? t("detail.review.titleInProgress")
                : status !== "review"
                  ? t("detail.review.titleWrongStatus")
                  : !reviewerId
                    ? t("detail.review.titleNoReviewer")
                    : t("detail.review.titleStart")
            }
          >
            {reviewLoading || isReviewActive ? <Loader2 className="animate-spin" /> : <Play />}
            {isReviewActive ? t("detail.review.inProgress") : t("detail.review.start")}
          </Button>
        }
      >
        <div className="space-y-3">
          {reviewError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {reviewError}
            </div>
          )}

          {reviewNotice && (
            <div className="rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-700 dark:text-green-300">
              {reviewNotice}
            </div>
          )}

          <div className="grid gap-2 md:grid-cols-3">
            <DetailStat
              label={t("detail.review.statReviewer")}
              value={reviewerName ?? t("detail.review.none")}
            />
            <DetailStat
              label={t("detail.review.statStatus")}
              value={getSessionStatusLabel(
                isReviewActive ? "running" : latestReview?.session.status,
              )}
            />
            <DetailStat
              label={t("detail.review.statLatestSession")}
              value={
                <span className="font-mono">
                  {latestReview?.session.cli_session_id ??
                    latestReview?.session.id ??
                    t("detail.review.none")}
                </span>
              }
            />
          </div>

          {(isReviewActive || hasReviewOutput) && reviewerId && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <p className="text-xs font-medium text-muted-foreground">
                  {t("detail.review.sessionLog")}
                </p>
                <span className="text-[11px] text-muted-foreground">
                  {isReviewActive ? t("detail.review.running") : t("detail.review.latestOutput")}
                </span>
              </div>
              <CodexTerminal
                taskId={taskId}
                sessionKind="review"
                showInputBar
                inputEmployeeId={reviewerId}
                inputProvider={reviewProvider}
                inputSessionLive={reviewLive || isReviewActive}
              />
            </div>
          )}
        </div>
      </DetailSection>

      <DetailSection
        icon={FileText}
        title={t("detail.review.resultTitle")}
        description={t("detail.review.resultDesc")}
        actions={
          <>
            {latestReviewLoading ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
            ) : (
              <button
                type="button"
                onClick={onRefreshReview}
                className="cursor-pointer text-[11px] text-primary hover:underline"
              >
                {t("detail.labels.refresh")}
              </button>
            )}
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={onCopyReview}
              disabled={!latestReview?.report?.trim() || reviewLoading || isReviewActive}
            >
              <Copy />
              {t("detail.review.copy")}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="xs"
              onClick={onOpenReviewFix}
              disabled={
                !latestReview?.report?.trim() ||
                !assigneeId ||
                reviewFixSubmitting ||
                reviewLoading ||
                isReviewActive
              }
              title={
                !assigneeId ? t("detail.review.noAssigneeTitle") : t("detail.review.createFixTitle")
              }
            >
              <Wrench />
              {t("detail.review.createFixTask")}
            </Button>
          </>
        }
      >
        <div className="space-y-2">
          {latestReview?.report ? (
            <ScrollArea className="h-72 overflow-hidden rounded-md border border-border/60 bg-background/80">
              <div className="whitespace-pre-wrap p-3 text-xs text-foreground">
                {latestReview.report}
              </div>
            </ScrollArea>
          ) : (
            <div className="rounded-md border border-dashed border-border/60 bg-background/70 px-3 py-6 text-center text-xs text-muted-foreground">
              {latestReview ? t("detail.review.noStructuredReport") : t("detail.review.noResult")}
            </div>
          )}

          {latestReview && (
            <div className="text-[11px] text-muted-foreground">
              {latestReview.reviewer_name ?? t("detail.review.unknownReviewer")} ·{" "}
              {formatDate(latestReview.session.started_at)}
            </div>
          )}
        </div>
      </DetailSection>

      <TaskFileChangeHistoryPanel
        title={t("detail.review.fileChangesTitle")}
        description={t("detail.review.fileChangesDesc")}
        history={executionChangeHistory}
        loading={executionChangeHistoryLoading}
        error={executionChangeHistoryError}
        emptyText={t("detail.review.fileChangesEmpty")}
        onRefresh={onRefreshHistory}
        onOpenChangeDetail={onOpenChangeDetail}
      />
    </div>
  );
}
