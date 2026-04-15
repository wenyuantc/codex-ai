import { Copy, Loader2, Play, Wrench } from "lucide-react";

import type { TaskLatestReview } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { CodexTerminal } from "@/components/codex/CodexTerminal";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getSessionStatusLabel } from "./taskDetailViewHelpers";

interface TaskReviewPanelProps {
  taskId: string;
  status: string;
  reviewerId: string;
  reviewerName?: string;
  isReviewRunning: boolean;
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
}

export function TaskReviewPanel({
  taskId,
  status,
  reviewerId,
  reviewerName,
  isReviewRunning,
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
}: TaskReviewPanelProps) {
  return (
    <div className="space-y-3 rounded-md border border-border/70 bg-muted/20 p-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="space-y-1">
          <p className="text-sm font-medium">代码审核</p>
          <p className="text-[11px] text-muted-foreground">
            审核当前项目仓库的工作区改动，默认由任务审查员发起只读 reviewer 会话。
          </p>
        </div>
        <button
          type="button"
          onClick={onStartReview}
          disabled={reviewLoading || status !== "review" || !reviewerId}
          className="flex items-center gap-1 rounded-md bg-amber-500 px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-amber-400 disabled:opacity-50"
          title={
            status !== "review"
              ? "仅“审核中”任务支持代码审核"
              : !reviewerId
                ? "请先指定审查员"
                : "启动代码审核"
          }
        >
          {reviewLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
          审核代码
        </button>
      </div>

      {reviewError && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {reviewError}
        </div>
      )}

      {reviewNotice && (
        <div className="rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-700">
          {reviewNotice}
        </div>
      )}

      <div className="grid gap-2 text-xs text-muted-foreground md:grid-cols-3">
        <div className="rounded-md border border-border bg-background/70 px-3 py-2">
          <span className="font-medium text-foreground">审查员：</span>
          {reviewerName ?? "未指定"}
        </div>
        <div className="rounded-md border border-border bg-background/70 px-3 py-2">
          <span className="font-medium text-foreground">最近状态：</span>
          {getSessionStatusLabel(isReviewRunning ? "running" : latestReview?.session.status)}
        </div>
        <div className="rounded-md border border-border bg-background/70 px-3 py-2">
          <span className="font-medium text-foreground">最近会话：</span>
          {latestReview?.session.cli_session_id ?? latestReview?.session.id ?? "暂无"}
        </div>
      </div>

      {status !== "review" && (
        <div className="rounded-md border border-border bg-background/70 px-3 py-2 text-xs text-muted-foreground">
          当前任务状态不是“审核中”，代码审核入口已禁用。
        </div>
      )}

      {(isReviewRunning || hasReviewOutput) && reviewerId && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <p className="text-xs font-medium text-muted-foreground">审核会话日志</p>
            <span className="text-[11px] text-muted-foreground">
              {isReviewRunning ? "运行中" : "最近一次审核输出"}
            </span>
          </div>
          <CodexTerminal taskId={taskId} sessionKind="review" />
        </div>
      )}

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <p className="text-xs font-medium text-muted-foreground">审核结果</p>
          <div className="flex items-center gap-2">
            {latestReviewLoading ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
            ) : (
              <button
                type="button"
                onClick={onRefreshReview}
                className="text-[11px] text-primary hover:underline"
              >
                刷新
              </button>
            )}
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={onCopyReview}
              disabled={!latestReview?.report?.trim()}
            >
              <Copy className="h-3 w-3" />
              复制
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="xs"
              onClick={onOpenReviewFix}
              disabled={!latestReview?.report?.trim() || !assigneeId || reviewFixSubmitting}
              title={!assigneeId ? "原任务未指派开发负责人" : "创建修复任务并立即运行"}
            >
              <Wrench className="h-3 w-3" />
              修复
            </Button>
          </div>
        </div>

        {latestReview?.report ? (
          <ScrollArea className="h-72 overflow-hidden rounded-md border bg-background/80">
            <div className="p-3 text-xs whitespace-pre-wrap text-foreground">
              {latestReview.report}
            </div>
          </ScrollArea>
        ) : (
          <div className="rounded-md border border-dashed border-border bg-background/70 px-3 py-6 text-center text-xs text-muted-foreground">
            {latestReview ? "最近一次审核尚未产出结构化报告。" : "还没有代码审核结果。"}
          </div>
        )}

        {latestReview && (
          <div className="text-[11px] text-muted-foreground">
            {latestReview.reviewer_name ?? "未知审查员"} ·{" "}
            {new Date(latestReview.session.started_at).toLocaleString("zh-CN")}
          </div>
        )}
      </div>
    </div>
  );
}
