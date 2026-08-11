import { useEffect, useMemo, useState } from "react";

import { CodexTerminal } from "@/components/codex/CodexTerminal";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useSessionLogAwaitFollowups } from "@/hooks/useSessionLogAwaitFollowups";
import { getCodexSessionLogLines } from "@/lib/backend";
import type { CodexSessionKind } from "@/lib/types";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";

interface SessionLogTarget {
  sessionRecordId: string | null;
  sessionId: string;
  displayName: string;
  employeeId: string | null;
  employeeName: string | null;
  taskId: string | null;
  taskTitle: string | null;
  sessionKind: CodexSessionKind | null;
  aiProvider?: string | null;
}

interface SessionLogDialogProps {
  open: boolean;
  session: SessionLogTarget | null;
  onOpenChange: (open: boolean) => void;
}

export function SessionLogDialog({ open, session, onOpenChange }: SessionLogDialogProps) {
  const hydrateSessionLog = useEmployeeStore((state) => state.hydrateSessionLog);
  const sessionLogs = useEmployeeStore((state) => state.sessionLogs);
  const taskLogs = useEmployeeStore((state) => state.taskLogs);
  const employees = useEmployeeStore((state) => state.employees);
  const employeeRuntime = useEmployeeStore((state) => state.employeeRuntime);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [loadedSessionHistories, setLoadedSessionHistories] = useState<Record<string, boolean>>({});
  const [keyword, setKeyword] = useState("");
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
      setKeyword("");
    }
  }, [open]);

  const rawLines = useMemo(() => {
    if (sessionRecordId) {
      return (sessionLogs[sessionRecordId] ?? []).map((entry) => entry.line);
    }
    if (session?.taskId) {
      const kind = session.sessionKind ?? "execution";
      return taskLogs[buildTaskLogKey(session.taskId, kind)] ?? [];
    }
    return [];
  }, [session?.sessionKind, session?.taskId, sessionLogs, sessionRecordId, taskLogs]);

  const trimmedKeyword = keyword.trim();
  const filteredLines = useMemo(() => {
    if (!trimmedKeyword) {
      return rawLines;
    }
    const lower = trimmedKeyword.toLowerCase();
    return rawLines.filter((line) => line.toLowerCase().includes(lower));
  }, [rawLines, trimmedKeyword]);

  const canShowLogs = Boolean(session?.sessionRecordId || session?.taskId);
  const matchLabel = trimmedKeyword
    ? `匹配 ${filteredLines.length} / ${rawLines.length} 行`
    : `共 ${rawLines.length} 行`;

  const inputContext = useMemo(() => {
    const employeeId = session?.employeeId ?? null;
    const employee = employeeId ? employees.find((item) => item.id === employeeId) : null;
    const runtimeSessions = employeeId ? (employeeRuntime[employeeId]?.sessions ?? []) : [];
    const liveSession = sessionRecordId
      ? runtimeSessions.find((item) => item.session_record_id === sessionRecordId)
      : undefined;
    const provider =
      session?.aiProvider ?? liveSession?.ai_provider ?? employee?.ai_provider ?? null;
    const live = Boolean(liveSession);
    return { employeeId, provider, live };
  }, [employeeRuntime, employees, session?.aiProvider, session?.employeeId, sessionRecordId]);

  useSessionLogAwaitFollowups(
    open,
    inputContext.employeeId,
    inputContext.provider,
    inputContext.live,
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]"
        showCloseButton
      >
        <DialogHeader>
          <DialogTitle>终端日志</DialogTitle>
          <DialogDescription>
            {session ? `查看对话“${session.displayName}”的实时终端输出。` : "查看对话终端输出"}
          </DialogDescription>
        </DialogHeader>

        {session && (
          <div className="rounded-lg border border-border bg-muted/20 px-3 py-2 text-xs text-muted-foreground">
            <div className="font-mono">对话记录 ID: {session.sessionRecordId ?? "暂无"}</div>
            <div className="font-mono">对话 ID: {session.sessionId}</div>
            <div className="mt-1">
              员工：{session.employeeName ?? session.employeeId ?? "未绑定"}
            </div>
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
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <Input
                value={keyword}
                onChange={(event) => setKeyword(event.target.value)}
                placeholder="关键字过滤日志…"
                className="h-8 max-w-sm text-sm"
                aria-label="日志关键字过滤"
              />
              {trimmedKeyword && (
                <button
                  type="button"
                  className="text-xs text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
                  onClick={() => setKeyword("")}
                >
                  清空过滤
                </button>
              )}
              <span className="text-xs text-muted-foreground">{matchLabel}</span>
            </div>
            {trimmedKeyword ? (
              <CodexTerminal lines={filteredLines} hideClear heightClassName="h-[28rem]" />
            ) : sessionRecordId ? (
              <CodexTerminal
                sessionRecordId={sessionRecordId}
                heightClassName="h-[28rem]"
                showInputBar
                inputEmployeeId={inputContext.employeeId}
                inputProvider={inputContext.provider}
                inputSessionLive={inputContext.live}
              />
            ) : session?.taskId ? (
              <CodexTerminal
                taskId={session.taskId}
                sessionKind={session.sessionKind ?? "execution"}
                heightClassName="h-[28rem]"
                showInputBar
                inputEmployeeId={inputContext.employeeId}
                inputProvider={inputContext.provider}
                inputSessionLive={inputContext.live}
              />
            ) : null}
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
