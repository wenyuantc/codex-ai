import type { RefObject } from "react";
import { Eraser, Loader2, Play, Square } from "lucide-react";

import type { CodexSessionFileChange, TaskExecutionChangeHistoryItem } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getLineColor } from "./taskDetailViewHelpers";
import { TaskFileChangeHistoryPanel } from "./TaskFileChangeHistoryPanel";

interface TaskExecutionPanelProps {
  taskStatus: string;
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
  taskStatus,
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
  const isArchivedTask = taskStatus === "archived";

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        {isArchivedTask ? (
          <span className="text-xs text-muted-foreground">已归档任务不可运行 Codex</span>
        ) : assigneeId ? (
          isRunning ? (
            <Button
              size="sm"
              onClick={onStop}
              disabled={codexLoading}
              className="bg-red-600 text-white hover:bg-red-700"
            >
              {codexLoading ? <Loader2 className="animate-spin" /> : <Square />}
              停止运行
            </Button>
          ) : isExecutionActive ? (
            <Button
              size="sm"
              disabled
              className="bg-green-600 text-white opacity-60"
              title="自动修复正在启动或运行中"
            >
              <Loader2 className="animate-spin" />
              运行中
            </Button>
          ) : (
            <Button
              size="sm"
              onClick={onRun}
              disabled={codexLoading}
              className="bg-green-600 text-white hover:bg-green-700"
            >
              {codexLoading ? <Loader2 className="animate-spin" /> : <Play />}
              运行
            </Button>
          )
        ) : (
          <span className="text-xs text-muted-foreground">请先指派员工以运行 Codex</span>
        )}
        {isExecutionActive && (
          <span className="flex items-center gap-1.5 text-xs text-green-500">
            <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-green-500" />
            运行中
          </span>
        )}
      </div>

      {(isExecutionActive || output.length > 0) && assigneeId && (
        <div className="overflow-hidden rounded-lg border border-zinc-800">
          <div className="flex items-center justify-between border-b border-zinc-800 bg-zinc-900 px-3 py-1.5">
            <span className="flex items-center gap-1.5 font-mono text-xs text-zinc-400">
              {isExecutionActive && (
                <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-green-500" />
              )}
              Codex 终端
            </span>
            <button
              onClick={onClearOutput}
              className="cursor-pointer p-0.5 text-zinc-500 transition-colors hover:text-zinc-300"
              title="清空日志"
            >
              <Eraser className="h-3 w-3" />
            </button>
          </div>
          <ScrollArea className="h-72 bg-black">
            <div className="space-y-0.5 p-2 font-mono text-xs">
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
        title="改动文件"
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
