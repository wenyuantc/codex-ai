import {
  aiGenerateCoordinatorTaskPlan,
  withCoordinatorPlanLogStream,
  type CoordinatorTaskPlanResult,
  type GenerateCoordinatorTaskPlanInput,
} from "@/lib/backend";
import { formatPlanUsageLogLine } from "@/lib/types";
import {
  getCoordinatorPlanSession,
  useCoordinatorPlanStore,
  type CoordinatorPlanSession,
} from "@/stores/coordinatorPlanStore";
import { useTaskStore } from "@/stores/taskStore";

const inFlight = new Map<string, Promise<string>>();

export function isCoordinatorPlanInFlight(taskId: string): boolean {
  return inFlight.has(taskId);
}

export function resetCoordinatorPlanInFlightForTests(): void {
  inFlight.clear();
}

export async function runExclusiveCoordinatorPlanGenerate(
  taskId: string,
  run: () => Promise<string>,
): Promise<string> {
  const existing = inFlight.get(taskId);
  if (existing) {
    return existing;
  }
  const promise = run().finally(() => {
    if (inFlight.get(taskId) === promise) {
      inFlight.delete(taskId);
    }
  });
  inFlight.set(taskId, promise);
  return promise;
}

export function shouldAutoGenerateCoordinatorPlan(input: {
  autoGenerate: boolean;
  savedPlan?: string | null;
  loading: boolean;
  inFlight: boolean;
}): boolean {
  if (!input.autoGenerate) return false;
  if (input.loading || input.inFlight) return false;
  return !input.savedPlan?.trim();
}

export function shouldHydrateCoordinatorPlanOnOpen(
  session: CoordinatorPlanSession | undefined,
): boolean {
  if (!session) return true;
  if (session.loading) return false;
  if (session.draft.trim() || session.logs.length > 0 || session.error) return false;
  return true;
}

export function prepareCoordinatorPlanOpen(
  taskId: string,
  savedPlan: string,
  autoGenerate: boolean,
): { shouldGenerate: boolean } {
  const store = useCoordinatorPlanStore.getState();
  const session = store.byTaskId[taskId];
  const inFlightNow = isCoordinatorPlanInFlight(taskId);
  if (session?.loading) {
    store.patch(taskId, { terminalVisible: true });
    return { shouldGenerate: false };
  }
  if (inFlightNow) {
    store.patch(taskId, { terminalVisible: true });
    // Run can join the in-flight IPC; merely opening the viewer must not start another one.
    return { shouldGenerate: autoGenerate };
  }
  if (shouldHydrateCoordinatorPlanOnOpen(session)) {
    store.hydrateFromSavedPlan(taskId, savedPlan);
  }
  const next = getCoordinatorPlanSession(useCoordinatorPlanStore.getState().byTaskId[taskId]);
  return {
    shouldGenerate: shouldAutoGenerateCoordinatorPlan({
      autoGenerate,
      savedPlan: savedPlan.trim() || next.draft,
      loading: false,
      inFlight: false,
    }),
  };
}

export interface CoordinatorPlanGenerateParams {
  taskId: string;
  coordinatorId: string | null | undefined;
  coordinatorName?: string | null;
  runtimeLabel: string;
  title: string;
  description?: string | null;
  status: string;
  priority: string;
  workingDir: string | null;
}

export interface CoordinatorPlanGenerateAdapters {
  generatePlan: (input: GenerateCoordinatorTaskPlanInput) => Promise<CoordinatorTaskPlanResult>;
  withLogStream: typeof withCoordinatorPlanLogStream;
  refreshTasks?: () => Promise<void>;
}

async function defaultRefreshTasks(): Promise<void> {
  const taskStore = useTaskStore.getState();
  await taskStore.fetchTasks(taskStore.activeProjectId);
}

export async function generateCoordinatorPlanForTask(
  params: CoordinatorPlanGenerateParams,
  adapters: CoordinatorPlanGenerateAdapters = {
    generatePlan: aiGenerateCoordinatorTaskPlan,
    withLogStream: withCoordinatorPlanLogStream,
    refreshTasks: defaultRefreshTasks,
  },
): Promise<string | null> {
  const store = useCoordinatorPlanStore.getState();
  const append = (line: string) => store.appendLog(params.taskId, line);
  const existing = inFlight.get(params.taskId);
  if (existing) {
    store.patch(params.taskId, { loading: true, terminalVisible: true, error: null });
    try {
      const plan = await existing;
      store.patch(params.taskId, { draft: plan, loading: false, error: null });
      return plan;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      store.patch(params.taskId, { loading: false, error: message });
      return null;
    }
  }

  if (!params.coordinatorId) {
    store.patch(params.taskId, { terminalVisible: true, error: "请先指定协调员。" });
    append("[ERROR] 当前任务未指定协调员，无法生成计划。");
    return null;
  }

  return runExclusiveCoordinatorPlanGenerate(params.taskId, async () => {
    const live = useCoordinatorPlanStore.getState();
    live.patch(params.taskId, { loading: true, terminalVisible: true, error: null });
    append(`[计划] 准备调用协调员：${params.coordinatorName ?? params.coordinatorId}`);
    append(`[计划] 运行配置：${params.runtimeLabel}`);
    append(`[计划] 工作目录：${params.workingDir ?? "未配置"}`);
    append("[计划] 正在生成协调员执行计划，可能需要一点时间...");
    try {
      const plan = await adapters.withLogStream(append, (requestId) =>
        adapters.generatePlan({
          task_id: params.taskId,
          coordinator_id: params.coordinatorId as string,
          title: params.title,
          description: params.description,
          status: params.status,
          priority: params.priority,
          working_dir: params.workingDir,
          request_id: requestId,
        }),
      );
      const trimmedPlan = plan.markdown.trim();
      if (!trimmedPlan) {
        append("[WARN] 协调员返回了空计划。");
        live.patch(params.taskId, { loading: false, error: "协调员未返回可用计划。" });
        throw new Error("协调员未返回可用计划。");
      }
      const usageLog = formatPlanUsageLogLine(plan.usage_line);
      if (usageLog) {
        append(usageLog);
      }
      append(`[计划] 已收到协调员计划，共 ${trimmedPlan.length} 字。`);
      append("[计划] 结构化工作包已落库，可在本弹窗「按计划编排」。");
      live.patch(params.taskId, { draft: trimmedPlan, loading: false, error: null });
      await adapters.refreshTasks?.();
      return trimmedPlan;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const current = useCoordinatorPlanStore.getState().byTaskId[params.taskId];
      if (current?.error !== message) {
        append(`[ERROR] ${message}`);
      }
      useCoordinatorPlanStore.getState().patch(params.taskId, { loading: false, error: message });
      throw error instanceof Error ? error : new Error(message);
    }
  }).catch(() => null);
}
