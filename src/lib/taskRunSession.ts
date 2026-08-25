import { startByProvider } from "@/lib/aiEngine";
import { checkClaudeSdkHealth, prepareTaskGitExecution } from "@/lib/backend";
import {
  confirmImageAttachmentSkip,
  ImageSkipCancelledError,
  resolveImageAttachmentSkip,
} from "@/lib/imageAttachmentSkip";
import type { Employee, StartSessionOutcome, Task } from "@/lib/types";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { useTaskStore } from "@/stores/taskStore";

export interface PreparedTaskRunInput {
  prompt: string;
  imagePaths: string[];
  resumeSessionId?: string;
}

function asStartSessionOutcome(value: StartSessionOutcome | null | undefined): StartSessionOutcome {
  if (value?.status === "queued") {
    const position = Number(value.position);
    return { status: "queued", position: Number.isFinite(position) && position > 0 ? position : 1 };
  }
  return { status: "started" };
}

export interface StartTaskRunSessionParams {
  task: Task;
  assigneeId: string;
  assignee?: Employee;
  projectRepoPath?: string | null;
  executionInput: PreparedTaskRunInput;
  clearTaskOutput?: boolean;
  imageSkipConfirmed?: boolean;
  planMode?: boolean;
}

/**
 * Shared kernel for starting a task execution session.
 * Invoke first; only mark busy / in_progress / timer when the session actually started.
 */
export async function startTaskRunSession({
  task,
  assigneeId,
  assignee,
  projectRepoPath,
  executionInput,
  clearTaskOutput = false,
  imageSkipConfirmed = false,
  planMode = false,
}: StartTaskRunSessionParams): Promise<StartSessionOutcome> {
  if (!assigneeId) {
    throw new Error("请先指定执行员工，再执行任务。");
  }
  if (task.status === "archived") {
    throw new Error("已归档任务不可启动执行会话");
  }

  const employeeStore = useEmployeeStore.getState();
  const taskStore = useTaskStore.getState();
  const existingQueued = taskStore.runQueue.find((item) => item.task_id === task.id);
  if (existingQueued) {
    return {
      status: "queued",
      position: existingQueued.position,
    };
  }

  if (clearTaskOutput) {
    employeeStore.clearTaskCodexOutput(task.id);
  }

  let workingDir = projectRepoPath ?? undefined;
  let taskGitContextId: string | undefined;

  if (task.use_worktree) {
    const prepared = await prepareTaskGitExecution(task.id);
    workingDir = prepared.working_dir;
    taskGitContextId = prepared.task_git_context_id;
  }

  if (!workingDir) {
    throw new Error("当前项目缺少可用工作目录，无法启动任务执行。");
  }

  if (!imageSkipConfirmed && executionInput.imagePaths.length > 0) {
    const project = useProjectStore
      .getState()
      .allProjects.find((item) => item.id === task.project_id);
    const projectType = project?.project_type ?? "local";
    let claudeEffectiveProvider: string | null = null;
    if (assignee?.ai_provider === "claude" && projectType !== "ssh") {
      try {
        claudeEffectiveProvider = (await checkClaudeSdkHealth()).effective_provider;
      } catch {
        claudeEffectiveProvider = "cli";
      }
    }
    const skipReason = resolveImageAttachmentSkip({
      imageCount: executionInput.imagePaths.length,
      provider: assignee?.ai_provider ?? "codex",
      projectType,
      claudeEffectiveProvider,
      hasTaskId: Boolean(task.id),
    });
    if (skipReason) {
      const confirmed = await confirmImageAttachmentSkip(
        skipReason,
        executionInput.imagePaths.length,
      );
      if (!confirmed) {
        throw new ImageSkipCancelledError();
      }
    }
  }

  const startOptions = {
    model: assignee?.model,
    reasoningEffort: assignee?.reasoning_effort,
    systemPrompt: assignee?.system_prompt,
    workingDir,
    taskId: task.id,
    taskGitContextId,
    resumeSessionId: executionInput.resumeSessionId,
    imagePaths: executionInput.imagePaths,
    planMode: planMode || undefined,
  };

  const outcome = asStartSessionOutcome(
    await startByProvider(
      assignee?.ai_provider ?? "codex",
      assigneeId,
      executionInput.prompt,
      startOptions,
    ),
  );

  if (outcome.status === "queued") {
    await taskStore.fetchRunQueue();
    return outcome;
  }

  await employeeStore.updateEmployeeStatus(assigneeId, "busy");
  await taskStore.updateTaskStatus(task.id, "in_progress");
  await taskStore.startTaskTimer(task.id);
  await employeeStore.refreshEmployeeRuntimeStatus(assigneeId);
  return outcome;
}

export async function reportTaskRunSessionError(
  error: unknown,
  assigneeId?: string | null,
  taskId?: string,
): Promise<string> {
  const message = error instanceof Error ? error.message : String(error);
  if (assigneeId) {
    const employeeStore = useEmployeeStore.getState();
    employeeStore.addCodexOutput(assigneeId, `[ERROR] ${message}`, taskId);
    const runtime = await employeeStore.refreshEmployeeRuntimeStatus(assigneeId);
    if (!runtime?.running) {
      await employeeStore.updateEmployeeStatus(assigneeId, "error");
    }
  }
  return message;
}
