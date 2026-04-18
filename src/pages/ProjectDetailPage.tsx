import { useEffect, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { getProjectGitOverview, reconcileTaskGitContext } from "@/lib/backend";
import type { Employee, ProjectGitOverview, TaskGitContext } from "@/lib/types";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EditProjectDialog } from "@/components/projects/EditProjectDialog";
import { DeleteProjectDialog } from "@/components/projects/DeleteProjectDialog";
import { ProjectGitActionDialog } from "@/components/projects/ProjectGitActionDialog";
import { RepoPathDisplay } from "@/components/projects/RepoPathDisplay";
import { ArrowLeft, Edit2, GitBranch, Loader2, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { getStatusLabel, getStatusColor, getPriorityLabel, formatDate } from "@/lib/utils";
import { getProjectWorkingDir, getProjectTypeLabel } from "@/lib/projects";

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
      return "已漂移";
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
  const [gitActionNotice, setGitActionNotice] = useState<{
    tone: "success" | "error";
    message: string;
  } | null>(null);
  const [reconcilingContextId, setReconcilingContextId] = useState<string | null>(null);
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

        {project.project_type === "ssh" && (
          <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-900">
            <div className="flex items-center gap-2 font-medium">
              <ShieldAlert className="h-3.5 w-3.5" />
              SSH v1 当前仅提供只读 Git 概览
            </div>
            <p className="mt-1 text-amber-900/80">
              自动创建 branch/worktree、merge-ready 沉淀，以及 merge/push/rebase/cherry-pick/stash/unstash/清理工作树等高风险操作暂不可用。
            </p>
          </div>
        )}

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
                        {project.project_type === "local" && (
                          <div className="mt-2 flex flex-wrap items-center gap-2">
                            {context.state === "drifted" ? (
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
                            ) : context.state !== "failed" && context.state !== "completed" ? (
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => setSelectedGitContext(context)}
                              >
                                Git 动作
                              </Button>
                            ) : null}
                          </div>
                        )}
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
                        {project.project_type === "local" && (
                          <div className="mt-2">
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => setSelectedGitContext(context)}
                            >
                              继续处理
                            </Button>
                          </div>
                        )}
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

      <ProjectGitActionDialog
        open={selectedGitContext !== null}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedGitContext(null);
          }
        }}
        context={selectedGitContext}
        projectBranches={gitOverview?.project_branches ?? []}
        onActionCompleted={handleGitActionCompleted}
      />
    </div>
  );
}
