import type { RefObject } from "react";
import { Eraser, Loader2, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getAiLogColor } from "./taskDetailViewHelpers";

interface TaskAiPanelProps {
  aiActionDisabled: boolean;
  aiLoading: string | null;
  planLoading: boolean;
  aiLogs: string[];
  aiLogRef: RefObject<HTMLDivElement | null>;
  aiResult: string | null;
  taskAiSuggestion: string | null;
  planError: string | null;
  planNotice: string | null;
  generatedPlan: string | null;
  insertSubmitting: boolean;
  onSuggest: () => void;
  onComplexity: () => void;
  onSplitSubtasks: () => void;
  onGeneratePlan: () => void;
  onGenerateComment: () => void;
  onClearLogs: () => void;
  onInsertPlan: () => void;
}

interface AiAction {
  key: string;
  label: string;
  loading: boolean;
  onClick: () => void;
}

export function TaskAiPanel({
  aiActionDisabled,
  aiLoading,
  planLoading,
  aiLogs,
  aiLogRef,
  aiResult,
  taskAiSuggestion,
  planError,
  planNotice,
  generatedPlan,
  insertSubmitting,
  onSuggest,
  onComplexity,
  onSplitSubtasks,
  onGeneratePlan,
  onGenerateComment,
  onClearLogs,
  onInsertPlan,
}: TaskAiPanelProps) {
  const actions: AiAction[] = [
    { key: "assignee", label: "AI建议指派", loading: aiLoading === "assignee", onClick: onSuggest },
    {
      key: "complexity",
      label: "复杂度分析",
      loading: aiLoading === "complexity",
      onClick: onComplexity,
    },
    {
      key: "subtasks",
      label: "AI拆分子任务",
      loading: aiLoading === "subtasks",
      onClick: onSplitSubtasks,
    },
    { key: "plan", label: "AI生成计划", loading: planLoading, onClick: onGeneratePlan },
    {
      key: "comment",
      label: "AI生成评论",
      loading: aiLoading === "comment",
      onClick: onGenerateComment,
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        {actions.map((action) => (
          <Button
            key={action.key}
            type="button"
            variant="outline"
            size="sm"
            onClick={action.onClick}
            disabled={aiActionDisabled}
          >
            {action.loading ? <Loader2 className="animate-spin" /> : <Sparkles />}
            {action.label}
          </Button>
        ))}
      </div>

      {(aiLoading !== null || planLoading || aiLogs.length > 0) && (
        <div className="overflow-hidden rounded-lg border border-zinc-800">
          <div className="flex items-center justify-between border-b border-zinc-800 bg-zinc-900 px-3 py-1.5">
            <span className="flex items-center gap-1.5 font-mono text-xs text-zinc-400">
              {(aiLoading !== null || planLoading) && (
                <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-green-500" />
              )}
              AI 执行日志
            </span>
            <button
              onClick={onClearLogs}
              className="cursor-pointer p-0.5 text-zinc-500 transition-colors hover:text-zinc-300"
              title="清空日志"
            >
              <Eraser className="h-3 w-3" />
            </button>
          </div>
          <ScrollArea className="h-40 overflow-hidden bg-black">
            <div className="space-y-0.5 p-2 font-mono text-xs">
              {aiLogs.length === 0 ? (
                <div className="text-zinc-600">等待执行...</div>
              ) : (
                aiLogs.map((line, index) => (
                  <div
                    key={`${line}-${index}`}
                    className={`whitespace-pre-wrap ${getAiLogColor(line)}`}
                  >
                    {line}
                  </div>
                ))
              )}
              <div ref={aiLogRef} />
            </div>
          </ScrollArea>
        </div>
      )}

      {aiResult && (
        <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">
          <span className="font-medium text-primary">AI 结果：</span>
          {aiResult}
        </div>
      )}

      {taskAiSuggestion && !aiResult && (
        <div className="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">
          <span className="font-medium text-primary">AI 建议：</span>
          {taskAiSuggestion}
        </div>
      )}

      {planError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {planError}
        </div>
      )}

      {planNotice && (
        <div className="rounded-lg border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-700 dark:text-green-300">
          {planNotice}
        </div>
      )}

      {generatedPlan && (
        <div className="space-y-3 rounded-lg border border-primary/20 bg-primary/5 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="space-y-1">
              <p className="text-xs font-medium text-primary">AI 计划预览</p>
              <p className="text-[11px] text-muted-foreground">确认后可插入到任务详情描述中</p>
            </div>
            <Button type="button" size="sm" onClick={onInsertPlan} disabled={insertSubmitting}>
              {insertSubmitting ? <Loader2 className="animate-spin" /> : null}
              插入详情
            </Button>
          </div>
          <ScrollArea className="h-80 overflow-hidden rounded-md border border-border/60 bg-background/80">
            <div className="whitespace-pre-wrap p-3 text-xs text-foreground">{generatedPlan}</div>
          </ScrollArea>
        </div>
      )}
    </div>
  );
}
