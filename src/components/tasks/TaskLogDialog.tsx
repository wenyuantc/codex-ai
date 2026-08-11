import { useMemo } from "react";

import type { CodexSessionKind, Task } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { CodexTerminal } from "@/components/codex/CodexTerminal";
import { useSessionLogAwaitFollowups } from "@/hooks/useSessionLogAwaitFollowups";
import { useEmployeeStore } from "@/stores/employeeStore";

interface TaskLogDialogProps {
  open: boolean;
  task: Task | null;
  assigneeName?: string;
  sessionKind?: CodexSessionKind;
  onOpenChange: (open: boolean) => void;
}

export function TaskLogDialog({
  open,
  task,
  assigneeName,
  sessionKind = "execution",
  onOpenChange,
}: TaskLogDialogProps) {
  const employees = useEmployeeStore((s) => s.employees);
  const employeeRuntime = useEmployeeStore((s) => s.employeeRuntime);
  const sessionLabel = sessionKind === "review" ? "审核终端输出" : "终端输出";

  const inputContext = useMemo(() => {
    if (!task) {
      return { employeeId: null as string | null, provider: null as string | null, live: false };
    }
    const employeeId =
      sessionKind === "review" ? (task.reviewer_id ?? null) : (task.assignee_id ?? null);
    const employee = employeeId ? employees.find((item) => item.id === employeeId) : null;
    const runtimeSessions = employeeId ? (employeeRuntime[employeeId]?.sessions ?? []) : [];
    const liveSession = runtimeSessions.find(
      (session) => session.task_id === task.id && session.session_kind === sessionKind,
    );
    return {
      employeeId,
      provider: liveSession?.ai_provider ?? employee?.ai_provider ?? null,
      live: Boolean(liveSession),
    };
  }, [employees, employeeRuntime, sessionKind, task]);

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
          <DialogTitle>{sessionLabel}</DialogTitle>
          <DialogDescription>
            {task
              ? `任务“${task.title}”${assigneeName ? ` · ${assigneeName}` : ""} 的${sessionLabel}`
              : `查看任务${sessionLabel}`}
          </DialogDescription>
        </DialogHeader>

        {task ? (
          <div className="[&_div[data-slot='scroll-area']]:h-[28rem]">
            <CodexTerminal
              taskId={task.id}
              sessionKind={sessionKind}
              showInputBar
              inputEmployeeId={inputContext.employeeId}
              inputProvider={inputContext.provider}
              inputSessionLive={inputContext.live}
            />
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-border p-6 text-sm text-muted-foreground">
            当前没有可查看的任务日志。
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
