import { useEffect, useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import type { Project } from "@/lib/types";
import { ProjectCard } from "./ProjectCard";
import { EditProjectDialog } from "./EditProjectDialog";
import { DeleteProjectDialog } from "./DeleteProjectDialog";

export function ProjectList() {
  const { projects, environmentMode, fetchProjects, deleteProject } = useProjectStore();
  const [filter, setFilter] = useState<string>("all");
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [deletingProject, setDeletingProject] = useState<Project | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  const filtered = filter === "all"
    ? projects
    : projects.filter((p) => p.status === filter);

  const handleDelete = async () => {
    if (!deletingProject) return;
    setDeleting(true);
    try {
      await deleteProject(deletingProject.id);
      setDeletingProject(null);
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="space-y-3">
      {/* Filter */}
      <div className="flex items-center gap-2">
        {["all", "active", "archived"].map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`px-2.5 py-1 text-xs rounded-md transition-colors ${
              filter === f
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent"
            }`}
          >
            {f === "all" ? "全部" : f === "active" ? "活跃" : "归档"}
          </button>
        ))}
        <span className="text-xs text-muted-foreground ml-auto">
          {filtered.length} 个项目
        </span>
      </div>

      {/* Cards */}
      <div className="flex flex-wrap items-start gap-4">
        {filtered.map((project) => (
          <ProjectCard
            key={project.id}
            project={project}
            onEdit={setEditingProject}
            onDelete={setDeletingProject}
          />
        ))}
      </div>

      {filtered.length === 0 && (
        <div className="text-center py-12 text-muted-foreground text-sm">
          {filter === "all"
            ? environmentMode === "ssh"
              ? "当前 SSH 视图还没有项目，请先创建 SSH 项目。"
              : "暂无项目"
            : `没有${filter === "active" ? "活跃" : "归档"}项目`}
        </div>
      )}

      <EditProjectDialog
        open={!!editingProject}
        onOpenChange={(open) => { if (!open) setEditingProject(null); }}
        project={editingProject}
      />

      <DeleteProjectDialog
        open={!!deletingProject}
        onOpenChange={(open) => {
          if (!open && !deleting) setDeletingProject(null);
        }}
        project={deletingProject}
        deleting={deleting}
        onConfirm={handleDelete}
      />
    </div>
  );
}
