import { Suspense, lazy, startTransition, useEffect, useRef, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import {
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
} from "@/lib/backend";
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
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import { ArrowDown, ArrowLeft, ArrowUp, Edit2, GitBranch, Loader2, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { getStatusLabel, getStatusColor, getPriorityLabel, formatDate } from "@/lib/utils";
import { getProjectWorkingDir, getProjectTypeLabel } from "@/lib/projects";

const ProjectGitFilePreviewDialog = lazy(async () => {
  const module = await import("@/components/projects/ProjectGitFilePreviewDialog");
  return { default: module.ProjectGitFilePreviewDialog };
});

const RECENT_COMMIT_SUMMARY_LIMIT = 5;
const RECENT_COMMIT_PAGE_SIZE = 20;
type ProjectGitPreviewSource = "working_tree" | "commit";

function getTaskGitContextStateLabel(state: string) {
  switch (state) {
    case "provisioning":
      return "准备中";
    case "ready":
      return "可执行";
    case "running":
      return "执行中";
    case "merge_ready":
      return "待合并";
    case "action_pending":
      return "待确认";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    case "drifted":
      return "上下文失效";
    default:
      return state;
  }
}

function getGitActionTypeLabel(actionType: string | null | undefined) {
  switch (actionType) {
    case "merge":
      return "将任务分支合并到目标分支";
    case "push":
      return "推送分支";
    case "rebase":
      return "变基到目标分支";
    case "cherry_pick":
      return "挑拣提交（Cherry-pick）";
    case "stash":
      return "暂存当前改动（Stash）";
    case "unstash":
      return "恢复暂存改动（Unstash）";
    case "cleanup_worktree":
      return "清理任务工作树";
    default:
      return actionType ?? "待确认操作";
  }
}

function getGitRuntimeStatusLabel(status: string) {
  switch (status) {
    case "ready":
      return "运行时已就绪";
    case "bootstrapping":
      return "运行时准备中";
    case "unavailable":
      return "运行时不可用";
    default:
      return status;
  }
}

function getWorkingTreeChangeLabel(changeType: ProjectGitWorkingTreeChange["change_type"]) {
  switch (changeType) {
    case "added":
      return "新增";
    case "modified":
      return "修改";
    case "deleted":
      return "删除";
    case "renamed":
      return "重命名";
    default:
      return changeType;
  }
}

function getWorkingTreeChangeClassName(changeType: ProjectGitWorkingTreeChange["change_type"]) {
  switch (changeType) {
    case "added":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
    case "modified":
      return "border-sky-500/30 bg-sky-500/10 text-sky-700";
    case "deleted":
      return "border-rose-500/30 bg-rose-500/10 text-rose-700";
    case "renamed":
      return "border-amber-500/30 bg-amber-500/10 text-amber-700";
    default:
      return "border-border/60 bg-secondary/40 text-foreground";
  }
}

function getWorkingTreeStageStatusLabel(stageStatus: ProjectGitWorkingTreeChange["stage_status"]) {
  switch (stageStatus) {
    case "staged":
      return "已暂存";
    case "unstaged":
      return "未暂存";
    case "partially_staged":
      return "部分暂存";
    case "untracked":
      return "未跟踪";
    default:
      return stageStatus;
  }
}

function getWorkingTreeStageStatusClassName(stageStatus: ProjectGitWorkingTreeChange["stage_status"]) {
  switch (stageStatus) {
    case "staged":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
    case "partially_staged":
      return "border-amber-500/30 bg-amber-500/10 text-amber-700";
    case "untracked":
      return "border-violet-500/30 bg-violet-500/10 text-violet-700";
    case "unstaged":
    default:
      return "border-border/60 bg-secondary/40 text-foreground";
  }
}

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { projects, deleteProject } = useProjectStore();
  const { tasks, fetchTasks } = useTaskStore();
  const { employees, fetchEmployees } = useEmployeeStore();
  const [projectEmployees, setProjectEmployees] = useState<Employee[]>([]);
  const [showEdit, setShowEdit] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
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
  const [selectedFilePreviewSource, setSelectedFilePreviewSource] = useState<ProjectGitPreviewSource | null>(null);
  const [selectedFilePreviewChange, setSelectedFilePreviewChange] = useState<ProjectGitFileChangeRef | null>(null);
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
  const [selectedRepoAction, setSelectedRepoAction] = useState<ProjectGitRepoActionType | null>(null);
  const [reconcilingContextId, setReconcilingContextId] = useState<string | null>(null);
  const [deletingContextId, setDeletingContextId] = useState<string | null>(null);
  const [pendingDeleteContext, setPendingDeleteContext] = useState<TaskGitContext | null>(null);
  const [gitOverviewReloadNonce, setGitOverviewReloadNonce] = useState(0);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [selectedFilesStageAction, setSelectedFilesStageAction] = useState<"stage" | "unstage" | null>(null);
  const [rollbackConfirm, setRollbackConfirm] = useState<{ target: "selected" | "all" } | null>(null);
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
        message: `已修复 Git 上下文，当前状态：${getTaskGitContextStateLabel(refreshed.state)}`,
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

  const handleOpenCommitFileDiff = async (commit: ProjectGitCommit, change: ProjectGitCommitFileChange) => {
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
          ? (change.change_type === "added" && !change.previous_path ? "untracked" : "unstaged")
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
            : (change.change_type === "added" && !change.previous_path ? "untracked" : "unstaged"),
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

  const handleStageSelectedFiles = async (action: "stage" | "unstage") => {
    if (!project || selectedFiles.size === 0) {
      return;
    }
    setSelectedFilesStageAction(action);
    setGitActionNotice(null);
    try {
      const paths = Array.from(selectedFiles);
      for (const path of paths) {
        if (action === "stage") {
          await stageProjectGitFile(project.id, path);
        } else {
          await unstageProjectGitFile(project.id, path);
        }
      }
      updateWorkingTreeStageStatuses((change) => {
        if (!selectedFiles.has(change.path)) return change;
        return {
          ...change,
          stage_status:
            action === "stage"
              ? "staged"
              : (change.change_type === "added" && !change.previous_path ? "untracked" : "unstaged"),
        };
      });
      setGitActionNotice({
        tone: "success",
        message: `已${action === "stage" ? "暂存" : "取消暂存"} ${paths.length} 个文件`,
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
    setRollbackInProgress(true);
    setGitActionNotice(null);
    try {
      let message: string;
      if (target === "all") {
        message = await rollbackAllProjectGitChanges(project.id);
      } else {
        message = await rollbackProjectGitFiles(project.id, Array.from(selectedFiles));
        setSelectedFiles(new Set());
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
        <p className="text-muted-foreground mb-4">项目不存在</p>
        <Link to="/projects" className="text-primary hover:underline">
          返回项目列表
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
  const hasStageableFiles = Boolean(
    gitOverview?.working_tree_changes.some(
      (change) =>
        change.stage_status === "unstaged"
        || change.stage_status === "untracked"
        || change.stage_status === "partially_staged",
    ),
  );
  const hasStagedFiles = Boolean(
    gitOverview?.working_tree_changes.some(
      (change) =>
        change.stage_status === "staged"
        || change.stage_status === "partially_staged",
    ),
  );
  const stagedFileCount = gitOverview?.working_tree_changes.filter(
    (change) =>
      change.stage_status === "staged"
      || change.stage_status === "partially_staged",
  ).length ?? 0;
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
              创建于 {formatDate(project.created_at)}
            </span>
          </div>
        </div>
        <Button variant="outline" size="sm" onClick={() => setShowEdit(true)}>
          <Edit2 className="h-3.5 w-3.5 mr-1" />
          编辑
        </Button>
        <Button variant="destructive" size="sm" onClick={() => setShowDeleteConfirm(true)}>
          <Trash2 className="h-3.5 w-3.5 mr-1" />
          删除
        </Button>
      </div>

      {/* Description */}
      {project.description && (
        <Card className="p-4">
          <p className="text-sm text-muted-foreground">{project.description}</p>
        </Card>
      )}

      <Card className="p-4">
        <h3 className="mb-3 text-sm font-semibold">仓库信息</h3>
        <RepoPathDisplay
          repoPath={getProjectWorkingDir(project)}
          projectType={project.project_type}
          showCopyAction
        />
      </Card>

      <Card className="p-4 space-y-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2">
              <GitBranch className="h-4 w-4 text-muted-foreground" />
              <h3 className="text-sm font-semibold">Git 工作流</h3>
            </div>
            <p className="mt-1 text-xs text-muted-foreground">
              项目级 Git 概览、任务上下文与待确认操作入口。
            </p>
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
                刷新
              </Button>
              <span className="text-[11px] text-muted-foreground">
                刷新于 {formatDate(gitOverview.refreshed_at)}
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
            正在加载 Git 概览...
          </div>
        ) : gitOverviewError ? (
          <div className="rounded-lg border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            Git 概览暂不可用：{gitOverviewError}
          </div>
        ) : gitOverview ? (
          <>
            <div className="grid gap-3 md:grid-cols-4">
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">Git 运行时</div>
                <div className="mt-1 text-sm font-medium">{getGitRuntimeStatusLabel(gitOverview.git_runtime_status)}</div>
                <div className="mt-1 text-[11px] text-muted-foreground">提供方：simple-git</div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">默认分支</div>
                <div className="mt-1 text-sm font-medium">{gitOverview.default_branch ?? "未知"}</div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">当前分支</div>
                <div className="mt-1 text-sm font-medium">{gitOverview.current_branch ?? "未知"}</div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">HEAD</div>
                <div className="mt-1 text-sm font-medium break-all">{gitOverview.head_commit_sha ?? "未知"}</div>
              </div>
              <div className="rounded-lg border border-border/60 px-3 py-2">
                <div className="text-[11px] text-muted-foreground">工作区摘要</div>
                <div className="mt-1 text-sm font-medium">{gitOverview.working_tree_summary ?? "工作区干净"}</div>
              </div>
            </div>

            <div className="rounded-lg border border-border/60 p-3">
              <div className="mb-3 flex flex-wrap items-start justify-between gap-3 rounded-lg border border-border/60 bg-secondary/20 px-3 py-3">
                <div>
                  <h4 className="text-sm font-medium">仓库操作</h4>
                  <p className="mt-1 text-[11px] text-muted-foreground">
                    直接基于当前项目仓库执行提交、推送和拉取，默认使用当前分支。
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || !hasStagedFiles}
                    onClick={() => setSelectedRepoAction("commit")}
                  >
                    提交
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || !gitOverview.current_branch || aheadCommits === 0}
                    onClick={() => setSelectedRepoAction("push")}
                  >
                    <span className="inline-flex items-center gap-1">
                      推送
                      <span className="inline-flex items-center gap-0.5 text-sky-600">
                        <ArrowUp className="h-3.5 w-3.5" />
                        {aheadCommits}
                      </span>
                    </span>
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!gitRuntimeReady || !gitOverview.current_branch || behindCommits === 0}
                    onClick={() => setSelectedRepoAction("pull")}
                  >
                    <span className="inline-flex items-center gap-1">
                      拉取
                      <span className="inline-flex items-center gap-0.5 text-amber-600">
                        <ArrowDown className="h-3.5 w-3.5" />
                        {behindCommits}
                      </span>
                    </span>
                  </Button>
                </div>
              </div>

              <div className="mb-2 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  {gitOverview.working_tree_changes.length > 0 && (
                    <input
                      type="checkbox"
                      className="h-3.5 w-3.5 cursor-pointer rounded"
                      checked={
                        gitOverview.working_tree_changes.length > 0 &&
                        gitOverview.working_tree_changes.slice(0, 20).every((c) => selectedFiles.has(c.path))
                      }
                      onChange={(e) => {
                        const paths = gitOverview.working_tree_changes.slice(0, 20).map((c) => c.path);
                        setSelectedFiles(e.target.checked ? new Set(paths) : new Set());
                      }}
                      title="全选/取消全选"
                    />
                  )}
                  <div>
                    <h4 className="text-sm font-medium">工作区文件</h4>
                    <p className="text-[11px] text-muted-foreground">
                      当前仓库实时变更文件列表，可直接在应用内预览文件内容。
                    </p>
                  </div>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-xs text-muted-foreground">{gitOverview.working_tree_changes.length} 条</span>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!hasStageableFiles || bulkStageAction !== null}
                    onClick={() => {
                      void handleBulkStageAction("stage_all");
                    }}
                  >
                    {bulkStageAction === "stage_all" ? "暂存中..." : "全部暂存"}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={!hasStagedFiles || bulkStageAction !== null}
                    onClick={() => {
                      void handleBulkStageAction("unstage_all");
                    }}
                  >
                    {bulkStageAction === "unstage_all" ? "取消中..." : "全部取消暂存"}
                  </Button>
                  {selectedFiles.size > 0 && (
                    <>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={selectedFilesStageAction !== null || rollbackInProgress}
                        onClick={() => void handleStageSelectedFiles("stage")}
                      >
                        {selectedFilesStageAction === "stage" ? "暂存中..." : `暂存选中 (${selectedFiles.size})`}
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={selectedFilesStageAction !== null || rollbackInProgress}
                        onClick={() => void handleStageSelectedFiles("unstage")}
                      >
                        {selectedFilesStageAction === "unstage" ? "取消中..." : `取消暂存选中 (${selectedFiles.size})`}
                      </Button>
                    </>
                  )}
                  {selectedFiles.size > 0 && (
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={rollbackInProgress}
                      onClick={() => setRollbackConfirm({ target: "selected" })}
                      className="border-orange-500/50 text-orange-700 hover:bg-orange-50 hover:text-orange-800"
                    >
                      回滚选中 ({selectedFiles.size})
                    </Button>
                  )}
                  {gitOverview.working_tree_changes.length > 0 && (
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={rollbackInProgress}
                      onClick={() => setRollbackConfirm({ target: "all" })}
                      className="border-red-500/50 text-red-700 hover:bg-red-50 hover:text-red-800"
                    >
                      全局回滚
                    </Button>
                  )}
                </div>
              </div>
              {gitOverview.working_tree_changes.length === 0 ? (
                <div className="rounded-md border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
                  当前没有可展示的工作区文件变更。
                </div>
              ) : (
                <div className="space-y-2">
                  {gitOverview.working_tree_changes.slice(0, 20).map((change) => (
                    <div
                      key={`${change.change_type}:${change.previous_path ?? ""}:${change.path}`}
                      className={`rounded-md border border-border/60 bg-secondary/20 px-3 py-2 text-xs ${change.can_open_file ? "cursor-pointer transition-colors hover:border-primary/40 hover:bg-secondary/30" : ""}`}
                      role={change.can_open_file ? "button" : undefined}
                      tabIndex={change.can_open_file ? 0 : undefined}
                      onClick={() => {
                        if (change.can_open_file) {
                          void handleOpenProjectGitFile(change);
                        }
                      }}
                      onKeyDown={(event) => {
                        if (!change.can_open_file) {
                          return;
                        }
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          void handleOpenProjectGitFile(change);
                        }
                      }}
                    >
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <div className="flex min-w-0 flex-wrap items-center gap-2">
                          <input
                            type="checkbox"
                            className="h-3.5 w-3.5 cursor-pointer rounded"
                            checked={selectedFiles.has(change.path)}
                            onChange={(e) => {
                              e.stopPropagation();
                              setSelectedFiles((prev) => {
                                const next = new Set(prev);
                                if (e.target.checked) {
                                  next.add(change.path);
                                } else {
                                  next.delete(change.path);
                                }
                                return next;
                              });
                            }}
                            onClick={(e) => e.stopPropagation()}
                          />
                          <span
                            className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getWorkingTreeChangeClassName(change.change_type)}`}
                          >
                            {getWorkingTreeChangeLabel(change.change_type)}
                          </span>
                          <span
                            className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getWorkingTreeStageStatusClassName(change.stage_status)}`}
                          >
                            {getWorkingTreeStageStatusLabel(change.stage_status)}
                          </span>
                          <span className="break-all font-mono text-foreground">{change.path}</span>
                        </div>
                        <div className="flex flex-wrap items-center gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={(event) => {
                              event.stopPropagation();
                              void handleOpenProjectGitFile(change);
                            }}
                            disabled={!change.can_open_file}
                            title={
                              change.can_open_file
                                ? "使用内置代码预览浏览当前文件"
                                : change.change_type === "deleted"
                                    ? "已删除文件无法直接浏览"
                                    : "当前文件暂不可浏览"
                            }
                          >
                            浏览文件
                          </Button>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={(event) => {
                              event.stopPropagation();
                              void handleToggleStageFile(change);
                            }}
                            disabled={stagingFilePath === change.path || bulkStageAction !== null}
                          >
                            {stagingFilePath === change.path
                              ? (change.stage_status === "staged" || change.stage_status === "partially_staged"
                                  ? "取消中..."
                                  : "暂存中...")
                              : (change.stage_status === "staged" || change.stage_status === "partially_staged"
                                  ? "取消暂存"
                                  : "暂存")}
                          </Button>
                        </div>
                      </div>
                      {change.previous_path && (
                        <div className="mt-1 text-[11px] text-muted-foreground">
                          原路径：<span className="break-all font-mono">{change.previous_path}</span>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>

            {gitOverview.git_runtime_message && (
              <div
                className={`rounded-lg border px-3 py-2 text-xs ${
                  gitOverview.git_runtime_status === "unavailable"
                    ? "border-amber-500/30 bg-amber-500/10 text-amber-900"
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

            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-lg border border-border/60 p-3">
                <div className="mb-2 flex items-center justify-between">
                  <h4 className="text-sm font-medium">活动任务上下文</h4>
                  <span className="text-xs text-muted-foreground">{gitOverview.active_contexts.length} 条</span>
                </div>
                {gitOverview.active_contexts.length === 0 ? (
                  <p className="text-xs text-muted-foreground">当前暂无活动中的 Git 执行上下文。</p>
                ) : (
                  <div className="space-y-2">
                    {gitOverview.active_contexts.slice(0, 3).map((context) => (
                      <div key={context.id} className="rounded-md bg-secondary/40 px-2.5 py-2 text-xs">
                        <div className="flex items-center justify-between gap-2">
                          <span className="font-medium">{context.task_branch ?? "未命名分支"}</span>
                          <Badge variant="outline">{getTaskGitContextStateLabel(context.state)}</Badge>
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          目标分支：{context.target_branch ?? "未设置"}
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          更新时间：{formatDate(context.updated_at)}
                        </div>
                        {context.last_error && (
                          <div className="mt-1 text-destructive">
                            最近错误：{context.last_error}
                          </div>
                        )}
                        {context.worktree_missing && (
                          <div className="mt-1 text-muted-foreground">
                            当前任务 worktree 已不存在，可直接删除这条上下文记录。
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
                                  {reconcilingContextId === context.id ? "修复中..." : "修复上下文"}
                                </Button>
                              )}
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => setPendingDeleteContext(context)}
                                disabled={deletingContextId === context.id}
                              >
                                {deletingContextId === context.id ? "删除中..." : "删除记录"}
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
                                {reconcilingContextId === context.id ? "修复中..." : "修复上下文"}
                              </Button>
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => openGitActionDialog(context, "cleanup_worktree")}
                              >
                                直接清理
                              </Button>
                            </>
                          ) : context.state !== "failed" && context.state !== "completed" ? (
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => openGitActionDialog(context)}
                            >
                              Git 动作
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
                  <h4 className="text-sm font-medium">待确认操作</h4>
                  <span className="text-xs text-muted-foreground">{gitOverview.pending_action_contexts.length} 条</span>
                </div>
                {gitOverview.pending_action_contexts.length === 0 ? (
                  <p className="text-xs text-muted-foreground">当前没有待确认的高风险 Git 操作。</p>
                ) : (
                  <div className="space-y-2">
                    {gitOverview.pending_action_contexts.slice(0, 3).map((context) => (
                      <div key={context.id} className="rounded-md bg-secondary/40 px-2.5 py-2 text-xs">
                        <div className="flex items-center justify-between gap-2">
                          <span className="font-medium">{getGitActionTypeLabel(context.pending_action_type)}</span>
                          <Badge variant="outline">{getTaskGitContextStateLabel(context.state)}</Badge>
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          请求时间：{context.pending_action_requested_at ? formatDate(context.pending_action_requested_at) : "未知"}
                        </div>
                        <div className="mt-1 text-muted-foreground">
                          过期时间：{context.pending_action_expires_at ? formatDate(context.pending_action_expires_at) : "未知"}
                        </div>
                        <div className="mt-2">
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => openGitActionDialog(context)}
                          >
                            继续处理
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
                  <h4 className="text-sm font-medium">最近提交</h4>
                  <p className="text-[11px] text-muted-foreground">
                    {recentCommitsExpanded
                      ? `已加载 ${recentCommits.length} 条提交记录`
                      : `默认展示最近 ${Math.min(recentCommits.length, RECENT_COMMIT_SUMMARY_LIMIT)} 条摘要`}
                  </p>
                </div>
                <span className="text-xs text-muted-foreground">
                  {recentCommitsExpanded ? `${recentCommits.length} 条` : `${visibleRecentCommits.length} 条`}
                </span>
              </div>
              {recentCommits.length === 0 ? (
                <p className="text-xs text-muted-foreground">暂无最近提交记录。</p>
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
                            {commit.author_name ?? "未知作者"} · {formatDate(commit.authored_at)}
                          </span>
                          <span className="text-primary">查看详情</span>
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
                          {recentCommitsLoading ? "加载中..." : "查看更多历史提交"}
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
                              {recentCommitsLoading ? "加载中..." : "查看更多"}
                            </Button>
                          )}
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setRecentCommitsExpanded(false)}
                          >
                            收起摘要
                          </Button>
                          {!recentCommitsHasMore && (
                            <span className="text-[11px] text-muted-foreground">已显示全部可用提交记录</span>
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
            暂无 Git 概览数据。
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

      {/* Task List */}
      <Card className="p-4">
        <h3 className="text-sm font-semibold mb-3">任务列表</h3>
        {activeTasks.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">暂无活跃任务</p>
        ) : (
          <div className="space-y-2">
            {activeTasks.map((task) => (
              <div
                key={task.id}
                className="flex items-center gap-3 p-2 rounded-md hover:bg-accent/50 text-sm"
              >
                <div className={`w-1.5 h-1.5 rounded-full ${getStatusColor(task.status)}`} />
                <span className="flex-1 font-medium truncate">{task.title}</span>
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
        <h3 className="text-sm font-semibold mb-3">团队成员</h3>
        {projectEmployees.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">暂无成员</p>
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
              change.stage_status === "staged"
              || change.stage_status === "partially_staged",
          ) ?? []
        }
        onOpenChange={(open) => {
          if (!open) {
            setSelectedRepoAction(null);
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
              {rollbackConfirm?.target === "all" ? "确认全局回滚" : `确认回滚 ${selectedFiles.size} 个文件`}
            </DialogTitle>
            <DialogDescription>
              此操作将丢弃所有本地未提交的变更，且无法撤销。请确认后再继续。
            </DialogDescription>
          </DialogHeader>

          {rollbackConfirm?.target === "selected" && selectedFiles.size > 0 && (
            <div className="max-h-40 overflow-y-auto rounded-md border border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
              {Array.from(selectedFiles).map((path) => (
                <div key={path} className="break-all font-mono py-0.5">{path}</div>
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
              取消
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void handleRollback(rollbackConfirm?.target ?? "all")}
              disabled={rollbackInProgress}
            >
              {rollbackInProgress ? "回滚中..." : "确认回滚"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
