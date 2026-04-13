import type { Task } from "@/lib/types";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { CodexTerminal } from "@/components/codex/CodexTerminal";

interface TaskLogDialogProps {
  open: boolean;
  task: Task | null;
  assigneeName?: string;
  onOpenChange: (open: boolean) => void;
}

export function TaskLogDialog({
  open,
  task,
  assigneeName,
  onOpenChange,
}: TaskLogDialogProps) {
  const employeeId = task?.assignee_id ?? null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-[min(96vw,88rem)] max-w-[min(96vw,88rem)] sm:max-w-[min(96vw,88rem)]"
        showCloseButton
      >
        <DialogHeader>
          <DialogTitle>终端日志</DialogTitle>
          <DialogDescription>
            {task
              ? `任务“${task.title}”${assigneeName ? ` · ${assigneeName}` : ""} 的终端输出`
              : "查看任务终端输出"}
          </DialogDescription>
        </DialogHeader>

        {employeeId ? (
          <div className="[&_div[data-slot='scroll-area']]:h-[28rem]">
            <CodexTerminal employeeId={employeeId} />
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-border p-6 text-sm text-muted-foreground">
            当前任务未指派员工，暂无可查看的终端日志。
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
