import { useState, useEffect, useRef } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Bot, Check, Copy } from "lucide-react";

import type {
  CodexSessionFileChange,
  CodexSessionFileChangeDetail,
  Task,
  TaskExecutionChangeHistoryItem,
  TaskLatestReview,
  TaskStatus,
} from "@/lib/types";
import {
  prepareTaskGitExecution,
  getCodexSessionFileChangeDetail,
  getTaskExecutionChangeHistory,
  getTaskLatestReview,
  openTaskAttachment,
} from "@/lib/backend";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import {
  dedupePaths,
  isTauriRuntime,
  normalizeDialogSelection,
} from "@/lib/taskAttachments";
import { startCodex } from "@/lib/codex";
import { startClaude } from "@/lib/claude";
import { startOpenCode } from "@/lib/opencode";
import { getProjectWorkingDir } from "@/lib/projects";
import type { TaskAutomationDisplayState } from "@/lib/utils";
import {
  formatDate,
  getTaskActionRuntimeState,
  getTaskAutomationDisplayState,
  getTaskAutomationStatusLabel,
} from "@/lib/utils";
import { DeleteTaskDialog } from "./DeleteTaskDialog";
import { InsertPlanConfirmDialog } from "./InsertPlanConfirmDialog";
import { ReviewFixConfirmDialog } from "./ReviewFixConfirmDialog";
import { useTaskExecutionActions } from "./hooks/useTaskExecutionActions";
import { useTaskReviewActions } from "./hooks/useTaskReviewActions";
import { useTaskAiActions } from "./hooks/useTaskAiActions";
import { TaskOverviewPanel } from "./detail/TaskOverviewPanel";
import { TaskExecutionPanel } from "./detail/TaskExecutionPanel";
import { TaskExecutionChangeDetailDialog } from "./detail/TaskExecutionChangeDetailDialog";
import { TaskReviewPanel } from "./detail/TaskReviewPanel";
import { TaskAiPanel } from "./detail/TaskAiPanel";
import { TaskCollaborationPanel } from "./detail/TaskCollaborationPanel";

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
  const {
    updateTask,
    deleteTask,
    addComment,
    fetchAttachments,
    fetchSubtasks,
    fetchTaskAutomationState,
    addTaskAttachments,
    deleteTaskAttachment,
    addSubtasks,
    createTask,
  } = useTaskStore();
  const persistedAutomationState = useTaskStore((state) => state.automationStates[task.id]);
  const {
    employees,
    fetchEmployees,
    updateEmployeeStatus,
    clearTaskCodexOutput,
    addCodexOutput,
    refreshEmployeeRuntimeStatus,
  } = useEmployeeStore();
  const projects = useProjectStore((s) => s.projects);
  const attachmentMap = useTaskStore((state) => state.attachments);
  const attachments = attachmentMap[task.id] ?? EMPTY_ATTACHMENTS;
  const project = projects.find((p) => p.id === task.project_id);
  const projectRepoPath = getProjectWorkingDir(project);
  const [title, setTitle] = useState(task.title);
  const [description, setDescription] = useState(task.description ?? "");
  const [priority, setPriority] = useState(task.priority);
  const [status, setStatus] = useState(task.status);
  const [assigneeId, setAssigneeId] = useState(task.assignee_id ?? "");
  const [reviewerId, setReviewerId] = useState(task.reviewer_id ?? "");
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
  const [executionChangeHistory, setExecutionChangeHistory] = useState<TaskExecutionChangeHistoryItem[]>([]);
  const [executionChangeHistoryLoading, setExecutionChangeHistoryLoading] = useState(false);
  const [executionChangeHistoryError, setExecutionChangeHistoryError] = useState<string | null>(null);
  const [executionChangeDetailOpen, setExecutionChangeDetailOpen] = useState(false);
  const [executionChangeDetailLoading, setExecutionChangeDetailLoading] = useState(false);
  const [executionChangeDetailError, setExecutionChangeDetailError] = useState<string | null>(null);
  const [selectedExecutionChange, setSelectedExecutionChange] = useState<CodexSessionFileChange | null>(null);
  const [executionChangeDetail, setExecutionChangeDetail] = useState<CodexSessionFileChangeDetail | null>(null);
  const latestReviewRequestIdRef = useRef(0);
  const executionChangeDetailRequestIdRef = useRef(0);
  const taskIdCopyResetTimerRef = useRef<number | null>(null);
  const terminalRef = useRef<HTMLDivElement>(null);
  const aiLogRef = useRef<HTMLDivElement>(null);
  const assignee = assigneeId ? employees.find((employee) => employee.id === assigneeId) : undefined;
  const reviewer = reviewerId ? employees.find((employee) => employee.id === reviewerId) : undefined;
  const reviewerCandidates = employees.filter((employee) => employee.role === "reviewer");
  const executionActions = useTaskExecutionActions({
    task,
    assigneeId,
    assignee,
    projectRepoPath,
    projectType: project?.project_type,
    prepareExecutionInput: async () => {
      await Promise.all([fetchSubtasks(task.id), fetchAttachments(task.id)]);
      const executionInput = buildTaskExecutionInput({
        title,
        description,
        subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
        attachments: useTaskStore.getState().attachments[task.id] ?? [],
      });

      return {
        prompt: executionInput.prompt,
        imagePaths: executionInput.imagePaths,
      };
    },
    clearTaskOutputOnRun: true,
    onStarted: () => {
      setStatus("in_progress");
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
  const resolvedAutomationState = automationState ?? getTaskAutomationDisplayState(task, persistedAutomationState ?? null);
  const runtimeState = getTaskActionRuntimeState({
    automationState: resolvedAutomationState,
    isExecutionRunning: executionActions.isRunning,
    isReviewRunning: reviewActions.isRunning,
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
  const output = executionActions.output;
  const reviewOutput = reviewActions.output;
  const codexLoading = executionActions.loading !== null;
  const reviewLoading = reviewActions.loading;

  useEffect(() => {
    if (open) {
      fetchEmployees();
      void fetchAttachments(task.id);
      if (task.automation_mode === "review_fix_loop_v1") {
        void fetchTaskAutomationState(task.id);
      }
      setTitle(task.title);
      setDescription(task.description ?? "");
      setPriority(task.priority);
      setStatus(task.status);
      setAssigneeId(task.assignee_id ?? "");
      setReviewerId(task.reviewer_id ?? "");
      setAttachmentError(null);
      setSaveError(null);
      setReviewError(null);
      setReviewNotice(null);
      void loadLatestReview();
      void loadExecutionChangeHistory();
    }
  }, [fetchAttachments, fetchEmployees, open, task]);

  useEffect(() => {
    if (!open) {
      setDeleteDialogOpen(false);
    }
  }, [open]);

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

    if (output[output.length - 1]?.startsWith("[EXIT]")) {
      void loadExecutionChangeHistory();
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
        await useTaskStore.getState().updateTaskStatus(task.id, value as TaskStatus);
      } else if (field === "assignee_id") {
        await updateTask(task.id, { assignee_id: value || null });
      } else if (field === "reviewer_id") {
        await updateTask(task.id, { reviewer_id: value || null });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSaveError(message);
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
      title: "选择任务附件",
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

  const loadExecutionChangeHistory = async () => {
    setExecutionChangeHistoryLoading(true);
    setExecutionChangeHistoryError(null);
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

  const handleOpenExecutionChangeDetail = async (change: CodexSessionFileChange) => {
    const requestId = executionChangeDetailRequestIdRef.current + 1;
    executionChangeDetailRequestIdRef.current = requestId;
    setSelectedExecutionChange(change);
    setExecutionChangeDetail(null);
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
    }
  };

  const handleRunCodex = async () => {
    await executionActions.runTask();
  };

  const handleStopCodex = async () => {
    await executionActions.stopTask();
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

      await updateEmployeeStatus(assigneeId, "busy");
      await useTaskStore.getState().updateTaskStatus(createdTask.id, "in_progress");
      clearTaskCodexOutput(createdTask.id, "execution");
      const executionInput = buildTaskExecutionInput({
        title: createdTask.title,
        description: createdTask.description,
      });
      let workingDir = projectRepoPath ?? undefined;
      let taskGitContextId: string | undefined;

      if (createdTask.use_worktree) {
        const prepared = await prepareTaskGitExecution(createdTask.id);
        workingDir = prepared.working_dir;
        taskGitContextId = prepared.task_git_context_id;
      }

      if (!workingDir) {
        throw new Error("当前项目缺少可用工作目录，无法启动修复任务。");
      }

      const startOptions = {
        model: assignee?.model,
        reasoningEffort: assignee?.reasoning_effort,
        systemPrompt: assignee?.system_prompt,
        workingDir,
        taskId: createdTask.id,
        taskGitContextId,
        imagePaths: executionInput.imagePaths,
      };

      if (assignee?.ai_provider === "claude") {
        await startClaude(assigneeId, executionInput.prompt, startOptions);
      } else if (assignee?.ai_provider === "opencode") {
        await startOpenCode({
          employeeId: assigneeId,
          taskDescription: executionInput.prompt,
          model: assignee.model,
          workingDir,
          taskId: createdTask.id,
          taskGitContextId,
          imagePaths: executionInput.imagePaths,
        });
      } else {
        await startCodex(assigneeId, executionInput.prompt, startOptions);
      }
      await refreshEmployeeRuntimeStatus(assigneeId);

      setReviewFixDialogOpen(false);
      setReviewNotice(`已创建修复任务“${createdTask.title}”，并开始运行。`);
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

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="w-[min(96vw,80rem)] max-w-[min(96vw,80rem)] sm:max-w-[min(96vw,80rem)] max-h-[85vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="sr-only">任务详情</DialogTitle>
            <DialogDescription className="sr-only">
              查看和编辑任务详情
            </DialogDescription>
          </DialogHeader>

          <div className="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-border/70 bg-background/80 px-4 py-3">
            <span className="text-xs font-medium text-muted-foreground">任务 ID</span>
            <button
              type="button"
              onClick={() => void handleCopyTaskId()}
              className="inline-flex items-center gap-1.5 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
              title={taskIdCopied ? "已复制任务 ID" : "点击复制任务 ID"}
              aria-label={taskIdCopied ? "已复制任务 ID" : "点击复制任务 ID"}
            >
              <Badge
                variant="outline"
                className="h-6 cursor-pointer rounded-md px-2.5 font-mono text-[11px] transition-colors hover:border-primary/40 hover:bg-primary/5"
              >
                {task.id}
                {taskIdCopied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
              </Badge>
            </button>
          </div>

          <Tabs defaultValue="overview" className="gap-4">
            <section className="space-y-3 rounded-lg border border-border/70 bg-muted/20 p-4">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <Bot className="h-4 w-4 text-primary" />
                  <p className="text-sm font-medium">自动质控</p>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    这里展示原任务内的自动审核与自动修复闭环状态；开关入口在任务卡片右键菜单。
                  </p>
                </div>
                <div
                  className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium ${
                    resolvedAutomationState.enabled
                      ? "bg-emerald-500/10 text-emerald-700"
                      : "bg-muted text-muted-foreground"
                  }`}
                >
                  {resolvedAutomationState.enabled ? "已开启" : "未开启"}
                </div>
              </div>

              <div className="grid gap-2 text-xs text-muted-foreground md:grid-cols-4">
                <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                  <span className="font-medium text-foreground">闭环阶段：</span>
                  {getTaskAutomationStatusLabel(resolvedAutomationState.status)}
                </div>
                <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                  <span className="font-medium text-foreground">自动修复轮次：</span>
                  {resolvedAutomationState.roundCount ?? 0}
                </div>
                <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                  <span className="font-medium text-foreground">最近更新时间：</span>
                  {resolvedAutomationState.updatedAt ? formatDate(resolvedAutomationState.updatedAt) : "暂无"}
                </div>
                <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                  <span className="font-medium text-foreground">状态来源：</span>
                  {resolvedAutomationState.source === "automation_state" ? "自动化状态" : "任务配置"}
                </div>
              </div>

              <div className="rounded-md border border-dashed border-border bg-background/70 px-3 py-2 text-xs text-muted-foreground">
                自动质控不会替代现有“审核结果 → 修复”手动路径。手动修复仍然通过创建新任务推进；自动质控接线后则在原任务内完成审核与修复闭环。
              </div>

              {resolvedAutomationState.note && (
                <div className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-800">
                  <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  <span>{resolvedAutomationState.note}</span>
                </div>
              )}
            </section>

            <div className="overflow-x-auto overflow-y-hidden pb-[5px]">
              <TabsList variant="line" className="w-full min-w-max justify-start">
                <TabsTrigger value="overview">概览</TabsTrigger>
                <TabsTrigger value="execution">执行</TabsTrigger>
                <TabsTrigger value="review">审核</TabsTrigger>
                <TabsTrigger value="ai">AI 助手</TabsTrigger>
                <TabsTrigger value="collaboration">协作</TabsTrigger>
              </TabsList>
            </div>

            <TabsContent value="overview">
              <TaskOverviewPanel
                title={title}
                description={description}
                status={status}
                priority={priority}
                assigneeId={assigneeId}
                reviewerId={reviewerId}
                employees={employees}
                reviewerCandidates={reviewerCandidates}
                saveError={saveError}
                isRunning={isRunning || isReviewRunning}
                deletingTask={deletingTask}
                onTitleChange={setTitle}
                onTitleBlur={() => void handleSave("title", title)}
                onDescriptionChange={setDescription}
                onDescriptionBlur={() => void handleSave("description", description)}
                onStatusChange={(value) => {
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
                onDeleteRequest={() => setDeleteDialogOpen(true)}
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
                executionChangeHistory={executionChangeHistory}
                executionChangeHistoryLoading={executionChangeHistoryLoading}
                executionChangeHistoryError={executionChangeHistoryError}
                onRun={() => void handleRunCodex()}
                onStop={() => void handleStopCodex()}
                onClearOutput={() => clearTaskCodexOutput(task.id)}
                onRefreshHistory={() => void loadExecutionChangeHistory()}
                onOpenChangeDetail={(change) => void handleOpenExecutionChangeDetail(change)}
              />
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
                onOpenChangeDetail={(change) => void handleOpenExecutionChangeDetail(change)}
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
          </Tabs>
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
      <TaskExecutionChangeDetailDialog
        open={executionChangeDetailOpen}
        loading={executionChangeDetailLoading}
        error={executionChangeDetailError}
        detail={executionChangeDetail ?? (selectedExecutionChange ? {
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
        } : null)}
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
    </>
  );
}
