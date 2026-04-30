import { useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import { Button } from "@/components/ui/button";
import { timeAgo } from "@/lib/utils";
import { ConfirmPermanentDeleteDialog } from "@/components/trash/ConfirmPermanentDeleteDialog";
import { Undo2, Trash2, FolderKanban } from "lucide-react";

export function TrashedProjectList() {
  const { trashedProjects, restoreProject, permanentlyDeleteProject } = useProjectStore();
  const [restoringId, setRestoringId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  if (trashedProjects.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-20 text-zinc-400">
        <FolderKanban className="h-12 w-12" />
        <p className="text-sm">回收站为空</p>
      </div>
    );
  }

  const handleRestore = async (id: string) => {
    setRestoringId(id);
    try {
      await restoreProject(id);
    } finally {
      setRestoringId(null);
    }
  };

  const handlePermanentDelete = async () => {
    if (!confirmDeleteId) return;
    setDeletingId(confirmDeleteId);
    try {
      await permanentlyDeleteProject(confirmDeleteId);
    } finally {
      setDeletingId(null);
      setConfirmDeleteId(null);
    }
  };

  const confirmTarget = trashedProjects.find((p) => p.id === confirmDeleteId);

  return (
    <>
      <div className="space-y-3">
        {trashedProjects.map((project) => (
          <div
            key={project.id}
            className="flex items-center justify-between rounded-lg border border-black/10 bg-white px-4 py-3 dark:border-white/10 dark:bg-zinc-900"
          >
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-zinc-900 dark:text-white">
                {project.name}
              </p>
              <p className="text-xs text-zinc-500">
                删除于 {timeAgo(project.deleted_at)}
                <span className="ml-2 rounded bg-zinc-100 px-1.5 py-0.5 text-xs dark:bg-zinc-800">
                  {project.project_type === "ssh" ? "SSH" : "本地"}
                </span>
              </p>
            </div>
            <div className="ml-4 flex items-center gap-2 shrink-0">
              <Button
                variant="outline"
                size="sm"
                onClick={() => handleRestore(project.id)}
                disabled={restoringId === project.id}
              >
                <Undo2 className="mr-1 h-3.5 w-3.5" />
                {restoringId === project.id ? "恢复中..." : "恢复"}
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onClick={() => setConfirmDeleteId(project.id)}
                disabled={deletingId === project.id}
              >
                <Trash2 className="mr-1 h-3.5 w-3.5" />
                {deletingId === project.id ? "删除中..." : "永久删除"}
              </Button>
            </div>
          </div>
        ))}
      </div>

      <ConfirmPermanentDeleteDialog
        open={confirmDeleteId !== null}
        title="确认永久删除项目"
        description={`确认永久删除项目"${confirmTarget?.name ?? ""}"吗？此操作会同时删除其所有关联数据，且不可恢复。`}
        deleting={deletingId !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmDeleteId(null);
        }}
        onConfirm={handlePermanentDelete}
      />
    </>
  );
}
