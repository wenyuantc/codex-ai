import { useState } from "react";
import { useTaskStore } from "@/stores/taskStore";
import { Button } from "@/components/ui/button";
import { getStatusLabel, timeAgo } from "@/lib/utils";
import { ConfirmPermanentDeleteDialog } from "@/components/trash/ConfirmPermanentDeleteDialog";
import { Undo2, Trash2 } from "lucide-react";

export function TrashedTaskList() {
  const { trashedTasks, restoreTask, permanentlyDeleteTask } = useTaskStore();
  const [restoringId, setRestoringId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  if (trashedTasks.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-20 text-zinc-400">
        <Trash2 className="h-12 w-12" />
        <p className="text-sm">回收站为空</p>
      </div>
    );
  }

  const handleRestore = async (id: string) => {
    setRestoringId(id);
    try {
      await restoreTask(id);
    } finally {
      setRestoringId(null);
    }
  };

  const handlePermanentDelete = async () => {
    if (!confirmDeleteId) return;
    setDeletingId(confirmDeleteId);
    try {
      await permanentlyDeleteTask(confirmDeleteId);
    } finally {
      setDeletingId(null);
      setConfirmDeleteId(null);
    }
  };

  const confirmTarget = trashedTasks.find((t) => t.id === confirmDeleteId);

  return (
    <>
      <div className="space-y-3">
        {trashedTasks.map((task) => (
          <div
            key={task.id}
            className="flex items-center justify-between rounded-lg border border-black/10 bg-white px-4 py-3 dark:border-white/10 dark:bg-zinc-900"
          >
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-zinc-900 dark:text-white">
                {task.title}
              </p>
              <p className="text-xs text-zinc-500">
                删除于 {timeAgo(task.deleted_at)}
                {task.status && (
                  <span className="ml-2 rounded bg-zinc-100 px-1.5 py-0.5 text-xs dark:bg-zinc-800">
                    {getStatusLabel(task.status)}
                  </span>
                )}
              </p>
            </div>
            <div className="ml-4 flex items-center gap-2 shrink-0">
              <Button
                variant="outline"
                size="sm"
                onClick={() => handleRestore(task.id)}
                disabled={restoringId === task.id}
              >
                <Undo2 className="mr-1 h-3.5 w-3.5" />
                {restoringId === task.id ? "恢复中..." : "恢复"}
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onClick={() => setConfirmDeleteId(task.id)}
                disabled={deletingId === task.id}
              >
                <Trash2 className="mr-1 h-3.5 w-3.5" />
                {deletingId === task.id ? "删除中..." : "永久删除"}
              </Button>
            </div>
          </div>
        ))}
      </div>

      <ConfirmPermanentDeleteDialog
        open={confirmDeleteId !== null}
        title="确认永久删除任务"
        description={`确认永久删除任务"${confirmTarget?.title ?? ""}"吗？此操作不可恢复。`}
        deleting={deletingId !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmDeleteId(null);
        }}
        onConfirm={handlePermanentDelete}
      />
    </>
  );
}
