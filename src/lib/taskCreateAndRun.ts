import { aiGenerateCoordinatorTaskPlan, addTaskDependency, setTaskTags } from "@/lib/backend";
import { runExclusiveCoordinatorPlanGenerate } from "@/lib/coordinatorPlanSession";
import { getProjectWorkingDir } from "@/lib/projects";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import { isImageSkipCancelled } from "@/lib/imageAttachmentSkip";
import { reportTaskRunSessionError, startTaskRunSession } from "@/lib/taskRunSession";
import type { Employee, Project, Task } from "@/lib/types";
import { useTaskBackgroundRunStore } from "@/stores/taskBackgroundRunStore";
import { useTaskStore } from "@/stores/taskStore";

export type CreateAndRunPhase = "idle" | "creating" | "planning" | "starting";

export interface CreateTaskPayload {
  title: string;
  description?: string;
  priority: string;
  project_id: string;
  use_worktree: boolean;
  assignee_id?: string;
  reviewer_id?: string;
  coordinator_id?: string;
  due_date?: string | null;
  milestone_id?: string | null;
  native_subagent_id?: string | null;
  attachment_source_paths?: string[];
  file_ref_paths?: string[];
}

export interface CreateTaskForRunOptions {
  payload: CreateTaskPayload;
  tagIds: string[];
  dependencyTaskIds: string[];
  refreshProjectId?: string;
}

export interface ContinueCreatedTaskRunOptions {
  task: Task;
  payload: CreateTaskPayload;
  project: Project;
  assignee: Employee;
}

export async function generateAndPersistCoordinatorPlan(params: {
  task: Task;
  coordinatorId: string;
  workingDir: string | null;
}): Promise<string> {
  return runExclusiveCoordinatorPlanGenerate(params.task.id, async () => {
    const plan = await aiGenerateCoordinatorTaskPlan({
      task_id: params.task.id,
      coordinator_id: params.coordinatorId,
      title: params.task.title,
      description: params.task.description,
      status: params.task.status,
      priority: params.task.priority,
      working_dir: params.workingDir,
    });
    const trimmedPlan = plan.markdown.trim();
    if (!trimmedPlan) {
      throw new Error("协调员未返回可用计划。");
    }
    await useTaskStore.getState().updateTask(params.task.id, { plan_content: trimmedPlan });
    return trimmedPlan;
  });
}

/** Create task + tags/deps only. Caller closes dialog, then continues in background. */
export async function createTaskForRun(options: CreateTaskForRunOptions): Promise<Task> {
  const { payload, tagIds, dependencyTaskIds, refreshProjectId } = options;

  if (!payload.assignee_id) {
    throw new Error("请先指定执行员工，再执行任务。");
  }

  const taskStore = useTaskStore.getState();
  const task = await taskStore.createTask(payload, { refreshProjectId });

  if (tagIds.length > 0) {
    await setTaskTags({ task_id: task.id, tag_ids: tagIds });
  }
  for (const dependsOnTaskId of dependencyTaskIds) {
    await addTaskDependency({
      task_id: task.id,
      depends_on_task_id: dependsOnTaskId,
    });
  }

  return task;
}

/**
 * After dialog closes: optional coordinator plan, then start execution.
 * Updates taskBackgroundRunStore for kanban badges.
 */
export async function continueCreatedTaskRun(
  options: ContinueCreatedTaskRunOptions,
): Promise<{ task: Task; planContent: string | null }> {
  const { payload, project, assignee } = options;
  let task = options.task;
  const taskId = task.id;
  const progress = useTaskBackgroundRunStore.getState();

  if (!payload.assignee_id) {
    progress.setPhase(taskId, "error", "请先指定执行员工，再执行任务。");
    throw new Error("请先指定执行员工，再执行任务。");
  }

  const workingDir = getProjectWorkingDir(project);
  let planContent: string | null = null;
  const taskStore = useTaskStore.getState();

  try {
    if (payload.coordinator_id) {
      progress.setPhase(taskId, "planning");
      planContent = await generateAndPersistCoordinatorPlan({
        task,
        coordinatorId: payload.coordinator_id,
        workingDir,
      });
      task = {
        ...task,
        plan_content: planContent,
        coordinator_id: payload.coordinator_id,
      };
    }

    progress.setPhase(taskId, "starting");
    await Promise.all([taskStore.fetchAttachments(task.id), taskStore.fetchFileRefs(task.id)]);
    const attachments = useTaskStore.getState().attachments[task.id] ?? [];
    const fileRefs = useTaskStore.getState().fileRefs[task.id] ?? [];
    const executionInput = buildTaskExecutionInput({
      title: task.title,
      description: task.description,
      planContent,
      subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
      attachments,
      fileRefs,
    });

    await startTaskRunSession({
      task,
      assigneeId: payload.assignee_id,
      assignee,
      projectRepoPath: workingDir,
      executionInput: {
        prompt: executionInput.prompt,
        imagePaths: executionInput.imagePaths,
      },
      clearTaskOutput: true,
    });

    progress.clear(taskId);
    return { task, planContent };
  } catch (error) {
    if (isImageSkipCancelled(error)) {
      progress.clear(taskId);
      return { task, planContent };
    }
    const message = error instanceof Error ? error.message : String(error);
    // Only report run-session style errors when we already entered starting;
    // plan failures still get a log line + badge.
    if (useTaskBackgroundRunStore.getState().byTaskId[taskId]?.phase === "starting") {
      await reportTaskRunSessionError(error, payload.assignee_id, task.id);
    } else if (payload.assignee_id) {
      await reportTaskRunSessionError(error, payload.assignee_id, task.id);
    }
    progress.setPhase(taskId, "error", message);
    throw error instanceof Error ? error : new Error(message);
  }
}

export function getCreateAndRunPhaseLabel(phase: CreateAndRunPhase): string {
  switch (phase) {
    case "creating":
      return "创建中...";
    case "planning":
      return "生成协调员计划...";
    case "starting":
      return "启动执行...";
    default:
      return "创建并执行";
  }
}
