import { CodexTerminal } from "@/components/codex/CodexTerminal";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface SessionLogTarget {
  sessionId: string;
  displayName: string;
  employeeId: string | null;
  employeeName: string | null;
  taskId: string | null;
  taskTitle: string | null;
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
  const canShowLogs = Boolean(session?.taskId || session?.employeeId);

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
              ? `查看 Session “${session.displayName}” 的实时终端输出。`
              : "查看 Session 终端输出"}
          </DialogDescription>
        </DialogHeader>

        {session && (
          <div className="rounded-lg border border-border bg-muted/20 px-3 py-2 text-xs text-muted-foreground">
            <div className="font-mono">session id: {session.sessionId}</div>
            <div className="mt-1">员工：{session.employeeName ?? session.employeeId ?? "未绑定"}</div>
            <div className="mt-1">任务：{session.taskTitle ?? "无关联任务"}</div>
          </div>
        )}

        {canShowLogs ? (
          <div className="[&_div[data-slot='scroll-area']]:h-[28rem]">
            {session?.taskId ? (
              <CodexTerminal taskId={session.taskId} />
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
