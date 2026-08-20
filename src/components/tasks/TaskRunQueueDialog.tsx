import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ListOrdered, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { formatDate } from "@/lib/utils";
import { useTaskStore } from "@/stores/taskStore";

interface TaskRunQueueDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function TaskRunQueueDialog({ open, onOpenChange }: TaskRunQueueDialogProps) {
  const { t } = useTranslation("kanban");
  const runQueue = useTaskStore((state) => state.runQueue);
  const tasks = useTaskStore((state) => state.tasks);
  const cancelQueuedRun = useTaskStore((state) => state.cancelQueuedRun);
  const [cancellingId, setCancellingId] = useState<string | null>(null);

  const handleCancel = async (taskId: string) => {
    setCancellingId(taskId);
    try {
      await cancelQueuedRun(taskId);
    } finally {
      setCancellingId(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ListOrdered className="h-4 w-4" />
            {t("runQueueTitle")}
          </DialogTitle>
          <DialogDescription>{t("runQueueDescription")}</DialogDescription>
        </DialogHeader>
        {runQueue.length === 0 ? (
          <p className="py-6 text-center text-sm text-muted-foreground">{t("runQueueEmpty")}</p>
        ) : (
          <ul className="max-h-80 space-y-2 overflow-auto">
            {runQueue.map((item) => {
              const task = tasks.find((entry) => entry.id === item.task_id);
              return (
                <li
                  key={item.id}
                  className="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2"
                >
                  <div className="min-w-0">
                    <p className="truncate text-sm font-medium">
                      {t("runQueuePosition", { position: item.position })}{" "}
                      {task?.title ?? item.task_id}
                    </p>
                    <p className="text-xs text-muted-foreground">{formatDate(item.enqueued_at)}</p>
                  </div>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={cancellingId === item.task_id}
                    onClick={() => void handleCancel(item.task_id)}
                  >
                    {cancellingId === item.task_id ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : null}
                    {t("runQueueCancel")}
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
