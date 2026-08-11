import { Suspense, lazy, startTransition, useEffect, useRef, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import {
  checkProjectRepoHealth,
  deleteTaskGitContextRecord,
  getProjectGitCommitDetail,
  getProjectGitCommitFilePreview,
  getProjectGitFilePreview,
  getProjectGitOverview,
  listProjectGitCommits,
  reconcileTaskGitContext,
  rollbackAllProjectGitChanges,
  rollbackProjectGitFiles,
  stageAllProjectGitFiles,
  stageProjectGitFile,
  unstageAllProjectGitFiles,
  unstageProjectGitFile,
  type ProjectRepoHealth,
} from "@/lib/backend";
import { countStagedGitFiles } from "@/lib/gitWorkingTree";
import type {
  Employee,
  GitActionType,
  ProjectGitCommit,
  ProjectGitCommitFileChange,
  ProjectGitCommitDetail,
  ProjectGitFileChangeRef,
  ProjectGitFilePreview,
  ProjectGitOverview,
  ProjectGitRepoActionType,
  ProjectGitBranchActionType,
  ProjectGitWorkingTreeChange,
  TaskGitContext,
} from "@/lib/types";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { EditProjectDialog } from "@/components/projects/EditProjectDialog";
import { DeleteProjectDialog } from "@/components/projects/DeleteProjectDialog";
import { DeleteTaskGitContextDialog } from "@/components/projects/DeleteTaskGitContextDialog";
import { ProjectGitActionDialog } from "@/components/projects/ProjectGitActionDialog";
import { ProjectGitCommitDetailDialog } from "@/components/projects/ProjectGitCommitDetailDialog";
import { ProjectGitRepoActionDialog } from "@/components/projects/ProjectGitRepoActionDialog";
import { ProjectGitBranchActionDialog } from "@/components/projects/ProjectGitBranchActionDialog";
import { ProjectMilestonesSection } from "@/components/projects/ProjectMilestonesSection";
import { GitChangesPanel } from "@/components/git/GitChangesPanel";
import { getGitActionButtonClassName } from "@/components/git/gitHelpers";
import { ProjectWorktreeSection } from "@/components/git/ProjectWorktreeSection";
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import {
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  Edit2,
  GitBranch,
  Loader2,
  RefreshCw,
  ShieldAlert,
  Trash2,
} from "lucide-react";
import {
  getStatusLabel,
  getStatusColor,
  getPriorityLabel,
  formatDate,
  isTaskOverdue,
} from "@/lib/utils";
import { getProjectWorkingDir, getProjectTypeLabel } from "@/lib/projects";
import i18n from "@/lib/i18n";

const ProjectGitFilePreviewDialog = lazy(async () => {
  const module = await import("@/components/projects/ProjectGitFilePreviewDialog");
  return { default: module.ProjectGitFilePreviewDialog };
});

const RECENT_COMMIT_SUMMARY_LIMIT = 5;
const RECENT_COMMIT_PAGE_SIZE = 20;
type ProjectGitPreviewSource = "working_tree" | "commit";

function getTaskGitContextStateLabel(state: string) {
  return i18n.t(`projects:gitContextState.${state}`, { defaultValue: state });
}

function getGitActionTypeLabel(actionType: string | null | undefined) {
  if (!actionType) {
    return i18n.t("projects:detailPage.pendingActionFallback");
  }
  return i18n.t(`projects:gitActionOptions.${actionType}`, {
    defaultValue: actionType ?? undefined,
  });
}

function getGitRuntimeStatusLabel(status: string) {
  return i18n.t(`projects:gitRuntimeStatus.${status}`, { defaultValue: status });
}

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation(["projects", "common"]);
  const { projects, deleteProject } = useProjectStore();
  const { tasks, fetchTasks } = useTaskStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const [projectEmployees, setProjectEmployees] = useState<Employee[]>([]);
  const [showEdit, setShowEdit] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [repoHealth, setRepoHealth] = useState<ProjectRepoHealth | null>(null);
  const [repoHealthLoading, setRepoHealthLoading] = useState(false);
  const [repoHealthError, setRepoHealthError] = useState<string | null>(null);
  const [gitOverview, setGitOverview] = useState<ProjectGitOverview | null>(null);
  const [gitOverviewLoading, setGitOverviewLoading] = useState(false);
  const [gitOverviewError, setGitOverviewError] = useState<string | null>(null);
  const [recentCommits, setRecentCommits] = useState<ProjectGitCommit[]>([]);
  const [recentCommitsExpanded, setRecentCommitsExpanded] = useState(false);
  const [recentCommitsHasMore, setRecentCommitsHasMore] = useState(false);
  const [recentCommitsLoading, setRecentCommitsLoading] = useState(false);
  const [recentCommitsError, setRecentCommitsError] = useState<string | null>(null);
  const [selectedGitContext, setSelectedGitContext] = useState<TaskGitContext | null>(null);
  const [selectedGitAction, setSelectedGitAction] = useState<GitActionType | null>(null);
  const [selectedFilePreviewSource, setSelectedFilePreviewSource] =
    useState<ProjectGitPreviewSource | null>(null);
  const [selectedFilePreviewChange, setSelectedFilePreviewChange] =
    useState<ProjectGitFileChangeRef | null>(null);
  const [gitFilePreview, setGitFilePreview] = useState<ProjectGitFilePreview | null>(null);
  const [gitFilePreviewLoading, setGitFilePreviewLoading] = useState(false);
  const [gitFilePreviewError, setGitFilePreviewError] = useState<string | null>(null);
  const [selectedCommit, setSelectedCommit] = useState<ProjectGitCommit | null>(null);
  const [commitDetail, setCommitDetail] = useState<ProjectGitCommitDetail | null>(null);
  const [commitDetailLoading, setCommitDetailLoading] = useState(false);
  const [commitDetailError, setCommitDetailError] = useState<string | null>(null);
  const [stagingFilePath, setStagingFilePath] = useState<string | null>(null);
  const [bulkStageAction, setBulkStageAction] = useState<"stage_all" | "unstage_all" | null>(null);
  const [gitActionNotice, setGitActionNotice] = useState<{
    tone: "success" | "error";
    message: string;
  } | null>(null);
  const [selectedRepoAction, setSelectedRepoAction] = useState<ProjectGitRepoActionType | null>(
    null,
  );
  const [selectedBranchAction, setSelectedBranchAction] =
    useState<ProjectGitBranchActionType | null>(null);
  const [reconcilingContextId, setReconcilingContextId] = useState<string | null>(null);
  const [deletingContextId, setDeletingContextId] = useState<string | null>(null);
  const [pendingDeleteContext, setPendingDeleteContext] = useState<TaskGitContext | null>(null);
  const [gitOverviewReloadNonce, setGitOverviewReloadNonce] = useState(0);
  const [selectedFilesStageAction, setSelectedFilesStageAction] = useState<
    "stage" | "unstage" | null
  >(null);
  const [rollbackConfirm, setRollbackConfirm] = useState<{
    target: "selected" | "all";
    paths: string[];
  } | null>(null);
  const [rollbackInProgress, setRollbackInProgress] = useState(false);
  const recentCommitsRequestIdRef = useRef(0);
  const commitDetailRequestIdRef = useRef(0);
  const filePreviewRequestIdRef = useRef(0);

  const project = projects.find((p) => p.id === id);

  const resetGitFilePreviewState = (nextSource: ProjectGitPreviewSource | null = null) => {
    filePreviewRequestIdRef.current += 1;
    setSelectedFilePreviewSource(nextSource);
    setSelectedFilePreviewChange(null);
    setGitFilePreview(null);
    setGitFilePreviewLoading(false);
    setGitFilePreviewError(null);
  };

  useEffect(() => {
    if (id) {
      fetchTasks(id);
      fetchEmployees();
    }
  }, [id, fetchTasks, fetchEmployees]);

  useEffect(() => {
    if (!id) {
      setProjectEmployees([]);
      return;
    }

    setProjectEmployees(employees.filter((employee) => employee.project_id === id));
  }, [employees, id]);

  useEffect(() => {
    if (!project) {
      setRepoHealth(null);
      setRepoHealthError(null);
      setRepoHealthLoading(false);
      return;
    }

    let active = true;
    setRepoHealthLoading(true);
    setRepoHealthError(null);
    void checkProjectRepoHealth(project.id)
      .then((health) => {
        if (active) {
          setRepoHealth(health);
        }
      })
      .catch((error) => {
        if (active) {
          setRepoHealth(null);
          setRepoHealthError(error instanceof Error ? error.message : String(error));
        }
      })
      .finally(() => {
        if (active) {
          setRepoHealthLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [project]);

  useEffect(() => {
    if (!project) {
      recentCommitsRequestIdRef.current += 1;
      commitDetailRequestIdRef.current += 1;
      setGitOverview(null);
      setGitOverviewError(null);
      setGitOverviewLoading(false);
      setRecentCommits([]);
      setRecentCommitsExpanded(false);
      setRecentCommitsHasMore(false);
      setRecentCommitsLoading(false);
      setRecentCommitsError(null);
      setSelectedGitContext(null);
      setSelectedGitAction(null);
      resetGitFilePreviewState(null);
      setSelectedCommit(null);
      setCommitDetail(null);
      setCommitDetailLoading(false);
      setCommitDetailError(null);
      setPendingDeleteContext(null);
      return;
    }

    let active = true;
    recentCommitsRequestIdRef.current += 1;
    setGitOverviewLoading(true);
    setGitOverviewError(null);
    setRecentCommitsLoading(false);

    void getProjectGitOverview(project.id)
      .then((overview) => {
        if (!active) {
          return;
        }
        recentCommitsRequestIdRef.current += 1;
        commitDetailRequestIdRef.current += 1;
        setGitOverview(overview);
        setRecentCommits(overview.recent_commits);
        setRecentCommitsExpanded(false);
        setRecentCommitsHasMore(overview.recent_commits_has_more);
        setRecentCommitsLoading(false);
        setRecentCommitsError(null);
        resetGitFilePreviewState(null);
        setSelectedCommit(null);
        setCommitDetail(null);
        setCommitDetailLoading(false);
        setCommitDetailError(null);
      })
      .catch((error) => {
        if (!active) {
          return;
        }
        recentCommitsRequestIdRef.current += 1;
        commitDetailRequestIdRef.current += 1;
        setGitOverview(null);
        setGitOverviewError(error instanceof Error ? error.message : String(error));
        setRecentCommits([]);
        setRecentCommitsExpanded(false);
        setRecentCommitsHasMore(false);
        setRecentCommitsLoading(false);
        setRecentCommitsError(null);
        resetGitFilePreviewState(null);
        setSelectedCommit(null);
        setCommitDetail(null);
        setCommitDetailLoading(false);
        setCommitDetailError(null);
      })
      .finally(() => {
        if (active) {
          setGitOverviewLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [gitOverviewReloadNonce, project]);

  const handleGitOverviewRefresh = () => {
    setGitOverviewReloadNonce((value) => value + 1);
  };

  const handleReconcileContext = async (contextId: string) => {
    setReconcilingContextId(contextId);
    setGitActionNotice(null);
    try {
      const refreshed = await reconcileTaskGitContext(contextId);
      setGitActionNotice({
        tone: "success",
        message: t("detailPage.reconcileSuccess", {
          state: getTaskGitContextStateLabel(refreshed.state),
        }),
      });
      setGitOverviewReloadNonce((value) => value + 1);
    } catch (error) {
      setGitActionNotice({
        tone: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setReconcilingContextId(null);
    }
  };

  const handleGitActionCompleted = async (message: string) => {
    setGitActionNotice({ tone: "success", message });
    setGitOverviewReloadNonce((value) => value + 1);
    setSelectedGitContext(null);
    setSelectedGitAction(null);
  };

  const handleDeleteGitContextRecord = async (contextId: string) => {
    setDeletingContextId(contextId);
    setGitActionNotice(null);
    try {
      const message = await deleteTaskGitContextRecord(contextId);
      setGitActionNotice({ tone: "success", message });
      setGitOverviewReloadNonce((value) => value + 1);
      setSelectedGitContext(null);
      setSelectedGitAction(null);
      setPendingDeleteContext(null);
    } catch (error) {
      setGitActionNotice({
        tone: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setDeletingContextId(null);
    }
  };

  const handleRepoActionCompleted = async (message: string) => {
    setGitActionNotice({ tone: "success", message });
    setGitOverviewReloadNonce((value) => value + 1);
    setSelectedRepoAction(null);
  };

  const openGitActionDialog = (context: TaskGitContext, preferredAction?: GitActionType) => {
    setSelectedGitContext(context);
    setSelectedGitAction(preferredAction ?? null);
  };

  const handleOpenProjectGitFile = async (change: ProjectGitWorkingTreeChange) => {
    if (!project) {
      return;
    }

    const requestId = filePreviewRequestIdRef.current + 1;
    filePreviewRequestIdRef.current = requestId;
    setSelectedFilePreviewSource("working_tree");
    setSelectedFilePreviewChange(change);
    setGitFilePreview(null);
    setGitFilePreviewError(null);
    setGitFilePreviewLoading(true);
    try {
      const preview = await getProjectGitFilePreview(
        project.id,
        change.path,
        change.previous_path,
        change.change_type,
      );
      if (filePreviewRequestIdRef.current !== requestId) {
        return;
      }
      setGitFilePreview(preview);
    } catch (error) {
      if (filePreviewRequestIdRef.current !== requestId) {
        return;
      }
      setGitFilePreviewError(error instanceof Error ? error.message : String(error));
    } finally {
      if (filePreviewRequestIdRef.current === requestId) {
        setGitFilePreviewLoading(false);
      }
    }
  };

  const handleOpenCommitFileDiff = async (
    commit: ProjectGitCommit,
    change: ProjectGitCommitFileChange,
  ) => {
    if (!project) {
      return;
    }

    const requestId = filePreviewRequestIdRef.current + 1;
    filePreviewRequestIdRef.current = requestId;
    setSelectedFilePreviewSource("commit");
    setSelectedFilePreviewChange(change);
    setGitFilePreview(null);
    setGitFilePreviewError(null);
    setGitFilePreviewLoading(true);
    try {
      const preview = await getProjectGitCommitFilePreview(
        project.id,
        commit.sha,
        change.path,
        change.previous_path,
        change.change_type,
      );
      if (filePreviewRequestIdRef.current !== requestId) {
        return;
      }
      setGitFilePreview(preview);
    } catch (error) {
      if (filePreviewRequestIdRef.current !== requestId) {
        return;
      }
      setGitFilePreviewError(error instanceof Error ? error.message : String(error));
    } finally {
      if (filePreviewRequestIdRef.current === requestId) {
        setGitFilePreviewLoading(false);
      }
    }
  };

  const handleLoadRecentCommits = async (reset = false) => {
    if (!project || recentCommitsLoading) {
      return;
    }

    const requestId = recentCommitsRequestIdRef.current + 1;
    recentCommitsRequestIdRef.current = requestId;
    const projectId = project.id;
    const offset = reset ? 0 : recentCommits.length;

    setRecentCommitsLoading(true);
    setRecentCommitsError(null);
    try {
      const history = await listProjectGitCommits(projectId, offset, RECENT_COMMIT_PAGE_SIZE);
      if (recentCommitsRequestIdRef.current !== requestId) {
        return;
      }
      setRecentCommits((current) => (reset ? history.commits : [...current, ...history.commits]));
      setRecentCommitsHasMore(history.has_more);
    } catch (error) {
      if (recentCommitsRequestIdRef.current !== requestId) {
        return;
      }
      setRecentCommitsError(error instanceof Error ? error.message : String(error));
    } finally {
      if (recentCommitsRequestIdRef.current === requestId) {
        setRecentCommitsLoading(false);
      }
    }
  };

  const handleExpandRecentCommits = () => {
    setRecentCommitsExpanded(true);
    if (recentCommits.length <= RECENT_COMMIT_SUMMARY_LIMIT && recentCommitsHasMore) {
      void handleLoadRecentCommits(true);
    }
  };

  const handleOpenCommitDetail = async (commit: ProjectGitCommit) => {
    if (!project) {
      return;
    }

    const requestId = commitDetailRequestIdRef.current + 1;
    commitDetailRequestIdRef.current = requestId;
    if (selectedFilePreviewSource === "commit") {
      resetGitFilePreviewState(null);
    }
    setSelectedCommit(commit);
    setCommitDetail(null);
    setCommitDetailError(null);
    setCommitDetailLoading(true);
    try {
      const detail = await getProjectGitCommitDetail(project.id, commit.sha);
      if (commitDetailRequestIdRef.current !== requestId) {
        return;
      }
      setCommitDetail(detail);
    } catch (error) {
      if (commitDetailRequestIdRef.current !== requestId) {
        return;
      }
      setCommitDetailError(error instanceof Error ? error.message : String(error));
    } finally {
      if (commitDetailRequestIdRef.current === requestId) {
        setCommitDetailLoading(false);
      }
    }
  };

  const updateWorkingTreeStageStatuses = (
    updater: (change: ProjectGitWorkingTreeChange) => ProjectGitWorkingTreeChange,
  ) => {
    startTransition(() => {
      setGitOverview((current) => {
        if (!current) {
          return current;
        }
        return {
          ...current,
          working_tree_changes: current.working_tree_changes.map(updater),
        };
      });
      setSelectedFilePreviewChange((current) => {
        if (!current || selectedFilePreviewSource !== "working_tree") {
          return current;
        }
        return updater(current as ProjectGitWorkingTreeChange);
      });
    });
  };

  const handleToggleStageFile = async (change: ProjectGitWorkingTreeChange) => {
    if (!project) {
      return;
    }

    setStagingFilePath(change.path);
    setGitActionNotice(null);
    try {
      const nextStageStatus: ProjectGitWorkingTreeChange["stage_status"] =
        change.stage_status === "staged" || change.stage_status === "partially_staged"
          ? change.change_type === "added" && !change.previous_path
            ? "untracked"
            : "unstaged"
          : "staged";
      const message =
        change.stage_status === "staged" || change.stage_status === "partially_staged"
          ? await unstageProjectGitFile(project.id, change.path)
          : await stageProjectGitFile(project.id, change.path);
      updateWorkingTreeStageStatuses((item) =>
        item.path === change.path
          ? {
              ...item,
              stage_status: nextStageStatus,
            }
          : item,
      );
      setGitActionNotice({ tone: "success", message });
    } catch (error) {
      setGitActionNotice({
        tone: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setStagingFilePath(null);
    }
  };

  const handleBulkStageAction = async (action: "stage_all" | "unstage_all") => {
    if (!project) {
      return;
    }

    setBulkStageAction(action);
    setGitActionNotice(null);
    try {
      const message =
        action === "stage_all"
          ? await stageAllProjectGitFiles(project.id)
          : await unstageAllProjectGitFiles(project.id);
      updateWorkingTreeStageStatuses((change) => ({
        ...change,
        stage_status:
          action === "stage_all"
            ? "staged"
            : change.change_type === "added" && !change.previous_path
              ? "untracked"
              : "unstaged",
      }));
      setGitActionNotice({ tone: "success", message });
    } catch (error) {
      setGitActionNotice({
        tone: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBulkStageAction(null);
    }
  };

  const handleStageSelectedFiles = async (action: "stage" | "unstage", paths: string[]) => {
    if (!project || paths.length === 0) {
      return;
    }
    setSelectedFilesStageAction(action);
    setGitActionNotice(null);
    try {
      for (const path of paths) {
        if (action === "stage") {
          await stageProjectGitFile(project.id, path);
        } else {
          await unstageProjectGitFile(project.id, path);
        }
      }
      updateWorkingTreeStageStatuses((change) => {
        if (!paths.includes(change.path)) return change;
        return {
          ...change,
          stage_status:
            action === "stage"
              ? "staged"
              : change.change_type === "added" && !change.previous_path
                ? "untracked"
                : "unstaged",
        };
      });
      setGitActionNotice({
        tone: "success",
        message:
          action === "stage"
            ? t("worktree.stagedNotice", { count: paths.length })
            : t("worktree.unstagedNotice", { count: paths.length }),
      });
    } catch (error) {
      setGitActionNotice({
        tone: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setSelectedFilesStageAction(null);
    }
  };

  const handleRollback = async (target: "selected" | "all") => {
    if (!project) {
      return;
    }
    if (target === "selected" && (rollbackConfirm?.paths.length ?? 0) === 0) {
      return;
    }
    setRollbackInProgress(true);
    setGitActionNotice(null);
    try {
      let message: string;
      if (target === "all") {
        message = await rollbackAllProjectGitChanges(project.id);
      } else {
        message = await rollbackProjectGitFiles(project.id, rollbackConfirm?.paths ?? []);
      }
      setGitActionNotice({ tone: "success", message });
      setRollbackConfirm(null);
      handleGitOverviewRefresh();
    } catch (error) {
      setGitActionNotice({
        tone: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setRollbackInProgress(false);
    }
  };

  if (!project) {
    return (
      <div className="text-center py-12">
        <p className="text-muted-foreground mb-4">{t("detailPage.projectNotFound")}</p>
        <Link to="/projects" className="text-primary hover:underline">
          {t("detailPage.backToProjects")}
        </Link>
      </div>
    );
  }

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await deleteProject(project.id);
      setShowDeleteConfirm(false);
      navigate("/projects");
    } finally {
      setDeleting(false);
    }
  };

  const activeTasks = tasks.filter((task) => task.status !== "archived");
  const tasksByStatus = {
    todo: activeTasks.filter((t) => t.status === "todo"),
    in_progress: activeTasks.filter((t) => t.status === "in_progress"),
    review: activeTasks.filter((t) => t.status === "review"),
    completed: activeTasks.filter((t) => t.status === "completed"),
    blocked: activeTasks.filter((t) => t.status === "blocked"),
  };
  const hasStagedFiles = countStagedGitFiles(gitOverview?.working_tree_changes ?? []) > 0;
  const stagedFileCount = countStagedGitFiles(gitOverview?.working_tree_changes ?? []);
  const gitRuntimeReady = gitOverview?.git_runtime_status === "ready";
  const aheadCommits = gitOverview?.ahead_commits ?? 0;
  const behindCommits = gitOverview?.behind_commits ?? 0;
  const visibleRecentCommits = recentCommitsExpanded
    ? recentCommits
    : recentCommits.slice(0, RECENT_COMMIT_SUMMARY_LIMIT);
  const canExpandRecentCommits =
    recentCommits.length > RECENT_COMMIT_SUMMARY_LIMIT || recentCommitsHasMore;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Link to="/projects" className="p-1 hover:bg-accent rounded">
          <ArrowLeft className="h-5 w-5" />
        </Link>
        <div className="flex-1">
          <h2 className="text-xl font-bold">{project.name}</h2>
          <div className="flex items-center gap-2 mt-1">
            <Badge variant={project.status === "active" ? "default" : "secondary"}>
              {getStatusLabel(project.status)}
            </Badge>
            <Badge variant="outline">{getProjectTypeLabel(project.project_type)}</Badge>
            <span className="text-xs text-muted-foreground">
              {t("detailPage.createdOn", { date: formatDate(project.created_at) })}
            </span>
          </div>
        </div>
        <Button variant="outline" size="sm" onClick={() => setShowEdit(true)}>
          <Edit2 className="h-3.5 w-3.5 mr-1" />
          {t("common:edit")}
        </Button>
        <Button variant="destructive" size="sm" onClick={() => setShowDeleteConfirm(true)}>
          <Trash2 className="h-3.5 w-3.5 mr-1" />
          {t("common:delete")}
        </Button>
      </div>

      {/* Description */}
      {project.description && (
        <Card className="p-4">
          <p className="text-sm text-muted-foreground">{project.description}</p>
        </Card>
      )}

      <Card className="p-4 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-sm font-semibold">{t("detailPage.repoInfoTitle")}</h3>
          <Button
            size="sm"
            variant="outline"
            disabled={repoHealthLoading || !project}
            onClick={() => {
              if (!project) return;
              setRepoHealthLoading(true);
              setRepoHealthError(null);
              void checkProjectRepoHealth(project.id)
                .then(setRepoHealth)
                .catch((error) => {
                  setRepoHealth(null);
                  setRepoHealthError(error instanceof Error ? error.message : String(error));
                })
                .finally(() => setRepoHealthLoading(false));
            }}
          >
            {repoHealthLoading ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            {t("detailPage.recheck")}
          </Button>
        </div>
        <RepoPathDisplay
          repoPath={getProjectWorkingDir(project)}
          projectType={project.project_type}
          showCopyAction
        />
        {repoHealthError && <p className="text-xs text-destructive">{repoHealthError}</p>}
        {repoHealth && (
          <div
            className={`rounded-md border px-3 py-2 text-xs ${
              repoHealth.accessible
                ? "border-green-500/30 bg-green-500/10 text-green-900 dark:text-green-100"
                : "border-amber-500/30 bg-amber-500/10 text-amber-950 dark:text-amber-100"
            }`}
          >
            <p className="font-medium">{repoHealth.message}</p>
            <ul className="mt-2 space-y-1">
              {repoHealth.checks.map((check) => (
                <li key={check.key} className="flex items-start gap-2">
                  <span
                    className={
                      check.passed
                        ? "text-green-700 dark:text-green-300"
                        : "text-amber-800 dark:text-amber-200"
                    }
                  >
                    {check.passed ? "✓" : "!"}
                  </span>
                  <span>
                    <span className="font-medium">{check.label}：</span>
                    {check.detail}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </Card>

      <Card className="p-4 space-y-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2">
              <GitBranch className="h-4 w-4 text-muted-foreground" />
              <h3 className="text-sm font-semibold">{t("detailPage.gitWorkflowTitle")}</h3>
            </div>
            <p className="mt-1 text-xs text-muted-foreground">{t("detailPage.gitWorkflowHint")}</p>
          </div>
          {gitOverview?.refreshed_at && (
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={handleGitOverviewRefresh}
                disabled={gitOverviewLoading}
              >
                <RefreshCw className="mr-1 h-3.5 w-3.5" />
                {t("common:refresh")}
              </Button>
              <span className="text-[11px] text-muted-foreground">
                {t("detailPage.refreshedAt", { date: formatDate(gitOverview.refreshed_at) })}
              </span>
            </div>
          )}
        </div>

        {gitActionNotice && (
          <div
            className={`rounded-lg border px-3 py-2 text-xs ${
              gitActionNotice.tone === "success"
                ? "border-primary/20 bg-primary/5 text-primary"
                : "border-destructive/20 bg-destructive/10 text-destructive"
            }`}
          >
            {gitActionNotice.message}
          </div>
        )}

        {gitOverviewLoading ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            {t("detailPage.loadingGitOverview")}
          </div>
        ) : gitOverviewError ? (
          <div className="rounded-lg border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            {t("detailPage.gitOverviewError", { message: gitOverviewError })}
          </div>
        ) : gitOverview ? (
          <>
            <div className="grid gap-3 md:grid-cols-4">
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">
                  {t("detailPage.gitRuntime")}
                </div>
                <div className="mt-1 text-sm font-medium">
                  {getGitRuntimeStatusLabel(gitOverview.git_runtime_status)}
                </div>
                <div className="mt-1 text-[11px] text-muted-foreground">
                  {t("detailPage.providerSimpleGit")}
                </div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">
                  {t("detailPage.defaultBranchField")}
                </div>
                <div className="mt-1 text-sm font-medium">
                  {gitOverview.default_branch ?? t("common:unknown")}
                </div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">
                  {t("detailPage.currentBranchField")}
                </div>
                <div className="mt-1 text-sm font-medium">
                  {gitOverview.current_branch ?? t("common:unknown")}
                </div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">HEAD</div>
                <div className="mt-1 text-sm font-medium break-all">
                  {gitOverview.head_commit_sha ?? t("common:unknown")}
                </div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">
                  {t("detailPage.workingTreeSummaryField")}
                </div>
                <div className="mt-1 text-sm font-medium">
                  {gitOverview.working_tree_summary ?? t("detailPage.workingTreeClean")}
                </div>
              </div>
            </div>

            <div className="rounded-lg border border-border/60 p-3">
              <div className="mb-3 flex flex-wrap items-start justify-between gap-3 rounded-lg border border-border/60 bg-secondary/20 px-3 py-3">
                <div>
                  <h4 className="text-sm font-medium">{t("detailPage.repoActions")}</h4>
                  <p className="mt-1 text-[11px] text-muted-foreground">
                    {t("detailPage.repoActionsHint")}
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || !hasStagedFiles}
                    onClick={() => setSelectedRepoAction("commit")}
                    className={getGitActionButtonClassName("positive")}
                  >
                    {t("detailPage.commitShort")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || !gitOverview.current_branch || aheadCommits === 0}
                    onClick={() => setSelectedRepoAction("push")}
                    className={getGitActionButtonClassName("info")}
                  >
                    <span className="inline-flex items-center gap-1">
                      {t("detailPage.pushShort")}
                      <span className="inline-flex items-center gap-0.5 text-sky-600 dark:text-sky-300">
                        <ArrowUp className="h-3.5 w-3.5" />
                        {aheadCommits}
                      </span>
                    </span>
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={
                      !gitRuntimeReady || !gitOverview.current_branch || behindCommits === 0
                    }
                    onClick={() => setSelectedRepoAction("pull")}
                    className={getGitActionButtonClassName("warning")}
                  >
                    <span className="inline-flex items-center gap-1">
                      {t("detailPage.pullShort")}
                      <span className="inline-flex items-center gap-0.5 text-amber-600 dark:text-amber-300">
                        <ArrowDown className="h-3.5 w-3.5" />
                        {behindCommits}
                      </span>
                    </span>
                  </Button>
                  <span className="mx-1 h-5 w-px bg-border/60" aria-hidden />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || gitOverview.project_branches.length < 2}
                    onClick={() => setSelectedBranchAction("switch")}
                    className={getGitActionButtonClassName("neutral")}
                  >
                    {t("gitBranchDialog.switch")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady}
                    onClick={() => setSelectedBranchAction("create")}
                    className={getGitActionButtonClassName("create")}
                  >
                    {t("gitBranchDialog.create")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || gitOverview.project_branches.length < 2}
                    onClick={() => setSelectedBranchAction("delete")}
                    className={getGitActionButtonClassName("danger")}
                  >
                    {t("gitBranchDialog.delete")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || gitOverview.project_branches.length < 2}
                    onClick={() => setSelectedBranchAction("merge")}
                    className={getGitActionButtonClassName("merge")}
                  >
                    {t("gitBranchDialog.merge")}
                  </Button>
                </div>
              </div>

              <GitChangesPanel
                title={t("detailPage.workingTreeTitle")}
                description={t("detailPage.workingTreeDescription")}
                changes={gitOverview.working_tree_changes}
                stagingFilePath={stagingFilePath}
                bulkStageAction={bulkStageAction}
                selectedFilesStageAction={selectedFilesStageAction}
                rollbackInProgress={rollbackInProgress}
                onToggleStage={(change) => {
                  void handleToggleStageFile(change);
                }}
                onBulkStage={(action) => {
                  void handleBulkStageAction(action);
                }}
                onStageSelected={(action, paths) => {
                  void handleStageSelectedFiles(action, paths);
                }}
                onRollback={(target, paths) => {
                  setRollbackConfirm({
                    target,
                    paths: paths ?? [],
                  });
                }}
                onPreview={(change) => {
                  void handleOpenProjectGitFile(change);
                }}
              />
            </div>

            {gitOverview.git_runtime_message && (
              <div
                className={`rounded-lg border px-3 py-2 text-xs ${
                  gitOverview.git_runtime_status === "unavailable"
                    ? "border-amber-500/30 bg-amber-500/10 text-amber-900 dark:text-amber-200"
                    : "border-border/60 bg-secondary/30 text-muted-foreground"
                }`}
              >
                <div className="flex items-center gap-2 font-medium">
                  <ShieldAlert className="h-3.5 w-3.5" />
                  {getGitRuntimeStatusLabel(gitOverview.git_runtime_status)}
                </div>
                <p className="mt-1">{gitOverview.git_runtime_message}</p>
              </div>
            )}

            <ProjectWorktreeSection
              projectId={project.id}
              currentBranch={gitOverview.current_branch}
              defaultBranch={gitOverview.default_branch}
              projectBranches={gitOverview.project_branches}
              onChanged={async () => {
                handleGitOverviewRefresh();
              }}
            />

            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-lg border border-border/60 p-3">
                <div className="mb-2 flex items-center justify-between">
                  <h4 className="text-sm font-medium">{t("detailPage.activeContextsTitle")}</h4>
                  <span className="text-xs text-muted-foreground">
                    {t("detailPage.countSuffix", { count: gitOverview.active_contexts.length })}
                  </span>
                </div>
                {gitOverview.active_contexts.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    {t("detailPage.noActiveContexts")}
                  </p>
                ) : (
                  <div className="space-y-2">
                    {gitOverview.active_contexts.slice(0, 3).map((context) => (
                      <div
                        key={context.id}
                        className="rounded-md bg-secondary/40 px-2.5 py-2 text-xs"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="font-medium">
                            {context.task_branch ?? t("unnamedBranch")}
                          </span>
                          <Badge variant="outline">
                            {getTaskGitContextStateLabel(context.state)}
                          </Badge>
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          {t("targetBranch", { branch: context.target_branch ?? t("notSet") })}
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          {t("detailPage.contextUpdatedAt", {
                            date: formatDate(context.updated_at),
                          })}
                        </div>
                        {context.last_error && (
                          <div className="mt-1 text-destructive">
                            {t("detailPage.contextLastError", {
                              message: context.last_error,
                            })}
                          </div>
                        )}
                        {context.worktree_missing && (
                          <div className="mt-1 text-muted-foreground">
                            {t("detailPage.contextWorktreeMissing")}
                          </div>
                        )}
                        <div className="mt-2 flex flex-wrap items-center gap-2">
                          {context.worktree_missing ? (
                            <>
                              {context.state === "drifted" && (
                                <Button
                                  variant="outline"
                                  size="sm"
                                  onClick={() => {
                                    void handleReconcileContext(context.id);
                                  }}
                                  disabled={reconcilingContextId === context.id}
                                >
                                  {reconcilingContextId === context.id
                                    ? t("detailPage.fixing")
                                    : t("detailPage.fixContext")}
                                </Button>
                              )}
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => setPendingDeleteContext(context)}
                                disabled={deletingContextId === context.id}
                              >
                                {deletingContextId === context.id
                                  ? t("worktree.deletingWorktree")
                                  : t("detailPage.deleteRecord")}
                              </Button>
                            </>
                          ) : context.state === "drifted" ? (
                            <>
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => {
                                  void handleReconcileContext(context.id);
                                }}
                                disabled={reconcilingContextId === context.id}
                              >
                                {reconcilingContextId === context.id
                                  ? t("detailPage.fixing")
                                  : t("detailPage.fixContext")}
                              </Button>
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => openGitActionDialog(context, "cleanup_worktree")}
                              >
                                {t("detailPage.cleanupDirectly")}
                              </Button>
                            </>
                          ) : context.state !== "failed" && context.state !== "completed" ? (
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => openGitActionDialog(context)}
                            >
                              {t("detailPage.gitAction")}
                            </Button>
                          ) : null}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="rounded-lg border border-border/60 p-3">
                <div className="mb-2 flex items-center justify-between">
                  <h4 className="text-sm font-medium">{t("detailPage.pendingActionsTitle")}</h4>
                  <span className="text-xs text-muted-foreground">
                    {t("detailPage.countSuffix", {
                      count: gitOverview.pending_action_contexts.length,
                    })}
                  </span>
                </div>
                {gitOverview.pending_action_contexts.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    {t("detailPage.noPendingActions")}
                  </p>
                ) : (
                  <div className="space-y-2">
                    {gitOverview.pending_action_contexts.slice(0, 3).map((context) => (
                      <div
                        key={context.id}
                        className="rounded-md bg-secondary/40 px-2.5 py-2 text-xs"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="font-medium">
                            {getGitActionTypeLabel(context.pending_action_type)}
                          </span>
                          <Badge variant="outline">
                            {getTaskGitContextStateLabel(context.state)}
                          </Badge>
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          {t("detailPage.requestedAt", {
                            time: context.pending_action_requested_at
                              ? formatDate(context.pending_action_requested_at)
                              : t("common:unknown"),
                          })}
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          {t("detailPage.expiresAtValue", {
                            time: context.pending_action_expires_at
                              ? formatDate(context.pending_action_expires_at)
                              : t("common:unknown"),
                          })}
                        </div>
                        <div className="mt-2">
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => openGitActionDialog(context)}
                          >
                            {t("detailPage.continueProcessing")}
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>

            <div className="rounded-lg border border-border/60 p-3">
              <div className="mb-2 flex items-center justify-between">
                <div>
                  <h4 className="text-sm font-medium">{t("detailPage.recentCommitsTitle")}</h4>
                  <p className="text-[11px] text-muted-foreground">
                    {recentCommitsExpanded
                      ? t("detailPage.loadedCount", { count: recentCommits.length })
                      : t("detailPage.defaultSummary", {
                          count: Math.min(recentCommits.length, RECENT_COMMIT_SUMMARY_LIMIT),
                        })}
                  </p>
                </div>
                <span className="text-xs text-muted-foreground">
                  {recentCommitsExpanded
                    ? t("detailPage.countSuffix", { count: recentCommits.length })
                    : t("detailPage.countSuffix", { count: visibleRecentCommits.length })}
                </span>
              </div>
              {recentCommits.length === 0 ? (
                <p className="text-xs text-muted-foreground">{t("detailPage.noRecentCommits")}</p>
              ) : (
                <>
                  <div className="space-y-2">
                    {visibleRecentCommits.map((commit) => (
                      <button
                        key={commit.sha}
                        type="button"
                        className="w-full rounded-md bg-secondary/30 px-2.5 py-2 text-left text-xs transition-colors hover:bg-secondary/50"
                        onClick={() => {
                          void handleOpenCommitDetail(commit);
                        }}
                      >
                        <div className="flex items-start justify-between gap-3">
                          <span className="font-medium break-all">{commit.subject}</span>
                          <span className="shrink-0 text-muted-foreground">
                            {commit.short_sha ?? commit.sha.slice(0, 7)}
                          </span>
                        </div>
                        <div className="mt-1 flex flex-wrap items-center justify-between gap-2 text-muted-foreground">
                          <span>
                            {commit.author_name ?? t("detailPage.unknownAuthor")} ·{" "}
                            {formatDate(commit.authored_at)}
                          </span>
                          <span className="text-primary">{t("viewDetails")}</span>
                        </div>
                      </button>
                    ))}
                  </div>

                  {recentCommitsError && (
                    <div className="mt-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                      {recentCommitsError}
                    </div>
                  )}

                  {(canExpandRecentCommits || recentCommitsExpanded) && (
                    <div className="mt-3 flex flex-wrap items-center gap-2">
                      {!recentCommitsExpanded ? (
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={recentCommitsLoading}
                          onClick={handleExpandRecentCommits}
                        >
                          {recentCommitsLoading
                            ? t("common:loading")
                            : t("detailPage.viewMoreHistory")}
                        </Button>
                      ) : (
                        <>
                          {recentCommitsHasMore && (
                            <Button
                              variant="outline"
                              size="sm"
                              disabled={recentCommitsLoading}
                              onClick={() => {
                                void handleLoadRecentCommits(false);
                              }}
                            >
                              {recentCommitsLoading
                                ? t("common:loading")
                                : t("detailPage.viewMore")}
                            </Button>
                          )}
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setRecentCommitsExpanded(false)}
                          >
                            {t("detailPage.collapseSummary")}
                          </Button>
                          {!recentCommitsHasMore && (
                            <span className="text-[11px] text-muted-foreground">
                              {t("detailPage.allCommitsShown")}
                            </span>
                          )}
                        </>
                      )}
                    </div>
                  )}
                </>
              )}
            </div>
          </>
        ) : (
          <div className="rounded-lg border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            {t("detailPage.noGitOverview")}
          </div>
        )}
      </Card>

      {/* Task Stats */}
      <div className="grid grid-cols-5 gap-3">
        {Object.entries(tasksByStatus).map(([status, items]) => (
          <Card key={status} className="p-3 text-center">
            <div className={`w-2 h-2 rounded-full mx-auto mb-1 ${getStatusColor(status)}`} />
            <div className="text-lg font-bold">{items.length}</div>
            <div className="text-xs text-muted-foreground">{getStatusLabel(status)}</div>
          </Card>
        ))}
      </div>

      <ProjectMilestonesSection projectId={project.id} />

      {/* Task List */}
      <Card className="p-4">
        <h3 className="text-sm font-semibold mb-3">{t("detailPage.taskListTitle")}</h3>
        {activeTasks.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">
            {t("detailPage.noActiveTasks")}
          </p>
        ) : (
          <div className="space-y-2">
            {activeTasks.map((task) => (
              <div
                key={task.id}
                className="flex items-center gap-3 p-2 rounded-md hover:bg-accent/50 text-sm"
              >
                <div className={`w-1.5 h-1.5 rounded-full ${getStatusColor(task.status)}`} />
                <span className="flex-1 font-medium truncate">{task.title}</span>
                {isTaskOverdue(task) && (
                  <span className="text-[11px] text-destructive">{t("detailPage.overdue")}</span>
                )}
                {task.due_date && !isTaskOverdue(task) && (
                  <span className="text-[11px] text-muted-foreground">
                    {t("detailPage.dueBy", { date: formatDate(task.due_date) })}
                  </span>
                )}
                <span className="text-xs text-muted-foreground">
                  {getPriorityLabel(task.priority)}
                </span>
              </div>
            ))}
          </div>
        )}
      </Card>

      {/* Team Members */}
      <Card className="p-4">
        <h3 className="text-sm font-semibold mb-3">{t("detailPage.teamMembers")}</h3>
        {projectEmployees.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">
            {t("detailPage.noMembers")}
          </p>
        ) : (
          <div className="flex flex-wrap gap-2">
            {projectEmployees.map((emp) => (
              <div
                key={emp.id}
                className="flex items-center gap-2 px-3 py-1.5 bg-secondary rounded-full text-sm"
              >
                <div className={`w-2 h-2 rounded-full ${getStatusColor(emp.status)}`} />
                <span>{emp.name}</span>
                <span className="text-xs text-muted-foreground">{emp.role}</span>
              </div>
            ))}
          </div>
        )}
      </Card>

      <EditProjectDialog open={showEdit} onOpenChange={setShowEdit} project={project} />

      <DeleteProjectDialog
        open={showDeleteConfirm}
        onOpenChange={(open) => {
          if (!open && !deleting) setShowDeleteConfirm(false);
        }}
        project={project}
        deleting={deleting}
        onConfirm={handleDelete}
      />

      <DeleteTaskGitContextDialog
        open={pendingDeleteContext !== null}
        context={pendingDeleteContext}
        deleting={pendingDeleteContext !== null && deletingContextId === pendingDeleteContext.id}
        onOpenChange={(open) => {
          if (!open && !deletingContextId) {
            setPendingDeleteContext(null);
          }
        }}
        onConfirm={() => {
          if (!pendingDeleteContext) {
            return;
          }
          return handleDeleteGitContextRecord(pendingDeleteContext.id);
        }}
      />

      <Suspense fallback={null}>
        {selectedFilePreviewChange && (
          <ProjectGitFilePreviewDialog
            open={selectedFilePreviewChange !== null}
            loading={gitFilePreviewLoading}
            error={gitFilePreviewError}
            preview={gitFilePreview}
            change={selectedFilePreviewChange}
            onOpenChange={(open) => {
              if (!open) {
                resetGitFilePreviewState(null);
              }
            }}
          />
        )}
      </Suspense>

      <ProjectGitCommitDetailDialog
        open={selectedCommit !== null}
        loading={commitDetailLoading}
        error={commitDetailError}
        detail={commitDetail}
        commit={selectedCommit}
        onOpenFileDiff={(changeIndex) => {
          if (!selectedCommit || !commitDetail?.changed_files[changeIndex]) {
            return;
          }
          void handleOpenCommitFileDiff(selectedCommit, commitDetail.changed_files[changeIndex]);
        }}
        onOpenChange={(open) => {
          if (!open) {
            commitDetailRequestIdRef.current += 1;
            if (selectedFilePreviewSource === "commit") {
              resetGitFilePreviewState(null);
            }
            setSelectedCommit(null);
            setCommitDetail(null);
            setCommitDetailLoading(false);
            setCommitDetailError(null);
          }
        }}
      />

      <ProjectGitActionDialog
        open={selectedGitContext !== null}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedGitContext(null);
            setSelectedGitAction(null);
          }
        }}
        context={selectedGitContext}
        projectBranches={gitOverview?.project_branches ?? []}
        preferredAction={selectedGitAction}
        onActionCompleted={handleGitActionCompleted}
      />

      <ProjectGitRepoActionDialog
        open={selectedRepoAction !== null}
        action={selectedRepoAction}
        projectId={project.id}
        currentBranch={gitOverview?.current_branch}
        workingTreeSummary={gitOverview?.working_tree_summary ?? null}
        projectBranches={gitOverview?.project_branches ?? []}
        stagedFileCount={stagedFileCount}
        stagedChanges={
          gitOverview?.working_tree_changes.filter(
            (change) =>
              change.stage_status === "staged" || change.stage_status === "partially_staged",
          ) ?? []
        }
        onOpenChange={(open) => {
          if (!open) {
            setSelectedRepoAction(null);
          }
        }}
        onActionCompleted={handleRepoActionCompleted}
      />

      <ProjectGitBranchActionDialog
        open={selectedBranchAction !== null}
        action={selectedBranchAction}
        projectId={project.id}
        currentBranch={gitOverview?.current_branch}
        defaultBranch={gitOverview?.default_branch}
        projectBranches={gitOverview?.project_branches ?? []}
        workingTreeSummary={gitOverview?.working_tree_summary ?? null}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedBranchAction(null);
          }
        }}
        onActionCompleted={handleRepoActionCompleted}
      />

      <Dialog
        open={rollbackConfirm !== null}
        onOpenChange={(open) => {
          if (!open && !rollbackInProgress) {
            setRollbackConfirm(null);
          }
        }}
      >
        <DialogContent className="max-w-md" showCloseButton={!rollbackInProgress}>
          <DialogHeader>
            <DialogTitle>
              {rollbackConfirm?.target === "all"
                ? t("detailPage.rollbackAllTitle")
                : t("worktree.rollbackSelectedTitle", {
                    count: rollbackConfirm?.paths.length ?? 0,
                  })}
            </DialogTitle>
            <DialogDescription>{t("detailPage.rollbackDescription")}</DialogDescription>
          </DialogHeader>

          {rollbackConfirm?.target === "selected" && (rollbackConfirm.paths.length ?? 0) > 0 && (
            <div className="max-h-40 overflow-y-auto rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
              {rollbackConfirm?.paths.map((path) => (
                <div key={path} className="break-all font-mono py-0.5">
                  {path}
                </div>
              ))}
            </div>
          )}

          <DialogFooter className="mt-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => setRollbackConfirm(null)}
              disabled={rollbackInProgress}
            >
              {t("common:cancel")}
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void handleRollback(rollbackConfirm?.target ?? "all")}
              disabled={rollbackInProgress}
            >
              {rollbackInProgress ? t("worktree.rollbacking") : t("worktree.confirmRollback")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
