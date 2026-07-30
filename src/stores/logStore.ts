import { create } from "zustand";
import { select } from "@/lib/database";
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
      const logs = await select<ActivityLog>(
        "SELECT * FROM activity_logs ORDER BY created_at DESC LIMIT $1",
        [limit],
      );
      set({ logs, loading: false });
    } catch (e) {
      console.error("Failed to fetch logs:", e);
      set({ loading: false });
    }
  },
}));
