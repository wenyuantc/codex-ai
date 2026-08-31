import { aiGenerateCoordinatorTaskPlan } from "@/lib/backend";
import { runExclusiveCoordinatorPlanGenerate } from "@/lib/coordinatorPlanSession";
import type { Task } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";

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
