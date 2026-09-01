import {
  generateCoordinatorPlanForTask,
  type CoordinatorPlanGenerateAdapters,
} from "@/lib/coordinatorPlanSession";
import { formatEmployeeRuntimeLabel, type Task } from "@/lib/types";
import { useCoordinatorPlanStore } from "@/stores/coordinatorPlanStore";
import { useEmployeeStore } from "@/stores/employeeStore";

export async function generateAndPersistCoordinatorPlan(
  params: {
    task: Task;
    coordinatorId: string;
    workingDir: string | null;
    coordinatorName?: string | null;
    runtimeLabel?: string;
  },
  adapters?: CoordinatorPlanGenerateAdapters,
): Promise<string> {
  const coordinator =
    useEmployeeStore.getState().employees.find((item) => item.id === params.coordinatorId) ?? null;
  const plan = await generateCoordinatorPlanForTask(
    {
      taskId: params.task.id,
      coordinatorId: params.coordinatorId,
      coordinatorName: params.coordinatorName ?? coordinator?.name,
      runtimeLabel: params.runtimeLabel ?? formatEmployeeRuntimeLabel(coordinator),
      title: params.task.title,
      description: params.task.description,
      status: params.task.status,
      priority: params.task.priority,
      workingDir: params.workingDir,
    },
    adapters,
  );
  if (!plan?.trim()) {
    const message =
      useCoordinatorPlanStore.getState().byTaskId[params.task.id]?.error ??
      "协调员未返回可用计划。";
    throw new Error(message);
  }
  return plan;
}
