import type { RefObject } from "react";
import { Eraser, Loader2, Play, Square } from "lucide-react";

import type { CodexSessionFileChange, TaskExecutionChangeHistoryItem } from "@/lib/types";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDate } from "@/lib/utils";
import {
  getExecutionChangeCaptureModeDescription,
  getExecutionChangeCaptureModeLabel,
  getExecutionChangeTypeClassName,
  getExecutionChangeTypeLabel,
  getLineColor,
  getSessionStatusLabel,
} from "./taskDetailViewHelpers";

interface TaskExecutionPanelProps {
  assigneeId: string;
  isRunning: boolean;
  codexLoading: boolean;
  output: string[];
  terminalRef: RefObject<HTMLDivElement | null>;
  executionChangeHistory: TaskExecutionChangeHistoryItem[];
  executionChangeHistoryLoading: boolean;
  executionChangeHistoryError: string | null;
  onRun: () => void;
  onStop: () => void;
  onClearOutput: () => void;
  onRefreshHistory: () => void;
  onOpenChangeDetail: (change: CodexSessionFileChange) => void;
}

export function TaskExecutionPanel({
  assigneeId,
  isRunning,
  codexLoading,
  output,
  terminalRef,
  executionChangeHistory,
  executionChangeHistoryLoading,
  executionChangeHistoryError,
  onRun,
  onStop,
  onClearOutput,
  onRefreshHistory,
  onOpenChangeDetail,
}: TaskExecutionPanelProps) {
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        {assigneeId ? (
          isRunning ? (
            <button
              onClick={onStop}
              disabled={codexLoading}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50"
            >
              {codexLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Square className="h-3 w-3" />}
              停止运行
            </button>
          ) : (
            <button
              onClick={onRun}
              disabled={codexLoading}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50"
            >
              {codexLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />}
              运行 Codex
            </button>
          )
        ) : (
          <span className="text-xs text-muted-foreground">请先指派员工以运行 Codex</span>
        )}
        {isRunning && (
          <span className="flex items-center gap-1 text-xs text-green-500">
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
            运行中
          </span>
        )}
      </div>

      {(isRunning || output.length > 0) && assigneeId && (
        <div>
          <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
            <span className="text-xs text-zinc-500 font-mono">Codex 终端</span>
            <button
              onClick={onClearOutput}
              className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
              title="清空日志"
            >
              <Eraser className="h-3 w-3" />
            </button>
          </div>
          <ScrollArea className="h-64 bg-black rounded-b">
            <div className="p-2 font-mono text-xs space-y-0.5">
              {output.length === 0 ? (
                <div className="text-zinc-600">等待输出...</div>
              ) : (
                output.map((line, i) => (
                  <div key={`${line}-${i}`} className={`whitespace-pre-wrap ${getLineColor(line)}`}>
                    {line}
                  </div>
                ))
              )}
              <div ref={terminalRef} />
            </div>
          </ScrollArea>
        </div>
      )}

      <div className="space-y-3 rounded-md border border-border/70 bg-muted/20 p-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <p className="text-sm font-medium">Codex 改动文件</p>
            <p className="text-[11px] text-muted-foreground">
              SDK 会话按 Codex 事件精确记录；CLI 会话仅在无法获取结构化事件时回退为 Git 快照估算。
            </p>
          </div>
          <button
            type="button"
            onClick={onRefreshHistory}
            className="text-[11px] text-primary hover:underline disabled:opacity-50"
            disabled={executionChangeHistoryLoading}
          >
            {executionChangeHistoryLoading ? "刷新中..." : "刷新"}
          </button>
        </div>

        {executionChangeHistoryError && (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {executionChangeHistoryError}
          </div>
        )}

        {executionChangeHistory.length > 0 ? (
          <div className="space-y-3">
            {executionChangeHistory.map((item) => (
              <div key={item.session.id} className="rounded-md border border-border bg-background/70 p-3">
                <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-muted-foreground">
                  <span>
                    {formatDate(item.session.started_at)}
                    {" · "}
                    {getSessionStatusLabel(item.session.status)}
                  </span>
                  <span className="font-mono">
                    {item.session.cli_session_id ?? item.session.id}
                  </span>
                </div>
                <div className="mt-2 rounded-md border border-border/60 bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground">
                  <span className="font-medium text-foreground">
                    {getExecutionChangeCaptureModeLabel(item.capture_mode)}
                  </span>
                  {" · "}
                  {getExecutionChangeCaptureModeDescription(item.capture_mode)}
                </div>

                {item.changes.length > 0 ? (
                  <div className="mt-3 space-y-2">
                    {item.changes.map((change) => (
                      <button
                        type="button"
                        key={change.id}
                        onClick={() => onOpenChangeDetail(change)}
                        className="flex w-full flex-col gap-1 rounded-md border border-border/60 bg-background px-3 py-2 text-left text-xs transition-colors hover:border-primary/40 hover:bg-muted/30"
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <span
                            className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getExecutionChangeTypeClassName(change.change_type)}`}
                          >
                            {getExecutionChangeTypeLabel(change.change_type)}
                          </span>
                          <span className="font-mono text-foreground break-all">{change.path}</span>
                        </div>
                        {change.previous_path && (
                          <div className="text-[11px] text-muted-foreground">
                            原路径：<span className="font-mono break-all">{change.previous_path}</span>
                          </div>
                        )}
                        <div className="text-[11px] text-primary">
                          点击查看该次会话保存的 diff / 内容快照
                        </div>
                      </button>
                    ))}
                  </div>
                ) : (
                  <div className="mt-3 rounded-md border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
                    {item.capture_mode === "sdk_event"
                      ? "本次运行没有结构化文件变更记录。"
                      : "本次 Git 快照估算未发现新增文件变更。"}
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : executionChangeHistoryLoading ? (
          <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
            正在加载修改文件历史...
          </div>
        ) : (
          <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
            还没有 execution 会话的文件记录。
          </div>
        )}
      </div>
    </div>
  );
}
