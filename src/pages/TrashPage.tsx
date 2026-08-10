import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { TrashedTaskList } from "@/components/trash/TrashedTaskList";
import { TrashedProjectList } from "@/components/trash/TrashedProjectList";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { Trash2 } from "lucide-react";

type TrashTab = "tasks" | "projects";

export function TrashPage() {
  const { t } = useTranslation("trash");
  const [tab, setTab] = useState<TrashTab>("tasks");
  const { trashedTasks, fetchTrashedTasks } = useTaskStore();
  const { trashedProjects, fetchTrashedProjects } = useProjectStore();

  useEffect(() => {
    fetchTrashedTasks();
    fetchTrashedProjects();
  }, [fetchTrashedTasks, fetchTrashedProjects]);

  const switchTab = (next: TrashTab) => {
    setTab(next);
    if (next === "tasks") {
      fetchTrashedTasks();
    } else {
      fetchTrashedProjects();
    }
  };

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Trash2 className="h-6 w-6 text-muted-foreground" />
        <h1 className="text-xl font-semibold text-foreground">{t("title")}</h1>
      </div>

      <div className="flex gap-2 border-b border-border">
        <button
          type="button"
          onClick={() => switchTab("tasks")}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            tab === "tasks"
              ? "border-foreground text-foreground"
              : "border-transparent text-muted-foreground hover:text-foreground"
          }`}
        >
          {t("deletedTasks")}
          {trashedTasks.length > 0 && (
            <span className="ml-1.5 rounded-full bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
              {trashedTasks.length}
            </span>
          )}
        </button>
        <button
          type="button"
          onClick={() => switchTab("projects")}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            tab === "projects"
              ? "border-foreground text-foreground"
              : "border-transparent text-muted-foreground hover:text-foreground"
          }`}
        >
          {t("deletedProjects")}
          {trashedProjects.length > 0 && (
            <span className="ml-1.5 rounded-full bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
              {trashedProjects.length}
            </span>
          )}
        </button>
      </div>

      {tab === "tasks" ? <TrashedTaskList /> : <TrashedProjectList />}
    </div>
  );
}
