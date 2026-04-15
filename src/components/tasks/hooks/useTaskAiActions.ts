import { useEffect, useState } from "react";

import {
  aiAnalyzeComplexity,
  aiGenerateComment,
  aiGeneratePlan,
  aiSplitSubtasks,
  aiSuggestAssignee,
} from "@/lib/codex";
import { getCodexSettings, healthCheck } from "@/lib/backend";
import type { Employee, Task } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";

type PlanInsertMode = "append" | "replace";

interface UseTaskAiActionsOptions {
  task: Task;
  open: boolean;
  title: string;
  description: string;
  status: string;
  priority: string;
  employees: Employee[];
  projectRepoPath?: string | null;
  fetchAttachments: (taskId: string) => Promise<void>;
  fetchSubtasks: (taskId: string) => Promise<void>;
  updateTask: (
    id: string,
    updates: Partial<
      Pick<
        Task,
        | "title"
        | "description"
        | "priority"
        | "status"
        | "assignee_id"
        | "reviewer_id"
        | "complexity"
        | "ai_suggestion"
        | "last_codex_session_id"
        | "last_review_session_id"
      >
    >,
  ) => Promise<void>;
  addComment: (taskId: string, content: string, employeeId?: string, isAiGenerated?: boolean) => Promise<void>;
  addSubtasks: (taskId: string, titles: string[]) => Promise<{ inserted: number; skipped: number }>;
  onDescriptionChange: (value: string) => void;
}

export function useTaskAiActions({
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
  onDescriptionChange,
}: UseTaskAiActionsOptions) {
  const [aiLoading, setAiLoading] = useState<string | null>(null);
  const [aiResult, setAiResult] = useState<string | null>(null);
  const [planLoading, setPlanLoading] = useState(false);
  const [generatedPlan, setGeneratedPlan] = useState<string | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [planNotice, setPlanNotice] = useState<string | null>(null);
  const [insertDialogOpen, setInsertDialogOpen] = useState(false);
  const [insertSubmitting, setInsertSubmitting] = useState(false);
  const [aiLogs, setAiLogs] = useState<string[]>([]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setAiLoading(null);
    setAiResult(null);
    setPlanLoading(false);
    setGeneratedPlan(null);
    setPlanError(null);
    setPlanNotice(null);
    setInsertDialogOpen(false);
    setInsertSubmitting(false);
    setAiLogs([]);
  }, [open, task.id]);

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

  const loadCurrentImagePaths = async () => {
    await fetchAttachments(task.id);
    return (useTaskStore.getState().attachments[task.id] ?? [])
      .map((attachment) => attachment.stored_path.trim())
      .filter((path) => path.length > 0);
  };

  const buildAiExecutionContext = () => ({
    taskId: task.id,
    workingDir: projectRepoPath ?? undefined,
  });

  const logOneShotAiContext = async (operation: string, imagePaths: string[]) => {
    appendAiLog(`[${operation}] 已载入任务图片 ${imagePaths.length} 张`);
    appendAiLog(
      `[${operation}] 当前项目目录：${
        projectRepoPath?.trim() ? projectRepoPath : "未配置（本次不会附带项目目录上下文）"
      }`,
    );

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

  const handleAiSuggest = async () => {
    resetAiLogs("AI建议指派");
    setAiLoading("assignee");
    setAiResult(null);
    try {
      appendAiLog("[AI建议指派] 正在准备任务图片与执行配置...");
      const imagePaths = await loadCurrentImagePaths();
      await logOneShotAiContext("AI建议指派", imagePaths);
      const employeeList = employees
        .map((employee) => `${employee.id}: ${employee.name} (${employee.role}, ${employee.specialization ?? "general"})`)
        .join("; ");
      const desc = task.description ?? task.title;
      appendAiLog("[AI建议指派] 已提交给 AI，等待响应...");
      const result = await aiSuggestAssignee(
        desc,
        employeeList,
        imagePaths,
        buildAiExecutionContext(),
      );
      setAiResult(result);
      await updateTask(task.id, { ai_suggestion: result });
      appendAiLog("[AI建议指派] 执行完成");
    } catch (error) {
      appendAiLog(`[ERROR] [AI建议指派] ${String(error)}`);
      setAiResult(`AI建议失败: ${error}`);
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
      const result = await aiAnalyzeComplexity(desc, imagePaths, buildAiExecutionContext());
      setAiResult(result);
      const match = result.match(/(\d+)/);
      if (match) {
        await updateTask(task.id, { complexity: parseInt(match[1], 10) });
      }
      appendAiLog("[复杂度分析] 执行完成");
    } catch (error) {
      appendAiLog(`[ERROR] [复杂度分析] ${String(error)}`);
      setAiResult(`复杂度分析失败: ${error}`);
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
        buildAiExecutionContext(),
      );
      await addComment(task.id, result, undefined, true);
      appendAiLog("[AI生成评论] 执行完成");
    } catch (error) {
      appendAiLog(`[ERROR] [AI生成评论] ${String(error)}`);
      console.error("AI comment failed:", error);
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
      const generatedSubtasks = await aiSplitSubtasks(
        taskTitle,
        taskDescription,
        imagePaths,
        buildAiExecutionContext(),
      );
      const { inserted, skipped } = await addSubtasks(task.id, generatedSubtasks);

      if (inserted === 0) {
        appendAiLog("[AI拆分子任务] 响应完成，但没有可新增的子任务");
        setAiResult(skipped > 0 ? "AI 已完成拆分，但结果与现有子任务重复，未新增内容。" : "AI 未生成可写入的子任务。");
        return;
      }

      setAiResult(`AI 已写入 ${inserted} 个子任务${skipped > 0 ? `，跳过 ${skipped} 个重复项` : ""}。`);
      appendAiLog(`[AI拆分子任务] 执行完成，新增 ${inserted} 个子任务`);
    } catch (error) {
      appendAiLog(`[ERROR] [AI拆分子任务] ${String(error)}`);
      setAiResult(`AI拆分子任务失败: ${error}`);
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
      const [, imagePaths] = await Promise.all([fetchSubtasks(task.id), loadCurrentImagePaths()]);
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
        buildAiExecutionContext(),
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
    setPlanError(null);
    setPlanNotice(null);
    onDescriptionChange(nextDescription);

    try {
      await updateTask(task.id, { description: nextDescription });
      setGeneratedPlan(null);
      setInsertDialogOpen(false);
      setPlanNotice("AI 计划已插入详情。");
    } catch (error) {
      onDescriptionChange(previousDescription);
      setPlanError(error instanceof Error ? error.message : String(error));
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

  return {
    aiLoading,
    aiResult,
    planLoading,
    generatedPlan,
    planError,
    planNotice,
    insertDialogOpen,
    insertSubmitting,
    aiLogs,
    aiActionDisabled: aiLoading !== null || planLoading || insertSubmitting,
    clearAiLogs: () => setAiLogs([]),
    handleAiSuggest,
    handleAiComplexity,
    handleAiComment,
    handleAiSplitSubtasks,
    handleAiGeneratePlan,
    handleInsertPlan,
    applyGeneratedPlan,
    setInsertDialogOpen,
  };
}
