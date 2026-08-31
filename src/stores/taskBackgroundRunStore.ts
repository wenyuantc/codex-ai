import { create } from "zustand";

import i18n from "@/lib/i18n";

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
      return i18n.t("tasks:card.backgroundPlanning");
    case "starting":
      return i18n.t("tasks:card.backgroundStarting");
    case "error":
      return i18n.t("tasks:card.backgroundFailed");
    default:
      return null;
  }
}
