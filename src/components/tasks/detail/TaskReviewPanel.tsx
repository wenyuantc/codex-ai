import { Copy, FileText, Loader2, Play, ScrollText, Wrench } from "lucide-react";

import type {
  CodexSessionFileChange,
  TaskExecutionChangeHistoryItem,
  TaskLatestReview,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import { CodexTerminal } from "@/components/codex/CodexTerminal";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDate } from "@/lib/utils";
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
  return (
    <div className="space-y-4">
      <DetailSection
        icon={ScrollText}
        title="代码审核"
        description="发起审查员 reviewer 会话，结合下方 Codex 改动文件记录逐个查看代码 diff。"
        actions={
          <Button
            type="button"
            size="sm"
            onClick={onStartReview}
            disabled={reviewLoading || isReviewActive || status !== "review" || !reviewerId}
            className="bg-amber-500 text-black hover:bg-amber-400"
            title={
              isReviewActive
                ? "审核进行中"
                : status !== "review"
                  ? "仅“审核中”任务支持代码审核"
                  : !reviewerId
                    ? "请先指定审查员"
                    : "启动代码审核"
            }
          >
            {reviewLoading || isReviewActive ? <Loader2 className="animate-spin" /> : <Play />}
            {isReviewActive ? "审核中" : "审核代码"}
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
            <DetailStat label="审查员" value={reviewerName ?? "未指定"} />
            <DetailStat
              label="当前状态"
              value={getSessionStatusLabel(
                isReviewActive ? "running" : latestReview?.session.status,
              )}
            />
            <DetailStat
              label="最近会话"
              value={
                <span className="font-mono">
                  {latestReview?.session.cli_session_id ?? latestReview?.session.id ?? "暂无"}
                </span>
              }
            />
          </div>

          {(isReviewActive || hasReviewOutput) && reviewerId && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <p className="text-xs font-medium text-muted-foreground">审核会话日志</p>
                <span className="text-[11px] text-muted-foreground">
                  {isReviewActive ? "运行中" : "最近一次审核输出"}
                </span>
              </div>
              <CodexTerminal taskId={taskId} sessionKind="review" />
            </div>
          )}
        </div>
      </DetailSection>

      <DetailSection
        icon={FileText}
        title="审核结果"
        description="手动修复基于审核结果新建任务；自动质控则在原任务内闭环。"
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
                刷新
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
              复制
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
                !assigneeId ? "原任务未指派开发负责人" : "手动修复会新建一个修复任务并立即运行"
              }
            >
              <Wrench />
              新建修复任务
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
              {latestReview ? "最近一次审核尚未产出结构化报告。" : "还没有代码审核结果。"}
            </div>
          )}

          {latestReview && (
            <div className="text-[11px] text-muted-foreground">
              {latestReview.reviewer_name ?? "未知审查员"} ·{" "}
              {formatDate(latestReview.session.started_at)}
            </div>
          )}
        </div>
      </DetailSection>

      <TaskFileChangeHistoryPanel
        title="Codex 改动文件审核"
        description="这里按任务执行会话汇总 Codex 改动过的文件记录，点击文件可查看对应的代码 diff 与快照。"
        history={executionChangeHistory}
        loading={executionChangeHistoryLoading}
        error={executionChangeHistoryError}
        emptyText="还没有可审核的 Codex 改动文件记录。"
        onRefresh={onRefreshHistory}
        onOpenChangeDetail={onOpenChangeDetail}
      />
    </div>
  );
}
