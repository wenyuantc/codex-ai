import type { RefObject } from "react";
import { Eraser, Loader2, Play, Square } from "lucide-react";

import type { CodexSessionFileChange, TaskExecutionChangeHistoryItem } from "@/lib/types";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  getLineColor,
} from "./taskDetailViewHelpers";
import { TaskFileChangeHistoryPanel } from "./TaskFileChangeHistoryPanel";

interface TaskExecutionPanelProps {
  assigneeId: string;
  isRunning: boolean;
  isExecutionActive: boolean;
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
  isExecutionActive,
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
          ) : isExecutionActive ? (
            <button
              disabled
              className="flex items-center gap-1 px-2 py-1 text-xs bg-green-600 text-white rounded opacity-50"
              title="自动修复正在启动或运行中"
            >
              <Loader2 className="h-3 w-3 animate-spin" />
              运行中
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
        {isExecutionActive && (
          <span className="flex items-center gap-1 text-xs text-green-500">
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
            运行中
          </span>
        )}
      </div>

      {(isExecutionActive || output.length > 0) && assigneeId && (
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

      <TaskFileChangeHistoryPanel
        title="Codex 改动文件"
        description="SDK 会话按 Codex 事件精确记录；CLI 会话仅在无法获取结构化事件时回退为 Git 快照估算。"
        history={executionChangeHistory}
        loading={executionChangeHistoryLoading}
        error={executionChangeHistoryError}
        emptyText="还没有 execution 会话的文件记录。"
        onRefresh={onRefreshHistory}
        onOpenChangeDetail={onOpenChangeDetail}
      />
    </div>
  );
}
