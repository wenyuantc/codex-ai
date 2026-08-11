import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation("tasks");
  const isArchivedTask = taskStatus === "archived";

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        {isArchivedTask ? (
          <span className="text-xs text-muted-foreground">
            {t("detail.execution.archivedCannotRun")}
          </span>
        ) : assigneeId ? (
          isRunning ? (
            <Button
              size="sm"
              onClick={onStop}
              disabled={codexLoading}
              className="bg-red-600 text-white hover:bg-red-700"
            >
              {codexLoading ? <Loader2 className="animate-spin" /> : <Square />}
              {t("detail.execution.stop")}
            </Button>
          ) : isExecutionActive ? (
            <Button
              size="sm"
              disabled
              className="bg-green-600 text-white opacity-60"
              title={t("detail.execution.autoFixRunningTitle")}
            >
              <Loader2 className="animate-spin" />
              {t("detail.execution.running")}
            </Button>
          ) : (
            <Button
              size="sm"
              onClick={onRun}
              disabled={codexLoading}
              className="bg-green-600 text-white hover:bg-green-700"
            >
              {codexLoading ? <Loader2 className="animate-spin" /> : <Play />}
              {t("detail.execution.run")}
            </Button>
          )
        ) : (
          <span className="text-xs text-muted-foreground">{t("detail.execution.assignFirst")}</span>
        )}
        {isExecutionActive && (
          <span className="flex items-center gap-1.5 text-xs text-green-500">
            <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-green-500" />
            {t("detail.execution.running")}
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
              {t("detail.execution.terminalTitle")}
            </span>
            <button
              onClick={onClearOutput}
              className="cursor-pointer p-0.5 text-zinc-500 transition-colors hover:text-zinc-300"
              title={t("detail.labels.clearLogs")}
            >
              <Eraser className="h-3 w-3" />
            </button>
          </div>
          <ScrollArea className="h-72 bg-black">
            <div className="space-y-0.5 p-2 font-mono text-xs">
              {output.length === 0 ? (
                <div className="text-zinc-600">{t("detail.execution.waitingOutput")}</div>
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
        title={t("detail.execution.fileChangesTitle")}
        description={t("detail.execution.fileChangesDesc")}
        history={executionChangeHistory}
        loading={executionChangeHistoryLoading}
        error={executionChangeHistoryError}
        emptyText={t("detail.execution.fileChangesEmpty")}
        onRefresh={onRefreshHistory}
        onOpenChangeDetail={onOpenChangeDetail}
      />
    </div>
  );
}
