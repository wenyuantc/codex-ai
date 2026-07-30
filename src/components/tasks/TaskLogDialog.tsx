import type { CodexSessionKind, Task } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { CodexTerminal } from "@/components/codex/CodexTerminal";

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
  const sessionLabel = sessionKind === "review" ? "审核终端输出" : "终端输出";

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
            <CodexTerminal taskId={task.id} sessionKind={sessionKind} />
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
