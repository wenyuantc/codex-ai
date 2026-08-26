import { create } from "zustand";

/** Task-scoped coordinator plan UI session. Survives dialog close/unmount. */
export interface CoordinatorPlanSession {
  loading: boolean;
  draft: string;
  logs: string[];
  error: string | null;
  terminalVisible: boolean;
}

export const EMPTY_COORDINATOR_PLAN_SESSION: CoordinatorPlanSession = {
  loading: false,
  draft: "",
  logs: [],
  error: null,
  terminalVisible: false,
};

const MAX_LOGS = 200;

interface CoordinatorPlanStore {
  byTaskId: Record<string, CoordinatorPlanSession>;
  patch: (taskId: string, partial: Partial<CoordinatorPlanSession>) => void;
  hydrateFromSavedPlan: (taskId: string, savedPlan: string) => void;
  appendLog: (taskId: string, line: string) => void;
  clearLogs: (taskId: string) => void;
  resetForTests: () => void;
}

function sessionOrEmpty(
  byTaskId: Record<string, CoordinatorPlanSession>,
  taskId: string,
): CoordinatorPlanSession {
  return byTaskId[taskId] ?? EMPTY_COORDINATOR_PLAN_SESSION;
}

export const useCoordinatorPlanStore = create<CoordinatorPlanStore>((set) => ({
  byTaskId: {},
  patch: (taskId, partial) =>
    set((state) => ({
      byTaskId: {
        ...state.byTaskId,
        [taskId]: {
          ...sessionOrEmpty(state.byTaskId, taskId),
          ...partial,
        },
      },
    })),
  hydrateFromSavedPlan: (taskId, savedPlan) => {
    const trimmed = savedPlan.trim();
    set((state) => ({
      byTaskId: {
        ...state.byTaskId,
        [taskId]: {
          loading: false,
          draft: trimmed,
          error: null,
          terminalVisible: !trimmed,
          logs: trimmed ? [`[计划] 已加载任务中保存的协调员计划，共 ${trimmed.length} 字。`] : [],
        },
      },
    }));
  },
  appendLog: (taskId, line) =>
    set((state) => {
      const current = sessionOrEmpty(state.byTaskId, taskId);
      return {
        byTaskId: {
          ...state.byTaskId,
          [taskId]: {
            ...current,
            logs: [...current.logs.slice(-(MAX_LOGS - 1)), line],
          },
        },
      };
    }),
  clearLogs: (taskId) =>
    set((state) => ({
      byTaskId: {
        ...state.byTaskId,
        [taskId]: {
          ...sessionOrEmpty(state.byTaskId, taskId),
          logs: [],
        },
      },
    })),
  resetForTests: () => set({ byTaskId: {} }),
}));

export function getCoordinatorPlanSession(
  entry: CoordinatorPlanSession | undefined,
): CoordinatorPlanSession {
  return entry ?? EMPTY_COORDINATOR_PLAN_SESSION;
}
