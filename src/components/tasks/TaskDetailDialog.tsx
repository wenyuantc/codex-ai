import { useState, useEffect, useRef } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";

import type {
  CodexSessionFileChange,
  CodexSessionFileChangeDetail,
  Task,
  TaskExecutionChangeHistoryItem,
  TaskLatestReview,
  TaskPipelineStep,
} from "@/lib/types";
import {
  abortTaskPipeline,
  aiGenerateCoordinatorTaskPlan,
  aiGenerateTesterAcceptance,
  getTaskAcceptanceRuns,
  getTaskCommitActionState,
  getTaskCommitOverview,
  listTaskPipelineSteps,
  getCodexSessionFileChangeDetail,
  getTaskExecutionChangeHistory,
  getTaskLatestReview,
  getTaskTokenUsage,
  type TokenUsageSummary,
  listTaskDependencies,
  openTaskAttachment,
  resumeTaskPipeline,
  retryTaskPipelineStep,
  runTaskAcceptance,
  runTaskPipelineStepManual,
  stopTaskPipelineStepManual,
  stageAllTaskCommitFiles,
  startTaskPipeline,
  updateTaskAcceptanceChecklist,
  updateTaskPipelineStep,
} from "@/lib/backend";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  getTaskBackgroundRunLabel,
  useTaskBackgroundRunStore,
} from "@/stores/taskBackgroundRunStore";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import { dedupePaths, isTauriRuntime, normalizeDialogSelection } from "@/lib/taskAttachments";
import { onTaskAutomationStateChanged } from "@/lib/codex";
import { startTaskRunSession } from "@/lib/taskRunSession";
import { getProjectWorkingDir } from "@/lib/projects";
import { countStageableGitFiles } from "@/lib/gitWorkingTree";
import { resolveTaskPrimaryCta } from "@/lib/taskPrimaryCta";
import type { TaskAutomationDisplayState } from "@/lib/utils";
import {
  getTaskActionRuntimeState,
  getTaskAutomationDisplayState,
  getTaskAutomationStatusLabel,
  isTaskPipelineRunning,
} from "@/lib/utils";
import { SessionLogDialog } from "@/components/sessions/SessionLogDialog";
import type { TaskCommitActionState, TaskCommitOverview } from "@/lib/types";
import { DeleteTaskDialog } from "./DeleteTaskDialog";
import { InsertPlanConfirmDialog } from "./InsertPlanConfirmDialog";
import { ReviewFixConfirmDialog } from "./ReviewFixConfirmDialog";
import { CoordinatorPlanDialog } from "./CoordinatorPlanDialog";
import { TaskGitCommitDialog } from "./TaskGitCommitDialog";
import { TaskPrimaryActionBar } from "./TaskPrimaryActionBar";
import { useTaskExecutionActions } from "./hooks/useTaskExecutionActions";
import { useTaskReviewActions } from "./hooks/useTaskReviewActions";
import { useTaskAiActions } from "./hooks/useTaskAiActions";
import { TaskDetailHeader } from "./detail/TaskDetailHeader";
import { TaskPropertiesSidebar } from "./detail/TaskPropertiesSidebar";
import { TaskOverviewPanel } from "./detail/TaskOverviewPanel";
import { TaskExecutionPanel } from "./detail/TaskExecutionPanel";
import { TaskExecutionChangeDetailDialog } from "./detail/TaskExecutionChangeDetailDialog";
import { TaskReviewPanel } from "./detail/TaskReviewPanel";
import { TaskAiPanel } from "./detail/TaskAiPanel";
import { TaskCollaborationPanel } from "./detail/TaskCollaborationPanel";
import { TaskSessionChainPanel } from "./detail/TaskSessionChainPanel";

const EMPTY_ATTACHMENTS: never[] = [];

interface TaskDetailDialogProps {
  task: Task;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  automationState?: TaskAutomationDisplayState;
}

export function TaskDetailDialog({
  task,
  open,
  onOpenChange,
  automationState,
}: TaskDetailDialogProps) {
  const { t } = useTranslation(["tasks", "common"]);
  const {
    updateTask,
    deleteTask,
    addComment,
    fetchAttachments,
    fetchSubtasks,
    fetchComments,
    fetchTaskAutomationState,
    addTaskAttachments,
    deleteTaskAttachment,
    addSubtasks,
    createTask,
  } = useTaskStore();
  const persistedAutomationState = useTaskStore((state) => state.automationStates[task.id]);
  const queuedItem = useTaskStore(
    (state) => state.runQueue.find((item) => item.task_id === task.id) ?? null,
  );
  const {
    employees,
    employeeRuntime,
    fetchEmployees,
    updateEmployeeStatus,
    clearTaskCodexOutput,
    addCodexOutput,
    refreshEmployeeRuntimeStatus,
  } = useEmployeeStore();
  const projects = useProjectStore((s) => s.projects);
  const storeTasks = useTaskStore((s) => s.tasks);
  const attachmentMap = useTaskStore((state) => state.attachments);
  const attachments = attachmentMap[task.id] ?? EMPTY_ATTACHMENTS;
  const project = projects.find((p) => p.id === task.project_id);
  const projectRepoPath = getProjectWorkingDir(project);
  const projectTasks = storeTasks.filter((item) => item.project_id === task.project_id);
  const [title, setTitle] = useState(task.title);
  const [description, setDescription] = useState(task.description ?? "");
  const [priority, setPriority] = useState(task.priority);
  const [status, setStatus] = useState(task.status);
  const [assigneeId, setAssigneeId] = useState(task.assignee_id ?? "");
  const [reviewerId, setReviewerId] = useState(task.reviewer_id ?? "");
  const [coordinatorId, setCoordinatorId] = useState(task.coordinator_id ?? "");
  const [dueDate, setDueDate] = useState(task.due_date ?? "");
  const [milestoneId, setMilestoneId] = useState(task.milestone_id ?? "");
  const [blockedReason, setBlockedReason] = useState(task.blocked_reason ?? "");
  const [blockReasonDialogOpen, setBlockReasonDialogOpen] = useState(false);
  const [pendingBlockedReason, setPendingBlockedReason] = useState("");
  const [blockReasonSubmitting, setBlockReasonSubmitting] = useState(false);
  const [planContent, setPlanContent] = useState(task.plan_content ?? "");
  const [planContentDraft, setPlanContentDraft] = useState(task.plan_content ?? "");
  const [planContentEditing, setPlanContentEditing] = useState(false);
  const [planContentSaving, setPlanContentSaving] = useState(false);
  const [pipelineSteps, setPipelineSteps] = useState<TaskPipelineStep[]>([]);
  const [pipelineLoading, setPipelineLoading] = useState(false);
  const [pipelineActionLoading, setPipelineActionLoading] = useState(false);
  const [pipelineError, setPipelineError] = useState<string | null>(null);
  const [pipelineNotice, setPipelineNotice] = useState<string | null>(null);
  const [incompleteDependencyTitles, setIncompleteDependencyTitles] = useState<string[]>([]);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingTask, setDeletingTask] = useState(false);
  const [attachmentLoading, setAttachmentLoading] = useState(false);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [deletingAttachmentId, setDeletingAttachmentId] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewNotice, setReviewNotice] = useState<string | null>(null);
  const [reviewFixDialogOpen, setReviewFixDialogOpen] = useState(false);
  const [reviewFixSubmitting, setReviewFixSubmitting] = useState(false);
  const [latestReview, setLatestReview] = useState<TaskLatestReview | null>(null);
  const [latestReviewLoading, setLatestReviewLoading] = useState(false);
  const [taskIdCopied, setTaskIdCopied] = useState(false);
  const [executionChangeHistory, setExecutionChangeHistory] = useState<
    TaskExecutionChangeHistoryItem[]
  >([]);
  const [executionChangeHistoryLoading, setExecutionChangeHistoryLoading] = useState(false);
  const [executionChangeHistoryError, setExecutionChangeHistoryError] = useState<string | null>(
    null,
  );
  const [taskTokenUsage, setTaskTokenUsage] = useState<TokenUsageSummary | null>(null);
  const [executionChangeDetailOpen, setExecutionChangeDetailOpen] = useState(false);
  const [executionChangeDetailLoading, setExecutionChangeDetailLoading] = useState(false);
  const [executionChangeDetailError, setExecutionChangeDetailError] = useState<string | null>(null);
  const [selectedExecutionChange, setSelectedExecutionChange] =
    useState<CodexSessionFileChange | null>(null);
  const [executionChangeDetail, setExecutionChangeDetail] =
    useState<CodexSessionFileChangeDetail | null>(null);
  const [executionChangeRevealLine, setExecutionChangeRevealLine] = useState<number | null>(null);
  const [executionChangeFindingMessage, setExecutionChangeFindingMessage] = useState<string | null>(
    null,
  );
  const [coordinatorPlanDialogOpen, setCoordinatorPlanDialogOpen] = useState(false);
  const [coordinatorPlanDraft, setCoordinatorPlanDraft] = useState("");
  const [coordinatorPlanLoading, setCoordinatorPlanLoading] = useState(false);
  const [coordinatorPlanSaving, setCoordinatorPlanSaving] = useState(false);
  const [coordinatorPlanExecuting, setCoordinatorPlanExecuting] = useState(false);
  const [coordinatorPlanError, setCoordinatorPlanError] = useState<string | null>(null);
  const [coordinatorPlanLogs, setCoordinatorPlanLogs] = useState<string[]>([]);
  const [coordinatorPlanTerminalVisible, setCoordinatorPlanTerminalVisible] = useState(false);
  const [pipelineStepLogTarget, setPipelineStepLogTarget] = useState<{
    sessionRecordId: string;
    stepTitle: string;
    employeeName: string | null;
  } | null>(null);
  const [testerAcceptanceLoading, setTesterAcceptanceLoading] = useState(false);
  const [testerAcceptanceError, setTesterAcceptanceError] = useState<string | null>(null);
  const [testerAcceptanceNotice, setTesterAcceptanceNotice] = useState<string | null>(null);
  const [acceptanceChecklist, setAcceptanceChecklist] = useState(task.acceptance_checklist ?? "");
  const [lastAcceptanceStatus, setLastAcceptanceStatus] = useState<string | null>(
    task.last_acceptance_status ?? null,
  );
  const [lastAcceptanceSummary, setLastAcceptanceSummary] = useState<string | null>(null);
  const [acceptanceRunning, setAcceptanceRunning] = useState(false);
  const [detailTab, setDetailTab] = useState("overview");
  const [commitActionState, setCommitActionState] = useState<TaskCommitActionState | null>(null);
  const [showCommitDialog, setShowCommitDialog] = useState(false);
  const [openingCommitDialog, setOpeningCommitDialog] = useState(false);
  const [initialCommitOverview, setInitialCommitOverview] = useState<TaskCommitOverview | null>(
    null,
  );
  const [initialCommitError, setInitialCommitError] = useState<string | null>(null);
  const [primaryActionNotice, setPrimaryActionNotice] = useState<string | null>(null);
  const latestReviewRequestIdRef = useRef(0);
  const executionChangeDetailRequestIdRef = useRef(0);
  const pipelineStepsRequestIdRef = useRef(0);
  const taskIdCopyResetTimerRef = useRef<number | null>(null);
  const executionStartErrorRef = useRef<string | null>(null);
  const terminalRef = useRef<HTMLDivElement>(null);
  const aiLogRef = useRef<HTMLDivElement>(null);
  const assignee = assigneeId
    ? employees.find((employee) => employee.id === assigneeId)
    : undefined;
  const reviewer = reviewerId
    ? employees.find((employee) => employee.id === reviewerId)
    : undefined;
  const coordinator = coordinatorId
    ? employees.find((employee) => employee.id === coordinatorId)
    : undefined;
  const coordinatorCandidates = employees.filter((employee) => employee.role === "coordinator");
  const reviewerCandidates = employees.filter((employee) => employee.role === "reviewer");
  const projectTesters = employees.filter(
    (employee) => employee.role === "tester" && employee.project_id === task.project_id,
  );
  const reviewerIsTester = Boolean(reviewer && reviewer.role === "tester");
  const canGenerateTesterAcceptance = reviewerIsTester || projectTesters.length > 0;
  const resolveTesterIdForAcceptance = () => {
    if (reviewerIsTester && reviewerId) {
      return reviewerId;
    }
    return projectTesters[0]?.id ?? null;
  };
  const planContentHasChanges = planContentDraft !== planContent;
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
    assigneeId,
    assignee,
    projectRepoPath,
    projectType: project?.project_type,
    prepareExecutionInput: async (followUpPrompt, options) => {
      await Promise.all([fetchSubtasks(task.id), fetchAttachments(task.id)]);
      const executionInput = buildTaskExecutionInput({
        title,
        description,
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
    onStarted: () => {
      setStatus("in_progress");
    },
    onError: (message) => {
      executionStartErrorRef.current = message;
      appendCoordinatorPlanLog(`[ERROR] ${message}`);
      setCoordinatorPlanError(message);
    },
  });
  const reviewActions = useTaskReviewActions({
    task,
    reviewerId,
    status,
    onStarted: () => {
      setReviewError(null);
      setReviewNotice(null);
      setLatestReview(null);
      void loadLatestReview();
    },
    onError: (message) => {
      setReviewError(message);
    },
  });
  const backgroundRun = useTaskBackgroundRunStore((state) => state.byTaskId[task.id]);
  const isBackgroundPlanning = backgroundRun?.phase === "planning";
  const isBackgroundStarting = backgroundRun?.phase === "starting";
  const isBackgroundRunBusy = isBackgroundPlanning || isBackgroundStarting;
  const backgroundRunLabel = getTaskBackgroundRunLabel(backgroundRun);
  const resolvedAutomationState =
    automationState ?? getTaskAutomationDisplayState(task, persistedAutomationState ?? null);
  const runtimeState = getTaskActionRuntimeState({
    automationState: resolvedAutomationState,
    isExecutionRunning: executionActions.isRunning || isBackgroundRunBusy,
    isReviewRunning: reviewActions.isRunning,
    pipelineState: persistedAutomationState ?? null,
  });
  const aiActions = useTaskAiActions({
    task,
    open,
    title,
    description,
    status,
    priority,
    employees,
    projectRepoPath,
    fetchAttachments,
    fetchSubtasks,
    updateTask,
    addComment,
    addSubtasks,
    onDescriptionChange: setDescription,
  });
  const isRunning = runtimeState.executionActive;
  const isReviewRunning = runtimeState.reviewActive;
  const isExecutionProcessRunning = executionActions.isRunning;
  const hasActiveSession = isRunning || isReviewRunning;
  const canCommitTaskCode = Boolean(
    !hasActiveSession && (commitActionState?.can_commit || commitActionState?.can_ai_commit),
  );
  const pipelineRunning = isTaskPipelineRunning(persistedAutomationState ?? null);
  const assigneeOtherSessions =
    assigneeId && employeeRuntime[assigneeId]
      ? employeeRuntime[assigneeId].sessions.filter(
          (session) =>
            session.task_id &&
            session.task_id !== task.id &&
            (session.session_kind === "execution" || session.session_kind === "review"),
        )
      : [];
  const primaryCta = resolveTaskPrimaryCta({
    status,
    executionActive: isRunning,
    reviewActive: isReviewRunning,
    canStopProcess: executionActions.isRunning || pipelineRunning,
    backgroundPlanning: isBackgroundPlanning,
    backgroundStarting: isBackgroundStarting,
    hasAssignee: Boolean(assigneeId),
    hasReviewer: Boolean(reviewerId),
    canCommit: canCommitTaskCode,
    canGenerateAcceptance: canGenerateTesterAcceptance,
    automationStatus: resolvedAutomationState.status,
    pipelineActive: pipelineRunning,
    assigneeBusyOnOtherTask: assigneeOtherSessions.length > 0,
    hasIncompleteDependencies: incompleteDependencyTitles.length > 0,
    incompleteDependencySummary:
      incompleteDependencyTitles.length === 0
        ? null
        : incompleteDependencyTitles.length <= 2
          ? incompleteDependencyTitles.join("、")
          : `${incompleteDependencyTitles.slice(0, 2).join("、")} 等`,
    queued: Boolean(queuedItem),
  });
  const primaryActionLoading =
    executionActions.loading !== null ||
    reviewActions.loading ||
    testerAcceptanceLoading ||
    openingCommitDialog ||
    isBackgroundRunBusy ||
    pipelineActionLoading;
  const hasActivePipelineStep = pipelineSteps.some(
    (step) => step.status === "launching" || step.status === "running",
  );
  const coordinatorPlanActionsLocked =
    isRunning ||
    isReviewRunning ||
    pipelineActionLoading ||
    coordinatorPlanExecuting ||
    hasActivePipelineStep;
  const output = executionActions.output;
  const reviewOutput = reviewActions.output;
  const codexLoading = executionActions.loading !== null;
  const reviewLoading = reviewActions.loading;

  useEffect(() => {
    if (open) {
      fetchEmployees();
      void fetchAttachments(task.id);
      void fetchTaskAutomationState(task.id);
      void listTaskDependencies(task.id)
        .then((deps) => {
          const incomplete = deps
            .map((dep) => storeTasks.find((item) => item.id === dep.depends_on_task_id))
            .filter((item): item is Task => item != null && item.status !== "completed")
            .map((item) => item.title);
          setIncompleteDependencyTitles(incomplete);
        })
        .catch(() => setIncompleteDependencyTitles([]));
      setPipelineLoading(true);
      setPipelineError(null);
      setPipelineNotice(null);
      setPipelineSteps([]);
      const pipelineRequestId = ++pipelineStepsRequestIdRef.current;
      void listTaskPipelineSteps(task.id)
        .then((steps) => {
          if (pipelineStepsRequestIdRef.current !== pipelineRequestId) {
            return;
          }
          setPipelineSteps(steps);
        })
        .catch((error) => {
          if (pipelineStepsRequestIdRef.current !== pipelineRequestId) {
            return;
          }
          setPipelineError(error instanceof Error ? error.message : String(error));
        })
        .finally(() => {
          if (pipelineStepsRequestIdRef.current === pipelineRequestId) {
            setPipelineLoading(false);
          }
        });
      setTitle(task.title);
      setDescription(task.description ?? "");
      setPriority(task.priority);
      setStatus(task.status);
      setAssigneeId(task.assignee_id ?? "");
      setReviewerId(task.reviewer_id ?? "");
      setCoordinatorId(task.coordinator_id ?? "");
      setDueDate(task.due_date ?? "");
      setMilestoneId(task.milestone_id ?? "");
      setBlockedReason(task.blocked_reason ?? "");
      setPlanContent(task.plan_content ?? "");
      setAttachmentError(null);
      setSaveError(null);
      setReviewError(null);
      setReviewNotice(null);
      setPrimaryActionNotice(null);
      setDetailTab("overview");
      void loadLatestReview();
      void loadExecutionChangeHistory();
    }
  }, [fetchAttachments, fetchEmployees, open, task, fetchTaskAutomationState]);

  useEffect(() => {
    if (!open) {
      setDeleteDialogOpen(false);
      setPipelineStepLogTarget(null);
      return;
    }

    let active = true;
    let unlisten: (() => void) | null = null;
    void onTaskAutomationStateChanged((event) => {
      if (!active || event.task_id !== task.id) {
        return;
      }
      const pipelineRequestId = ++pipelineStepsRequestIdRef.current;
      void listTaskPipelineSteps(task.id)
        .then((steps) => {
          if (!active || pipelineStepsRequestIdRef.current !== pipelineRequestId) {
            return;
          }
          setPipelineSteps(steps);
        })
        .catch((error) => {
          if (!active || pipelineStepsRequestIdRef.current !== pipelineRequestId) {
            return;
          }
          setPipelineError(error instanceof Error ? error.message : String(error));
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
  }, [open, task.id, fetchTaskAutomationState]);

  useEffect(() => {
    if (!open) {
      setCommitActionState(null);
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
  }, [open, task.id, task.updated_at, task.status, task.use_worktree, hasActiveSession]);

  useEffect(() => {
    return () => {
      if (taskIdCopyResetTimerRef.current !== null) {
        window.clearTimeout(taskIdCopyResetTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!open) {
      latestReviewRequestIdRef.current += 1;
      return;
    }

    setAttachmentLoading(false);
    setDeletingAttachmentId(null);
    setLatestReview(null);
    setLatestReviewLoading(false);
    setExecutionChangeHistory([]);
    setExecutionChangeHistoryLoading(false);
    setExecutionChangeHistoryError(null);
    setReviewError(null);
    setReviewNotice(null);
    setReviewFixDialogOpen(false);
    setReviewFixSubmitting(false);
    setCoordinatorPlanDialogOpen(false);
    setCoordinatorPlanDraft("");
    setCoordinatorPlanLoading(false);
    setCoordinatorPlanSaving(false);
    setCoordinatorPlanExecuting(false);
    setCoordinatorPlanError(null);
    setCoordinatorPlanLogs([]);
    setCoordinatorPlanTerminalVisible(false);
    setTesterAcceptanceLoading(false);
    setTesterAcceptanceError(null);
    setTesterAcceptanceNotice(null);
    setAcceptanceChecklist(task.acceptance_checklist ?? "");
    setLastAcceptanceStatus(task.last_acceptance_status ?? null);
    setLastAcceptanceSummary(null);
    setAcceptanceRunning(false);
    setPlanContentDraft(task.plan_content ?? "");
    setPlanContentEditing(false);
    setPlanContentSaving(false);
  }, [open, task.id]);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void (async () => {
      try {
        const runs = await getTaskAcceptanceRuns(task.id);
        if (cancelled || runs.length === 0) return;
        const latest = runs[0];
        setLastAcceptanceStatus(latest.status);
        setLastAcceptanceSummary(latest.summary);
        if (!acceptanceChecklist.trim() && latest.acceptance_checklist) {
          setAcceptanceChecklist(latest.acceptance_checklist);
        }
      } catch {
        // 历史加载失败不阻断主流程
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, task.id]);

  useEffect(() => {
    terminalRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [output.length]);

  useEffect(() => {
    aiLogRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [aiActions.aiLogs.length]);

  useEffect(() => {
    if (!open || !task.last_review_session_id) {
      return;
    }

    void loadLatestReview();
  }, [open, task.last_review_session_id]);

  useEffect(() => {
    if (!open || reviewOutput.length === 0) {
      return;
    }

    if (reviewOutput[reviewOutput.length - 1]?.startsWith("[EXIT]")) {
      void loadLatestReview();
    }
  }, [open, reviewOutput]);

  useEffect(() => {
    if (!open || output.length === 0) {
      return;
    }

    const lastLine = output[output.length - 1] ?? "";
    if (lastLine.startsWith("[EXIT]")) {
      void loadExecutionChangeHistory();
    } else if (lastLine.startsWith("[用量]")) {
      void loadTaskTokenUsage();
    }
  }, [open, output]);

  const handleSave = async (field: string, value: string) => {
    setSaveError(null);
    try {
      if (field === "title" && value.trim()) {
        await updateTask(task.id, { title: value.trim() });
      } else if (field === "description") {
        await updateTask(task.id, { description: value || null });
      } else if (field === "priority") {
        await updateTask(task.id, { priority: value });
      } else if (field === "status") {
        if (value === "blocked") {
          setPendingBlockedReason(blockedReason);
          setBlockReasonDialogOpen(true);
          return;
        }
        await updateTask(task.id, {
          status: value,
          ...(task.status === "blocked" ? { blocked_reason: null } : {}),
        });
        if (task.status === "blocked") {
          setBlockedReason("");
        }
      } else if (field === "assignee_id") {
        await updateTask(task.id, { assignee_id: value || null });
      } else if (field === "reviewer_id") {
        await updateTask(task.id, { reviewer_id: value || null });
      } else if (field === "coordinator_id") {
        const previousCoordinatorId = task.coordinator_id ?? "";
        await updateTask(task.id, { coordinator_id: value || null });
        if (previousCoordinatorId !== value) {
          setPlanContent("");
          setPlanContentDraft("");
          setPlanContentEditing(false);
          setCoordinatorPlanDraft("");
        }
      } else if (field === "due_date") {
        await updateTask(task.id, { due_date: value || null });
      } else if (field === "milestone_id") {
        await updateTask(task.id, { milestone_id: value || null });
      } else if (field === "blocked_reason") {
        await updateTask(task.id, { blocked_reason: value.trim() || null });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSaveError(message);
      if (field === "status") {
        setStatus(task.status);
      }
    }
  };

  const handleConfirmBlockedStatus = async () => {
    const reason = pendingBlockedReason.trim();
    if (!reason) {
      setSaveError(t("detail.blockReason.required"));
      return;
    }
    setBlockReasonSubmitting(true);
    setSaveError(null);
    try {
      await updateTask(task.id, { status: "blocked", blocked_reason: reason });
      setStatus("blocked");
      setBlockedReason(reason);
      setBlockReasonDialogOpen(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSaveError(message);
      setStatus(task.status);
    } finally {
      setBlockReasonSubmitting(false);
    }
  };

  const handleDelete = async () => {
    setDeletingTask(true);
    try {
      await deleteTask(task.id);
      setDeleteDialogOpen(false);
      onOpenChange(false);
    } catch (error) {
      console.error("Failed to delete task:", error);
    } finally {
      setDeletingTask(false);
    }
  };

  const handleSelectAttachments = async () => {
    const selected = await openFileDialog({
      directory: false,
      multiple: true,
      title: t("detail.fileDialogTitle"),
    });
    const sourcePaths = dedupePaths(normalizeDialogSelection(selected));

    if (sourcePaths.length === 0) {
      return;
    }

    setAttachmentLoading(true);
    setAttachmentError(null);
    try {
      await addTaskAttachments(task.id, sourcePaths);
    } catch (error) {
      setAttachmentError(error instanceof Error ? error.message : String(error));
    } finally {
      setAttachmentLoading(false);
    }
  };

  const handleOpenAttachment = async (path: string) => {
    try {
      await openTaskAttachment(path);
    } catch (error) {
      setAttachmentError(error instanceof Error ? error.message : String(error));
    }
  };

  const handleDeleteAttachment = async (attachmentId: string) => {
    setDeletingAttachmentId(attachmentId);
    setAttachmentError(null);
    try {
      await deleteTaskAttachment(task.id, attachmentId);
    } catch (error) {
      setAttachmentError(error instanceof Error ? error.message : String(error));
    } finally {
      setDeletingAttachmentId(null);
    }
  };

  const loadLatestReview = async () => {
    const requestId = latestReviewRequestIdRef.current + 1;
    latestReviewRequestIdRef.current = requestId;
    setLatestReviewLoading(true);
    try {
      const review = await getTaskLatestReview(task.id);
      if (latestReviewRequestIdRef.current !== requestId) {
        return;
      }
      setLatestReview(review);
    } catch (error) {
      if (latestReviewRequestIdRef.current !== requestId) {
        return;
      }
      console.error("Failed to load latest task review:", error);
      setReviewError(error instanceof Error ? error.message : String(error));
    } finally {
      if (latestReviewRequestIdRef.current === requestId) {
        setLatestReviewLoading(false);
      }
    }
  };

  const loadTaskTokenUsage = async () => {
    try {
      setTaskTokenUsage(await getTaskTokenUsage(task.id));
    } catch (error) {
      console.error("Failed to load task token usage:", error);
      setTaskTokenUsage(null);
    }
  };

  const loadExecutionChangeHistory = async () => {
    setExecutionChangeHistoryLoading(true);
    setExecutionChangeHistoryError(null);
    void loadTaskTokenUsage();
    try {
      const history = await getTaskExecutionChangeHistory(task.id);
      setExecutionChangeHistory(history);
    } catch (error) {
      console.error("Failed to load task execution file changes:", error);
      setExecutionChangeHistoryError(error instanceof Error ? error.message : String(error));
    } finally {
      setExecutionChangeHistoryLoading(false);
    }
  };

  const handleOpenExecutionChangeDetail = async (
    change: CodexSessionFileChange,
    options?: { line?: number | null; message?: string },
  ) => {
    const requestId = executionChangeDetailRequestIdRef.current + 1;
    executionChangeDetailRequestIdRef.current = requestId;
    setSelectedExecutionChange(change);
    setExecutionChangeDetail(null);
    setExecutionChangeRevealLine(options?.line && options.line > 0 ? options.line : null);
    setExecutionChangeFindingMessage(options?.message?.trim() ? options.message.trim() : null);
    setExecutionChangeDetailOpen(true);
    setExecutionChangeDetailLoading(true);
    setExecutionChangeDetailError(null);
    try {
      const detail = await getCodexSessionFileChangeDetail(change.id);
      if (executionChangeDetailRequestIdRef.current !== requestId) {
        return;
      }
      setExecutionChangeDetail(detail);
    } catch (error) {
      if (executionChangeDetailRequestIdRef.current !== requestId) {
        return;
      }
      console.error("Failed to load session file change detail:", error);
      setExecutionChangeDetail(null);
      setExecutionChangeDetailError(error instanceof Error ? error.message : String(error));
    } finally {
      if (executionChangeDetailRequestIdRef.current === requestId) {
        setExecutionChangeDetailLoading(false);
      }
    }
  };

  const handleExecutionChangeDetailOpenChange = (nextOpen: boolean) => {
    setExecutionChangeDetailOpen(nextOpen);
    if (!nextOpen) {
      executionChangeDetailRequestIdRef.current += 1;
      setExecutionChangeDetailLoading(false);
      setExecutionChangeDetailError(null);
      setExecutionChangeDetail(null);
      setSelectedExecutionChange(null);
      setExecutionChangeRevealLine(null);
      setExecutionChangeFindingMessage(null);
    }
  };

  const refreshPipelineSteps = async () => {
    const pipelineRequestId = ++pipelineStepsRequestIdRef.current;
    setPipelineLoading(true);
    setPipelineError(null);
    try {
      const steps = await listTaskPipelineSteps(task.id);
      if (pipelineStepsRequestIdRef.current !== pipelineRequestId) {
        return;
      }
      setPipelineSteps(steps);
    } catch (error) {
      if (pipelineStepsRequestIdRef.current !== pipelineRequestId) {
        return;
      }
      setPipelineError(error instanceof Error ? error.message : String(error));
    } finally {
      if (pipelineStepsRequestIdRef.current === pipelineRequestId) {
        setPipelineLoading(false);
      }
    }
  };

  useEffect(() => {
    if (!coordinatorPlanDialogOpen) {
      return;
    }
    void refreshPipelineSteps();
    void fetchTaskAutomationState(task.id);
  }, [coordinatorPlanDialogOpen, task.id, fetchTaskAutomationState]);

  const generateCoordinatorPlan = async () => {
    setCoordinatorPlanTerminalVisible(true);
    if (!coordinatorId) {
      appendCoordinatorPlanLog("[ERROR] 当前任务未指定协调员，无法生成计划。");
      setCoordinatorPlanError("请先指定协调员。");
      return;
    }

    setCoordinatorPlanLoading(true);
    setCoordinatorPlanError(null);
    appendCoordinatorPlanLog(`[计划] 准备调用协调员：${coordinator?.name ?? coordinatorId}`);
    appendCoordinatorPlanLog(`[计划] 运行配置：${getCoordinatorPlanRuntimeLabel()}`);
    appendCoordinatorPlanLog(`[计划] 工作目录：${projectRepoPath ?? "未配置"}`);
    appendCoordinatorPlanLog("[计划] 正在生成协调员执行计划，可能需要一点时间...");
    try {
      const plan = await aiGenerateCoordinatorTaskPlan({
        task_id: task.id,
        coordinator_id: coordinatorId,
        title: title.trim() || task.title,
        description: description.trim() || null,
        status,
        priority,
        working_dir: projectRepoPath ?? null,
      });
      const trimmedPlan = plan.trim();
      if (!trimmedPlan) {
        appendCoordinatorPlanLog("[WARN] 协调员返回了空计划。");
        setCoordinatorPlanError("协调员未返回可用计划。");
        return;
      }
      setCoordinatorPlanDraft(trimmedPlan);
      setPlanContent(trimmedPlan);
      setPlanContentDraft(trimmedPlan);
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
        if (!coordinatorId) {
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
      setPipelineNotice("编排已停止并转人工。未完成步骤可点「手动运行」或「转自动」。");
      await refreshPipelineSteps();
      await fetchTaskAutomationState(task.id);
    } catch (error) {
      setPipelineError(error instanceof Error ? error.message : String(error));
    } finally {
      setPipelineActionLoading(false);
    }
  };

  const handleResumeAutoPipeline = async () => {
    setPipelineActionLoading(true);
    setPipelineError(null);
    setPipelineNotice(null);
    try {
      await resumeTaskPipeline(task.id);
      setPipelineNotice("编排已转自动，将继续串行执行。");
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

  const handleManualStopPipelineStep = async (step: TaskPipelineStep) => {
    setPipelineActionLoading(true);
    setPipelineError(null);
    setPipelineNotice(null);
    try {
      await stopTaskPipelineStepManual(task.id, step.id);
      setPipelineNotice(`已手动停止步骤 ${step.step_index + 1}「${step.title}」。`);
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
    const existingPlan = planContent.trim();
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

  const refreshTasksAfterAcceptanceChange = async () => {
    await useTaskStore.getState().fetchTasks(useTaskStore.getState().activeProjectId);
  };

  const handleGenerateTesterAcceptance = async () => {
    const testerId = resolveTesterIdForAcceptance();
    if (!testerId) {
      setTesterAcceptanceError("当前项目没有可用的测试员，无法生成验收清单。");
      setTesterAcceptanceNotice(null);
      return;
    }

    setTesterAcceptanceLoading(true);
    setTesterAcceptanceError(null);
    setTesterAcceptanceNotice(null);
    try {
      const checklist = await aiGenerateTesterAcceptance({
        task_id: task.id,
        tester_id: testerId,
        working_dir: projectRepoPath ?? null,
      });
      await fetchComments(task.id);
      // 后端同时写入 tasks.acceptance_checklist（原始正文）与评论（带前缀）
      const plain = checklist.replace(/^\[验收清单\]\s*/m, "").trim();
      setAcceptanceChecklist(plain || checklist);
      await refreshTasksAfterAcceptanceChange();
      setTesterAcceptanceNotice(
        `验收清单已生成（共 ${checklist.trim().length} 字），并写入任务字段与评论。`,
      );
    } catch (error) {
      setTesterAcceptanceError(error instanceof Error ? error.message : String(error));
    } finally {
      setTesterAcceptanceLoading(false);
    }
  };

  const handleSaveAcceptanceChecklist = async () => {
    const next = acceptanceChecklist.trim();
    const current = (task.acceptance_checklist ?? "").trim();
    if (next === current) {
      return;
    }
    try {
      const updated = await updateTaskAcceptanceChecklist(task.id, acceptanceChecklist);
      setAcceptanceChecklist(updated.acceptance_checklist ?? "");
      setLastAcceptanceStatus(updated.last_acceptance_status ?? lastAcceptanceStatus);
      await refreshTasksAfterAcceptanceChange();
      setTesterAcceptanceNotice("验收清单已保存。");
      setTesterAcceptanceError(null);
    } catch (error) {
      setTesterAcceptanceError(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRunAcceptance = async () => {
    setAcceptanceRunning(true);
    setTesterAcceptanceError(null);
    setTesterAcceptanceNotice(null);
    try {
      if (
        acceptanceChecklist.trim() &&
        acceptanceChecklist.trim() !== (task.acceptance_checklist ?? "").trim()
      ) {
        await updateTaskAcceptanceChecklist(task.id, acceptanceChecklist);
      }
      const run = await runTaskAcceptance(task.id, "manual");
      setLastAcceptanceStatus(run.status);
      setLastAcceptanceSummary(run.summary);
      await refreshTasksAfterAcceptanceChange();
      if (run.status === "passed") {
        setTesterAcceptanceNotice(run.summary ?? "验收通过");
      } else if (run.status === "skipped") {
        setTesterAcceptanceNotice(run.summary ?? "验收已跳过");
      } else {
        setTesterAcceptanceError(run.summary ?? "验收失败");
      }
    } catch (error) {
      setTesterAcceptanceError(error instanceof Error ? error.message : String(error));
    } finally {
      setAcceptanceRunning(false);
    }
  };

  const handleRunCodex = async () => {
    if (isRunning || isReviewRunning || queuedItem) {
      return;
    }
    if (coordinatorId) {
      await openCoordinatorPlanFlow();
      return;
    }

    await executionActions.runTask();
  };

  const handleStopCodex = async () => {
    if (isTaskPipelineRunning(persistedAutomationState ?? null)) {
      setPipelineActionLoading(true);
      setPipelineError(null);
      setPipelineNotice(null);
      try {
        await abortTaskPipeline(task.id);
        setPipelineNotice("编排已停止并转人工。未完成步骤可点「手动运行」或「转自动」。");
        await refreshPipelineSteps();
        await fetchTaskAutomationState(task.id);
        await useTaskStore.getState().fetchTasks(useTaskStore.getState().activeProjectId);
      } catch (error) {
        setPipelineError(error instanceof Error ? error.message : String(error));
      } finally {
        setPipelineActionLoading(false);
      }
      return;
    }
    await executionActions.stopTask();
  };

  const openCommitDialog = async () => {
    if (!canCommitTaskCode) {
      setPrimaryActionNotice(t("detail.chrome.noCommitableChanges"));
      return;
    }
    setOpeningCommitDialog(true);
    setPrimaryActionNotice(null);

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

  const handlePrimaryCtaAction = async () => {
    if (primaryCta.disabled || primaryActionLoading) {
      return;
    }

    switch (primaryCta.kind) {
      case "stop":
        setDetailTab("execution");
        await handleStopCodex();
        return;
      case "run":
        setDetailTab("execution");
        await handleRunCodex();
        return;
      case "review":
        setDetailTab("review");
        await handleStartCodeReview();
        return;
      case "blocked":
        setDetailTab("overview");
        setPrimaryActionNotice(
          blockedReason.trim()
            ? t("detail.chrome.blockedReasonNotice", { reason: blockedReason.trim() })
            : t("detail.chrome.blockedOpenOverview"),
        );
        return;
      case "commit":
        await openCommitDialog();
        return;
      case "acceptance":
        setDetailTab("overview");
        await handleGenerateTesterAcceptance();
        return;
      default:
        return;
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
      setPlanContent(plan);
      setPlanContentDraft(plan);
      setPlanContentEditing(false);
      appendCoordinatorPlanLog("[计划] 协调员计划已保存到任务。");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      appendCoordinatorPlanLog(`[ERROR] ${message}`);
      setCoordinatorPlanError(message);
    } finally {
      setCoordinatorPlanSaving(false);
    }
  };

  const handleStartPlanContentEdit = () => {
    setSaveError(null);
    setPlanContentDraft(planContent);
    setPlanContentEditing(true);
  };

  const handleCancelPlanContentEdit = () => {
    setSaveError(null);
    setPlanContentDraft(planContent);
    setPlanContentEditing(false);
  };

  const handleSavePlanContent = async () => {
    const nextPlanContent = planContentDraft.trim();
    if (!nextPlanContent) {
      setSaveError("计划内容不能为空。");
      return;
    }
    if (!planContentHasChanges) {
      setPlanContentEditing(false);
      return;
    }

    setPlanContentSaving(true);
    setSaveError(null);
    try {
      await updateTask(task.id, { plan_content: nextPlanContent });
      setPlanContent(nextPlanContent);
      setPlanContentDraft(nextPlanContent);
      setPlanContentEditing(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSaveError(message);
    } finally {
      setPlanContentSaving(false);
    }
  };

  const handleExecuteCoordinatorPlan = async () => {
    const plan = coordinatorPlanDraft.trim();
    if (!plan) {
      setCoordinatorPlanError("计划内容不能为空。");
      return;
    }
    if (!assigneeId) {
      setCoordinatorPlanTerminalVisible(true);
      appendCoordinatorPlanLog("[ERROR] 当前任务未指定执行员工，无法执行计划。");
      setCoordinatorPlanError("请先指定执行员工，再执行计划。");
      return;
    }

    setCoordinatorPlanExecuting(true);
    setCoordinatorPlanError(null);
    executionStartErrorRef.current = null;
    setCoordinatorPlanTerminalVisible(true);
    appendCoordinatorPlanLog(`[执行] 正在把计划交给执行员工：${assignee?.name ?? assigneeId}`);
    try {
      await executionActions.runTask(plan);
      if (!executionStartErrorRef.current) {
        appendCoordinatorPlanLog("[执行] 执行会话已启动，完整终端输出会继续写入任务日志。");
        setCoordinatorPlanDialogOpen(false);
      }
    } finally {
      setCoordinatorPlanExecuting(false);
    }
  };

  const buildReviewFixTaskDescription = (reviewReport: string) => {
    const sections = [
      "基于代码审核结果创建的修复任务。",
      `原任务：${task.title}`,
      task.description?.trim() ? `原任务描述：\n${task.description.trim()}` : null,
      latestReview?.session.cli_session_id
        ? `审核会话：${latestReview.session.cli_session_id}`
        : latestReview?.session.id
          ? `审核记录：${latestReview.session.id}`
          : null,
      `审核结果：\n${reviewReport.trim()}`,
    ].filter(Boolean);

    return sections.join("\n\n");
  };

  const handleStartCodeReview = async () => {
    if (!reviewerId) {
      setReviewError("请先指定审查员，再执行代码审核。");
      return;
    }

    setReviewError(null);
    setReviewNotice(null);
    await reviewActions.startReview();
  };

  const handleCopyReviewReport = async () => {
    if (!latestReview?.report?.trim()) {
      setReviewError("当前没有可复制的审核结果。");
      return;
    }

    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("当前环境不支持剪贴板写入");
      }

      await navigator.clipboard.writeText(latestReview.report);
      setReviewError(null);
      setReviewNotice("审核结果已复制到剪贴板。");
    } catch (error) {
      setReviewNotice(null);
      setReviewError(error instanceof Error ? error.message : String(error));
    }
  };

  const handleCopyTaskId = async () => {
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("当前环境不支持剪贴板写入");
      }

      await navigator.clipboard.writeText(task.id);
      setTaskIdCopied(true);

      if (taskIdCopyResetTimerRef.current !== null) {
        window.clearTimeout(taskIdCopyResetTimerRef.current);
      }

      taskIdCopyResetTimerRef.current = window.setTimeout(() => {
        setTaskIdCopied(false);
      }, 1600);
    } catch (error) {
      setTaskIdCopied(false);
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  };

  const handleConfirmReviewFix = async () => {
    const reviewReport = latestReview?.report?.trim();
    if (!reviewReport) {
      setReviewError("请先生成审核结果，再创建修复任务。");
      return;
    }
    if (!assigneeId) {
      setReviewError("原任务未指派开发负责人，无法创建并运行修复任务。");
      return;
    }
    if (!projectRepoPath) {
      setReviewError("当前项目未配置仓库路径，无法立即运行修复任务。");
      return;
    }

    const fixTaskTitle = `修复：${task.title}`.slice(0, 120);
    const fixTaskDescription = buildReviewFixTaskDescription(reviewReport);

    setReviewFixSubmitting(true);
    setReviewError(null);
    setReviewNotice(null);
    try {
      const createdTask = await createTask({
        title: fixTaskTitle,
        description: fixTaskDescription,
        priority,
        project_id: task.project_id,
        use_worktree: task.use_worktree,
        assignee_id: assigneeId,
      });

      const executionInput = buildTaskExecutionInput({
        title: createdTask.title,
        description: createdTask.description,
      });
      const outcome = await startTaskRunSession({
        task: createdTask,
        assigneeId,
        assignee,
        projectRepoPath,
        executionInput,
        clearTaskOutput: true,
      });

      setReviewFixDialogOpen(false);
      setReviewNotice(
        outcome.status === "queued"
          ? t("detail.review.fixCreatedQueued", {
              title: createdTask.title,
              position: outcome.position,
            })
          : t("detail.review.fixCreatedStarted", { title: createdTask.title }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setReviewError(message);
      addCodexOutput(assigneeId, `[ERROR] ${message}`);
      const runtime = await refreshEmployeeRuntimeStatus(assigneeId);
      if (!runtime?.running) {
        await updateEmployeeStatus(assigneeId, "error");
      }
    } finally {
      setReviewFixSubmitting(false);
    }
  };

  const primarySecondaryActions = [
    canCommitTaskCode && primaryCta.kind !== "commit"
      ? {
          key: "commit",
          label: t("detail.secondary.commit"),
          disabled: primaryActionLoading,
          onSelect: () => {
            void openCommitDialog();
          },
        }
      : null,
    canGenerateTesterAcceptance && primaryCta.kind !== "acceptance"
      ? {
          key: "acceptance",
          label: t("detail.secondary.generateChecklist"),
          disabled: testerAcceptanceLoading,
          onSelect: () => {
            setDetailTab("overview");
            void handleGenerateTesterAcceptance();
          },
        }
      : null,
    primaryCta.kind !== "run" &&
    primaryCta.kind !== "queued" &&
    Boolean(assigneeId) &&
    !hasActiveSession
      ? {
          key: "run",
          label: t("detail.secondary.run"),
          disabled: primaryActionLoading,
          onSelect: () => {
            setDetailTab("execution");
            void handleRunCodex();
          },
        }
      : null,
    primaryCta.kind !== "review" && Boolean(reviewerId) && status === "review" && !isReviewRunning
      ? {
          key: "review",
          label: t("detail.secondary.reviewCode"),
          disabled: primaryActionLoading,
          onSelect: () => {
            setDetailTab("review");
            void handleStartCodeReview();
          },
        }
      : null,
  ].filter((item): item is NonNullable<typeof item> => item != null);

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="flex max-h-[min(92vh,calc(100vh-2rem))] w-[min(96vw,80rem)] max-w-[min(96vw,80rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(96vw,80rem)]">
          <DialogHeader className="sr-only">
            <DialogTitle>{t("detail.dialogTitle")}</DialogTitle>
            <DialogDescription>{t("detail.dialogDescription")}</DialogDescription>
          </DialogHeader>

          <TaskDetailHeader
            title={title}
            onTitleChange={setTitle}
            onTitleBlur={() => void handleSave("title", title)}
            taskId={task.id}
            taskIdCopied={taskIdCopied}
            onCopyTaskId={() => void handleCopyTaskId()}
            status={status}
            projectName={project?.name}
            createdAt={task.created_at}
          />

          <Tabs
            value={detailTab}
            onValueChange={setDetailTab}
            className="flex min-h-0 flex-1 flex-col gap-0"
          >
            <div className="shrink-0 overflow-x-auto border-b border-border/70 px-5">
              <TabsList
                variant="line"
                className="w-full min-w-max justify-start gap-2 group-data-horizontal/tabs:h-10"
              >
                <TabsTrigger value="overview">{t("detail.tabs.overview")}</TabsTrigger>
                <TabsTrigger value="execution">{t("detail.tabs.execution")}</TabsTrigger>
                <TabsTrigger value="chain">{t("detail.tabs.chain")}</TabsTrigger>
                <TabsTrigger value="review">{t("detail.tabs.review")}</TabsTrigger>
                <TabsTrigger value="ai">{t("detail.tabs.ai")}</TabsTrigger>
                <TabsTrigger value="collaboration">{t("detail.tabs.collaboration")}</TabsTrigger>
              </TabsList>
            </div>

            <div className="flex min-h-0 flex-1">
              <div className="min-w-0 flex-1 overflow-y-auto px-5 py-4">
                {(primaryActionNotice ||
                  (primaryCta.kind === "starting" && backgroundRunLabel) ||
                  testerAcceptanceNotice ||
                  testerAcceptanceError) && (
                  <div className="mb-4 rounded-lg border border-border/60 bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
                    {primaryActionNotice && (
                      <p className="text-foreground/90">{primaryActionNotice}</p>
                    )}
                    {primaryCta.kind === "starting" && backgroundRunLabel && (
                      <p>{backgroundRunLabel}</p>
                    )}
                    {testerAcceptanceNotice && (
                      <p className="text-emerald-700 dark:text-emerald-300">
                        {testerAcceptanceNotice}
                      </p>
                    )}
                    {testerAcceptanceError && (
                      <p className="text-destructive">{testerAcceptanceError}</p>
                    )}
                  </div>
                )}

                <TabsContent value="overview">
                  <TaskOverviewPanel
                    task={task}
                    description={description}
                    status={status}
                    coordinatorId={coordinatorId}
                    coordinatorName={coordinator?.name}
                    blockedReason={blockedReason}
                    planContent={planContent}
                    planContentDraft={planContentDraft}
                    planEditing={planContentEditing}
                    planSaving={planContentSaving}
                    planHasChanges={planContentHasChanges}
                    employees={employees}
                    coordinatorCandidates={coordinatorCandidates}
                    saveError={saveError}
                    automationDisplay={resolvedAutomationState}
                    canGenerateTesterAcceptance={canGenerateTesterAcceptance}
                    testerAcceptanceLoading={testerAcceptanceLoading}
                    testerAcceptanceError={testerAcceptanceError}
                    testerAcceptanceNotice={testerAcceptanceNotice}
                    pipelineSteps={pipelineSteps}
                    pipelineAutomation={persistedAutomationState ?? null}
                    pipelineLoading={pipelineLoading}
                    pipelineError={pipelineError}
                    onRefreshPipeline={() => void refreshPipelineSteps()}
                    onOpenPipelineStepSession={(step) => {
                      if (!step.session_id) {
                        return;
                      }
                      const stepEmployee = employees.find((item) => item.id === step.employee_id);
                      setPipelineStepLogTarget({
                        sessionRecordId: step.session_id,
                        stepTitle: step.title,
                        employeeName: stepEmployee?.name ?? null,
                      });
                    }}
                    acceptanceChecklist={acceptanceChecklist}
                    lastAcceptanceStatus={lastAcceptanceStatus}
                    lastAcceptanceSummary={lastAcceptanceSummary}
                    acceptanceRunning={acceptanceRunning}
                    onAcceptanceChecklistChange={setAcceptanceChecklist}
                    onAcceptanceChecklistBlur={() => void handleSaveAcceptanceChecklist()}
                    onRunAcceptance={() => void handleRunAcceptance()}
                    onDescriptionChange={setDescription}
                    onDescriptionBlur={() => void handleSave("description", description)}
                    onBlockedReasonChange={setBlockedReason}
                    onBlockedReasonBlur={() => {
                      if (status === "blocked") {
                        void handleSave("blocked_reason", blockedReason);
                      }
                    }}
                    onOpenCoordinatorPlan={() => void openCoordinatorPlanFlow()}
                    onGenerateTesterAcceptance={() => void handleGenerateTesterAcceptance()}
                    onPlanEditStart={handleStartPlanContentEdit}
                    onPlanEditCancel={handleCancelPlanContentEdit}
                    onPlanDraftChange={setPlanContentDraft}
                    onPlanSave={() => void handleSavePlanContent()}
                  />
                </TabsContent>

                <TabsContent value="execution">
                  <TaskExecutionPanel
                    taskStatus={status}
                    assigneeId={assigneeId}
                    isRunning={isExecutionProcessRunning}
                    isExecutionActive={isRunning}
                    codexLoading={codexLoading}
                    output={output}
                    terminalRef={terminalRef}
                    tokenUsage={taskTokenUsage}
                    executionChangeHistory={executionChangeHistory}
                    executionChangeHistoryLoading={executionChangeHistoryLoading}
                    executionChangeHistoryError={executionChangeHistoryError}
                    queued={Boolean(queuedItem)}
                    queuedPosition={queuedItem?.position}
                    onRun={() => void handleRunCodex()}
                    onStop={() => void handleStopCodex()}
                    onClearOutput={() => clearTaskCodexOutput(task.id)}
                    onRefreshHistory={() => void loadExecutionChangeHistory()}
                    onOpenChangeDetail={(change) => void handleOpenExecutionChangeDetail(change)}
                  />
                </TabsContent>

                <TabsContent value="chain">
                  <TaskSessionChainPanel taskId={task.id} active={open && detailTab === "chain"} />
                </TabsContent>

                <TabsContent value="review">
                  <TaskReviewPanel
                    taskId={task.id}
                    status={status}
                    reviewerId={reviewerId}
                    reviewerName={reviewer?.name}
                    isReviewActive={isReviewRunning}
                    reviewLoading={reviewLoading}
                    reviewError={reviewError}
                    reviewNotice={reviewNotice}
                    latestReview={latestReview}
                    latestReviewLoading={latestReviewLoading}
                    hasReviewOutput={reviewOutput.length > 0}
                    assigneeId={assigneeId}
                    reviewFixSubmitting={reviewFixSubmitting}
                    onStartReview={() => void handleStartCodeReview()}
                    onRefreshReview={() => void loadLatestReview()}
                    onCopyReview={() => void handleCopyReviewReport()}
                    onOpenReviewFix={() => setReviewFixDialogOpen(true)}
                    executionChangeHistory={executionChangeHistory}
                    executionChangeHistoryLoading={executionChangeHistoryLoading}
                    executionChangeHistoryError={executionChangeHistoryError}
                    onRefreshHistory={() => void loadExecutionChangeHistory()}
                    onOpenChangeDetail={(change, options) =>
                      void handleOpenExecutionChangeDetail(change, options)
                    }
                  />
                </TabsContent>

                <TabsContent value="ai">
                  <TaskAiPanel
                    aiActionDisabled={aiActions.aiActionDisabled}
                    aiLoading={aiActions.aiLoading}
                    planLoading={aiActions.planLoading}
                    aiLogs={aiActions.aiLogs}
                    aiLogRef={aiLogRef}
                    aiResult={aiActions.aiResult}
                    taskAiSuggestion={task.ai_suggestion}
                    planError={aiActions.planError}
                    planNotice={aiActions.planNotice}
                    generatedPlan={aiActions.generatedPlan}
                    insertSubmitting={aiActions.insertSubmitting}
                    onSuggest={() => void aiActions.handleAiSuggest()}
                    onComplexity={() => void aiActions.handleAiComplexity()}
                    onSplitSubtasks={() => void aiActions.handleAiSplitSubtasks()}
                    onGeneratePlan={() => void aiActions.handleAiGeneratePlan()}
                    onGenerateComment={() => void aiActions.handleAiComment()}
                    onClearLogs={aiActions.clearAiLogs}
                    onInsertPlan={() => void aiActions.handleInsertPlan()}
                  />
                </TabsContent>

                <TabsContent value="collaboration">
                  <TaskCollaborationPanel
                    taskId={task.id}
                    attachments={attachments}
                    deletingAttachmentId={deletingAttachmentId}
                    attachmentLoading={attachmentLoading}
                    attachmentError={attachmentError}
                    isTauriRuntime={isTauriRuntime()}
                    onSelectAttachments={() => void handleSelectAttachments()}
                    onOpenAttachment={(path) => void handleOpenAttachment(path)}
                    onDeleteAttachment={(attachmentId) => void handleDeleteAttachment(attachmentId)}
                  />
                </TabsContent>
              </div>

              <aside className="hidden w-72 shrink-0 overflow-y-auto border-l border-border/70 bg-muted/10 lg:block">
                <TaskPropertiesSidebar
                  task={task}
                  projectTasks={projectTasks}
                  status={status}
                  priority={priority}
                  assigneeId={assigneeId}
                  reviewerId={reviewerId}
                  coordinatorId={coordinatorId}
                  dueDate={dueDate}
                  milestoneId={milestoneId}
                  timeStartedAt={task.time_started_at}
                  timeSpentSeconds={task.time_spent_seconds}
                  completedAt={task.completed_at}
                  tokenUsage={taskTokenUsage}
                  employees={employees}
                  reviewerCandidates={reviewerCandidates}
                  coordinatorCandidates={coordinatorCandidates}
                  isRunning={isRunning || isReviewRunning}
                  deletingTask={deletingTask}
                  onStatusChange={(value) => {
                    if (value === "blocked") {
                      setPendingBlockedReason(blockedReason);
                      setBlockReasonDialogOpen(true);
                      return;
                    }
                    setStatus(value);
                    void handleSave("status", value);
                  }}
                  onPriorityChange={(value) => {
                    setPriority(value);
                    void handleSave("priority", value);
                  }}
                  onAssigneeChange={(value) => {
                    setAssigneeId(value);
                    void handleSave("assignee_id", value);
                  }}
                  onReviewerChange={(value) => {
                    setReviewerId(value);
                    void handleSave("reviewer_id", value);
                  }}
                  onCoordinatorChange={(value) => {
                    setCoordinatorId(value);
                    void handleSave("coordinator_id", value);
                  }}
                  onDueDateChange={setDueDate}
                  onDueDateBlur={() => void handleSave("due_date", dueDate)}
                  onMilestoneChange={(value) => {
                    setMilestoneId(value);
                    void handleSave("milestone_id", value);
                  }}
                  onDeliveryError={setSaveError}
                  onDeleteRequest={() => setDeleteDialogOpen(true)}
                />
              </aside>
            </div>
          </Tabs>

          {primaryCta.kind !== "none" && (
            <div className="shrink-0 border-t border-border/70 bg-popover px-4">
              <TaskPrimaryActionBar
                primaryCta={primaryCta}
                automationLabel={
                  resolvedAutomationState.enabled
                    ? getTaskAutomationStatusLabel(resolvedAutomationState.status)
                    : null
                }
                loading={primaryActionLoading}
                onPrimaryAction={() => void handlePrimaryCtaAction()}
                secondaryActions={primarySecondaryActions}
              />
            </div>
          )}
        </DialogContent>
      </Dialog>
      {deleteDialogOpen && (
        <DeleteTaskDialog
          open={deleteDialogOpen}
          task={task}
          deleting={deletingTask}
          onOpenChange={setDeleteDialogOpen}
          onConfirm={handleDelete}
        />
      )}
      <TaskGitCommitDialog
        open={showCommitDialog}
        onOpenChange={(nextOpen) => {
          setShowCommitDialog(nextOpen);
          if (!nextOpen) {
            setInitialCommitOverview(null);
            setInitialCommitError(null);
          }
        }}
        task={task}
        initialOverview={initialCommitOverview}
        initialError={initialCommitError}
        onCommitted={async () => {
          try {
            const nextState = await getTaskCommitActionState(task.id);
            setCommitActionState(nextState);
          } catch {
            setCommitActionState(null);
          }
        }}
      />
      <TaskExecutionChangeDetailDialog
        open={executionChangeDetailOpen}
        loading={executionChangeDetailLoading}
        error={executionChangeDetailError}
        revealLine={executionChangeRevealLine}
        findingMessage={executionChangeFindingMessage}
        detail={
          executionChangeDetail ??
          (selectedExecutionChange
            ? {
                change: selectedExecutionChange,
                working_dir: null,
                absolute_path: null,
                previous_absolute_path: null,
                before_status: "missing",
                before_text: null,
                before_truncated: false,
                after_status: "missing",
                after_text: null,
                after_truncated: false,
                diff_text: null,
                diff_truncated: false,
                snapshot_status: "unavailable",
                snapshot_message: null,
              }
            : null)
        }
        onOpenChange={handleExecutionChangeDetailOpenChange}
      />
      {aiActions.insertDialogOpen && (
        <InsertPlanConfirmDialog
          open={aiActions.insertDialogOpen}
          taskTitle={title.trim() || task.title}
          inserting={aiActions.insertSubmitting}
          onOpenChange={aiActions.setInsertDialogOpen}
          onAppend={() => void aiActions.applyGeneratedPlan("append")}
          onReplace={() => void aiActions.applyGeneratedPlan("replace")}
        />
      )}
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
          (!assigneeId ? "请先指定执行员工，再执行计划。" : null)
        }
        canExecute={Boolean(assigneeId)}
        canStartPipeline={task.status !== "archived"}
        actionsLocked={coordinatorPlanActionsLocked}
        taskTitle={title.trim() || task.title}
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
        onResumeAutoPipeline={() => void handleResumeAutoPipeline()}
        onManualRunPipelineStep={(step) => void handleManualRunPipelineStep(step)}
        onManualStopPipelineStep={(step) => void handleManualStopPipelineStep(step)}
        onRefreshPipeline={() => void refreshPipelineSteps()}
        onPipelineEmployeeChange={(stepId, employeeId) =>
          void handlePipelineEmployeeChange(stepId, employeeId)
        }
        onRegenerate={() => void generateCoordinatorPlan()}
        onSave={() => void handleSaveCoordinatorPlan()}
        onToggleTerminal={() => setCoordinatorPlanTerminalVisible((visible) => !visible)}
        onClearTerminal={() => setCoordinatorPlanLogs([])}
      />
      <SessionLogDialog
        open={pipelineStepLogTarget !== null}
        session={
          pipelineStepLogTarget
            ? {
                sessionRecordId: pipelineStepLogTarget.sessionRecordId,
                sessionId: pipelineStepLogTarget.sessionRecordId,
                displayName: `步骤：${pipelineStepLogTarget.stepTitle}`,
                employeeId: null,
                employeeName: pipelineStepLogTarget.employeeName,
                taskId: task.id,
                taskTitle: title.trim() || task.title,
                sessionKind: "execution",
              }
            : null
        }
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            setPipelineStepLogTarget(null);
          }
        }}
      />
      {reviewFixDialogOpen && assignee && (
        <ReviewFixConfirmDialog
          open={reviewFixDialogOpen}
          sourceTaskTitle={task.title}
          assigneeName={assignee.name}
          creating={reviewFixSubmitting}
          onOpenChange={setReviewFixDialogOpen}
          onConfirm={handleConfirmReviewFix}
        />
      )}
      <Dialog
        open={blockReasonDialogOpen}
        onOpenChange={(open) => {
          if (blockReasonSubmitting) {
            return;
          }
          setBlockReasonDialogOpen(open);
          if (!open) {
            setStatus(task.status);
            setPendingBlockedReason(blockedReason);
          }
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("detail.blockReason.title")}</DialogTitle>
            <DialogDescription>{t("detail.blockReason.description")}</DialogDescription>
          </DialogHeader>
          <Textarea
            value={pendingBlockedReason}
            onChange={(e) => setPendingBlockedReason(e.target.value)}
            placeholder={t("detail.blockReason.placeholder")}
            className="min-h-[96px] resize-y"
            disabled={blockReasonSubmitting}
          />
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={blockReasonSubmitting}
              onClick={() => {
                setBlockReasonDialogOpen(false);
                setStatus(task.status);
                setPendingBlockedReason(blockedReason);
              }}
              className="rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:opacity-50"
            >
              {t("common:cancel")}
            </button>
            <button
              type="button"
              disabled={blockReasonSubmitting || !pendingBlockedReason.trim()}
              onClick={() => void handleConfirmBlockedStatus()}
              className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {blockReasonSubmitting
                ? t("detail.blockReason.saving")
                : t("detail.blockReason.confirm")}
            </button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
