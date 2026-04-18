import { Suspense, lazy, useEffect, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { deleteTaskGitContextRecord, getProjectGitFilePreview, getProjectGitOverview, reconcileTaskGitContext } from "@/lib/backend";
import type { Employee, GitActionType, ProjectGitFilePreview, ProjectGitOverview, ProjectGitWorkingTreeChange, TaskGitContext } from "@/lib/types";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EditProjectDialog } from "@/components/projects/EditProjectDialog";
import { DeleteProjectDialog } from "@/components/projects/DeleteProjectDialog";
import { DeleteTaskGitContextDialog } from "@/components/projects/DeleteTaskGitContextDialog";
import { ProjectGitActionDialog } from "@/components/projects/ProjectGitActionDialog";
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import { ArrowLeft, Edit2, GitBranch, Loader2, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { getStatusLabel, getStatusColor, getPriorityLabel, formatDate } from "@/lib/utils";
import { getProjectWorkingDir, getProjectTypeLabel } from "@/lib/projects";

const ProjectGitFilePreviewDialog = lazy(async () => {
  const module = await import("@/components/projects/ProjectGitFilePreviewDialog");
  return { default: module.ProjectGitFilePreviewDialog };
});

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
      return "合并目标分支";
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
  const [selectedGitContext, setSelectedGitContext] = useState<TaskGitContext | null>(null);
  const [selectedGitAction, setSelectedGitAction] = useState<GitActionType | null>(null);
  const [selectedWorkingTreeChange, setSelectedWorkingTreeChange] = useState<ProjectGitWorkingTreeChange | null>(null);
  const [workingTreePreview, setWorkingTreePreview] = useState<ProjectGitFilePreview | null>(null);
  const [workingTreePreviewLoading, setWorkingTreePreviewLoading] = useState(false);
  const [workingTreePreviewError, setWorkingTreePreviewError] = useState<string | null>(null);
  const [gitActionNotice, setGitActionNotice] = useState<{
    tone: "success" | "error";
    message: string;
  } | null>(null);
  const [reconcilingContextId, setReconcilingContextId] = useState<string | null>(null);
  const [deletingContextId, setDeletingContextId] = useState<string | null>(null);
  const [pendingDeleteContext, setPendingDeleteContext] = useState<TaskGitContext | null>(null);
  const [gitOverviewReloadNonce, setGitOverviewReloadNonce] = useState(0);

  const project = projects.find((p) => p.id === id);

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
      setGitOverview(null);
      setGitOverviewError(null);
      setGitOverviewLoading(false);
      setSelectedGitContext(null);
      setSelectedGitAction(null);
      setSelectedWorkingTreeChange(null);
      setWorkingTreePreview(null);
      setWorkingTreePreviewLoading(false);
      setWorkingTreePreviewError(null);
      setPendingDeleteContext(null);
      return;
    }

    let active = true;
    setGitOverviewLoading(true);
    setGitOverviewError(null);

    void getProjectGitOverview(project.id)
      .then((overview) => {
        if (!active) {
          return;
        }
        setGitOverview(overview);
      })
      .catch((error) => {
        if (!active) {
          return;
        }
        setGitOverview(null);
        setGitOverviewError(error instanceof Error ? error.message : String(error));
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

  const openGitActionDialog = (context: TaskGitContext, preferredAction?: GitActionType) => {
    setSelectedGitContext(context);
    setSelectedGitAction(preferredAction ?? null);
  };

  const handleOpenProjectGitFile = async (change: ProjectGitWorkingTreeChange) => {
    if (!project) {
      return;
    }

    setSelectedWorkingTreeChange(change);
    setWorkingTreePreview(null);
    setWorkingTreePreviewError(null);
    setWorkingTreePreviewLoading(true);
    try {
      const preview = await getProjectGitFilePreview(
        project.id,
        change.path,
        change.previous_path,
        change.change_type,
      );
      setWorkingTreePreview(preview);
    } catch (error) {
      setWorkingTreePreviewError(error instanceof Error ? error.message : String(error));
    } finally {
      setWorkingTreePreviewLoading(false);
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

  const tasksByStatus = {
    todo: tasks.filter((t) => t.status === "todo"),
    in_progress: tasks.filter((t) => t.status === "in_progress"),
    review: tasks.filter((t) => t.status === "review"),
    completed: tasks.filter((t) => t.status === "completed"),
    blocked: tasks.filter((t) => t.status === "blocked"),
  };

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
              <div className="mb-2 flex items-center justify-between">
                <div>
                  <h4 className="text-sm font-medium">工作区文件</h4>
                  <p className="text-[11px] text-muted-foreground">
                    当前仓库实时变更文件列表，可直接在应用内预览文件内容。
                  </p>
                </div>
                <span className="text-xs text-muted-foreground">{gitOverview.working_tree_changes.length} 条</span>
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
                      className="rounded-md border border-border/60 bg-secondary/20 px-3 py-2 text-xs"
                    >
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <div className="flex min-w-0 flex-wrap items-center gap-2">
                          <span
                            className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getWorkingTreeChangeClassName(change.change_type)}`}
                          >
                            {getWorkingTreeChangeLabel(change.change_type)}
                          </span>
                          <span className="break-all font-mono text-foreground">{change.path}</span>
                        </div>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => {
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
                <h4 className="text-sm font-medium">最近提交</h4>
                <span className="text-xs text-muted-foreground">{gitOverview.recent_commits.length} 条</span>
              </div>
              {gitOverview.recent_commits.length === 0 ? (
                <p className="text-xs text-muted-foreground">暂无最近提交记录。</p>
              ) : (
                <div className="space-y-2">
                  {gitOverview.recent_commits.slice(0, 5).map((commit) => (
                    <div key={commit.sha} className="rounded-md bg-secondary/30 px-2.5 py-2 text-xs">
                      <div className="flex items-center justify-between gap-3">
                        <span className="font-medium">{commit.subject}</span>
                        <span className="text-muted-foreground">{commit.short_sha ?? commit.sha.slice(0, 7)}</span>
                      </div>
                      <div className="mt-1 text-muted-foreground">
                        {commit.author_name ?? "未知作者"} · {formatDate(commit.authored_at)}
                      </div>
                    </div>
                  ))}
                </div>
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
        {tasks.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">暂无任务</p>
        ) : (
          <div className="space-y-2">
            {tasks.map((task) => (
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
        {selectedWorkingTreeChange && (
          <ProjectGitFilePreviewDialog
            open={selectedWorkingTreeChange !== null}
            loading={workingTreePreviewLoading}
            error={workingTreePreviewError}
            preview={workingTreePreview}
            change={selectedWorkingTreeChange}
            onOpenChange={(open) => {
              if (!open) {
                setSelectedWorkingTreeChange(null);
                setWorkingTreePreview(null);
                setWorkingTreePreviewLoading(false);
                setWorkingTreePreviewError(null);
              }
            }}
          />
        )}
      </Suspense>

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
    </div>
  );
}
