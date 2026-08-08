import { memo, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type {
  CodexSessionKind,
  Tag,
  Task,
  TaskCommitActionState,
  TaskCommitOverview,
  TaskDependency,
  TaskGitContext,
  TaskPipelineStep,
} from "@/lib/types";
import {
  abortTaskPipeline,
  aiCommitTaskChanges,
  aiGenerateCoordinatorTaskPlan,
  aiGenerateTesterAcceptance,
  getTaskCommitActionState,
  getTaskCommitOverview,
  listTaskDependencies,
  listTaskPipelineSteps,
  listTaskTags,
  retryTaskPipelineStep,
  runTaskPipelineStepManual,
  stageAllTaskCommitFiles,
  startTaskPipeline,
  updateTaskPipelineStep,
} from "@/lib/backend";
import {
  formatDate,
  getAcceptanceStatusClassName,
  getAcceptanceStatusLabel,
  getPriorityColor,
  getPriorityLabel,
  getTaskActionRuntimeState,
  getTaskAutomationDisplayState,
  getTaskAutomationStatusLabel,
  isTaskOverdue,
} from "@/lib/utils";
import { getPipelineKanbanBadgeLabel } from "@/lib/pipelineUi";
import { onTaskAutomationStateChanged } from "@/lib/codex";
import { resolveTaskPrimaryCta } from "@/lib/taskPrimaryCta";
import { countStageableGitFiles } from "@/lib/gitWorkingTree";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import {
  AlertTriangle,
  Archive,
  Calendar,
  CircleCheckBig,
  Bot,
  ClipboardCheck,
  FolderKanban,
  GitBranch,
  GripVertical,
  Link2,
  Loader2,
  MessageSquarePlus,
  Network,
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
import { CoordinatorPlanDialog } from "./CoordinatorPlanDialog";
import { TaskElapsedSummary } from "./TaskElapsedSummary";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { ProjectGitActionDialog } from "@/components/projects/ProjectGitActionDialog";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";
import {
  getTaskBackgroundRunLabel,
  useTaskBackgroundRunStore,
} from "@/stores/taskBackgroundRunStore";
import {
  getTaskAiCommitLabel,
  isTaskAiCommitBusy,
  useTaskAiCommitStore,
} from "@/stores/taskAiCommitStore";
import { useTaskExecutionActions } from "./hooks/useTaskExecutionActions";
import { useTaskReviewActions } from "./hooks/useTaskReviewActions";
import { getProjectWorkingDir } from "@/lib/projects";

interface TaskCardProps {
  task: Task;
  isOverlay?: boolean;
  hideRunAction?: boolean;
  highlighted?: boolean;
  selected?: boolean;
  onToggleSelected?: (taskId: string) => void;
  gitContext?: TaskGitContext | null;
  projectBranches?: string[];
  /** Board-level tags to avoid per-card listTaskTags N+1. */
  tags?: Tag[];
  /** Board-level milestone name for badge display. */
  milestoneName?: string | null;
  /** Notify board/page after tags change (detail edit) so filters stay correct. */
  onTaskTagsChange?: (taskId: string, tagIds: string[]) => void;
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
      className: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-200",
      title: `任务分支 ${context.task_branch ?? "未命名分支"} 已合并到 ${context.target_branch ?? "目标分支"}`,
    };
  }

  if (context.state === "merge_ready") {
    return {
      label: "待合并",
      className: "bg-sky-500/10 text-sky-700 dark:text-sky-200",
      title: `任务分支 ${context.task_branch ?? "未命名分支"} 已提交，等待合并到 ${context.target_branch ?? "目标分支"}`,
    };
  }

  if (context.state === "failed") {
    return {
      label: "失败",
      className: "bg-rose-500/10 text-rose-700 dark:text-rose-200",
      title: context.last_error ?? "任务 Git 上下文执行失败",
    };
  }

  if (context.last_error) {
    return {
      label: "合并失败",
      className: "bg-rose-500/10 text-rose-700 dark:text-rose-200",
      title: context.last_error,
    };
  }

  return null;
}

function TaskCardComponent({
  task,
  isOverlay,
  hideRunAction = false,
  highlighted = false,
  selected = false,
  onToggleSelected,
  gitContext = null,
  projectBranches = [],
  tags: tagsFromBoard,
  milestoneName = null,
  onTaskTagsChange,
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
  const [initialCommitOverview, setInitialCommitOverview] = useState<TaskCommitOverview | null>(
    null,
  );
  const [initialCommitError, setInitialCommitError] = useState<string | null>(null);
  const [commitActionState, setCommitActionState] = useState<TaskCommitActionState | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [automationSubmitting, setAutomationSubmitting] = useState(false);
  const [automationRestarting, setAutomationRestarting] = useState(false);
  const [coordinatorPlanDialogOpen, setCoordinatorPlanDialogOpen] = useState(false);
  const [coordinatorPlanDraft, setCoordinatorPlanDraft] = useState("");
  const [coordinatorPlanLoading, setCoordinatorPlanLoading] = useState(false);
  const [coordinatorPlanSaving, setCoordinatorPlanSaving] = useState(false);
  const [coordinatorPlanExecuting, setCoordinatorPlanExecuting] = useState(false);
  const [coordinatorPlanError, setCoordinatorPlanError] = useState<string | null>(null);
  const [coordinatorPlanLogs, setCoordinatorPlanLogs] = useState<string[]>([]);
  const [coordinatorPlanTerminalVisible, setCoordinatorPlanTerminalVisible] = useState(false);
  const [pipelineSteps, setPipelineSteps] = useState<TaskPipelineStep[]>([]);
  const [pipelineLoading, setPipelineLoading] = useState(false);
  const [pipelineActionLoading, setPipelineActionLoading] = useState(false);
  const [pipelineError, setPipelineError] = useState<string | null>(null);
  const [pipelineNotice, setPipelineNotice] = useState<string | null>(null);
  const [testerAcceptanceLoading, setTesterAcceptanceLoading] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [taskTags, setTaskTags] = useState<Tag[]>([]);
  const [dependencyCount, setDependencyCount] = useState(0);
  const executionStartErrorRef = useRef<string | null>(null);
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
  const updateTask = useTaskStore((s) => s.updateTask);
  const updateTaskStatus = useTaskStore((s) => s.updateTaskStatus);
  const fetchComments = useTaskStore((s) => s.fetchComments);
  const assignee = task.assignee_id
    ? employees.find((employee) => employee.id === task.assignee_id)
    : undefined;
  const reviewer = task.reviewer_id
    ? employees.find((employee) => employee.id === task.reviewer_id)
    : undefined;
  const coordinator = task.coordinator_id
    ? employees.find((employee) => employee.id === task.coordinator_id)
    : undefined;
  const projectTesters = employees.filter(
    (employee) => employee.role === "tester" && employee.project_id === task.project_id,
  );
  const reviewerIsTester = Boolean(reviewer && reviewer.role === "tester");
  const canGenerateTesterAcceptance = reviewerIsTester || projectTesters.length > 0;
  const automationState = getTaskAutomationDisplayState(task, persistedAutomationState ?? null);
  const appendCoordinatorPlanLog = (line: string) => {
    setCoordinatorPlanLogs((current) => [...current.slice(-199), line]);
  };

  const getCoordinatorPlanRuntimeLabel = () => {
    const provider = coordinator?.ai_provider ?? "codex";
    const model = coordinator?.model?.trim() || "默认模型";
    const reasoningEffort = coordinator?.reasoning_effort?.trim() || "默认推理等级";
    return `${provider} / ${model} / ${reasoningEffort}`;
  };

  const executionActions = useTaskExecutionActions({
    task,
    assigneeId: task.assignee_id,
    assignee,
    projectRepoPath,
    projectType: project?.project_type,
    prepareExecutionInput: async (followUpPrompt, options) => {
      await Promise.all([fetchSubtasks(task.id), fetchAttachments(task.id)]);
      const executionInput = buildTaskExecutionInput({
        title: task.title,
        description: task.description,
        planContent: options?.planContent,
        subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
        attachments: useTaskStore.getState().attachments[task.id] ?? [],
        followUpPrompt,
      });

      return {
        prompt: executionInput.prompt,
        imagePaths: executionInput.imagePaths,
        resumeSessionId: followUpPrompt ? (task.last_codex_session_id ?? undefined) : undefined,
      };
    },
    clearTaskOutputOnRun: true,
    onStarted: (action) => {
      if (action === "continue") {
        setShowContinueDialog(false);
      }
    },
    onError: (message) => {
      executionStartErrorRef.current = message;
      appendCoordinatorPlanLog(`[ERROR] ${message}`);
      setCoordinatorPlanError(message);
    },
  });
  const reviewActions = useTaskReviewActions({
    task,
    reviewerId: task.reviewer_id,
    status: task.status,
  });
  const backgroundRun = useTaskBackgroundRunStore((state) => state.byTaskId[task.id]);
  const backgroundRunLabel = getTaskBackgroundRunLabel(backgroundRun);
  const isBackgroundPlanning = backgroundRun?.phase === "planning";
  const isBackgroundStarting = backgroundRun?.phase === "starting";
  const isBackgroundRunBusy = isBackgroundPlanning || isBackgroundStarting;
  const aiCommitEntry = useTaskAiCommitStore((state) => state.byTaskId[task.id]);
  const setAiCommitPhase = useTaskAiCommitStore((state) => state.setPhase);
  const clearAiCommitPhase = useTaskAiCommitStore((state) => state.clear);
  const aiCommitLabel = getTaskAiCommitLabel(aiCommitEntry);
  const aiCommitting = isTaskAiCommitBusy(aiCommitEntry);
  const runtimeState = getTaskActionRuntimeState({
    automationState,
    isExecutionRunning: executionActions.isRunning || isBackgroundRunBusy,
    isReviewRunning: reviewActions.isRunning,
    pipelineState: persistedAutomationState ?? null,
  });
  const isRunning = runtimeState.executionActive;
  const isReviewRunning = runtimeState.reviewActive;
  const pipelineKanbanBadge = getPipelineKanbanBadgeLabel(persistedAutomationState ?? null);
  const hasActivePipelineStep = pipelineSteps.some(
    (step) => step.status === "launching" || step.status === "running",
  );
  const coordinatorPlanActionsLocked =
    isRunning ||
    isReviewRunning ||
    pipelineActionLoading ||
    coordinatorPlanExecuting ||
    hasActivePipelineStep;
  const isReviewTask = task.status === "review" || isReviewRunning;
  const hasActiveSession = isRunning || isReviewRunning;
  const isActionLoading =
    executionActions.loading !== null ||
    reviewActions.loading ||
    automationSubmitting ||
    automationRestarting ||
    openingCommitDialog ||
    aiCommitting ||
    isBackgroundRunBusy ||
    testerAcceptanceLoading;
  const isWorktreeModeEnabled = task.use_worktree;
  const isWorktreeReady = Boolean(gitContext?.worktree_path) && !gitContext?.worktree_missing;
  const complexityScore =
    typeof task.complexity === "number" && task.complexity > 0
      ? Math.min(10, task.complexity)
      : null;
  const canDeleteWorktree = Boolean(isWorktreeModeEnabled && isWorktreeReady && !hasActiveSession);
  const canArchiveTask = !hasActiveSession && task.status !== "archived";
  const canMarkCompleted = task.status !== "completed" && task.status !== "archived";
  const gitContextBadge = getGitContextBadge(gitContext);
  const backendCanCommit = Boolean(
    commitActionState?.can_commit || commitActionState?.can_ai_commit,
  );
  const backendCanMerge = Boolean(commitActionState?.can_merge);
  const canTriggerMergeAction = Boolean(
    !hasActiveSession &&
    ((backendCanMerge && gitContext) ||
      (gitContext &&
        !gitContext.worktree_missing &&
        gitContext.state !== "failed" &&
        gitContext.state !== "completed" &&
        gitContext.state !== "drifted" &&
        commitActionState === null)),
  );
  // Prefer backend dirty/unmerged state; keep automation-aware fallback while state loads.
  const canCommitTaskCode = Boolean(
    !hasActiveSession &&
    (backendCanCommit ||
      (commitActionState === null &&
        gitContext &&
        !gitContext.worktree_missing &&
        gitContext.state !== "failed" &&
        gitContext.state !== "completed" &&
        gitContext.state !== "merge_ready" &&
        gitContext.state !== "drifted" &&
        gitContext.state !== "action_pending" &&
        (automationState.status === "commit_failed" ||
          automationState.status === "blocked" ||
          automationState.status === "manual_control" ||
          !automationState.enabled ||
          task.status === "completed"))),
  );
  const canAiCommitTaskCode = Boolean(
    !hasActiveSession && (commitActionState?.can_ai_commit || canCommitTaskCode),
  );
  const primaryCta = resolveTaskPrimaryCta({
    status: task.status,
    executionActive: isRunning,
    reviewActive: isReviewRunning,
    canStopProcess: executionActions.isRunning,
    backgroundPlanning: isBackgroundPlanning,
    backgroundStarting: isBackgroundStarting,
    hasAssignee: Boolean(task.assignee_id),
    hasReviewer: Boolean(task.reviewer_id),
    canCommit: canCommitTaskCode,
    canGenerateAcceptance: canGenerateTesterAcceptance,
    automationStatus: automationState.status,
    pipelineActive: Boolean(persistedAutomationState?.pipeline_active),
  });
  // Show primary bar for stop/review/locked always; for run respect hideRunAction (completed column).
  const shouldShowPrimaryCta =
    primaryCta.kind !== "none" && (primaryCta.kind !== "run" || !hideRunAction);
  const shouldShowActionBar = !isOverlay && shouldShowPrimaryCta;
  const shouldShowTaskActionBar = shouldShowActionBar;
  const shouldShowPrimaryMenuAction = shouldShowPrimaryCta;
  const hasPreLogActions =
    shouldShowPrimaryMenuAction ||
    Boolean(task.last_codex_session_id) ||
    canCommitTaskCode ||
    canAiCommitTaskCode ||
    canTriggerMergeAction ||
    canDeleteWorktree;
  const canRestartAutomation =
    automationState.enabled &&
    [
      "launching_review",
      "waiting_review",
      "launching_fix",
      "waiting_execution",
      "review_launch_failed",
      "fix_launch_failed",
      "blocked",
      "manual_control",
    ].includes(automationState.status);
  const overdue = isTaskOverdue(task);

  useEffect(() => {
    if (tagsFromBoard !== undefined) {
      setTaskTags(tagsFromBoard);
    }
  }, [tagsFromBoard]);

  useEffect(() => {
    if (isOverlay) {
      return;
    }
    let cancelled = false;
    // Prefer board-level tags when provided; only fetch tags as fallback.
    const tagsPromise =
      tagsFromBoard !== undefined ? Promise.resolve(tagsFromBoard) : listTaskTags(task.id);
    void Promise.all([tagsPromise, listTaskDependencies(task.id)])
      .then(([tags, deps]: [Tag[], TaskDependency[]]) => {
        if (cancelled) {
          return;
        }
        setTaskTags(tags);
        setDependencyCount(deps.length);
      })
      .catch(() => {
        if (!cancelled) {
          if (tagsFromBoard === undefined) {
            setTaskTags([]);
          }
          setDependencyCount(0);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [isOverlay, task.id, task.updated_at, tagsFromBoard]);

  const refreshDeliveryBadges = async () => {
    try {
      const [nextTags, deps] = await Promise.all([
        listTaskTags(task.id),
        listTaskDependencies(task.id),
      ]);
      setTaskTags(nextTags);
      setDependencyCount(deps.length);
      onTaskTagsChange?.(
        task.id,
        nextTags.map((tag) => tag.id),
      );
    } catch (error) {
      console.error("Failed to refresh task delivery badges:", error);
    }
  };

  useEffect(() => {
    if (isOverlay) {
      return;
    }
    let cancelled = false;
    void getTaskCommitActionState(task.id)
      .then((state) => {
        if (!cancelled) {
          setCommitActionState(state);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setCommitActionState(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [
    isOverlay,
    task.id,
    task.updated_at,
    task.status,
    task.use_worktree,
    gitContext?.id,
    gitContext?.state,
    gitContext?.updated_at,
    gitContext?.worktree_missing,
  ]);

  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
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
    if (typeof persistedAutomationState === "undefined") {
      void fetchTaskAutomationState(task.id);
    }
  }, [fetchTaskAutomationState, persistedAutomationState, task.id]);

  useEffect(() => {
    if (isOverlay || !coordinatorPlanDialogOpen) {
      return;
    }

    let active = true;
    let unlisten: (() => void) | null = null;
    void onTaskAutomationStateChanged((event) => {
      if (!active || event.task_id !== task.id) {
        return;
      }
      void listTaskPipelineSteps(task.id)
        .then((steps) => {
          if (active) {
            setPipelineSteps(steps);
          }
        })
        .catch((error) => {
          if (active) {
            setPipelineError(error instanceof Error ? error.message : String(error));
          }
        });
      void fetchTaskAutomationState(task.id);
    }).then((fn) => {
      if (!active) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [isOverlay, coordinatorPlanDialogOpen, task.id, fetchTaskAutomationState]);

  const handleRun = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setContextMenu(null);
    if (isRunning || isReviewRunning) {
      return;
    }
    if (task.coordinator_id) {
      await openCoordinatorPlanFlow();
      return;
    }

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

  const handlePrimaryCtaClick = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    if (primaryCta.disabled || isActionLoading) {
      return;
    }
    switch (primaryCta.kind) {
      case "stop":
        await handleStop(e);
        return;
      case "review":
        await handleReviewCode(e);
        return;
      case "run":
        await handleRun(e);
        return;
      case "commit":
        await openCommitDialog();
        return;
      case "acceptance":
        await handleGenerateTesterAcceptance();
        return;
      case "blocked":
        setContextMenu(null);
        setShowDetail(true);
        return;
      default:
        return;
    }
  };

  const primaryCtaButtonClass = (() => {
    switch (primaryCta.tone) {
      case "danger":
        return "flex items-center gap-1 px-2 py-0.5 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50";
      case "warning":
        return "flex items-center gap-1 px-2 py-0.5 text-xs bg-amber-500 text-black rounded hover:bg-amber-400 transition-colors disabled:opacity-50";
      case "muted":
        return "flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded opacity-50";
      case "primary":
      default:
        return "flex items-center gap-1 px-2 py-0.5 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50";
    }
  })();

  const primaryCtaIcon = (() => {
    switch (primaryCta.kind) {
      case "stop":
        return executionActions.loading === "stop" ? (
          <Square className="h-3 w-3" />
        ) : (
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-white animate-pulse" />
        );
      case "starting":
      case "running_locked":
        return <Loader2 className="h-3 w-3 animate-spin" />;
      case "review":
        return reviewActions.loading || isReviewRunning ? (
          <Loader2 className="h-3 w-3 animate-spin" />
        ) : (
          <ScrollText className="h-3 w-3" />
        );
      case "blocked":
        return <AlertTriangle className="h-3 w-3" />;
      case "commit":
        return openingCommitDialog ? (
          <Loader2 className="h-3 w-3 animate-spin" />
        ) : (
          <GitBranch className="h-3 w-3" />
        );
      case "acceptance":
        return testerAcceptanceLoading ? (
          <Loader2 className="h-3 w-3 animate-spin" />
        ) : (
          <ClipboardCheck className="h-3 w-3" />
        );
      case "run":
      default:
        return <Play className="h-3 w-3" />;
    }
  })();

  const handleContextMenu = (e: React.MouseEvent<HTMLDivElement>) => {
    if (isOverlay) return;
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      x: Math.max(8, Math.min(e.clientX, window.innerWidth - 200)),
      y: Math.max(8, Math.min(e.clientY, window.innerHeight - 160)),
    });
  };

  const openDeleteDialog = () => {
    if (hasActiveSession) return;
    setContextMenu(null);
    setShowDeleteDialog(true);
  };

  const handleArchiveTask = async () => {
    if (!canArchiveTask) {
      return;
    }

    setContextMenu(null);

    try {
      await updateTaskStatus(task.id, "archived");
    } catch (error) {
      console.error("Failed to archive task:", error);
    }
  };

  const handleMarkCompleted = async () => {
    if (!canMarkCompleted) {
      return;
    }

    setContextMenu(null);

    try {
      await updateTaskStatus(task.id, "completed");
    } catch (error) {
      console.error("Failed to mark task as completed:", error);
    }
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
      await setTaskAutomationMode(task.id, automationState.enabled ? null : "review_fix_loop_v1");
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

  const refreshPipelineSteps = async () => {
    setPipelineLoading(true);
    setPipelineError(null);
    try {
      const steps = await listTaskPipelineSteps(task.id);
      setPipelineSteps(steps);
    } catch (error) {
      setPipelineError(error instanceof Error ? error.message : String(error));
    } finally {
      setPipelineLoading(false);
    }
  };

  const generateCoordinatorPlan = async () => {
    setCoordinatorPlanTerminalVisible(true);
    if (!task.coordinator_id) {
      appendCoordinatorPlanLog("[ERROR] 当前任务未指定协调员，无法生成计划。");
      setCoordinatorPlanError("请先指定协调员。");
      return;
    }

    setCoordinatorPlanLoading(true);
    setCoordinatorPlanError(null);
    appendCoordinatorPlanLog(`[计划] 准备调用协调员：${coordinator?.name ?? task.coordinator_id}`);
    appendCoordinatorPlanLog(`[计划] 运行配置：${getCoordinatorPlanRuntimeLabel()}`);
    appendCoordinatorPlanLog(`[计划] 工作目录：${projectRepoPath ?? "未配置"}`);
    appendCoordinatorPlanLog("[计划] 正在生成协调员执行计划，可能需要一点时间...");
    try {
      const plan = await aiGenerateCoordinatorTaskPlan({
        task_id: task.id,
        coordinator_id: task.coordinator_id,
        title: task.title,
        description: task.description,
        status: task.status,
        priority: task.priority,
        working_dir: projectRepoPath ?? null,
      });
      const trimmedPlan = plan.trim();
      if (!trimmedPlan) {
        appendCoordinatorPlanLog("[WARN] 协调员返回了空计划。");
        setCoordinatorPlanError("协调员未返回可用计划。");
        return;
      }
      setCoordinatorPlanDraft(trimmedPlan);
      appendCoordinatorPlanLog(`[计划] 已收到协调员计划，共 ${trimmedPlan.length} 字。`);
      appendCoordinatorPlanLog("[计划] 结构化工作包已落库，可在本弹窗「按计划编排」。");
      await refreshPipelineSteps();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      appendCoordinatorPlanLog(`[ERROR] ${message}`);
      setCoordinatorPlanError(message);
    } finally {
      setCoordinatorPlanLoading(false);
    }
  };

  const handleStartPipeline = async () => {
    setPipelineActionLoading(true);
    setPipelineError(null);
    setPipelineNotice(null);
    setCoordinatorPlanTerminalVisible(true);
    appendCoordinatorPlanLog("[编排] 准备按计划编排...");
    try {
      let steps = pipelineSteps;
      if (steps.length === 0) {
        if (!task.coordinator_id) {
          throw new Error("请先指定协调员并生成结构化计划。");
        }
        await generateCoordinatorPlan();
        steps = await listTaskPipelineSteps(task.id);
        setPipelineSteps(steps);
        if (steps.length === 0) {
          throw new Error("生成计划后仍无工作包，请检查协调员输出后重试。");
        }
      }
      await startTaskPipeline(task.id);
      setPipelineNotice("已启动按计划编排。");
      appendCoordinatorPlanLog("[编排] 已启动按计划编排，步骤将串行执行。");
      await refreshPipelineSteps();
      await fetchTaskAutomationState(task.id);
      await useTaskStore.getState().fetchTasks(useTaskStore.getState().activeProjectId);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setPipelineError(message);
      appendCoordinatorPlanLog(`[ERROR] ${message}`);
    } finally {
      setPipelineActionLoading(false);
    }
  };

  const handleRetryPipeline = async () => {
    setPipelineActionLoading(true);
    setPipelineError(null);
    setPipelineNotice(null);
    try {
      await retryTaskPipelineStep(task.id);
      setPipelineNotice("已重试当前编排步骤。");
      await refreshPipelineSteps();
      await fetchTaskAutomationState(task.id);
    } catch (error) {
      setPipelineError(error instanceof Error ? error.message : String(error));
    } finally {
      setPipelineActionLoading(false);
    }
  };

  const handleAbortPipeline = async () => {
    setPipelineActionLoading(true);
    setPipelineError(null);
    setPipelineNotice(null);
    try {
      await abortTaskPipeline(task.id);
      setPipelineNotice("编排已转人工。未完成步骤可点「手动运行」。");
      await refreshPipelineSteps();
      await fetchTaskAutomationState(task.id);
    } catch (error) {
      setPipelineError(error instanceof Error ? error.message : String(error));
    } finally {
      setPipelineActionLoading(false);
    }
  };

  const handleManualRunPipelineStep = async (step: TaskPipelineStep) => {
    setPipelineActionLoading(true);
    setPipelineError(null);
    setPipelineNotice(null);
    try {
      await runTaskPipelineStepManual(task.id, step.id);
      setPipelineNotice(`已手动启动步骤 ${step.step_index + 1}「${step.title}」。`);
      await refreshPipelineSteps();
      await fetchTaskAutomationState(task.id);
    } catch (error) {
      setPipelineError(error instanceof Error ? error.message : String(error));
    } finally {
      setPipelineActionLoading(false);
    }
  };

  const handlePipelineEmployeeChange = async (stepId: string, employeeId: string) => {
    setPipelineError(null);
    try {
      const updated = await updateTaskPipelineStep({
        step_id: stepId,
        employee_id: employeeId ? employeeId : null,
      });
      setPipelineSteps((current) => current.map((step) => (step.id === stepId ? updated : step)));
    } catch (error) {
      setPipelineError(error instanceof Error ? error.message : String(error));
    }
  };

  const openCoordinatorPlanFlow = async () => {
    const existingPlan = task.plan_content?.trim() ?? "";
    setCoordinatorPlanError(null);
    setPipelineError(null);
    setPipelineNotice(null);
    setCoordinatorPlanDraft(existingPlan);
    setCoordinatorPlanLogs(
      existingPlan ? [`[计划] 已加载任务中保存的协调员计划，共 ${existingPlan.length} 字。`] : [],
    );
    setCoordinatorPlanTerminalVisible(!existingPlan);
    setCoordinatorPlanDialogOpen(true);
    await refreshPipelineSteps();
    if (!existingPlan) {
      await generateCoordinatorPlan();
    }
  };

  const handleGenerateTesterAcceptance = async () => {
    setContextMenu(null);
    const testerId = reviewerIsTester ? task.reviewer_id : (projectTesters[0]?.id ?? null);
    if (!testerId) {
      return;
    }

    setTesterAcceptanceLoading(true);
    try {
      await aiGenerateTesterAcceptance({
        task_id: task.id,
        tester_id: testerId,
        working_dir: projectRepoPath ?? null,
      });
      await fetchComments(task.id);
    } catch (error) {
      console.error("Failed to generate tester acceptance checklist:", error);
    } finally {
      setTesterAcceptanceLoading(false);
    }
  };

  const handleSaveCoordinatorPlan = async () => {
    const plan = coordinatorPlanDraft.trim();
    if (!plan) {
      setCoordinatorPlanError("计划内容不能为空。");
      return;
    }

    setCoordinatorPlanSaving(true);
    setCoordinatorPlanError(null);
    setCoordinatorPlanTerminalVisible(true);
    appendCoordinatorPlanLog(`[计划] 正在保存协调员计划，共 ${plan.length} 字。`);
    try {
      await updateTask(task.id, { plan_content: plan });
      appendCoordinatorPlanLog("[计划] 协调员计划已保存到任务。");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      appendCoordinatorPlanLog(`[ERROR] ${message}`);
      setCoordinatorPlanError(message);
    } finally {
      setCoordinatorPlanSaving(false);
    }
  };

  const handleExecuteCoordinatorPlan = async () => {
    const plan = coordinatorPlanDraft.trim();
    if (!plan) {
      setCoordinatorPlanError("计划内容不能为空。");
      return;
    }
    if (!task.assignee_id) {
      setCoordinatorPlanTerminalVisible(true);
      appendCoordinatorPlanLog("[ERROR] 当前任务未指定执行员工，无法执行计划。");
      setCoordinatorPlanError("请先指定执行员工，再执行计划。");
      return;
    }

    setCoordinatorPlanExecuting(true);
    setCoordinatorPlanError(null);
    executionStartErrorRef.current = null;
    setCoordinatorPlanTerminalVisible(true);
    appendCoordinatorPlanLog(
      `[执行] 正在把计划交给执行员工：${assignee?.name ?? task.assignee_id}`,
    );
    try {
      onOpenLog?.(task.id, "execution");
      await executionActions.runTask(plan);
      if (!executionStartErrorRef.current) {
        appendCoordinatorPlanLog("[执行] 执行会话已启动，完整终端输出会继续写入任务日志。");
        setCoordinatorPlanDialogOpen(false);
      }
    } finally {
      setCoordinatorPlanExecuting(false);
    }
  };

  const openMergeDialog = () => {
    if (!canTriggerMergeAction) {
      return;
    }
    setContextMenu(null);
    setShowGitActionDialog(true);
  };

  const openCommitDialog = async () => {
    if (!canCommitTaskCode) {
      return;
    }
    setContextMenu(null);
    setOpeningCommitDialog(true);
    clearAiCommitPhase(task.id);

    let nextOverview: TaskCommitOverview | null = null;
    let nextError: string | null = null;

    try {
      nextOverview = await getTaskCommitOverview(task.id);
      if (countStageableGitFiles(nextOverview.working_tree_changes) > 0) {
        await stageAllTaskCommitFiles(task.id);
        nextOverview = await getTaskCommitOverview(task.id);
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

  const handleAiCommit = async () => {
    if (!canAiCommitTaskCode || aiCommitting) {
      return;
    }
    setContextMenu(null);
    setAiCommitPhase(task.id, "committing");
    try {
      const result = await aiCommitTaskChanges(task.id);
      const detail = result.conflict_resolved
        ? `${result.detail}（已自动解决冲突）`
        : result.detail;
      setAiCommitPhase(task.id, "success", { detail });
      await onGitActionCompleted?.(task.project_id, detail);
      const nextState = await getTaskCommitActionState(task.id);
      setCommitActionState(nextState);
      window.setTimeout(() => {
        const current = useTaskAiCommitStore.getState().byTaskId[task.id];
        if (current?.phase === "success") {
          clearAiCommitPhase(task.id);
        }
      }, 5000);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setAiCommitPhase(task.id, "error", { error: message });
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
      if (
        automationState.status === "waiting_review" ||
        automationState.status === "launching_review" ||
        automationState.status === "review_launch_failed"
      ) {
        onOpenLog?.(task.id, "review");
      } else if (
        automationState.status === "waiting_execution" ||
        automationState.status === "launching_fix" ||
        automationState.status === "fix_launch_failed"
      ) {
        onOpenLog?.(task.id, "execution");
      }
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
          isDragging ? "opacity-50 shadow-lg" : "hover:shadow-sm cursor-pointer"
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
            <div className="flex items-start gap-2">
              <p className="min-w-0 flex-1 text-sm font-medium truncate">{task.title}</p>
              {onToggleSelected && (
                <label
                  className="mt-0.5 shrink-0 cursor-pointer rounded p-0.5 hover:bg-muted"
                  title={selected ? "取消选择" : "选择任务"}
                  onClick={(e) => e.stopPropagation()}
                  onPointerDown={(e) => e.stopPropagation()}
                >
                  <input
                    type="checkbox"
                    className="h-3.5 w-3.5 accent-primary"
                    checked={selected}
                    onChange={() => onToggleSelected(task.id)}
                    aria-label={`选择任务 ${task.title}`}
                  />
                </label>
              )}
            </div>
            {task.description && (
              <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{task.description}</p>
            )}
            <div className="flex items-center gap-2 mt-2 flex-wrap">
              <span className={`text-xs font-medium ${getPriorityColor(task.priority)}`}>
                {getPriorityLabel(task.priority)}
              </span>
              {complexityScore !== null && (
                <span className="text-xs text-muted-foreground">复杂度: {complexityScore}/10</span>
              )}
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${
                  automationState.enabled
                    ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-200"
                    : "bg-muted text-muted-foreground"
                }`}
                title={
                  automationState.note ??
                  (automationState.enabled ? "自动质控已开启" : "自动质控未开启")
                }
              >
                <Bot className="h-3 w-3" />
                自动质控·{getTaskAutomationStatusLabel(automationState.status)}
              </span>
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${getAcceptanceStatusClassName(task.last_acceptance_status)}`}
                title="最近一次测试验收状态"
              >
                验收·{getAcceptanceStatusLabel(task.last_acceptance_status)}
              </span>
              {isWorktreeModeEnabled && (
                <span
                  className="inline-flex items-center gap-1 rounded-full bg-sky-500/10 px-2 py-0.5 text-[11px] text-sky-700 dark:text-sky-200"
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
              {backgroundRunLabel && (
                <span
                  className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${
                    backgroundRun?.phase === "error"
                      ? "bg-destructive/10 text-destructive"
                      : "bg-violet-500/10 text-violet-700 dark:text-violet-200"
                  }`}
                  title={
                    backgroundRun?.phase === "error"
                      ? (backgroundRun.error ?? backgroundRunLabel)
                      : backgroundRunLabel
                  }
                >
                  {backgroundRun?.phase === "error" ? (
                    <Network className="h-3 w-3" />
                  ) : (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  )}
                  {backgroundRunLabel}
                </span>
              )}
              {aiCommitLabel && (
                <span
                  className={`inline-flex max-w-full items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${
                    aiCommitEntry?.phase === "error"
                      ? "bg-destructive/10 text-destructive"
                      : aiCommitEntry?.phase === "success"
                        ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-200"
                        : "bg-indigo-500/10 text-indigo-700 dark:text-indigo-200"
                  }`}
                  title={
                    aiCommitEntry?.phase === "error"
                      ? (aiCommitEntry.error ?? aiCommitLabel)
                      : (aiCommitEntry?.detail ?? aiCommitLabel)
                  }
                  onClick={(e) => {
                    if (aiCommitEntry?.phase === "error" || aiCommitEntry?.phase === "success") {
                      e.stopPropagation();
                      clearAiCommitPhase(task.id);
                    }
                  }}
                >
                  {aiCommitting ? (
                    <Loader2 className="h-3 w-3 shrink-0 animate-spin" />
                  ) : (
                    <Bot className="h-3 w-3 shrink-0" />
                  )}
                  <span className="truncate">{aiCommitLabel}</span>
                </span>
              )}
              {pipelineKanbanBadge && !isBackgroundPlanning && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void openCoordinatorPlanFlow();
                  }}
                  className={`inline-flex max-w-full items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${
                    persistedAutomationState?.phase === "pipeline_step_failed"
                      ? "bg-destructive/10 text-destructive hover:bg-destructive/15"
                      : "bg-violet-500/10 text-violet-700 hover:bg-violet-500/15 dark:text-violet-200"
                  }`}
                  title="打开编排面板查看阶段进度"
                >
                  <GitBranch className="h-3 w-3 shrink-0" />
                  <span className="truncate">{pipelineKanbanBadge}</span>
                </button>
              )}
              {task.coordinator_id && !isBackgroundPlanning && !pipelineKanbanBadge && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void openCoordinatorPlanFlow();
                  }}
                  className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 text-[11px] text-primary hover:bg-primary/20"
                  title={
                    coordinator ? `协调员：${coordinator.name} · 打开协调员计划` : "打开协调员计划"
                  }
                >
                  <Network className="h-3 w-3" />
                  协调员计划
                </button>
              )}
              {task.due_date && (
                <span
                  className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] ${
                    overdue
                      ? "bg-destructive/10 text-destructive"
                      : "bg-muted text-muted-foreground"
                  }`}
                  title={
                    overdue
                      ? `已逾期：${formatDate(task.due_date)}`
                      : `截止：${formatDate(task.due_date)}`
                  }
                >
                  <Calendar className="h-3 w-3" />
                  {overdue ? "逾期" : "截止"}·{formatDate(task.due_date)}
                </span>
              )}
              {task.milestone_id && milestoneName && (
                <span
                  className="inline-flex max-w-[120px] items-center gap-1 truncate rounded-full bg-violet-500/10 px-2 py-0.5 text-[11px] text-violet-700 dark:text-violet-200"
                  title={`里程碑：${milestoneName}`}
                >
                  里程碑·{milestoneName}
                </span>
              )}
              {taskTags.slice(0, 3).map((tag) => (
                <span
                  key={tag.id}
                  className="inline-flex max-w-[88px] items-center gap-1 truncate rounded-full bg-secondary px-2 py-0.5 text-[11px] text-secondary-foreground"
                  title={tag.name}
                >
                  <span
                    className="h-1.5 w-1.5 shrink-0 rounded-full"
                    style={{ backgroundColor: tag.color || "var(--primary)" }}
                  />
                  {tag.name}
                </span>
              ))}
              {taskTags.length > 3 && (
                <span className="text-[11px] text-muted-foreground">+{taskTags.length - 3}</span>
              )}
              {dependencyCount > 0 && (
                <span
                  className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-[11px] text-muted-foreground"
                  title={`依赖 ${dependencyCount} 个任务`}
                >
                  <Link2 className="h-3 w-3" />
                  依赖·{dependencyCount}
                </span>
              )}
              {task.status === "blocked" && (
                <span
                  className="inline-flex items-center gap-1 rounded-full bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-800 dark:text-amber-100"
                  title={
                    task.blocked_reason?.trim() || "任务已阻塞，建议指定协调员并生成协调员计划"
                  }
                >
                  阻塞·建议协调
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
                <TaskElapsedSummary task={task} />
              </div>
              {task.assignee_id && (
                <span className="inline-block w-3.5 h-3.5 rounded-full bg-primary/10 text-primary text-[8px] leading-[14px] text-center self-start">
                  {task.assignee_id[0]}
                </span>
              )}
            </div>
          </div>
        </div>
        {/* Primary CTA — driven by resolveTaskPrimaryCta */}
        {shouldShowTaskActionBar && (
          <div className="flex items-center gap-1 mt-2 pt-2 border-t border-border/50">
            {primaryCta.kind === "run" && !task.assignee_id ? (
              <span
                className="text-xs text-muted-foreground/50"
                title={primaryCta.reason ?? "请先指派员工"}
              >
                <Play className="h-3 w-3 inline mr-0.5" />
                未指派
              </span>
            ) : primaryCta.kind === "review" && !task.reviewer_id ? (
              <span
                className="text-xs text-muted-foreground/50"
                title={primaryCta.reason ?? "请先指定审查员"}
              >
                <ScrollText className="h-3 w-3 inline mr-0.5" />
                未指定审查员
              </span>
            ) : primaryCta.kind === "starting" ? (
              <button
                type="button"
                disabled
                className="flex items-center gap-1 px-2 py-0.5 text-xs bg-violet-600 text-white rounded opacity-90"
                title={backgroundRunLabel ?? primaryCta.reason ?? "后台启动中"}
              >
                <Loader2 className="h-3 w-3 animate-spin" />
                {primaryCta.label}
              </button>
            ) : (
              <button
                type="button"
                onClick={(e) => void handlePrimaryCtaClick(e)}
                disabled={primaryCta.disabled || isActionLoading}
                title={
                  primaryCta.kind === "review" && task.reviewer_id
                    ? `由 ${reviewer?.name ?? "审查员"} 发起代码审核`
                    : (primaryCta.reason ?? primaryCta.label)
                }
                className={primaryCtaButtonClass}
              >
                {primaryCtaIcon}
                {primaryCta.label}
              </button>
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
            onOpenChange={(nextOpen) => {
              setShowDetail(nextOpen);
              if (!nextOpen) {
                void refreshDeliveryBadges();
              }
            }}
            automationState={automationState}
          />
        </ErrorBoundary>
      )}
      {!isOverlay &&
        contextMenu &&
        createPortal(
          <>
            <div className="fixed inset-0 z-40" onMouseDown={() => setContextMenu(null)} />
            <div
              className="fixed z-50 w-48 rounded-lg border border-border bg-popover p-1 shadow-lg"
              style={{ left: contextMenu.x, top: contextMenu.y }}
              role="menu"
              aria-label={`${task.title} 操作菜单`}
              onMouseDown={(e) => e.stopPropagation()}
              onClick={(e) => e.stopPropagation()}
            >
              {shouldShowPrimaryMenuAction && (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => void handlePrimaryCtaClick()}
                  disabled={primaryCta.disabled || isActionLoading}
                  title={
                    primaryCta.kind === "review"
                      ? task.reviewer_id
                        ? `由 ${reviewer?.name ?? "审查员"} 发起代码审核`
                        : "请先指定审查员"
                      : (primaryCta.reason ?? primaryCta.label)
                  }
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                >
                  {primaryCta.kind === "stop" ? (
                    <Square className="h-4 w-4" />
                  ) : primaryCta.kind === "starting" || primaryCta.kind === "running_locked" ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : primaryCta.kind === "review" ? (
                    <ScrollText className="h-4 w-4" />
                  ) : primaryCta.kind === "blocked" ? (
                    <AlertTriangle className="h-4 w-4" />
                  ) : primaryCta.kind === "commit" ? (
                    <GitBranch className="h-4 w-4" />
                  ) : primaryCta.kind === "acceptance" ? (
                    <ClipboardCheck className="h-4 w-4" />
                  ) : (
                    <Play className="h-4 w-4" />
                  )}
                  {primaryCta.kind === "review" && !isReviewRunning ? "审核代码" : primaryCta.label}
                </button>
              )}
              {canMarkCompleted && (
                <>
                  {shouldShowPrimaryMenuAction && <div className="my-1 h-px bg-border" />}
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => void handleMarkCompleted()}
                    disabled={!canMarkCompleted || isActionLoading}
                    title={
                      task.status === "completed"
                        ? "任务已完成"
                        : task.status === "archived"
                          ? "已归档任务不能标记为已完成"
                          : "将任务标记为已完成，自动移动到已完成列"
                    }
                    className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left text-emerald-600 hover:bg-emerald-500/10 disabled:pointer-events-none disabled:opacity-50"
                  >
                    <CircleCheckBig className="h-4 w-4" />
                    标记已完成
                  </button>
                </>
              )}
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
              {task.coordinator_id && (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setContextMenu(null);
                    void openCoordinatorPlanFlow();
                  }}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground"
                >
                  <Network className="h-4 w-4" />
                  协调员计划
                </button>
              )}
              {canGenerateTesterAcceptance && primaryCta.kind !== "acceptance" && (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => void handleGenerateTesterAcceptance()}
                  disabled={testerAcceptanceLoading}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                >
                  {testerAcceptanceLoading ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <ClipboardCheck className="h-4 w-4" />
                  )}
                  {testerAcceptanceLoading ? "生成中…" : "生成验收清单"}
                </button>
              )}
              <button
                type="button"
                role="menuitem"
                onClick={() => void handleArchiveTask()}
                disabled={!canArchiveTask || isActionLoading}
                title={
                  hasActiveSession
                    ? "运行中的任务不能归档，请先停止相关会话"
                    : "将任务移出主看板并保留记录"
                }
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
              >
                <Archive className="h-4 w-4" />
                归档
              </button>
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
              {canCommitTaskCode && primaryCta.kind !== "commit" && (
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
              {canAiCommitTaskCode && (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => void handleAiCommit()}
                  disabled={isActionLoading}
                  title={
                    commitActionState?.warnings?.[0] ||
                    (commitActionState?.mode === "project_repo"
                      ? "将 AI 提交项目主仓库当前工作区改动"
                      : "AI 生成提交说明并提交；如有冲突将自动尝试解决")
                  }
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                >
                  {aiCommitting ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Bot className="h-4 w-4" />
                  )}
                  {aiCommitting ? "AI 提交中" : "AI 提交"}
                </button>
              )}
              {aiCommitEntry?.phase === "error" && aiCommitEntry.error && (
                <div className="px-2 py-1 text-[11px] text-destructive" title={aiCommitEntry.error}>
                  {aiCommitEntry.error}
                </div>
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
          document.body,
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
      {!isOverlay && (
        <CoordinatorPlanDialog
          open={coordinatorPlanDialogOpen}
          coordinatorName={coordinator?.name}
          plan={coordinatorPlanDraft}
          loading={coordinatorPlanLoading}
          saving={coordinatorPlanSaving}
          executing={coordinatorPlanExecuting}
          error={
            coordinatorPlanError ??
            pipelineError ??
            (!task.assignee_id ? "请先指定执行员工，再执行计划。" : null)
          }
          canExecute={Boolean(task.assignee_id)}
          canStartPipeline={task.status !== "archived"}
          actionsLocked={coordinatorPlanActionsLocked}
          taskTitle={task.title}
          terminalLogs={coordinatorPlanLogs}
          terminalVisible={coordinatorPlanTerminalVisible}
          pipelineSteps={pipelineSteps}
          pipelineAutomation={persistedAutomationState ?? null}
          pipelineLoading={pipelineLoading}
          pipelineActionLoading={pipelineActionLoading}
          pipelineError={pipelineError}
          pipelineNotice={pipelineNotice}
          employees={employees}
          projectId={task.project_id}
          onOpenChange={setCoordinatorPlanDialogOpen}
          onPlanChange={setCoordinatorPlanDraft}
          onExecute={() => void handleExecuteCoordinatorPlan()}
          onStartPipeline={() => void handleStartPipeline()}
          onRetryPipeline={() => void handleRetryPipeline()}
          onAbortPipeline={() => void handleAbortPipeline()}
          onManualRunPipelineStep={(step) => void handleManualRunPipelineStep(step)}
          onRefreshPipeline={() => void refreshPipelineSteps()}
          onPipelineEmployeeChange={(stepId, employeeId) =>
            void handlePipelineEmployeeChange(stepId, employeeId)
          }
          onRegenerate={() => void generateCoordinatorPlan()}
          onSave={() => void handleSaveCoordinatorPlan()}
          onToggleTerminal={() => setCoordinatorPlanTerminalVisible((visible) => !visible)}
          onClearTerminal={() => setCoordinatorPlanLogs([])}
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
      {!isOverlay && (
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
          initialOverview={initialCommitOverview}
          initialError={initialCommitError}
          onCommitted={async (message) => {
            const nextState = await getTaskCommitActionState(task.id);
            setCommitActionState(nextState);
            await onGitActionCompleted?.(task.project_id, message);
          }}
        />
      )}
    </>
  );
}

export const TaskCard = memo(TaskCardComponent);
