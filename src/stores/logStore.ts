import { create } from "zustand";
import { listActivityLogs } from "@/lib/backend";
import type { ActivityLog } from "@/lib/types";

interface LogStore {
  logs: ActivityLog[];
  loading: boolean;
  fetchLogs: (limit?: number) => Promise<void>;
}

export const useLogStore = create<LogStore>((set) => ({
  logs: [],
  loading: false,

  fetchLogs: async (limit = 50) => {
    set({ loading: true });
    try {
      const page = await listActivityLogs({ limit });
      set({ logs: page.items, loading: false });
    } catch (e) {
      console.error("Failed to fetch logs:", e);
      set({ loading: false });
    }
  },
}));
