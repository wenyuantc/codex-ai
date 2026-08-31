import { getProjectWorkingDir } from "@/lib/projects";
import { hasSavedTaskPlan } from "@/lib/taskPlanRun";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import { isImageSkipCancelled } from "@/lib/imageAttachmentSkip";
import { reportTaskRunSessionError, startTaskRunSession } from "@/lib/taskRunSession";
import { generateAndPersistCoordinatorPlan } from "@/lib/coordinatorPlanPersist";
import { shouldGenerateCoordinatorPlanForBackgroundRun } from "@/lib/taskBackgroundRunPolicy";
import type { Employee, Project, StartSessionOutcome, Task } from "@/lib/types";
import { useTaskBackgroundRunStore } from "@/stores/taskBackgroundRunStore";
import { useTaskStore } from "@/stores/taskStore";

export {
  canOfferTaskBackgroundRun,
  isTaskBackgroundRunDisabled,
  resolveTaskBackgroundRunInitialPhase,
  shouldGenerateCoordinatorPlanForBackgroundRun,
} from "@/lib/taskBackgroundRunPolicy";

export type TaskBackgroundRunStatus = StartSessionOutcome["status"] | "cancelled";

export type TaskBackgroundRunOutcome = {
  status: TaskBackgroundRunStatus;
  position?: number;
  planContent: string | null;
};

const inFlightBackgroundRuns = new Map<string, Promise<TaskBackgroundRunOutcome>>();

export function resetTaskBackgroundRunInFlightForTests(): void {
  inFlightBackgroundRuns.clear();
}

export function isTaskBackgroundRunInFlight(taskId: string): boolean {
  return inFlightBackgroundRuns.has(taskId);
}

export interface RunExistingTaskInBackgroundOptions {
  task: Task;
  assignee: Employee;
  project: Project | null | undefined;
  /**
   * Create-and-run always regenerates a coordinator plan.
   * Kanban background run reuses a saved plan when present.
   */
  regenerateCoordinatorPlan?: boolean;
}

async function runExistingTaskInBackgroundUntracked(
  options: RunExistingTaskInBackgroundOptions,
): Promise<TaskBackgroundRunOutcome> {
  const { assignee } = options;
  let task = options.task;
  const taskId = task.id;
  const progress = useTaskBackgroundRunStore.getState();
  const assigneeId = task.assignee_id ?? assignee.id;

  if (!assigneeId) {
    const message = "请先指定执行员工，再执行任务。";
    progress.setPhase(taskId, "error", message);
    throw new Error(message);
  }

  const workingDir = getProjectWorkingDir(options.project);
  const regenerateCoordinatorPlan = options.regenerateCoordinatorPlan === true;
  let planContent = hasSavedTaskPlan(task.plan_content)
    ? (task.plan_content?.trim() ?? null)
    : null;
  const coordinatorId = task.coordinator_id;
  const shouldGeneratePlan =
    Boolean(coordinatorId) &&
    (regenerateCoordinatorPlan ||
      shouldGenerateCoordinatorPlanForBackgroundRun({
        coordinatorId,
        savedPlan: task.plan_content,
      }));

  try {
    if (shouldGeneratePlan && coordinatorId) {
      progress.setPhase(taskId, "planning");
      planContent = await generateAndPersistCoordinatorPlan({
        task,
        coordinatorId,
        workingDir,
      });
      task = {
        ...task,
        plan_content: planContent,
        coordinator_id: coordinatorId,
      };
    }

    progress.setPhase(taskId, "starting");
    const taskStore = useTaskStore.getState();
    await Promise.all([
      taskStore.fetchSubtasks(task.id),
      taskStore.fetchAttachments(task.id),
      taskStore.fetchFileRefs(task.id),
    ]);
    const latest = useTaskStore.getState();
    const executionInput = buildTaskExecutionInput({
      title: task.title,
      description: task.description,
      planContent,
      subtasks: latest.subtasks[task.id] ?? [],
      attachments: latest.attachments[task.id] ?? [],
      fileRefs: latest.fileRefs[task.id] ?? [],
    });

    const outcome = await startTaskRunSession({
      task,
      assigneeId,
      assignee,
      projectRepoPath: workingDir,
      executionInput: {
        prompt: executionInput.prompt,
        imagePaths: executionInput.imagePaths,
      },
      clearTaskOutput: true,
    });

    progress.clear(taskId);
    return outcome.status === "queued"
      ? { status: "queued", position: outcome.position, planContent }
      : { status: "started", planContent };
  } catch (error) {
    if (isImageSkipCancelled(error)) {
      progress.clear(taskId);
      return { status: "cancelled", planContent };
    }
    const message = error instanceof Error ? error.message : String(error);
    await reportTaskRunSessionError(error, assigneeId, task.id);
    progress.setPhase(taskId, "error", message);
    throw error instanceof Error ? error : new Error(message);
  }
}

/**
 * Start an existing kanban task without opening the log/detail dialogs.
 * Shares startTaskRunSession (provider / SSH / worktree / queue / image skip).
 */
export async function runExistingTaskInBackground(
  options: RunExistingTaskInBackgroundOptions,
): Promise<TaskBackgroundRunOutcome> {
  const taskId = options.task.id;
  const existing = inFlightBackgroundRuns.get(taskId);
  if (existing) {
    return existing;
  }

  const promise = runExistingTaskInBackgroundUntracked(options).finally(() => {
    if (inFlightBackgroundRuns.get(taskId) === promise) {
      inFlightBackgroundRuns.delete(taskId);
    }
  });
  inFlightBackgroundRuns.set(taskId, promise);
  return promise;
}
