import { useEffect, useMemo, useState } from "react";

import { getCodexSessionLogLines } from "@/lib/backend";
import type { Employee, EmployeeRunningSession } from "@/lib/types";
import { formatDate } from "@/lib/utils";
import { CodexTerminal } from "@/components/codex/CodexTerminal";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useEmployeeStore } from "@/stores/employeeStore";

interface EmployeeRunningSessionsDialogProps {
  open: boolean;
  employee: Employee;
  sessions: EmployeeRunningSession[];
  onOpenChange: (open: boolean) => void;
}

function formatSessionKind(sessionKind: EmployeeRunningSession["session_kind"]) {
  return sessionKind === "review" ? "审核" : "执行";
}

  function formatAiProvider(provider: EmployeeRunningSession["ai_provider"]) {
    return provider === "claude" ? "Claude" : provider === "opencode" ? "OpenCode" : "Codex";
  }

export function EmployeeRunningSessionsDialog({
  open,
  employee,
  sessions,
  onOpenChange,
}: EmployeeRunningSessionsDialogProps) {
  const hydrateSessionLog = useEmployeeStore((state) => state.hydrateSessionLog);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [loadedSessionHistories, setLoadedSessionHistories] = useState<Record<string, boolean>>({});

  const sortedSessions = useMemo(() => (
    [...sessions].sort((left, right) => (
      right.started_at.localeCompare(left.started_at)
      || right.session_record_id.localeCompare(left.session_record_id)
    ))
  ), [sessions]);

  useEffect(() => {
    if (!open || sortedSessions.length === 0) {
      return;
    }

    const missingSessions = sortedSessions.filter((session) => !loadedSessionHistories[session.session_record_id]);
    if (missingSessions.length === 0) {
      return;
    }

    let active = true;
    setLoadingHistory(true);
    setHistoryError(null);

    void Promise.all(
      missingSessions.map(async (session) => {
        const lines = await getCodexSessionLogLines(session.session_record_id);
        if (!active) {
          return;
        }
        hydrateSessionLog(session.session_record_id, lines);
        setLoadedSessionHistories((current) => ({
          ...current,
          [session.session_record_id]: true,
        }));
      }),
    )
      .catch((error) => {
        if (!active) {
          return;
        }
        setHistoryError(error instanceof Error ? error.message : "读取运行日志失败");
      })
      .finally(() => {
        if (active) {
          setLoadingHistory(false);
        }
      });

    return () => {
      active = false;
    };
  }, [hydrateSessionLog, loadedSessionHistories, open, sortedSessions]);

  useEffect(() => {
    if (!open) {
      setHistoryError(null);
      setLoadingHistory(false);
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]">
        <DialogHeader>
          <DialogTitle>运行终端</DialogTitle>
          <DialogDescription>
            查看员工“{employee.name}”当前所有运行任务的终端日志。
          </DialogDescription>
        </DialogHeader>

        {historyError && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            {historyError}
          </div>
        )}

        {loadingHistory && (
          <div className="rounded-lg border border-border bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
            正在加载运行中会话的日志...
          </div>
        )}

        {sortedSessions.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border p-6 text-sm text-muted-foreground">
            当前没有运行中的任务日志。
          </div>
        ) : (
          <div className="space-y-4 max-h-[70vh] overflow-y-auto pr-1">
            {sortedSessions.map((session) => (
              <section key={session.session_record_id} className="rounded-lg border border-border bg-muted/15 p-3">
                <div className="mb-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                  <span className="rounded-full bg-secondary px-2 py-0.5 text-foreground">
                    {formatSessionKind(session.session_kind)}
                  </span>
                  <span className="rounded-full bg-secondary px-2 py-0.5 text-foreground">
                    {formatAiProvider(session.ai_provider)}
                  </span>
                  <span className="font-medium text-foreground">
                    {session.task_title ?? session.task_id ?? "未关联任务"}
                  </span>
                  <span className="font-mono">记录 ID: {session.session_record_id}</span>
                  <span>{formatDate(session.started_at)}</span>
                </div>

                <div className="[&_div[data-slot='scroll-area']]:h-56">
                  <CodexTerminal sessionRecordId={session.session_record_id} />
                </div>
              </section>
            ))}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
