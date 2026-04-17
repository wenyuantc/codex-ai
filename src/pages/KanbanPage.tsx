import { useEffect, useState } from "react";
import { KanbanBoard } from "@/components/tasks/KanbanBoard";
import { CreateTaskDialog } from "@/components/tasks/CreateTaskDialog";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Plus } from "lucide-react";

export function KanbanPage() {
  const { fetchTasks } = useTaskStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const projects = useProjectStore((state) => state.projects);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const { fetchEmployees } = useEmployeeStore();
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const visibleProjectIdsKey = projects.map((project) => project.id).join(",");

  useEffect(() => {
    void fetchEmployees();
  }, [fetchEmployees]);

  useEffect(() => {
    void fetchTasks(currentProjectId);
  }, [currentProjectId, environmentMode, visibleProjectIdsKey, fetchTasks]);

  const hasProjects = projects.length > 0;

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold">看板</h2>
        <button
          onClick={() => setShowCreateDialog(true)}
          className="flex items-center gap-1 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
        >
          <Plus className="h-4 w-4" />
          新建任务
        </button>
      </div>
      <div className="flex-1 overflow-hidden">
        {hasProjects ? (
          <KanbanBoard projectId={currentProjectId} />
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
    </div>
  );
}
