import { addTaskDependency, setTaskTags } from "@/lib/backend";
import { generateAndPersistCoordinatorPlan } from "@/lib/coordinatorPlanPersist";
import { runExistingTaskInBackground } from "@/lib/taskBackgroundRun";
import type { Employee, Project, Task } from "@/lib/types";
import { useTaskBackgroundRunStore } from "@/stores/taskBackgroundRunStore";
import { useTaskStore } from "@/stores/taskStore";

export { generateAndPersistCoordinatorPlan };

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

export type ReviewFixSourceTask = Pick<
  Task,
  "project_id" | "use_worktree" | "reviewer_id" | "coordinator_id" | "native_subagent_id"
>;

export interface ReviewFixCreatePayloadInput {
  sourceTask: ReviewFixSourceTask;
  title: string;
  description: string;
  priority: string;
  assigneeId: string;
  reviewerId?: string;
  coordinatorId?: string;
  nativeSubagentId?: string;
}

function firstAssignedId(...values: Array<string | null | undefined>): string | undefined {
  for (const value of values) {
    const trimmed = value?.trim();
    if (trimmed) {
      return trimmed;
    }
  }
  return undefined;
}

/** Copy assignment fields from the reviewed task onto the follow-up fix task. */
export function buildReviewFixCreatePayload(input: ReviewFixCreatePayloadInput): CreateTaskPayload {
  return {
    title: input.title,
    description: input.description,
    priority: input.priority,
    project_id: input.sourceTask.project_id,
    use_worktree: input.sourceTask.use_worktree,
    assignee_id: input.assigneeId,
    reviewer_id: firstAssignedId(input.reviewerId, input.sourceTask.reviewer_id),
    coordinator_id: firstAssignedId(input.coordinatorId, input.sourceTask.coordinator_id),
    native_subagent_id: firstAssignedId(
      input.nativeSubagentId,
      input.sourceTask.native_subagent_id,
    ),
  };
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
): Promise<{ task: Task; planContent: string | null; cancelled?: boolean }> {
  const { payload, project, assignee } = options;
  const task = {
    ...options.task,
    assignee_id: payload.assignee_id ?? options.task.assignee_id,
    coordinator_id: payload.coordinator_id ?? options.task.coordinator_id,
  };

  if (!payload.assignee_id) {
    const message = "请先指定执行员工，再执行任务。";
    useTaskBackgroundRunStore.getState().setPhase(task.id, "error", message);
    throw new Error(message);
  }

  const outcome = await runExistingTaskInBackground({
    task,
    assignee,
    project,
    regenerateCoordinatorPlan: Boolean(payload.coordinator_id),
  });

  return {
    task: {
      ...task,
      plan_content: outcome.planContent ?? task.plan_content,
    },
    planContent: outcome.planContent,
    cancelled: outcome.status === "cancelled",
  };
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
