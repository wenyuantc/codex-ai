import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";

import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { KanbanBoard } from "@/components/tasks/KanbanBoard";
import { ArchiveManagementDialog } from "@/components/tasks/ArchiveManagementDialog";
import { CreateTaskDialog } from "@/components/tasks/CreateTaskDialog";
import { Button } from "@/components/ui/button";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Archive, Plus } from "lucide-react";

export function KanbanPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { fetchTasks } = useTaskStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const projects = useProjectStore((state) => state.projects);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const { fetchEmployees } = useEmployeeStore();
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [showArchiveDialog, setShowArchiveDialog] = useState(false);
  const visibleProjectIdsKey = projects.map((project) => project.id).join(",");
  const targetTaskId = searchParams.get("taskId");

  useEffect(() => {
    void fetchEmployees();
  }, [fetchEmployees]);

  useEffect(() => {
    void fetchTasks(currentProjectId);
  }, [currentProjectId, environmentMode, visibleProjectIdsKey, fetchTasks]);

  const hasProjects = projects.length > 0;

  useHotkeys("n", () => setShowCreateDialog(true), { preventDefault: true });
  useHotkeys("a", () => setShowArchiveDialog(true), { preventDefault: true });

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold">看板列表</h2>
        <div className="flex items-center gap-2">
          <Button onClick={() => setShowCreateDialog(true)}>
            <Plus className="h-4 w-4" />
            新建任务
            <Kbd variant="primary" size="xs" className="ml-1.5">N</Kbd>
          </Button>
          <Button variant="outline" onClick={() => setShowArchiveDialog(true)}>
            <Archive className="h-4 w-4" />
            归档管理
            <Kbd variant="subtle" size="xs" className="ml-1.5">A</Kbd>
          </Button>
        </div>
      </div>
      <div className="flex-1 overflow-hidden">
        {hasProjects ? (
          <KanbanBoard
            projectId={currentProjectId}
            targetTaskId={targetTaskId}
            onClearTargetTask={() => {
              if (!targetTaskId) {
                return;
              }

              const nextSearchParams = new URLSearchParams(searchParams);
              nextSearchParams.delete("taskId");
              setSearchParams(nextSearchParams, { replace: true });
            }}
          />
        ) : (
          <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-border text-sm text-muted-foreground">
            {environmentMode === "ssh"
              ? "当前 SSH 视图还没有项目，请先到项目管理创建 SSH 项目。"
              : "当前没有可展示的本地项目。"}
          </div>
        )}
      </div>
      {showCreateDialog && (
        <CreateTaskDialog
          open={showCreateDialog}
          onOpenChange={setShowCreateDialog}
          projectId={currentProjectId}
        />
      )}
      {showArchiveDialog && (
        <ArchiveManagementDialog
          open={showArchiveDialog}
          onOpenChange={setShowArchiveDialog}
          defaultProjectId={currentProjectId}
        />
      )}
    </div>
  );
}
