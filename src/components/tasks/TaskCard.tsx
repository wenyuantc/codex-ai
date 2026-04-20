import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type {
  CodexSessionKind,
  Task,
  TaskGitCommitOverview,
  TaskGitContext,
} from "@/lib/types";
import {
  getTaskGitCommitOverview,
  stageAllTaskGitFiles,
} from "@/lib/backend";
import {
  formatDate,
  getPriorityColor,
  getPriorityLabel,
  getTaskActionRuntimeState,
  getTaskAutomationDisplayState,
  getTaskAutomationStatusLabel,
} from "@/lib/utils";
import { countStageableGitFiles } from "@/lib/gitWorkingTree";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import {
  Bot,
  Clock,
  FolderKanban,
  GitBranch,
  GripVertical,
  Loader2,
  MessageSquarePlus,
  Play,
  RotateCcw,
  ScrollText,
  Square,
  Trash2,
} from "lucide-react";
import { ContinueConversationDialog } from "./ContinueConversationDialog";
import { TaskDetailDialog } from "./TaskDetailDialog";
import { DeleteTaskDialog } from "./DeleteTaskDialog";
import { DeleteTaskWorktreeDialog } from "./DeleteTaskWorktreeDialog";
import { TaskGitCommitDialog } from "./TaskGitCommitDialog";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { ProjectGitActionDialog } from "@/components/projects/ProjectGitActionDialog";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";
import { useTaskExecutionActions } from "./hooks/useTaskExecutionActions";
import { useTaskReviewActions } from "./hooks/useTaskReviewActions";
import { getProjectWorkingDir } from "@/lib/projects";

interface TaskCardProps {
  task: Task;
  isOverlay?: boolean;
  hideRunAction?: boolean;
  highlighted?: boolean;
  gitContext?: TaskGitContext | null;
  projectBranches?: string[];
  onOpenLog?: (taskId: string, sessionKind?: CodexSessionKind) => void;
  onGitActionCompleted?: (projectId: string, message: string) => Promise<void> | void;
}

function getGitContextBadge(context: TaskGitContext | null): {
  label: string;
  className: string;
  title: string;
} | null {
  if (!context) {
    return null;
  }

  if (context.state === "completed") {
    return {
      label: "已合并",
      className: "bg-emerald-500/10 text-emerald-700",
      title: `任务分支 ${context.task_branch ?? "未命名分支"} 已合并到 ${context.target_branch ?? "目标分支"}`,
    };
  }

  if (context.state === "merge_ready") {
    return {
      label: "待合并",
      className: "bg-sky-500/10 text-sky-700",
      title: `任务分支 ${context.task_branch ?? "未命名分支"} 已提交，等待合并到 ${context.target_branch ?? "目标分支"}`,
    };
  }

  if (context.state === "failed") {
    return {
      label: "失败",
      className: "bg-rose-500/10 text-rose-700",
      title: context.last_error ?? "任务 Git 上下文执行失败",
    };
  }

  if (context.last_error) {
    return {
      label: "合并失败",
      className: "bg-rose-500/10 text-rose-700",
      title: context.last_error,
    };
  }

  return null;
}

export function TaskCard({
  task,
  isOverlay,
  hideRunAction = false,
  highlighted = false,
  gitContext = null,
  projectBranches = [],
  onOpenLog,
  onGitActionCompleted,
}: TaskCardProps) {
  const [showDetail, setShowDetail] = useState(false);
  const [showContinueDialog, setShowContinueDialog] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [showDeleteWorktreeDialog, setShowDeleteWorktreeDialog] = useState(false);
  const [showGitActionDialog, setShowGitActionDialog] = useState(false);
  const [showCommitDialog, setShowCommitDialog] = useState(false);
  const [openingCommitDialog, setOpeningCommitDialog] = useState(false);
  const [initialCommitOverview, setInitialCommitOverview] = useState<TaskGitCommitOverview | null>(null);
  const [initialCommitError, setInitialCommitError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [automationSubmitting, setAutomationSubmitting] = useState(false);
  const [automationRestarting, setAutomationRestarting] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const projects = useProjectStore((s) => s.projects);
  const employees = useEmployeeStore((s) => s.employees);
  const project = projects.find((p) => p.id === task.project_id);
  const projectName = project?.name;
  const projectRepoPath = getProjectWorkingDir(project);
  const fetchAttachments = useTaskStore((s) => s.fetchAttachments);
  const fetchSubtasks = useTaskStore((s) => s.fetchSubtasks);
  const fetchTaskAutomationState = useTaskStore((s) => s.fetchTaskAutomationState);
  const persistedAutomationState = useTaskStore((s) => s.automationStates[task.id]);
  const setTaskAutomationMode = useTaskStore((s) => s.setTaskAutomationMode);
  const restartTaskAutomation = useTaskStore((s) => s.restartTaskAutomation);
  const deleteTask = useTaskStore((s) => s.deleteTask);
  const assignee = task.assignee_id ? employees.find((employee) => employee.id === task.assignee_id) : undefined;
  const reviewer = task.reviewer_id ? employees.find((employee) => employee.id === task.reviewer_id) : undefined;
  const automationState = getTaskAutomationDisplayState(task, persistedAutomationState ?? null);
  const executionActions = useTaskExecutionActions({
    task,
    assigneeId: task.assignee_id,
    assignee,
    projectRepoPath,
    projectType: project?.project_type,
    prepareExecutionInput: async (followUpPrompt) => {
      await Promise.all([fetchSubtasks(task.id), fetchAttachments(task.id)]);
      const executionInput = buildTaskExecutionInput({
        title: task.title,
        description: task.description,
        subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
        attachments: useTaskStore.getState().attachments[task.id] ?? [],
        followUpPrompt,
      });

      return {
        prompt: executionInput.prompt,
        imagePaths: executionInput.imagePaths,
        resumeSessionId: followUpPrompt ? task.last_codex_session_id ?? undefined : undefined,
      };
    },
    clearTaskOutputOnRun: true,
    onStarted: (action) => {
      if (action === "continue") {
        setShowContinueDialog(false);
      }
    },
  });
  const reviewActions = useTaskReviewActions({
    task,
    reviewerId: task.reviewer_id,
    status: task.status,
  });
  const runtimeState = getTaskActionRuntimeState({
    automationState,
    isExecutionRunning: executionActions.isRunning,
    isReviewRunning: reviewActions.isRunning,
  });
  const isRunning = runtimeState.executionActive;
  const isReviewRunning = runtimeState.reviewActive;
  const isReviewTask = task.status === "review" || isReviewRunning;
  const hasActiveSession = isRunning || isReviewRunning;
  const isActionLoading =
    executionActions.loading !== null
    || reviewActions.loading
    || automationSubmitting
    || automationRestarting
    || openingCommitDialog;
  const shouldShowActionBar = !isOverlay && (isRunning || isReviewTask || !hideRunAction);
  const shouldShowPrimaryMenuAction = isRunning || isReviewTask || !hideRunAction;
  const isWorktreeModeEnabled = task.use_worktree;
  const isWorktreeReady = Boolean(gitContext?.worktree_path) && !gitContext?.worktree_missing;
  const canDeleteWorktree = Boolean(
    isWorktreeModeEnabled
    && isWorktreeReady
    && !hasActiveSession,
  );
  const shouldShowTaskActionBar = shouldShowActionBar;
  const gitContextBadge = getGitContextBadge(gitContext);
  const canTriggerMergeAction = Boolean(
    gitContext
    && !gitContext.worktree_missing
    && gitContext.state !== "failed"
    && gitContext.state !== "completed"
    && gitContext.state !== "drifted",
  );
  const canCommitTaskCode = Boolean(
    gitContext
    && !gitContext.worktree_missing
    && !hasActiveSession
    && gitContext.state !== "failed"
    && gitContext.state !== "completed"
    && gitContext.state !== "merge_ready"
    && gitContext.state !== "drifted"
    && gitContext.state !== "action_pending"
    && (
      automationState.status === "commit_failed"
      || automationState.status === "blocked"
      || automationState.status === "manual_control"
      || !automationState.enabled
    ),
  );
  const hasPreLogActions = shouldShowPrimaryMenuAction
    || Boolean(task.last_codex_session_id)
    || canCommitTaskCode
    || canTriggerMergeAction
    || canDeleteWorktree;
  const canRestartAutomation = automationState.enabled && [
    "launching_review",
    "waiting_review",
    "launching_fix",
    "waiting_execution",
    "review_launch_failed",
    "fix_launch_failed",
  ].includes(automationState.status);

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: task.id,
    data: { type: "task", status: task.status },
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  useEffect(() => {
    if (!contextMenu) return;

    const handleClose = () => setContextMenu(null);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    };

    window.addEventListener("resize", handleClose);
    document.addEventListener("scroll", handleClose, true);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("resize", handleClose);
      document.removeEventListener("scroll", handleClose, true);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (
      task.automation_mode === "review_fix_loop_v1"
      && typeof persistedAutomationState === "undefined"
    ) {
      void fetchTaskAutomationState(task.id);
    }
  }, [fetchTaskAutomationState, persistedAutomationState, task.automation_mode, task.id]);

  const handleRun = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setContextMenu(null);
    onOpenLog?.(task.id, "execution");
    await executionActions.runTask();
  };

  const handleStop = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setContextMenu(null);
    await executionActions.stopTask();
  };

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await deleteTask(task.id);
      setShowDeleteDialog(false);
      setShowDetail(false);
      setContextMenu(null);
    } catch (error) {
      console.error("Failed to delete task:", error);
    } finally {
      setDeleting(false);
    }
  };

  const handleReviewCode = async (e?: React.MouseEvent) => {
    e?.stopPropagation();

    if (task.status !== "review" || !task.reviewer_id || isReviewRunning) return;

    setContextMenu(null);
    onOpenLog?.(task.id, "review");
    await reviewActions.startReview();
  };

  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    if (isOverlay) return;
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      x: Math.max(8, Math.min(e.clientX, window.innerWidth - 184)),
      y: Math.max(8, Math.min(e.clientY, window.innerHeight - 120)),
    });
  };

  const openDeleteDialog = () => {
    if (hasActiveSession) return;
    setContextMenu(null);
    setShowDeleteDialog(true);
  };

  const openLogDialog = () => {
    setContextMenu(null);
    onOpenLog?.(task.id, isReviewTask ? "review" : "execution");
  };

  const openContinueDialog = () => {
    if (!task.last_codex_session_id || isRunning) return;
    setContextMenu(null);
    setShowContinueDialog(true);
  };

  const handleToggleAutomation = async () => {
    setContextMenu(null);
    setAutomationSubmitting(true);

    try {
      await setTaskAutomationMode(
        task.id,
        automationState.enabled ? null : "review_fix_loop_v1",
      );
    } catch (error) {
      console.error("Failed to toggle task automation:", error);
    } finally {
      setAutomationSubmitting(false);
    }
  };

  const handleContinueConversation = async (prompt: string) => {
    if (!task.last_codex_session_id) return;
    await executionActions.continueTask(prompt);
  };

  const openMergeDialog = () => {
    if (!canTriggerMergeAction) {
      return;
    }
    setContextMenu(null);
    setShowGitActionDialog(true);
  };

  const openCommitDialog = async () => {
    if (!canCommitTaskCode || !gitContext) {
      return;
    }
    setContextMenu(null);
    setOpeningCommitDialog(true);

    let nextOverview = null;
    let nextError: string | null = null;

    try {
      nextOverview = await getTaskGitCommitOverview(gitContext.id);
      if (countStageableGitFiles(nextOverview.working_tree_changes) > 0) {
        await stageAllTaskGitFiles(gitContext.id);
        nextOverview = await getTaskGitCommitOverview(gitContext.id);
      }
    } catch (error) {
      nextError = error instanceof Error ? error.message : String(error);
    } finally {
      setInitialCommitOverview(nextOverview);
      setInitialCommitError(nextError);
      setShowCommitDialog(true);
      setOpeningCommitDialog(false);
    }
  };

  const openDeleteWorktreeDialog = () => {
    if (!canDeleteWorktree) {
      return;
    }
    setContextMenu(null);
    setShowDeleteWorktreeDialog(true);
  };

  const handleRestartAutomation = async () => {
    if (!canRestartAutomation) return;
    setContextMenu(null);
    setAutomationRestarting(true);

    try {
      await restartTaskAutomation(task.id);
      onOpenLog?.(
        task.id,
        automationState.status === "waiting_review" || automationState.status === "launching_review" || automationState.status === "review_launch_failed"
          ? "review"
          : "execution",
      );
    } catch (error) {
      console.error("Failed to restart task automation:", error);
    } finally {
      setAutomationRestarting(false);
    }
  };

  return (
    <>
      <div
        id={`task-card-${task.id}`}
        ref={setNodeRef}
        style={style}
        className={`group rounded-md border bg-card p-3 ${
          highlighted ? "border-primary ring-2 ring-primary/20" : "border-border"
        } ${
          isDragging
            ? "opacity-50 shadow-lg"
            : "hover:shadow-sm cursor-pointer"
        } transition-shadow`}
        onClick={() => !isDragging && setShowDetail(true)}
        onContextMenu={handleContextMenu}
        {...attributes}
      >
        <div className="flex items-start gap-2">
          <button
            className="mt-0.5 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing shrink-0"
            {...listeners}
            onClick={(e) => e.stopPropagation()}
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium truncate">{task.title}</p>
            {task.description && (
              <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                {task.description}
              </p>
            )}
            <div className="flex items-center gap-2 mt-2 flex-wrap">
              <span
                className={`text-xs font-medium ${getPriorityColor(
                  task.priority
                )}`}
              >
                {getPriorityLabel(task.priority)}
              </span>
              {task.complexity && (
                <span className="text-xs text-muted-foreground">
                  复杂度: {task.complexity}/10
                </span>
              )}
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${
                  automationState.enabled
                    ? "bg-emerald-500/10 text-emerald-700"
                    : "bg-muted text-muted-foreground"
                }`}
                title={automationState.note ?? (automationState.enabled ? "自动质控已开启" : "自动质控未开启")}
              >
                <Bot className="h-3 w-3" />
                自动质控·{getTaskAutomationStatusLabel(automationState.status)}
              </span>
              {isWorktreeModeEnabled && (
                <span
                  className="inline-flex items-center gap-1 rounded-full bg-sky-500/10 px-2 py-0.5 text-[11px] text-sky-700"
                  title={
                    isWorktreeReady
                      ? `任务已绑定 worktree：${gitContext?.task_branch ?? "未命名分支"} · ${gitContext?.target_branch ?? "未设置目标分支"}`
                      : "任务已开启 Worktree 模式，首次运行后会准备独立 worktree"
                  }
                >
                  <GitBranch className="h-3 w-3" />
                  Worktree 模式
                </span>
              )}
              {gitContextBadge && (
                <span
                  className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${gitContextBadge.className}`}
                  title={gitContextBadge.title}
                >
                  <GitBranch className="h-3 w-3" />
                  {gitContextBadge.label}
                </span>
              )}
            </div>
            <div className="flex items-center justify-between mt-1.5 text-xs text-muted-foreground">
              <div className="flex flex-col gap-0.5">
                {projectName && (
                  <span className="flex items-center gap-0.5">
                    <FolderKanban className="h-3 w-3" />
                    {projectName}
                  </span>
                )}
                <span className="flex items-center gap-0.5">
                  <Clock className="h-3 w-3" />
                  {formatDate(task.created_at)}
                </span>
              </div>
              {task.assignee_id && (
                <span className="inline-block w-3.5 h-3.5 rounded-full bg-primary/10 text-primary text-[8px] leading-[14px] text-center self-start">
                  {task.assignee_id[0]}
                </span>
              )}
            </div>
          </div>
        </div>
        {/* Run/Stop Codex */}
        {shouldShowTaskActionBar && (
          <div className="flex items-center gap-1 mt-2 pt-2 border-t border-border/50">
            {shouldShowActionBar && (
              isRunning ? (
                executionActions.isRunning ? (
                  <button
                    onClick={handleStop}
                    disabled={isActionLoading}
                    className="flex items-center gap-1 px-2 py-0.5 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50"
                  >
                    {executionActions.loading === "stop" ? (
                      <Square className="h-3 w-3" />
                    ) : (
                      <span className="inline-block w-1.5 h-1.5 rounded-full bg-white animate-pulse" />
                    )}
                    停止
                  </button>
                ) : (
                  <button
                    disabled
                    className="flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded opacity-50"
                    title="自动修复正在启动或运行中"
                  >
                    <Loader2 className="h-3 w-3 animate-spin" />
                    运行中
                  </button>
                )
              ) : isReviewTask ? (
                task.reviewer_id ? (
                  <button
                    onClick={(e) => void handleReviewCode(e)}
                    disabled={isActionLoading || isReviewRunning}
                    title={`由 ${reviewer?.name ?? "审查员"} 发起代码审核`}
                    className="flex items-center gap-1 px-2 py-0.5 text-xs bg-amber-500 text-black rounded hover:bg-amber-400 transition-colors disabled:opacity-50"
                  >
                    {reviewActions.loading || isReviewRunning ? (
                      <Loader2 className="h-3 w-3 animate-spin" />
                    ) : (
                      <ScrollText className="h-3 w-3" />
                    )}
                    {isReviewRunning ? "审核中" : "审核"}
                  </button>
                ) : (
                  <span className="text-xs text-muted-foreground/50" title="请先指定审查员">
                    <ScrollText className="h-3 w-3 inline mr-0.5" />
                    未指定审查员
                  </span>
                )
              ) : task.assignee_id ? (
                <button
                  onClick={handleRun}
                  disabled={isActionLoading}
                  className="flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50"
                >
                  <Play className="h-3 w-3" />
                  运行
                </button>
              ) : (
                <span className="text-xs text-muted-foreground/50" title="请先指派员工">
                  <Play className="h-3 w-3 inline mr-0.5" />
                  未指派
                </span>
              )
            )}
          </div>
        )}
      </div>
      {!isOverlay && showDetail && (
        <ErrorBoundary
          fallbackTitle="任务详情渲染失败"
          fallbackDescription="详情弹窗出现了运行时异常，下面是具体错误。"
        >
          <TaskDetailDialog
            task={task}
            open={showDetail}
            onOpenChange={setShowDetail}
            automationState={automationState}
          />
        </ErrorBoundary>
      )}
      {!isOverlay && contextMenu && createPortal(
        <>
          <div
            className="fixed inset-0 z-40"
            onMouseDown={() => setContextMenu(null)}
          />
          <div
            className="fixed z-50 w-44 rounded-lg border border-border bg-popover p-1 shadow-lg"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            role="menu"
            aria-label={`${task.title} 操作菜单`}
            onMouseDown={(e) => e.stopPropagation()}
            onClick={(e) => e.stopPropagation()}
          >
            {shouldShowPrimaryMenuAction && (isRunning ? (
              executionActions.isRunning ? (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => void handleStop()}
                  disabled={isActionLoading}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                >
                  <Square className="h-4 w-4" />
                  停止
                </button>
              ) : (
                <button
                  type="button"
                  role="menuitem"
                  disabled
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left disabled:pointer-events-none disabled:opacity-50"
                  title="自动修复正在启动或运行中"
                >
                  <Loader2 className="h-4 w-4 animate-spin" />
                  运行中
                </button>
              )
            ) : isReviewTask ? (
              <button
                type="button"
                role="menuitem"
                onClick={() => void handleReviewCode()}
                disabled={!task.reviewer_id || isActionLoading || isReviewRunning}
                title={task.reviewer_id ? `由 ${reviewer?.name ?? "审查员"} 发起代码审核` : "请先指定审查员"}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
              >
                <ScrollText className="h-4 w-4" />
                {isReviewRunning ? "审核中" : "审核代码"}
              </button>
            ) : (
              <button
                type="button"
                role="menuitem"
                onClick={() => void handleRun()}
                disabled={!task.assignee_id || isActionLoading}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
              >
                <Play className="h-4 w-4" />
                运行
              </button>
            ))}
            {task.last_codex_session_id && (
              <>
                {shouldShowPrimaryMenuAction && <div className="my-1 h-px bg-border" />}
                <button
                  type="button"
                  role="menuitem"
                  onClick={openContinueDialog}
                  disabled={isRunning || isActionLoading}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                >
                  <MessageSquarePlus className="h-4 w-4" />
                  继续对话
                </button>
              </>
            )}
            {canTriggerMergeAction && (
              <button
                type="button"
                role="menuitem"
                onClick={openMergeDialog}
                disabled={isActionLoading}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
              >
                <GitBranch className="h-4 w-4" />
                合并到目标分支
              </button>
            )}
            {canCommitTaskCode && (
              <button
                type="button"
                role="menuitem"
                onClick={() => void openCommitDialog()}
                disabled={isActionLoading}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
              >
                {openingCommitDialog ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <GitBranch className="h-4 w-4" />
                )}
                {openingCommitDialog ? "准备提交中" : "提交代码"}
              </button>
            )}
            {canDeleteWorktree && (
              <button
                type="button"
                role="menuitem"
                onClick={openDeleteWorktreeDialog}
                disabled={isActionLoading}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left text-destructive hover:bg-destructive/10 disabled:pointer-events-none disabled:opacity-50"
              >
                <Trash2 className="h-4 w-4" />
                删除 Worktree
              </button>
            )}
            {hasPreLogActions && <div className="my-1 h-px bg-border" />}
            <button
              type="button"
              role="menuitem"
              onClick={() => void handleToggleAutomation()}
              disabled={automationSubmitting}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground"
            >
              {automationSubmitting ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Bot className="h-4 w-4" />
              )}
              {automationState.enabled ? "关闭自动质控" : "开启自动质控"}
            </button>
            {canRestartAutomation && (
              <button
                type="button"
                role="menuitem"
                onClick={() => void handleRestartAutomation()}
                disabled={automationRestarting}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
              >
                {automationRestarting ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <RotateCcw className="h-4 w-4" />
                )}
                重启自动化
              </button>
            )}
            <div className="px-2 pb-1 text-[11px] text-muted-foreground">
              当前：{getTaskAutomationStatusLabel(automationState.status)}
            </div>
            <div className="my-1 h-px bg-border" />
            <button
              type="button"
              role="menuitem"
              onClick={openLogDialog}
              disabled={!task.assignee_id}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
            >
              <ScrollText className="h-4 w-4" />
              查看终端日志
            </button>
            <div className="my-1 h-px bg-border" />
            <button
              type="button"
              role="menuitem"
              onClick={openDeleteDialog}
              disabled={hasActiveSession || deleting}
              title={hasActiveSession ? "任务有进行中的执行或审核，请先停止" : "删除任务"}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left text-destructive hover:bg-destructive/10 disabled:pointer-events-none disabled:opacity-50"
            >
              <Trash2 className="h-4 w-4" />
              删除
            </button>
          </div>
        </>,
        document.body
      )}
      {!isOverlay && showContinueDialog && (
        <ContinueConversationDialog
          open={showContinueDialog}
          task={task}
          submitting={executionActions.loading === "continue"}
          onOpenChange={setShowContinueDialog}
          onConfirm={handleContinueConversation}
        />
      )}
      {!isOverlay && showDeleteDialog && (
        <DeleteTaskDialog
          open={showDeleteDialog}
          task={task}
          deleting={deleting}
          onOpenChange={setShowDeleteDialog}
          onConfirm={handleDelete}
        />
      )}
      {!isOverlay && gitContext && (
        <DeleteTaskWorktreeDialog
          open={showDeleteWorktreeDialog}
          context={gitContext}
          onOpenChange={setShowDeleteWorktreeDialog}
          onCompleted={async (message) => {
            await onGitActionCompleted?.(task.project_id, message);
          }}
        />
      )}
      {!isOverlay && gitContext && (
        <ProjectGitActionDialog
          open={showGitActionDialog}
          onOpenChange={setShowGitActionDialog}
          context={gitContext}
          projectBranches={projectBranches}
          preferredAction="merge"
          lockActionSelection
          onActionStateChanged={async () => {
            await onGitActionCompleted?.(task.project_id, "");
          }}
          onActionCompleted={async (message) => {
            await onGitActionCompleted?.(task.project_id, message);
          }}
        />
      )}
      {!isOverlay && gitContext && (
        <TaskGitCommitDialog
          open={showCommitDialog}
          onOpenChange={(open) => {
            setShowCommitDialog(open);
            if (!open) {
              setInitialCommitOverview(null);
              setInitialCommitError(null);
            }
          }}
          task={task}
          gitContext={gitContext}
          initialOverview={initialCommitOverview}
          initialError={initialCommitError}
          onCommitted={async (message) => {
            await onGitActionCompleted?.(task.project_id, message);
          }}
        />
      )}
    </>
  );
}
