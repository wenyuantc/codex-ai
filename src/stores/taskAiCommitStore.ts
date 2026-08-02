import { create } from "zustand";

/** Client-visible AI commit progress on kanban task cards. */
export type TaskAiCommitPhase =
  | "committing"
  | "resolving_conflicts"
  | "success"
  | "error";

export interface TaskAiCommitEntry {
  phase: TaskAiCommitPhase;
  detail?: string;
  error?: string;
  updatedAt: number;
}

interface TaskAiCommitStore {
  byTaskId: Record<string, TaskAiCommitEntry>;
  setPhase: (
    taskId: string,
    phase: TaskAiCommitPhase,
    options?: { detail?: string; error?: string },
  ) => void;
  clear: (taskId: string) => void;
}

export const useTaskAiCommitStore = create<TaskAiCommitStore>((set) => ({
  byTaskId: {},
  setPhase: (taskId, phase, options) =>
    set((state) => ({
      byTaskId: {
        ...state.byTaskId,
        [taskId]: {
          phase,
          detail: options?.detail,
          error:
            phase === "error"
              ? (options?.error ?? state.byTaskId[taskId]?.error)
              : undefined,
          updatedAt: Date.now(),
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

export function getTaskAiCommitLabel(entry: TaskAiCommitEntry | undefined): string | null {
  if (!entry) {
    return null;
  }
  switch (entry.phase) {
    case "committing":
      return "AI 提交中";
    case "resolving_conflicts":
      return "AI 解冲突中";
    case "success":
      return entry.detail?.trim() ? "AI 提交完成" : "AI 提交完成";
    case "error":
      return "AI 提交失败";
    default:
      return null;
  }
}

export function isTaskAiCommitBusy(entry: TaskAiCommitEntry | undefined): boolean {
  return entry?.phase === "committing" || entry?.phase === "resolving_conflicts";
}
