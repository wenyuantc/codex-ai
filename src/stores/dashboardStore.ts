import { create } from "zustand";

import { select } from "@/lib/database";
import { filterProjectsByScope, normalizeProject } from "@/lib/projects";
import type { ActivityLog, EnvironmentMode, Project, Task } from "@/lib/types";
import { getActivityActionLabel, getActivityDetailsLabel, parseDateValue } from "@/lib/utils";

const SSH_GLOBAL_ACTIVITY_ACTIONS = new Set([
  "environment_mode_switched",
  "ssh_host_selected",
  "ssh_config_created",
  "ssh_config_updated",
  "ssh_config_deleted",
  "remote_codex_validated",
  "remote_codex_verified",
  "remote_sdk_installed",
  "git_runtime_installed",
  "git_runtime_install_failed",
  "remote_git_runtime_installed",
  "remote_git_runtime_install_failed",
  "remote_session_artifact_captured",
  "remote_session_artifact_limited",
  "remote_artifact_capture_limited",
  "global_search_navigated",
  "notification_created",
  "notification_resolved",
]);

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

interface EmployeeLookup {
  id: string;
  project_id: string | null;
  status: string;
}

interface NotificationLookup {
  severity: string;
  is_read: number;
}

interface DashboardStore {
  stats: DashboardStats | null;
  recentActivities: ActivityLog[];
  loading: boolean;
  fetchStats: (environmentMode: EnvironmentMode, selectedSshConfigId?: string | null, projectId?: string) => Promise<void>;
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

const ACTIVITY_SELECT = `SELECT a.*, e.name as employee_name, p.name as project_name
  FROM activity_logs a
  LEFT JOIN employees e ON a.employee_id = e.id
  LEFT JOIN projects p ON a.project_id = p.id
  ORDER BY a.created_at DESC, a.id DESC`;

async function loadProjects() {
  const rows = await select<Project>("SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY updated_at DESC");
  return rows.map((project) => normalizeProject(project));
}

async function loadActivities() {
  return select<ActivityLog>(ACTIVITY_SELECT);
}

function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").toLocaleLowerCase().trim();
}

function createDayBoundary(date: string | undefined, endOfDay: boolean) {
  if (!date) {
    return null;
  }

  const normalized = endOfDay ? `${date}T23:59:59.999` : `${date}T00:00:00.000`;
  const parsed = new Date(normalized);

  return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
}

function buildActivitySearchText(activity: ActivityLog) {
  return [
    getActivityActionLabel(activity.action),
    activity.action,
    getActivityDetailsLabel(activity.action, activity.details),
    activity.details,
    activity.project_name,
    activity.employee_name,
  ]
    .map((value) => normalizeSearchText(value))
    .filter(Boolean)
    .join("\n");
}

function filterActivitiesByProject(activities: ActivityLog[], projectId?: string) {
  if (!projectId) {
    return activities;
  }

  return activities.filter((activity) => activity.project_id === projectId);
}

function filterActivitiesByCriteria(activities: ActivityLog[], filters: ActivityFilters) {
  const normalizedKeyword = normalizeSearchText(filters.keyword);
  const startTimestamp = createDayBoundary(filters.startDate, false);
  const endTimestamp = createDayBoundary(filters.endDate, true);

  if (
    startTimestamp !== null
    && endTimestamp !== null
    && startTimestamp > endTimestamp
  ) {
    return [];
  }

  return activities.filter((activity) => {
    if (filters.action && activity.action !== filters.action) {
      return false;
    }

    if (normalizedKeyword && !buildActivitySearchText(activity).includes(normalizedKeyword)) {
      return false;
    }

    if (startTimestamp === null && endTimestamp === null) {
      return true;
    }

    const activityTimestamp = parseDateValue(activity.created_at)?.getTime();
    if (activityTimestamp === undefined) {
      return false;
    }

    if (startTimestamp !== null && activityTimestamp < startTimestamp) {
      return false;
    }

    if (endTimestamp !== null && activityTimestamp > endTimestamp) {
      return false;
    }

    return true;
  });
}

function getAvailableActivityActions(activities: ActivityLog[]) {
  return Array.from(new Set(activities.map((activity) => activity.action))).sort((left, right) => (
    getActivityActionLabel(left).localeCompare(getActivityActionLabel(right), "zh-CN")
  ));
}

function matchesEnvironmentActivity(
  activity: ActivityLog,
  visibleProjectIds: Set<string>,
  environmentMode: EnvironmentMode,
) {
  if (activity.project_id) {
    return visibleProjectIds.has(activity.project_id);
  }

  if (environmentMode === "ssh") {
    return SSH_GLOBAL_ACTIVITY_ACTIONS.has(activity.action);
  }

  return true;
}

async function loadScopedActivities(
  environmentMode: EnvironmentMode,
  selectedSshConfigId?: string | null,
) {
  const projects = await loadProjects();
  const visibleProjectIds = new Set(
    filterProjectsByScope(projects, environmentMode, selectedSshConfigId).map((project) => project.id),
  );

  return (await loadActivities()).filter((activity) => (
    matchesEnvironmentActivity(activity, visibleProjectIds, environmentMode)
  ));
}

export const useDashboardStore = create<DashboardStore>((set) => ({
  stats: null,
  recentActivities: [],
  loading: false,

  fetchStats: async (environmentMode, selectedSshConfigId, projectId) => {
    set({ loading: true });
    try {
      const [projects, tasks, employees, notifications] = await Promise.all([
        loadProjects(),
        select<Task>("SELECT * FROM tasks WHERE deleted_at IS NULL ORDER BY updated_at DESC"),
        select<EmployeeLookup>("SELECT id, project_id, status FROM employees"),
        select<NotificationLookup>(
          "SELECT severity, is_read FROM notifications WHERE state = 'active' ORDER BY last_triggered_at DESC",
        ),
      ]);

      const visibleProjects = filterProjectsByScope(
        projects,
        environmentMode,
        selectedSshConfigId,
      );
      const visibleProjectIds = new Set(visibleProjects.map((project) => project.id));
      const scopedProjectIds = projectId && visibleProjectIds.has(projectId)
        ? new Set([projectId])
        : visibleProjectIds;

      const filteredTasks = tasks.filter((task) => scopedProjectIds.has(task.project_id));
      const filteredEmployees = employees.filter((employee) => (
        employee.project_id ? scopedProjectIds.has(employee.project_id) : !projectId
      ));

      const tasksByStatus: Record<string, number> = {};
      let completed = 0;
      for (const task of filteredTasks) {
        tasksByStatus[task.status] = (tasksByStatus[task.status] ?? 0) + 1;
        if (task.status === "completed") {
          completed += 1;
        }
      }

      const scopedProjects = visibleProjects.filter((project) => scopedProjectIds.has(project.id));
      const onlineEmployees = filteredEmployees.filter((employee) => employee.status === "online" || employee.status === "busy");
      const unreadNotifications = notifications.filter((notification) => notification.is_read === 0).length;
      const highSeverityNotifications = notifications.filter((notification) => (
        notification.severity === "error" || notification.severity === "critical"
      )).length;

      set({
        stats: {
          totalProjects: scopedProjects.length,
          activeProjects: scopedProjects.filter((project) => project.status === "active").length,
          totalTasks: filteredTasks.length,
          tasksByStatus,
          totalEmployees: filteredEmployees.length,
          onlineEmployees: onlineEmployees.length,
          completionRate: filteredTasks.length > 0 ? Math.round((completed / filteredTasks.length) * 100) : 0,
          unreadNotifications,
          highSeverityNotifications,
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
      const activities = filterActivitiesByProject(
        await loadScopedActivities(environmentMode, selectedSshConfigId),
        projectId,
      );

      set({ recentActivities: activities.slice(0, limit) });
    } catch (error) {
      console.error("Failed to fetch activities:", error);
      set({ recentActivities: [] });
    }
  },

  fetchActivitiesPage: async (environmentMode, selectedSshConfigId, page = 1, pageSize = 20, filters = {}) => {
    const safePage = Math.max(1, page);
    const safePageSize = Math.max(1, pageSize);
    const offset = (safePage - 1) * safePageSize;
    const scopedActivities = await loadScopedActivities(environmentMode, selectedSshConfigId);
    const projectScopedActivities = filterActivitiesByProject(scopedActivities, filters.projectId);
    const filteredItems = filterActivitiesByCriteria(projectScopedActivities, filters);

    return {
      items: filteredItems.slice(offset, offset + safePageSize),
      total: filteredItems.length,
      availableActions: getAvailableActivityActions(projectScopedActivities),
    };
  },
}));
