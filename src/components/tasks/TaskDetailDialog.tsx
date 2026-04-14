import { useState, useEffect, useRef } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import type { Task, TaskExecutionChangeHistoryItem, TaskLatestReview, TaskStatus } from "@/lib/types";
import { TASK_STATUSES, PRIORITIES } from "@/lib/types";
import {
  getCodexSettings,
  getTaskExecutionChangeHistory,
  getTaskLatestReview,
  healthCheck,
  openTaskAttachment,
  startTaskCodeReview,
} from "@/lib/backend";
import { useTaskStore } from "@/stores/taskStore";
import { buildTaskLogKey, useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import {
  IMAGE_FILE_FILTERS,
  dedupePaths,
  isTauriRuntime,
  normalizeDialogSelection,
} from "@/lib/taskAttachments";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Trash2, Sparkles, Loader2, Play, Square, Eraser, ImagePlus, Copy, Wrench } from "lucide-react";
import {
  aiSuggestAssignee,
  aiAnalyzeComplexity,
  aiGenerateComment,
  aiGeneratePlan,
  aiSplitSubtasks,
  startCodex,
  stopCodex,
} from "@/lib/codex";
import { SubtaskList } from "./SubtaskList";
import { CommentList } from "./CommentList";
import { DeleteTaskDialog } from "./DeleteTaskDialog";
import { InsertPlanConfirmDialog } from "./InsertPlanConfirmDialog";
import { ReviewFixConfirmDialog } from "./ReviewFixConfirmDialog";
import { TaskAttachmentGrid } from "./TaskAttachmentGrid";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { CodexTerminal } from "@/components/codex/CodexTerminal";

const UNASSIGNED_VALUE = "__unassigned__";
const EMPTY_ATTACHMENTS: never[] = [];
type PlanInsertMode = "append" | "replace";

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
    taskLogs,
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
  const [aiLoading, setAiLoading] = useState<string | null>(null);
  const [aiResult, setAiResult] = useState<string | null>(null);
  const [planLoading, setPlanLoading] = useState(false);
  const [generatedPlan, setGeneratedPlan] = useState<string | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [planNotice, setPlanNotice] = useState<string | null>(null);
  const [insertDialogOpen, setInsertDialogOpen] = useState(false);
  const [insertSubmitting, setInsertSubmitting] = useState(false);
  const [codexLoading, setCodexLoading] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingTask, setDeletingTask] = useState(false);
  const [attachmentLoading, setAttachmentLoading] = useState(false);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [deletingAttachmentId, setDeletingAttachmentId] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewNotice, setReviewNotice] = useState<string | null>(null);
  const [reviewLoading, setReviewLoading] = useState(false);
  const [reviewFixDialogOpen, setReviewFixDialogOpen] = useState(false);
  const [reviewFixSubmitting, setReviewFixSubmitting] = useState(false);
  const [latestReview, setLatestReview] = useState<TaskLatestReview | null>(null);
  const [latestReviewLoading, setLatestReviewLoading] = useState(false);
  const [executionChangeHistory, setExecutionChangeHistory] = useState<TaskExecutionChangeHistoryItem[]>([]);
  const [executionChangeHistoryLoading, setExecutionChangeHistoryLoading] = useState(false);
  const [executionChangeHistoryError, setExecutionChangeHistoryError] = useState<string | null>(null);
  const terminalRef = useRef<HTMLDivElement>(null);
  const aiLogRef = useRef<HTMLDivElement>(null);
  const [aiLogs, setAiLogs] = useState<string[]>([]);
  const assignee = assigneeId ? employees.find((employee) => employee.id === assigneeId) : undefined;
  const reviewer = reviewerId ? employees.find((employee) => employee.id === reviewerId) : undefined;
  const reviewerCandidates = employees.filter((employee) => employee.role === "reviewer");

  const codexProcess = assigneeId ? codexProcesses[assigneeId] : undefined;
  const isRunning = (codexProcess?.running ?? false) && codexProcess?.activeTaskId === task.id;
  const reviewerProcess = reviewerId ? codexProcesses[reviewerId] : undefined;
  const isReviewRunning =
    (reviewerProcess?.running ?? false) && reviewerProcess?.activeTaskId === task.id;
  const output = taskLogs[buildTaskLogKey(task.id, "execution")] ?? [];
  const reviewOutput = taskLogs[buildTaskLogKey(task.id, "review")] ?? [];

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
      setAiResult(null);
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
      setInsertDialogOpen(false);
    }
  }, [open]);

  useEffect(() => {
    if (open) {
      setPlanLoading(false);
      setGeneratedPlan(null);
      setPlanError(null);
      setPlanNotice(null);
      setInsertDialogOpen(false);
      setInsertSubmitting(false);
      setAttachmentLoading(false);
      setDeletingAttachmentId(null);
      setAiLogs([]);
      setReviewLoading(false);
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
  }, [aiLogs.length]);

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

  const formatLogTime = () =>
    new Date().toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });

  const appendAiLog = (message: string) => {
    setAiLogs((current) => [...current.slice(-199), `${formatLogTime()} ${message}`]);
  };

  const resetAiLogs = (operation: string) => {
    setAiLogs([`${formatLogTime()} [${operation}] 开始执行`]);
  };

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

  const loadCurrentImagePaths = async () => {
    await fetchAttachments(task.id);
    return (useTaskStore.getState().attachments[task.id] ?? [])
      .map((attachment) => attachment.stored_path.trim())
      .filter((path) => path.length > 0);
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

  const logOneShotAiContext = async (operation: string, imagePaths: string[]) => {
    appendAiLog(`[${operation}] 已载入任务图片 ${imagePaths.length} 张`);

    const [settingsResult, healthResult] = await Promise.allSettled([
      getCodexSettings(),
      healthCheck(),
    ]);

    if (settingsResult.status === "fulfilled") {
      appendAiLog(
        `[${operation}] 一次性 AI 配置：模型 ${settingsResult.value.one_shot_model} / 推理 ${settingsResult.value.one_shot_reasoning_effort}`,
      );
    } else {
      appendAiLog(`[WARN] [${operation}] 读取一次性 AI 配置失败：${String(settingsResult.reason)}`);
    }

    if (healthResult.status === "fulfilled") {
      const provider =
        healthResult.value.one_shot_effective_provider === "sdk" ? "SDK" : "exec（自动回退）";
      appendAiLog(`[${operation}] 当前执行通道：${provider}`);
    } else {
      appendAiLog(`[WARN] [${operation}] 读取运行时状态失败：${String(healthResult.reason)}`);
    }
  };

  const handleRunCodex = async () => {
    if (!assigneeId) return;
    setCodexLoading(true);
    try {
      await updateEmployeeStatus(assigneeId, "busy");
      await updateTaskStatus(task.id, "in_progress");
      setStatus("in_progress");
      setCodexRunning(assigneeId, true, task.id);
      clearCodexOutput(assigneeId);
      clearTaskCodexOutput(task.id);
      await Promise.all([fetchSubtasks(task.id), fetchAttachments(task.id)]);
      const executionInput = buildTaskExecutionInput({
        title,
        description,
        subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
        attachments: useTaskStore.getState().attachments[task.id] ?? [],
      });
      await startCodex(assigneeId, executionInput.prompt, {
        model: assignee?.model,
        reasoningEffort: assignee?.reasoning_effort,
        systemPrompt: assignee?.system_prompt,
        workingDir: projectRepoPath ?? undefined,
        taskId: task.id,
        imagePaths: executionInput.imagePaths,
      });
    } catch (err) {
      console.error("Failed to start codex:", err);
      setCodexRunning(assigneeId, false, null);
      addCodexOutput(assigneeId, `[ERROR] ${String(err)}`, task.id);
      await refreshCodexRuntimeStatus(assigneeId);
    } finally {
      setCodexLoading(false);
    }
  };

  const handleStopCodex = async () => {
    if (!assigneeId) return;
    setCodexLoading(true);
    try {
      await stopCodex(assigneeId);
      setCodexRunning(assigneeId, false, null);
      await updateEmployeeStatus(assigneeId, "offline");
      await refreshCodexRuntimeStatus(assigneeId);
    } catch (err) {
      console.error("Failed to stop codex:", err);
      addCodexOutput(assigneeId, `[ERROR] ${String(err)}`, task.id);
      await refreshCodexRuntimeStatus(assigneeId);
    } finally {
      setCodexLoading(false);
    }
  };

  const handleStartCodeReview = async () => {
    if (!reviewerId) {
      setReviewError("请先指定审查员，再执行代码审核。");
      return;
    }

    setReviewLoading(true);
    setReviewError(null);
    try {
      await updateEmployeeStatus(reviewerId, "busy");
      setCodexRunning(reviewerId, true, task.id);
      clearTaskCodexOutput(task.id, "review");
      await startTaskCodeReview(task.id);
      await loadLatestReview();
    } catch (error) {
      console.error("Failed to start task code review:", error);
      setCodexRunning(reviewerId, false, null);
      addCodexOutput(reviewerId, `[ERROR] ${String(error)}`, task.id, "review");
      setReviewError(error instanceof Error ? error.message : String(error));
      await refreshCodexRuntimeStatus(reviewerId);
    } finally {
      setReviewLoading(false);
    }
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

  const handleAiSuggest = async () => {
    resetAiLogs("AI建议指派");
    setAiLoading("assignee");
    setAiResult(null);
    try {
      appendAiLog("[AI建议指派] 正在准备任务图片与执行配置...");
      const imagePaths = await loadCurrentImagePaths();
      await logOneShotAiContext("AI建议指派", imagePaths);
      const employeeList = employees
        .map((e) => `${e.id}: ${e.name} (${e.role}, ${e.specialization ?? "general"})`)
        .join("; ");
      const desc = task.description ?? task.title;
      appendAiLog("[AI建议指派] 已提交给 AI，等待响应...");
      const result = await aiSuggestAssignee(desc, employeeList, imagePaths);
      setAiResult(result);
      await updateTask(task.id, { ai_suggestion: result });
      appendAiLog("[AI建议指派] 执行完成");
    } catch (e) {
      appendAiLog(`[ERROR] [AI建议指派] ${String(e)}`);
      setAiResult(`AI建议失败: ${e}`);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiComplexity = async () => {
    resetAiLogs("复杂度分析");
    setAiLoading("complexity");
    setAiResult(null);
    try {
      appendAiLog("[复杂度分析] 正在准备任务图片与执行配置...");
      const imagePaths = await loadCurrentImagePaths();
      await logOneShotAiContext("复杂度分析", imagePaths);
      const desc = task.description ?? task.title;
      appendAiLog("[复杂度分析] 已提交给 AI，等待响应...");
      const result = await aiAnalyzeComplexity(desc, imagePaths);
      setAiResult(result);
      const match = result.match(/(\d+)/);
      if (match) {
        await updateTask(task.id, { complexity: parseInt(match[1], 10) });
      }
      appendAiLog("[复杂度分析] 执行完成");
    } catch (e) {
      appendAiLog(`[ERROR] [复杂度分析] ${String(e)}`);
      setAiResult(`复杂度分析失败: ${e}`);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiComment = async () => {
    resetAiLogs("AI生成评论");
    setAiLoading("comment");
    try {
      appendAiLog("[AI生成评论] 正在准备任务图片与执行配置...");
      const imagePaths = await loadCurrentImagePaths();
      await logOneShotAiContext("AI生成评论", imagePaths);
      appendAiLog("[AI生成评论] 已提交给 AI，等待响应...");
      const result = await aiGenerateComment(
        task.title,
        task.description ?? "",
        `Status: ${task.status}, Priority: ${task.priority}`,
        imagePaths,
      );
      await addComment(task.id, result, undefined, true);
      appendAiLog("[AI生成评论] 执行完成");
    } catch (e) {
      appendAiLog(`[ERROR] [AI生成评论] ${String(e)}`);
      console.error("AI comment failed:", e);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiSplitSubtasks = async () => {
    const taskTitle = title.trim();
    const taskDescription = description.trim();

    if (!taskTitle && !taskDescription) {
      setAiResult("请先填写任务标题或描述，再执行 AI 拆分。");
      return;
    }

    resetAiLogs("AI拆分子任务");
    setAiLoading("subtasks");
    setAiResult(null);
    try {
      appendAiLog("[AI拆分子任务] 正在准备任务图片与执行配置...");
      const imagePaths = await loadCurrentImagePaths();
      await logOneShotAiContext("AI拆分子任务", imagePaths);
      appendAiLog("[AI拆分子任务] 已提交给 AI，等待响应...");
      const generatedSubtasks = await aiSplitSubtasks(taskTitle, taskDescription, imagePaths);
      const { inserted, skipped } = await addSubtasks(task.id, generatedSubtasks);

      if (inserted === 0) {
        appendAiLog("[AI拆分子任务] 响应完成，但没有可新增的子任务");
        setAiResult(skipped > 0 ? "AI 已完成拆分，但结果与现有子任务重复，未新增内容。" : "AI 未生成可写入的子任务。");
        return;
      }

      setAiResult(`AI 已写入 ${inserted} 个子任务${skipped > 0 ? `，跳过 ${skipped} 个重复项` : ""}。`);
      appendAiLog(`[AI拆分子任务] 执行完成，新增 ${inserted} 个子任务`);
    } catch (e) {
      appendAiLog(`[ERROR] [AI拆分子任务] ${String(e)}`);
      setAiResult(`AI拆分子任务失败: ${e}`);
    } finally {
      setAiLoading(null);
    }
  };

  const handleAiGeneratePlan = async () => {
    const taskTitle = title.trim();
    const taskDescription = description.trim();

    if (!taskTitle && !taskDescription) {
      setPlanError("请先填写任务标题或描述，再执行 AI 生成计划。");
      setPlanNotice(null);
      return;
    }

    resetAiLogs("AI生成计划");
    setPlanLoading(true);
    setGeneratedPlan(null);
    setPlanError(null);
    setPlanNotice(null);

    try {
      appendAiLog("[AI生成计划] 正在准备任务图片、子任务与执行配置...");
      const [_, imagePaths] = await Promise.all([fetchSubtasks(task.id), loadCurrentImagePaths()]);
      await logOneShotAiContext("AI生成计划", imagePaths);
      const latestSubtasks = (useTaskStore.getState().subtasks[task.id] ?? []).map((subtask) => subtask.title);
      appendAiLog(`[AI生成计划] 已载入子任务 ${latestSubtasks.length} 个`);
      appendAiLog("[AI生成计划] 已提交给 AI，等待响应...");
      const plan = await aiGeneratePlan(
        taskTitle,
        taskDescription,
        status,
        priority,
        latestSubtasks,
        imagePaths,
      );
      const trimmedPlan = plan.trim();

      if (!trimmedPlan) {
        appendAiLog("[AI生成计划] AI 未返回可展示的计划内容");
        setPlanError("AI 未返回可展示的计划内容。");
        return;
      }

      setGeneratedPlan(trimmedPlan);
      appendAiLog("[AI生成计划] 执行完成");
    } catch (error) {
      appendAiLog(`[ERROR] [AI生成计划] ${String(error)}`);
      setPlanError(error instanceof Error ? error.message : String(error));
    } finally {
      setPlanLoading(false);
    }
  };

  const applyGeneratedPlan = async (mode: PlanInsertMode) => {
    const plan = generatedPlan?.trim();
    if (!plan) {
      setPlanError("请先生成计划，再执行插入。");
      return;
    }

    const previousDescription = description;
    const hasExistingDescription = description.trim().length > 0;
    const nextDescription =
      mode === "append" && hasExistingDescription
        ? `${description.trimEnd()}\n\n---\n\n${plan}`
        : plan;

    setInsertSubmitting(true);
    setSaveError(null);
    setPlanError(null);
    setPlanNotice(null);
    setDescription(nextDescription);

    try {
      await updateTask(task.id, { description: nextDescription });
      setGeneratedPlan(null);
      setInsertDialogOpen(false);
      setPlanNotice("AI 计划已插入详情。");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setDescription(previousDescription);
      setPlanError(message);
    } finally {
      setInsertSubmitting(false);
    }
  };

  const handleInsertPlan = async () => {
    if (!generatedPlan?.trim()) {
      setPlanError("请先生成计划，再执行插入。");
      return;
    }

    if (description.trim().length === 0) {
      await applyGeneratedPlan("replace");
      return;
    }

    setInsertDialogOpen(true);
  };

  function getLineColor(line: string): string {
    if (line.startsWith("[ERROR]")) return "text-red-400";
    if (line.startsWith("[EXIT]")) return "text-yellow-400";
    return "text-green-400";
  }

  function getAiLogColor(line: string): string {
    if (line.includes("[ERROR]")) return "text-red-400";
    if (line.includes("[WARN]")) return "text-yellow-400";
    return "text-zinc-300";
  }

  function getSessionStatusLabel(statusValue: string | null | undefined) {
    switch (statusValue) {
      case "pending":
        return "准备中";
      case "running":
        return "运行中";
      case "stopping":
        return "停止中";
      case "exited":
        return "已完成";
      case "failed":
        return "失败";
      default:
        return "未开始";
    }
  }

  function getExecutionChangeTypeLabel(changeType: string) {
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

  function getExecutionChangeTypeClassName(changeType: string) {
    switch (changeType) {
      case "added":
        return "border-emerald-500/25 bg-emerald-500/10 text-emerald-700";
      case "modified":
        return "border-blue-500/25 bg-blue-500/10 text-blue-700";
      case "deleted":
        return "border-red-500/25 bg-red-500/10 text-red-700";
      case "renamed":
        return "border-amber-500/25 bg-amber-500/10 text-amber-700";
      default:
        return "border-border bg-muted/40 text-muted-foreground";
    }
  }

  const aiActionDisabled = aiLoading !== null || planLoading || insertSubmitting;

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

          <div className="space-y-4">
            {/* Title */}
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onBlur={() => handleSave("title", title)}
              className="text-lg font-semibold border-none px-0 focus-visible:ring-0"
              placeholder="任务标题"
            />

            {saveError && (
              <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {saveError}
              </div>
            )}

            {/* Meta row */}
            <div className="flex flex-wrap items-center gap-3">
              {/* Status */}
              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">状态</span>
                <Select
                  value={status}
                  onValueChange={(value) => {
                    if (!value) return;
                    setStatus(value);
                    handleSave("status", value);
                  }}
                >
                  <SelectTrigger className="h-7 w-[104px] shrink-0 rounded-md px-2 text-xs">
                    <SelectValue>
                      {(value) =>
                        typeof value === "string"
                          ? TASK_STATUSES.find((item) => item.value === value)?.label ?? value
                          : "选择状态"
                      }
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {TASK_STATUSES.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {item.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Priority */}
              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">优先级</span>
                <Select
                  value={priority}
                  onValueChange={(value) => {
                    if (!value) return;
                    setPriority(value);
                    handleSave("priority", value);
                  }}
                >
                  <SelectTrigger className="h-7 w-[92px] shrink-0 rounded-md px-2 text-xs">
                    <SelectValue>
                      {(value) =>
                        typeof value === "string"
                          ? PRIORITIES.find((item) => item.value === value)?.label ?? value
                          : "选择优先级"
                      }
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {PRIORITIES.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {item.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Assignee */}
              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">指派</span>
                <Select
                  value={assigneeId || UNASSIGNED_VALUE}
                  onValueChange={(value) => {
                    const nextValue =
                      !value || value === UNASSIGNED_VALUE ? "" : value;
                    setAssigneeId(nextValue);
                    handleSave("assignee_id", nextValue);
                  }}
                >
                  <SelectTrigger className="h-7 w-[176px] shrink-0 rounded-md px-2 text-xs">
                    <SelectValue>
                      {(value) => {
                        if (!value || value === UNASSIGNED_VALUE) {
                          return "未指派";
                        }

                        return employees.find((emp) => emp.id === value)?.name ?? "未指派";
                      }}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={UNASSIGNED_VALUE}>未指派</SelectItem>
                    {employees.map((emp) => (
                      <SelectItem key={emp.id} value={emp.id}>
                        {emp.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="flex shrink-0 items-center gap-2">
                <span className="shrink-0 whitespace-nowrap text-xs text-muted-foreground">审查员</span>
                <Select
                  value={reviewerId || UNASSIGNED_VALUE}
                  onValueChange={(value) => {
                    const nextValue =
                      !value || value === UNASSIGNED_VALUE ? "" : value;
                    setReviewerId(nextValue);
                    void handleSave("reviewer_id", nextValue);
                  }}
                >
                  <SelectTrigger className="h-7 w-[176px] shrink-0 rounded-md px-2 text-xs">
                    <SelectValue>
                      {(value) => {
                        if (!value || value === UNASSIGNED_VALUE) {
                          return "未指定";
                        }

                        return employees.find((emp) => emp.id === value)?.name ?? "未指定";
                      }}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={UNASSIGNED_VALUE}>未指定</SelectItem>
                    {reviewerCandidates.map((emp) => (
                      <SelectItem key={emp.id} value={emp.id}>
                        {emp.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Delete */}
              <button
                onClick={() => setDeleteDialogOpen(true)}
                disabled={isRunning || deletingTask}
                className="ml-auto p-1 text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
                title={isRunning ? "运行中的任务不能删除，请先停止" : "删除任务"}
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>

          {/* Codex Run Controls */}
          <div className="flex items-center gap-2">
            {assigneeId ? (
              isRunning ? (
                <button
                  onClick={handleStopCodex}
                  disabled={codexLoading}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50"
                >
                  {codexLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Square className="h-3 w-3" />}
                  停止运行
                </button>
              ) : (
                <button
                  onClick={handleRunCodex}
                  disabled={codexLoading}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50"
                >
                  {codexLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />}
                  运行 Codex
                </button>
              )
            ) : (
              <span className="text-xs text-muted-foreground">请先指派员工以运行 Codex</span>
            )}
            {isRunning && (
              <span className="flex items-center gap-1 text-xs text-green-500">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                运行中
              </span>
            )}
          </div>

          {/* Codex Terminal Output */}
          {(isRunning || output.length > 0) && assigneeId && (
            <div>
              <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
                <span className="text-xs text-zinc-500 font-mono">Codex 终端</span>
                <button
                  onClick={() => clearTaskCodexOutput(task.id)}
                  className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
                  title="清空日志"
                >
                  <Eraser className="h-3 w-3" />
                </button>
              </div>
              <ScrollArea className="h-40 bg-black rounded-b">
                <div className="p-2 font-mono text-xs space-y-0.5">
                  {output.length === 0 ? (
                    <div className="text-zinc-600">等待输出...</div>
                  ) : (
                    output.map((line, i) => (
                      <div key={i} className={`whitespace-pre-wrap ${getLineColor(line)}`}>
                        {line}
                      </div>
                    ))
                  )}
                  <div ref={terminalRef} />
                </div>
              </ScrollArea>
            </div>
          )}

          <div className="space-y-3 rounded-md border border-border/70 bg-muted/20 p-3">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="space-y-1">
                <p className="text-sm font-medium">修改文件</p>
                <p className="text-[11px] text-muted-foreground">
                  按 execution 会话记录本次新增影响到的文件，包含新增、修改、删除和重命名。
                </p>
              </div>
              <button
                type="button"
                onClick={() => void loadExecutionChangeHistory()}
                className="text-[11px] text-primary hover:underline disabled:opacity-50"
                disabled={executionChangeHistoryLoading}
              >
                {executionChangeHistoryLoading ? "刷新中..." : "刷新"}
              </button>
            </div>

            {executionChangeHistoryError && (
              <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {executionChangeHistoryError}
              </div>
            )}

            {executionChangeHistory.length > 0 ? (
              <div className="space-y-3">
                {executionChangeHistory.map((item) => (
                  <div key={item.session.id} className="rounded-md border border-border bg-background/70 p-3">
                    <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-muted-foreground">
                      <span>
                        {new Date(item.session.started_at).toLocaleString("zh-CN")}
                        {" · "}
                        {getSessionStatusLabel(item.session.status)}
                      </span>
                      <span className="font-mono">
                        {item.session.cli_session_id ?? item.session.id}
                      </span>
                    </div>

                    {item.changes.length > 0 ? (
                      <div className="mt-3 space-y-2">
                        {item.changes.map((change) => (
                          <div
                            key={change.id}
                            className="flex flex-col gap-1 rounded-md border border-border/60 bg-background px-3 py-2 text-xs"
                          >
                            <div className="flex flex-wrap items-center gap-2">
                              <span
                                className={`rounded-md border px-1.5 py-0.5 text-[11px] font-medium ${getExecutionChangeTypeClassName(change.change_type)}`}
                              >
                                {getExecutionChangeTypeLabel(change.change_type)}
                              </span>
                              <span className="font-mono text-foreground break-all">{change.path}</span>
                            </div>
                            {change.previous_path && (
                              <div className="text-[11px] text-muted-foreground">
                                原路径：<span className="font-mono break-all">{change.previous_path}</span>
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="mt-3 rounded-md border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
                        本次运行未产生新增文件变更。
                      </div>
                    )}
                  </div>
                ))}
              </div>
            ) : executionChangeHistoryLoading ? (
              <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                正在加载修改文件历史...
              </div>
            ) : (
              <div className="rounded-md border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                还没有 execution 会话的文件记录。
              </div>
            )}
          </div>

          <div className="space-y-3 rounded-md border border-border/70 bg-muted/20 p-3">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="space-y-1">
                <p className="text-sm font-medium">代码审核</p>
                <p className="text-[11px] text-muted-foreground">
                  审核当前项目仓库的工作区改动，默认由任务审查员发起只读 reviewer 会话。
                </p>
              </div>
              <button
                type="button"
                onClick={() => void handleStartCodeReview()}
                disabled={reviewLoading || status !== "review" || !reviewerId}
                className="flex items-center gap-1 rounded-md bg-amber-500 px-2.5 py-1.5 text-xs font-medium text-black transition-colors hover:bg-amber-400 disabled:opacity-50"
                title={
                  status !== "review"
                    ? "仅“审核中”任务支持代码审核"
                    : !reviewerId
                      ? "请先指定审查员"
                      : "启动代码审核"
                }
              >
                {reviewLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
                审核代码
              </button>
            </div>

            {reviewError && (
              <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {reviewError}
              </div>
            )}

            {reviewNotice && (
              <div className="rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-700">
                {reviewNotice}
              </div>
            )}

            <div className="grid gap-2 text-xs text-muted-foreground md:grid-cols-3">
              <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                <span className="font-medium text-foreground">审查员：</span>
                {reviewer?.name ?? "未指定"}
              </div>
              <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                <span className="font-medium text-foreground">最近状态：</span>
                {getSessionStatusLabel(
                  isReviewRunning ? "running" : latestReview?.session.status,
                )}
              </div>
              <div className="rounded-md border border-border bg-background/70 px-3 py-2">
                <span className="font-medium text-foreground">最近会话：</span>
                {latestReview?.session.cli_session_id ?? latestReview?.session.id ?? "暂无"}
              </div>
            </div>

            {status !== "review" && (
              <div className="rounded-md border border-border bg-background/70 px-3 py-2 text-xs text-muted-foreground">
                当前任务状态不是“审核中”，代码审核入口已禁用。
              </div>
            )}

            {(isReviewRunning || reviewOutput.length > 0) && reviewerId && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <p className="text-xs font-medium text-muted-foreground">审核会话日志</p>
                  <span className="text-[11px] text-muted-foreground">
                    {isReviewRunning ? "运行中" : "最近一次审核输出"}
                  </span>
                </div>
                <CodexTerminal taskId={task.id} sessionKind="review" />
              </div>
            )}

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <p className="text-xs font-medium text-muted-foreground">审核结果</p>
                <div className="flex items-center gap-2">
                  {latestReviewLoading ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                  ) : (
                    <button
                      type="button"
                      onClick={() => void loadLatestReview()}
                      className="text-[11px] text-primary hover:underline"
                    >
                      刷新
                    </button>
                  )}
                  <Button
                    type="button"
                    variant="outline"
                    size="xs"
                    onClick={() => void handleCopyReviewReport()}
                    disabled={!latestReview?.report?.trim()}
                  >
                    <Copy className="h-3 w-3" />
                    复制
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    size="xs"
                    onClick={() => setReviewFixDialogOpen(true)}
                    disabled={!latestReview?.report?.trim() || !assigneeId || reviewFixSubmitting}
                    title={
                      !assigneeId
                        ? "原任务未指派开发负责人"
                        : "创建修复任务并立即运行"
                    }
                  >
                    <Wrench className="h-3 w-3" />
                    修复
                  </Button>
                </div>
              </div>

              {latestReview?.report ? (
                <ScrollArea className="h-56 overflow-hidden rounded-md border bg-background/80">
                  <div className="p-3 text-xs whitespace-pre-wrap text-foreground">
                    {latestReview.report}
                  </div>
                </ScrollArea>
              ) : (
                <div className="rounded-md border border-dashed border-border bg-background/70 px-3 py-6 text-center text-xs text-muted-foreground">
                  {latestReview
                    ? "最近一次审核尚未产出结构化报告。"
                    : "还没有代码审核结果。"}
                </div>
              )}

              {latestReview && (
                <div className="text-[11px] text-muted-foreground">
                  {latestReview.reviewer_name ?? "未知审查员"} ·
                  {" "}
                  {new Date(latestReview.session.started_at).toLocaleString("zh-CN")}
                </div>
              )}
            </div>
          </div>

          {/* AI Actions */}
          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={handleAiSuggest}
              disabled={aiActionDisabled}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "assignee" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI建议指派
            </button>
            <button
              onClick={handleAiComplexity}
              disabled={aiActionDisabled}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "complexity" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              复杂度分析
            </button>
            <button
              onClick={handleAiSplitSubtasks}
              disabled={aiActionDisabled}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "subtasks" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI拆分子任务
            </button>
            <button
              onClick={handleAiGeneratePlan}
              disabled={aiActionDisabled}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {planLoading ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI生成计划
            </button>
            <button
              onClick={handleAiComment}
              disabled={aiActionDisabled}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors disabled:opacity-50"
            >
              {aiLoading === "comment" ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              AI生成评论
            </button>
          </div>

          {(aiLoading !== null || planLoading || aiLogs.length > 0) && (
            <div>
              <div className="flex items-center justify-between px-2 py-1 bg-black/80 rounded-t border-b border-zinc-800">
                <span className="text-xs text-zinc-500 font-mono">AI 执行日志</span>
                <button
                  onClick={() => setAiLogs([])}
                  className="p-0.5 text-zinc-500 hover:text-zinc-300 transition-colors"
                  title="清空日志"
                >
                  <Eraser className="h-3 w-3" />
                </button>
              </div>
              <ScrollArea className="h-28 overflow-hidden bg-black rounded-b">
                <div className="p-2 font-mono text-xs space-y-0.5">
                  {aiLogs.length === 0 ? (
                    <div className="text-zinc-600">等待执行...</div>
                  ) : (
                    aiLogs.map((line, index) => (
                      <div key={`${line}-${index}`} className={`whitespace-pre-wrap ${getAiLogColor(line)}`}>
                        {line}
                      </div>
                    ))
                  )}
                  <div ref={aiLogRef} />
                </div>
              </ScrollArea>
            </div>
          )}

          {/* AI Result */}
          {aiResult && (
            <div className="bg-primary/5 rounded-md p-3 text-xs text-muted-foreground">
              <span className="font-medium text-primary">AI 结果: </span>
              {aiResult}
            </div>
          )}

          {/* AI Suggestion (persisted) */}
          {task.ai_suggestion && !aiResult && (
            <div className="bg-primary/5 rounded-md p-3 text-xs text-muted-foreground">
              <span className="font-medium text-primary">AI 建议: </span>
              {task.ai_suggestion}
            </div>
          )}

          {planError && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {planError}
            </div>
          )}

          {planNotice && (
            <div className="rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-700">
              {planNotice}
            </div>
          )}

          {generatedPlan && (
            <div className="space-y-3 rounded-md border border-primary/20 bg-primary/5 p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <p className="text-xs font-medium text-primary">AI 计划预览</p>
                  <p className="text-[11px] text-muted-foreground">确认后可插入到任务详情描述中</p>
                </div>
                <button
                  onClick={() => void handleInsertPlan()}
                  disabled={insertSubmitting}
                  className="flex items-center gap-1 rounded px-2 py-1 text-xs bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
                >
                  {insertSubmitting ? <Loader2 className="h-3 w-3 animate-spin" /> : null}
                  插入详情
                </button>
              </div>
              <ScrollArea className="h-72 overflow-hidden rounded-md border bg-background/80">
                <div className="p-3 text-xs text-foreground whitespace-pre-wrap">
                  {generatedPlan}
                </div>
              </ScrollArea>
            </div>
          )}

          <Separator />

          {/* Description */}
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              描述
            </label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onBlur={() => handleSave("description", description)}
              className="mt-1 min-h-[80px] resize-y"
              placeholder="添加任务描述..."
            />
          </div>

          <Separator />

          <div className="space-y-3">
            <div className="flex items-start justify-between gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  图片附件
                </label>
                <p className="text-[11px] text-muted-foreground">
                  当前任务的图片会在每次启动和续聊时自动附带给 Codex。
                </p>
              </div>
              <button
                type="button"
                onClick={() => void handleSelectAttachments()}
                disabled={!isTauriRuntime() || attachmentLoading}
                className="flex items-center gap-1 rounded-md border border-input px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-50"
                title={isTauriRuntime() ? "上传图片" : "仅桌面端支持上传图片"}
              >
                {attachmentLoading ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <ImagePlus className="h-3.5 w-3.5" />
                )}
                添加图片
              </button>
            </div>

            {!isTauriRuntime() && (
              <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                当前环境不支持任务图片上传，请在桌面端使用该功能。
              </div>
            )}

            {attachmentError && (
              <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {attachmentError}
              </div>
            )}

            <ErrorBoundary
              fallbackTitle="图片附件区渲染失败"
              fallbackDescription="附件数据已保留，但缩略图区域发生了运行时异常。"
            >
              <TaskAttachmentGrid
                items={attachments.map((attachment) => ({
                  id: attachment.id,
                  name: attachment.original_name,
                  path: attachment.stored_path,
                  fileSize: attachment.file_size,
                  mimeType: attachment.mime_type,
                  removable: deletingAttachmentId !== attachment.id,
                  onOpen: isTauriRuntime()
                    ? () => void handleOpenAttachment(attachment.stored_path)
                    : undefined,
                  onRemove: () => void handleDeleteAttachment(attachment.id),
                }))}
                emptyText="当前任务还没有图片"
              />
            </ErrorBoundary>
          </div>

          <Separator />

          {/* Subtasks */}
          <SubtaskList taskId={task.id} />

          <Separator />

          {/* Comments */}
          <CommentList taskId={task.id} />
          </div>
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
      {insertDialogOpen && (
        <InsertPlanConfirmDialog
          open={insertDialogOpen}
          taskTitle={title.trim() || task.title}
          inserting={insertSubmitting}
          onOpenChange={setInsertDialogOpen}
          onAppend={() => applyGeneratedPlan("append")}
          onReplace={() => applyGeneratedPlan("replace")}
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
