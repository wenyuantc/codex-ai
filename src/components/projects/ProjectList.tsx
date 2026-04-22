import { useEffect, useMemo, useState } from "react";
import { useProjectStore } from "@/stores/projectStore";
import { getProjectGitOverview } from "@/lib/backend";
import type { Project, ProjectGitOverview, ProjectGitRepoActionType } from "@/lib/types";
import { ProjectCard } from "./ProjectCard";
import { EditProjectDialog } from "./EditProjectDialog";
import { DeleteProjectDialog } from "./DeleteProjectDialog";
import { ProjectGitRepoActionDialog } from "./ProjectGitRepoActionDialog";

type ProjectGitSyncMap = Record<string, ProjectGitOverview>;

export function ProjectList() {
  const { projects, environmentMode, fetchProjects, deleteProject } = useProjectStore();
  const [filter, setFilter] = useState<string>("all");
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [deletingProject, setDeletingProject] = useState<Project | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [projectGitSyncMap, setProjectGitSyncMap] = useState<ProjectGitSyncMap>({});
  const [selectedRepoAction, setSelectedRepoAction] = useState<ProjectGitRepoActionType | null>(null);
  const [selectedRepoProject, setSelectedRepoProject] = useState<Project | null>(null);
  const [repoActionNotice, setRepoActionNotice] = useState<{
    tone: "success" | "error";
    message: string;
  } | null>(null);
  const [gitOverviewReloadNonce, setGitOverviewReloadNonce] = useState(0);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  const filtered = useMemo(
    () => (filter === "all" ? projects : projects.filter((p) => p.status === filter)),
    [filter, projects],
  );
  const filteredIdsKey = useMemo(
    () => filtered.map((project) => project.id).join("|"),
    [filtered],
  );

  useEffect(() => {
    if (filtered.length === 0) {
      setProjectGitSyncMap({});
      return;
    }

    let active = true;
    const visibleProjectIds = new Set(filtered.map((project) => project.id));

    void Promise.allSettled(
      filtered.map(async (project) => {
        const overview = await getProjectGitOverview(project.id);
        return [
          project.id,
          overview,
        ] as const;
      }),
    ).then((results) => {
      if (!active) {
        return;
      }

      const nextMap: ProjectGitSyncMap = {};
      for (const result of results) {
        if (result.status !== "fulfilled") {
          continue;
        }
        const [projectId, syncState] = result.value;
        if (visibleProjectIds.has(projectId)) {
          nextMap[projectId] = syncState;
        }
      }
      setProjectGitSyncMap(nextMap);
    });

    return () => {
      active = false;
    };
  }, [filtered, filteredIdsKey, gitOverviewReloadNonce]);

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

  const handleOpenRepoAction = (project: Project, action: ProjectGitRepoActionType) => {
    setSelectedRepoProject(project);
    setSelectedRepoAction(action);
    setRepoActionNotice(null);
  };

  const handleRepoActionCompleted = async (message: string) => {
    setRepoActionNotice({ tone: "success", message });
    setSelectedRepoAction(null);
    setSelectedRepoProject(null);
    setGitOverviewReloadNonce((value) => value + 1);
  };

  const selectedRepoOverview = selectedRepoProject
    ? projectGitSyncMap[selectedRepoProject.id] ?? null
    : null;
  const selectedStagedChanges = selectedRepoOverview?.working_tree_changes.filter(
    (change) =>
      change.stage_status === "staged"
      || change.stage_status === "partially_staged",
  ) ?? [];

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

      {repoActionNotice && (
        <div
          className={`rounded-lg border px-3 py-2 text-xs ${
            repoActionNotice.tone === "success"
              ? "border-primary/20 bg-primary/5 text-primary"
              : "border-destructive/20 bg-destructive/10 text-destructive"
          }`}
        >
          {repoActionNotice.message}
        </div>
      )}

      {/* Cards */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        {filtered.map((project) => (
          <ProjectCard
            key={project.id}
            project={project}
            aheadCommits={projectGitSyncMap[project.id]?.ahead_commits ?? null}
            behindCommits={projectGitSyncMap[project.id]?.behind_commits ?? null}
            onPushRequested={(targetProject) => handleOpenRepoAction(targetProject, "push")}
            onPullRequested={(targetProject) => handleOpenRepoAction(targetProject, "pull")}
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

      <ProjectGitRepoActionDialog
        open={selectedRepoAction !== null && selectedRepoProject !== null}
        action={selectedRepoAction}
        projectId={selectedRepoProject?.id ?? null}
        currentBranch={selectedRepoOverview?.current_branch ?? null}
        workingTreeSummary={selectedRepoOverview?.working_tree_summary ?? null}
        projectBranches={selectedRepoOverview?.project_branches ?? []}
        stagedFileCount={selectedStagedChanges.length}
        stagedChanges={selectedStagedChanges}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedRepoAction(null);
            setSelectedRepoProject(null);
          }
        }}
        onActionCompleted={handleRepoActionCompleted}
      />
    </div>
  );
}
