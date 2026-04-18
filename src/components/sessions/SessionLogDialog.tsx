import { useEffect, useState } from "react";

import { CodexTerminal } from "@/components/codex/CodexTerminal";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { getCodexSessionLogLines } from "@/lib/backend";
import type { CodexSessionKind } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";

interface SessionLogTarget {
  sessionRecordId: string | null;
  sessionId: string;
  displayName: string;
  employeeId: string | null;
  employeeName: string | null;
  taskId: string | null;
  taskTitle: string | null;
  sessionKind: CodexSessionKind | null;
}

interface SessionLogDialogProps {
  open: boolean;
  session: SessionLogTarget | null;
  onOpenChange: (open: boolean) => void;
}

export function SessionLogDialog({
  open,
  session,
  onOpenChange,
}: SessionLogDialogProps) {
  const hydrateSessionLog = useEmployeeStore((state) => state.hydrateSessionLog);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [loadedSessionHistories, setLoadedSessionHistories] = useState<Record<string, boolean>>({});
  const sessionRecordId = session?.sessionRecordId ?? null;

  useEffect(() => {
    if (!open || !sessionRecordId || loadedSessionHistories[sessionRecordId]) {
      return;
    }

    let active = true;
    setLoadingHistory(true);
    setHistoryError(null);

    void getCodexSessionLogLines(sessionRecordId)
      .then((lines) => {
        if (!active) {
          return;
        }
        hydrateSessionLog(sessionRecordId, lines);
        setLoadedSessionHistories((current) => ({
          ...current,
          [sessionRecordId]: true,
        }));
      })
      .catch((error) => {
        if (!active) {
          return;
        }
        setHistoryError(error instanceof Error ? error.message : "读取对话日志失败");
      })
      .finally(() => {
        if (active) {
          setLoadingHistory(false);
        }
      });

    return () => {
      active = false;
    };
  }, [hydrateSessionLog, loadedSessionHistories, open, sessionRecordId]);

  useEffect(() => {
    if (!open) {
      setHistoryError(null);
      setLoadingHistory(false);
    }
  }, [open]);

  const canShowLogs = Boolean(session?.sessionRecordId || session?.taskId || session?.employeeId);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]"
        showCloseButton
      >
        <DialogHeader>
          <DialogTitle>终端日志</DialogTitle>
          <DialogDescription>
            {session
              ? `查看对话“${session.displayName}”的实时终端输出。`
              : "查看对话终端输出"}
          </DialogDescription>
        </DialogHeader>

        {session && (
          <div className="rounded-lg border border-border bg-muted/20 px-3 py-2 text-xs text-muted-foreground">
            <div className="font-mono">对话记录 ID: {session.sessionRecordId ?? "暂无"}</div>
            <div className="font-mono">对话 ID: {session.sessionId}</div>
            <div className="mt-1">员工：{session.employeeName ?? session.employeeId ?? "未绑定"}</div>
            <div className="mt-1">任务：{session.taskTitle ?? "无关联任务"}</div>
          </div>
        )}

        {historyError && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            {historyError}
          </div>
        )}

        {loadingHistory && (
          <div className="rounded-lg border border-border bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
            正在加载对话历史日志...
          </div>
        )}

        {canShowLogs ? (
          <div className="[&_div[data-slot='scroll-area']]:h-[28rem]">
            {session?.sessionRecordId ? (
              <CodexTerminal sessionRecordId={session.sessionRecordId} />
            ) : session?.taskId ? (
              <CodexTerminal taskId={session.taskId} sessionKind={session.sessionKind ?? "execution"} />
            ) : (
              <CodexTerminal employeeId={session?.employeeId ?? undefined} />
            )}
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-border p-6 text-sm text-muted-foreground">
            当前没有可查看的终端日志。
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

export type { SessionLogTarget };
