import { create } from "zustand";

/** Background phases after create-and-run closes the dialog. */
export type TaskBackgroundRunPhase = "planning" | "starting" | "error";

export interface TaskBackgroundRunEntry {
  phase: TaskBackgroundRunPhase;
  error?: string;
}

interface TaskBackgroundRunStore {
  byTaskId: Record<string, TaskBackgroundRunEntry>;
  setPhase: (taskId: string, phase: TaskBackgroundRunPhase, error?: string) => void;
  clear: (taskId: string) => void;
}

export const useTaskBackgroundRunStore = create<TaskBackgroundRunStore>((set) => ({
  byTaskId: {},
  setPhase: (taskId, phase, error) =>
    set((state) => ({
      byTaskId: {
        ...state.byTaskId,
        [taskId]: {
          phase,
          error: phase === "error" ? (error ?? state.byTaskId[taskId]?.error) : undefined,
        },
      },
    })),
  clear: (taskId) =>
    set((state) => {
      if (!(taskId in state.byTaskId)) {
        return state;
      }
      const next = { ...state.byTaskId };
      delete next[taskId];
      return { byTaskId: next };
    }),
}));

export function getTaskBackgroundRunLabel(
  entry: TaskBackgroundRunEntry | undefined,
): string | null {
  if (!entry) {
    return null;
  }
  switch (entry.phase) {
    case "planning":
      return "协调员生成计划中";
    case "starting":
      return "启动执行中";
    case "error":
      return entry.error?.trim() ? `后台启动失败` : "后台启动失败";
    default:
      return null;
  }
}
