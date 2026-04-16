import type { Project } from "@/lib/types";
import { getStatusLabel, getStatusColor, formatDate } from "@/lib/utils";
import { Trash2, Edit2, FolderKanban } from "lucide-react";
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import { getProjectTypeLabel, getProjectWorkingDir } from "@/lib/projects";
import { Badge } from "@/components/ui/badge";

interface ProjectCardProps {
  project: Project;
  taskCount?: number;
  onEdit: (project: Project) => void;
  onDelete: (project: Project) => void;
}

export function ProjectCard({ project, taskCount, onEdit, onDelete }: ProjectCardProps) {
  return (
    <div className="flex min-h-44 w-fit max-w-full min-w-[min(100%,22rem)] flex-col rounded-lg border border-border bg-card p-4 transition-shadow hover:shadow-sm">
      <div className="flex flex-1 items-start justify-between gap-3">
        <div className="flex min-w-0 flex-1 items-start gap-3">
          <div className="h-9 w-9 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
            <FolderKanban className="h-4 w-4 text-primary" />
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="truncate font-medium text-sm">{project.name}</h3>
            {project.description && (
              <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">{project.description}</p>
            )}
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <Badge variant={project.project_type === "ssh" ? "secondary" : "outline"}>
                {getProjectTypeLabel(project.project_type)}
              </Badge>
            </div>
            <RepoPathDisplay
              repoPath={getProjectWorkingDir(project)}
              projectType={project.project_type}
              compact
              className="mt-2"
            />
          </div>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <button
            onClick={() => onEdit(project)}
            className="p-1 text-muted-foreground hover:text-foreground transition-colors"
            title="编辑项目"
          >
            <Edit2 className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={() => onDelete(project)}
            className="p-1 text-muted-foreground hover:text-destructive transition-colors"
            title="删除项目"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <div className="flex items-center gap-3 mt-3">
        <span className={`inline-flex items-center gap-1 text-xs`}>
          <span className={`w-1.5 h-1.5 rounded-full ${getStatusColor(project.status)}`} />
          {getStatusLabel(project.status)}
        </span>
        {taskCount !== undefined && (
          <span className="text-xs text-muted-foreground">
            {taskCount} 个任务
          </span>
        )}
        <span className="text-xs text-muted-foreground ml-auto">
          {formatDate(project.created_at)}
        </span>
      </div>
    </div>
  );
}
