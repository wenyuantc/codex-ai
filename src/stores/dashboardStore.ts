import { create } from "zustand";

import { getDashboardStats, listActivityLogs, type ListActivityLogsInput } from "@/lib/backend";
import { TASK_STATUSES, type ActivityLog, type EnvironmentMode } from "@/lib/types";
import { getActivityActionLabel, getStatusLabel } from "@/lib/utils";
import { getDateLocale, getLocalePreference } from "@/lib/i18n/locale";

interface DashboardStats {
  totalProjects: number;
  activeProjects: number;
  totalTasks: number;
  tasksByStatus: Record<string, number>;
  totalEmployees: number;
  onlineEmployees: number;
  completionRate: number;
  unreadNotifications: number;
  highSeverityNotifications: number;
}

interface ActivityPageResult {
  items: ActivityLog[];
  total: number;
  availableActions: string[];
}

export interface ActivityFilters {
  projectId?: string;
  action?: string;
  keyword?: string;
  startDate?: string;
  endDate?: string;
}

interface DashboardStore {
  stats: DashboardStats | null;
  recentActivities: ActivityLog[];
  loading: boolean;
  fetchStats: (
    environmentMode: EnvironmentMode,
    selectedSshConfigId?: string | null,
    projectId?: string,
  ) => Promise<void>;
  fetchRecentActivities: (
    environmentMode: EnvironmentMode,
    selectedSshConfigId?: string | null,
    limit?: number,
    projectId?: string,
  ) => Promise<void>;
  fetchActivitiesPage: (
    environmentMode: EnvironmentMode,
    selectedSshConfigId?: string | null,
    page?: number,
    pageSize?: number,
    filters?: ActivityFilters,
  ) => Promise<ActivityPageResult>;
}

function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").toLocaleLowerCase().trim();
}

function normalizeDateToTimestamp(date: string | undefined, endOfDay: boolean) {
  if (!date) {
    return null;
  }

  const normalized = endOfDay ? `${date}T23:59:59.999` : `${date}T00:00:00.000`;
  const parsed = new Date(normalized);

  return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
}

export function isInvalidDateRange(filters: ActivityFilters) {
  const startTimestamp = normalizeDateToTimestamp(filters.startDate, false);
  const endTimestamp = normalizeDateToTimestamp(filters.endDate, true);

  return startTimestamp !== null && endTimestamp !== null && startTimestamp > endTimestamp;
}

export function getKeywordMatchedActions(keyword: string, availableActions: string[]) {
  return availableActions.filter((action) =>
    normalizeSearchText(getActivityActionLabel(action)).includes(keyword),
  );
}

export function getKeywordMatchedStatuses(keyword: string) {
  return TASK_STATUSES.filter((status) =>
    normalizeSearchText(getStatusLabel(status.value)).includes(keyword),
  ).map((status) => status.value);
}

export function getAvailableActivityActions(actions: string[]) {
  return Array.from(new Set(actions)).sort((left, right) =>
    getActivityActionLabel(left).localeCompare(
      getActivityActionLabel(right),
      getDateLocale(getLocalePreference()),
    ),
  );
}

export function buildActivityScopeInput(
  environmentMode: EnvironmentMode,
  selectedSshConfigId?: string | null,
  filters: ActivityFilters = {},
): ListActivityLogsInput {
  return {
    environmentMode,
    selectedSshConfigId: selectedSshConfigId ?? null,
    projectId: filters.projectId ?? null,
    action: filters.action ?? null,
    keyword: filters.keyword ?? null,
    startDate: filters.startDate ?? null,
    endDate: filters.endDate ?? null,
  };
}

export const useDashboardStore = create<DashboardStore>((set) => ({
  stats: null,
  recentActivities: [],
  loading: false,

  fetchStats: async (environmentMode, selectedSshConfigId, projectId) => {
    set({ loading: true });
    try {
      const stats = await getDashboardStats({
        environmentMode,
        selectedSshConfigId,
        projectId,
      });

      set({
        stats: {
          totalProjects: stats.total_projects,
          activeProjects: stats.active_projects,
          totalTasks: stats.total_tasks,
          tasksByStatus: stats.tasks_by_status ?? {},
          totalEmployees: stats.total_employees,
          onlineEmployees: stats.online_employees,
          completionRate: stats.completion_rate,
          unreadNotifications: stats.unread_notifications,
          highSeverityNotifications: stats.high_severity_notifications,
        },
        loading: false,
      });
    } catch (error) {
      console.error("Failed to fetch dashboard stats:", error);
      set({ loading: false });
    }
  },

  fetchRecentActivities: async (environmentMode, selectedSshConfigId, limit = 20, projectId) => {
    try {
      const safeLimit = Math.max(1, limit);
      const page = await listActivityLogs({
        ...buildActivityScopeInput(environmentMode, selectedSshConfigId, { projectId }),
        limit: safeLimit,
        offset: 0,
      });

      set({ recentActivities: page.items });
    } catch (error) {
      console.error("Failed to fetch activities:", error);
      set({ recentActivities: [] });
    }
  },

  fetchActivitiesPage: async (
    environmentMode,
    selectedSshConfigId,
    page = 1,
    pageSize = 20,
    filters = {},
  ) => {
    const safePage = Math.max(1, page);
    const safePageSize = Math.max(1, pageSize);
    const offset = (safePage - 1) * safePageSize;
    const scope = buildActivityScopeInput(environmentMode, selectedSshConfigId, filters);

    // Scope-only pass for available actions (and keyword Chinese reverse-match).
    const scopePage = await listActivityLogs({
      environmentMode,
      selectedSshConfigId: selectedSshConfigId ?? null,
      projectId: filters.projectId ?? null,
      limit: 1,
      offset: 0,
      includeTotal: true,
    });
    const availableActions = getAvailableActivityActions(scopePage.available_actions);

    if (isInvalidDateRange(filters)) {
      return {
        items: [],
        total: 0,
        availableActions,
      };
    }

    const normalizedKeyword = normalizeSearchText(filters.keyword);
    const matchedActions = normalizedKeyword
      ? getKeywordMatchedActions(normalizedKeyword, availableActions)
      : [];
    const matchedStatuses = normalizedKeyword ? getKeywordMatchedStatuses(normalizedKeyword) : [];

    const result = await listActivityLogs({
      ...scope,
      limit: safePageSize,
      offset,
      includeTotal: true,
      matchedActions: matchedActions.length > 0 ? matchedActions : null,
      matchedStatuses: matchedStatuses.length > 0 ? matchedStatuses : null,
    });

    return {
      items: result.items,
      total: result.total,
      availableActions: getAvailableActivityActions(
        result.available_actions.length > 0 ? result.available_actions : availableActions,
      ),
    };
  },
}));
