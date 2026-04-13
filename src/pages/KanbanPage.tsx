import { useEffect, useState } from "react";
import { KanbanBoard } from "@/components/tasks/KanbanBoard";
import { CreateTaskDialog } from "@/components/tasks/CreateTaskDialog";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Plus } from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const ALL_PROJECTS_VALUE = "__all_projects__";

export function KanbanPage() {
  const { fetchTasks } = useTaskStore();
  const { projects, fetchProjects } = useProjectStore();
  const { fetchEmployees } = useEmployeeStore();
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [selectedProjectId, setSelectedProjectId] = useState<string | undefined>();

  useEffect(() => {
    fetchProjects();
    fetchEmployees();
    fetchTasks();
  }, [fetchEmployees, fetchProjects, fetchTasks]);

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold">看板</h2>
          <Select
            value={selectedProjectId ?? ALL_PROJECTS_VALUE}
            onValueChange={(value) => {
              const val =
                !value || value === ALL_PROJECTS_VALUE ? undefined : value;
              setSelectedProjectId(val);
              fetchTasks(val);
            }}
          >
            <SelectTrigger className="w-[220px] bg-background">
              <SelectValue>
                {(value) => {
                  if (!value || value === ALL_PROJECTS_VALUE) {
                    return "全部项目";
                  }

                  return projects.find((project) => project.id === value)?.name ?? "全部项目";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL_PROJECTS_VALUE}>全部项目</SelectItem>
              {projects.map((project) => (
                <SelectItem key={project.id} value={project.id}>
                  {project.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <button
          onClick={() => setShowCreateDialog(true)}
          className="flex items-center gap-1 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
        >
          <Plus className="h-4 w-4" />
          新建任务
        </button>
      </div>
      <div className="flex-1 overflow-hidden">
        <KanbanBoard projectId={selectedProjectId} />
      </div>
      <CreateTaskDialog
        open={showCreateDialog}
        onOpenChange={setShowCreateDialog}
        projectId={selectedProjectId}
      />
    </div>
  );
}
