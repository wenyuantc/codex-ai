import type { RefObject } from "react";
import { Eraser, Loader2, Sparkles } from "lucide-react";

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
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <button
          onClick={onSuggest}
          disabled={aiActionDisabled}
          className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
        >
          {aiLoading === "assignee" ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Sparkles className="h-3 w-3" />
          )}
          AI建议指派
        </button>
        <button
          onClick={onComplexity}
          disabled={aiActionDisabled}
          className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
        >
          {aiLoading === "complexity" ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Sparkles className="h-3 w-3" />
          )}
          复杂度分析
        </button>
        <button
          onClick={onSplitSubtasks}
          disabled={aiActionDisabled}
          className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
        >
          {aiLoading === "subtasks" ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Sparkles className="h-3 w-3" />
          )}
          AI拆分子任务
        </button>
        <button
          onClick={onGeneratePlan}
          disabled={aiActionDisabled}
          className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
        >
          {planLoading ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Sparkles className="h-3 w-3" />
          )}
          AI生成计划
        </button>
        <button
          onClick={onGenerateComment}
          disabled={aiActionDisabled}
          className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
        >
          {aiLoading === "comment" ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Sparkles className="h-3 w-3" />
          )}
          AI生成评论
        </button>
      </div>

      {(aiLoading !== null || planLoading || aiLogs.length > 0) && (
        <div>
          <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
            <span className="text-xs text-zinc-500 font-mono">AI 执行日志</span>
            <button
              onClick={onClearLogs}
              className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
              title="清空日志"
            >
              <Eraser className="h-3 w-3" />
            </button>
          </div>
          <ScrollArea className="h-40 overflow-hidden bg-black rounded-b">
            <div className="p-2 font-mono text-xs space-y-0.5">
              {aiLogs.length === 0 ? (
                <div className="text-zinc-600">等待执行...</div>
              ) : (
                aiLogs.map((line, index) => (
                  <div key={`${line}-${index}`} className={`whitespace-pre-wrap ${getAiLogColor(line)}`}>
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
        <div className="bg-primary/5 rounded-md p-3 text-xs text-muted-foreground">
          <span className="font-medium text-primary">AI 结果: </span>
          {aiResult}
        </div>
      )}

      {taskAiSuggestion && !aiResult && (
        <div className="bg-primary/5 rounded-md p-3 text-xs text-muted-foreground">
          <span className="font-medium text-primary">AI 建议: </span>
          {taskAiSuggestion}
        </div>
      )}

      {planError && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {planError}
        </div>
      )}

      {planNotice && (
        <div className="rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-700">
          {planNotice}
        </div>
      )}

      {generatedPlan && (
        <div className="space-y-3 rounded-md border border-primary/20 bg-primary/5 p-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p className="text-xs font-medium text-primary">AI 计划预览</p>
              <p className="text-[11px] text-muted-foreground">确认后可插入到任务详情描述中</p>
            </div>
            <button
              onClick={onInsertPlan}
              disabled={insertSubmitting}
              className="flex items-center gap-1 rounded px-2 py-1 text-xs bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              {insertSubmitting ? <Loader2 className="h-3 w-3 animate-spin" /> : null}
              插入详情
            </button>
          </div>
          <ScrollArea className="h-80 overflow-hidden rounded-md border bg-background/80">
            <div className="p-3 text-xs text-foreground whitespace-pre-wrap">
              {generatedPlan}
            </div>
          </ScrollArea>
        </div>
      )}
    </div>
  );
}
