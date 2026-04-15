import { useState, useEffect, useRef } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import type { Task, TaskExecutionChangeHistoryItem, TaskLatestReview, TaskStatus } from "@/lib/types";
import {
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import {
  IMAGE_FILE_FILTERS,
  dedupePaths,
  isTauriRuntime,
  normalizeDialogSelection,
} from "@/lib/taskAttachments";
import { startCodex } from "@/lib/codex";
import { DeleteTaskDialog } from "./DeleteTaskDialog";
import { InsertPlanConfirmDialog } from "./InsertPlanConfirmDialog";
import { ReviewFixConfirmDialog } from "./ReviewFixConfirmDialog";
import { useTaskExecutionActions } from "./hooks/useTaskExecutionActions";
import { useTaskReviewActions } from "./hooks/useTaskReviewActions";
import { useTaskAiActions } from "./hooks/useTaskAiActions";
import { TaskOverviewPanel } from "./detail/TaskOverviewPanel";
import { TaskExecutionPanel } from "./detail/TaskExecutionPanel";
import { TaskReviewPanel } from "./detail/TaskReviewPanel";
import { TaskAiPanel } from "./detail/TaskAiPanel";
import { TaskCollaborationPanel } from "./detail/TaskCollaborationPanel";

const EMPTY_ATTACHMENTS: never[] = [];

interface TaskDetailDialogProps {
  task: Task;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function TaskDetailDialog({
  task,
  open,
  onOpenChange,
}: TaskDetailDialogProps) {
  const {
    updateTask,
    deleteTask,
    addComment,
    updateTaskStatus,
    fetchAttachments,
    fetchSubtasks,
    addTaskAttachments,
    deleteTaskAttachment,
    addSubtasks,
    createTask,
  } = useTaskStore();
  const {
    employees,
    fetchEmployees,
    codexProcesses,
    updateEmployeeStatus,
    setCodexRunning,
    clearCodexOutput,
    clearTaskCodexOutput,
    addCodexOutput,
    refreshCodexRuntimeStatus,
  } = useEmployeeStore();
  const projects = useProjectStore((s) => s.projects);
  const attachmentMap = useTaskStore((state) => state.attachments);
  const attachments = attachmentMap[task.id] ?? EMPTY_ATTACHMENTS;
  const projectRepoPath = projects.find((p) => p.id === task.project_id)?.repo_path;
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
  const [executionChangeHistory, setExecutionChangeHistory] = useState<TaskExecutionChangeHistoryItem[]>([]);
  const [executionChangeHistoryLoading, setExecutionChangeHistoryLoading] = useState(false);
  const [executionChangeHistoryError, setExecutionChangeHistoryError] = useState<string | null>(null);
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
    clearEmployeeOutputOnRun: true,
    onStarted: () => {
      setStatus("in_progress");
    },
  });
  const reviewActions = useTaskReviewActions({
    task,
    reviewerId,
    status,
    onStarted: () => {
      void loadLatestReview();
    },
    onError: (message) => {
      setReviewError(message);
    },
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
  const isRunning = executionActions.isRunning;
  const isReviewRunning = reviewActions.isRunning;
  const output = executionActions.output;
  const reviewOutput = reviewActions.output;
  const codexLoading = executionActions.loading !== null;
  const reviewLoading = reviewActions.loading;

  useEffect(() => {
    if (open) {
      fetchEmployees();
      void fetchAttachments(task.id);
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
      void loadExecutionChangeHistory();
      void loadLatestReview();
    }
  }, [fetchAttachments, fetchEmployees, open, task]);

  useEffect(() => {
    if (!open) {
      setDeleteDialogOpen(false);
    }
  }, [open]);

  useEffect(() => {
    if (open) {
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
    }
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
      filters: IMAGE_FILE_FILTERS,
      title: "选择任务图片",
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
    setLatestReviewLoading(true);
    try {
      const review = await getTaskLatestReview(task.id);
      setLatestReview(review);
    } catch (error) {
      console.error("Failed to load latest task review:", error);
      setReviewError(error instanceof Error ? error.message : String(error));
    } finally {
      setLatestReviewLoading(false);
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

  const handleRunCodex = async () => {
    await executionActions.runTask();
  };

  const handleStopCodex = async () => {
    await executionActions.stopTask();
  };

  const handleStartCodeReview = async () => {
    if (!reviewerId) {
      setReviewError("请先指定审查员，再执行代码审核。");
      return;
    }

    setReviewError(null);
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
    if (codexProcesses[assigneeId]?.running) {
      setReviewError("当前开发负责人仍有运行中的 Codex 会话，请先停止后再创建修复任务。");
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
        assignee_id: assigneeId,
      });

      await updateEmployeeStatus(assigneeId, "busy");
      await updateTaskStatus(createdTask.id, "in_progress");
      setCodexRunning(assigneeId, true, createdTask.id);
      clearCodexOutput(assigneeId);
      clearTaskCodexOutput(createdTask.id, "execution");
      const executionInput = buildTaskExecutionInput({
        title: createdTask.title,
        description: createdTask.description,
      });
      await startCodex(assigneeId, executionInput.prompt, {
        model: assignee?.model,
        reasoningEffort: assignee?.reasoning_effort,
        systemPrompt: assignee?.system_prompt,
        workingDir: projectRepoPath,
        taskId: createdTask.id,
        imagePaths: executionInput.imagePaths,
      });

      setReviewFixDialogOpen(false);
      setReviewNotice(`已创建修复任务“${createdTask.title}”，并开始运行。`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setReviewError(message);
      addCodexOutput(assigneeId, `[ERROR] ${message}`);
      setCodexRunning(assigneeId, false, null);
      await refreshCodexRuntimeStatus(assigneeId);
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

          <Tabs defaultValue="overview" className="gap-4">
            <div className="overflow-x-auto">
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
                isRunning={isRunning}
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
                assigneeId={assigneeId}
                isRunning={isRunning}
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
              />
            </TabsContent>

            <TabsContent value="review">
              <TaskReviewPanel
                taskId={task.id}
                status={status}
                reviewerId={reviewerId}
                reviewerName={reviewer?.name}
                isReviewRunning={isReviewRunning}
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
