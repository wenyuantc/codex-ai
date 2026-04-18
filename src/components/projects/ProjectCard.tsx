import type { Project } from "@/lib/types";
import { getStatusLabel, getStatusColor, formatDate } from "@/lib/utils";
import { Trash2, Edit2, FolderKanban, ArrowRight, ArrowDown, ArrowUp } from "lucide-react";
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import { getProjectTypeLabel, getProjectWorkingDir } from "@/lib/projects";
import { Badge } from "@/components/ui/badge";
import { Link } from "react-router-dom";

interface ProjectCardProps {
  project: Project;
  taskCount?: number;
  aheadCommits?: number | null;
  behindCommits?: number | null;
  onPushRequested?: (project: Project) => void;
  onPullRequested?: (project: Project) => void;
  onEdit: (project: Project) => void;
  onDelete: (project: Project) => void;
}

export function ProjectCard({
  project,
  taskCount,
  aheadCommits,
  behindCommits,
  onPushRequested,
  onPullRequested,
  onEdit,
  onDelete,
}: ProjectCardProps) {
  return (
    <div className="flex min-h-44 w-fit max-w-full min-w-[min(100%,22rem)] flex-col rounded-lg border border-border bg-card p-4 transition-shadow hover:shadow-sm">
      <div className="flex flex-1 items-start justify-between gap-3">
        <div className="flex min-w-0 flex-1 items-start gap-3">
          <div className="h-9 w-9 rounded-md bg-primary/10 flex items-center justify-center shrink-0">
            <FolderKanban className="h-4 w-4 text-primary" />
          </div>
          <div className="min-w-0 flex-1">
            <Link
              to={`/projects/${project.id}`}
              className="inline-flex max-w-full items-center gap-1 truncate text-sm font-medium transition-colors hover:text-primary"
            >
              <span className="truncate">{project.name}</span>
              <ArrowRight className="h-3.5 w-3.5 shrink-0" />
            </Link>
            {project.description && (
              <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">{project.description}</p>
            )}
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <Badge variant={project.project_type === "ssh" ? "secondary" : "outline"}>
                {getProjectTypeLabel(project.project_type)}
              </Badge>
              {(aheadCommits ?? 0) > 0 && (
                <button
                  type="button"
                  className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-sm font-semibold text-sky-600 transition-colors hover:bg-sky-500/10"
                  title={`当前有 ${aheadCommits} 个提交待推送`}
                  onClick={() => onPushRequested?.(project)}
                >
                  <ArrowUp className="h-4 w-4" />
                  {aheadCommits}
                </button>
              )}
              {(behindCommits ?? 0) > 0 && (
                <button
                  type="button"
                  className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-sm font-semibold text-amber-600 transition-colors hover:bg-amber-500/10"
                  title={`当前有 ${behindCommits} 个提交待拉取`}
                  onClick={() => onPullRequested?.(project)}
                >
                  <ArrowDown className="h-4 w-4" />
                  {behindCommits}
                </button>
              )}
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

      <div className="mt-3 flex items-center justify-end border-t border-border/60 pt-3">
        <Link
          to={`/projects/${project.id}`}
          className="inline-flex items-center gap-1 text-xs font-medium text-primary transition-colors hover:text-primary/80"
        >
          查看详情
          <ArrowRight className="h-3.5 w-3.5" />
        </Link>
      </div>
    </div>
  );
}
