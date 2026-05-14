import { create } from "zustand";

import { select } from "@/lib/database";
import { filterProjectsByScope, normalizeProject } from "@/lib/projects";
import { TASK_STATUSES, type ActivityLog, type EnvironmentMode, type Project, type Task } from "@/lib/types";
import { getActivityActionLabel, getStatusLabel } from "@/lib/utils";

const GLOBAL_ACTIVITY_PREFIXES = [
  "environment_",
  "global_",
  "notification_",
  "opencode_",
  "remote_",
  "ssh_",
] as const;
const GLOBAL_ACTIVITY_ACTIONS = new Set([
  "employee_project_membership_conflict_migrated",
  "git_runtime_installed",
  "git_runtime_install_failed",
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

interface CountRow {
  total: number;
}

interface ActivityActionRow {
  action: string;
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

const ACTIVITY_FROM = `FROM activity_logs a
  LEFT JOIN employees e ON a.employee_id = e.id
  LEFT JOIN projects p ON a.project_id = p.id`;

const ACTIVITY_SELECT = `SELECT a.*, e.name as employee_name, p.name as project_name
  ${ACTIVITY_FROM}`;

const ACTIVITY_ORDER = "ORDER BY a.created_at DESC, a.id DESC";

let pendingProjectsLoad: Promise<Project[]> | null = null;

async function loadProjects() {
  pendingProjectsLoad ??= select<Project>("SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY updated_at DESC")
    .then((rows) => rows.map((project) => normalizeProject(project)))
    .finally(() => {
      pendingProjectsLoad = null;
    });

  return pendingProjectsLoad;
}

function createPlaceholders(count: number) {
  return Array.from({ length: count }, () => "?").join(", ");
}

function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").toLocaleLowerCase().trim();
}

function escapeSqlLike(value: string) {
  return value.replace(/[\\%_]/g, "\\$&");
}

function normalizeDateToTimestamp(date: string | undefined, endOfDay: boolean) {
  if (!date) {
    return null;
  }

  const normalized = endOfDay ? `${date}T23:59:59.999` : `${date}T00:00:00.000`;
  const parsed = new Date(normalized);

  return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
}

function isInvalidDateRange(filters: ActivityFilters) {
  const startTimestamp = normalizeDateToTimestamp(filters.startDate, false);
  const endTimestamp = normalizeDateToTimestamp(filters.endDate, true);

  return (
    startTimestamp !== null
    && endTimestamp !== null
    && startTimestamp > endTimestamp
  );
}

function appendGlobalActivityCondition(params: unknown[], tableAlias = "a") {
  const prefixConditions = GLOBAL_ACTIVITY_PREFIXES.map(() => `${tableAlias}.action LIKE ? ESCAPE '\\'`);
  params.push(...GLOBAL_ACTIVITY_PREFIXES.map((prefix) => `${escapeSqlLike(prefix)}%`));

  if (GLOBAL_ACTIVITY_ACTIONS.size === 0) {
    return `(${prefixConditions.join(" OR ")})`;
  }

  const actions = Array.from(GLOBAL_ACTIVITY_ACTIONS);
  params.push(...actions);

  return `(${prefixConditions.join(" OR ")} OR ${tableAlias}.action IN (${createPlaceholders(actions.length)}))`;
}

async function loadVisibleProjectIds(
  environmentMode: EnvironmentMode,
  selectedSshConfigId?: string | null,
) {
  const projects = await loadProjects();
  return new Set(
    filterProjectsByScope(projects, environmentMode, selectedSshConfigId).map((project) => project.id),
  );
}

function appendActivityScopeConditions(
  where: string[],
  params: unknown[],
  visibleProjectIds: Set<string>,
  environmentMode: EnvironmentMode,
  projectId?: string,
) {
  if (projectId) {
    if (!visibleProjectIds.has(projectId)) {
      where.push("1 = 0");
      return;
    }

    where.push("a.project_id = ?");
    params.push(projectId);
    return;
  }

  const scopeConditions: string[] = [];
  const projectIds = Array.from(visibleProjectIds);
  if (projectIds.length > 0) {
    scopeConditions.push(`a.project_id IN (${createPlaceholders(projectIds.length)})`);
    params.push(...projectIds);
  }

  if (environmentMode === "ssh") {
    scopeConditions.push(`(a.project_id IS NULL AND ${appendGlobalActivityCondition(params)})`);
  } else {
    scopeConditions.push("a.project_id IS NULL");
  }

  where.push(`(${scopeConditions.join(" OR ")})`);
}

function appendLikeCondition(
  conditions: string[],
  params: unknown[],
  expression: string,
  pattern: string,
) {
  conditions.push(`LOWER(${expression}) LIKE ? ESCAPE '\\'`);
  params.push(pattern);
}

function getKeywordMatchedActions(keyword: string, availableActions: string[]) {
  return availableActions.filter((action) => (
    normalizeSearchText(getActivityActionLabel(action)).includes(keyword)
  ));
}

function getKeywordMatchedStatuses(keyword: string) {
  return TASK_STATUSES
    .filter((status) => normalizeSearchText(getStatusLabel(status.value)).includes(keyword))
    .map((status) => status.value);
}

function appendActivityFilterConditions(
  where: string[],
  params: unknown[],
  filters: ActivityFilters,
  availableActions: string[] = [],
) {
  if (filters.action) {
    where.push("a.action = ?");
    params.push(filters.action);
  }

  const normalizedKeyword = normalizeSearchText(filters.keyword);
  if (normalizedKeyword) {
    const pattern = `%${escapeSqlLike(normalizedKeyword)}%`;
    const keywordConditions: string[] = [];

    appendLikeCondition(keywordConditions, params, "a.action", pattern);
    appendLikeCondition(keywordConditions, params, "COALESCE(a.details, '')", pattern);
    appendLikeCondition(keywordConditions, params, "COALESCE(p.name, '')", pattern);
    appendLikeCondition(keywordConditions, params, "COALESCE(e.name, '')", pattern);

    const matchedActions = getKeywordMatchedActions(normalizedKeyword, availableActions);
    if (matchedActions.length > 0) {
      keywordConditions.push(`a.action IN (${createPlaceholders(matchedActions.length)})`);
      params.push(...matchedActions);
    }

    const matchedStatuses = getKeywordMatchedStatuses(normalizedKeyword);
    for (const status of matchedStatuses) {
      keywordConditions.push(`a.details LIKE ? ESCAPE '\\'`);
      params.push(`%${escapeSqlLike(status)}%`);
    }

    where.push(`(${keywordConditions.join(" OR ")})`);
  }

  const startTimestamp = normalizeDateToTimestamp(filters.startDate, false);
  if (startTimestamp !== null) {
    where.push("CAST(strftime('%s', a.created_at) AS INTEGER) >= ?");
    params.push(Math.floor(startTimestamp / 1000));
  }

  const endTimestamp = normalizeDateToTimestamp(filters.endDate, true);
  if (endTimestamp !== null) {
    where.push("CAST(strftime('%s', a.created_at) AS INTEGER) <= ?");
    params.push(Math.floor(endTimestamp / 1000));
  }
}

function buildActivityWhere(
  visibleProjectIds: Set<string>,
  environmentMode: EnvironmentMode,
  filters: ActivityFilters = {},
  availableActions: string[] = [],
) {
  const where: string[] = [];
  const params: unknown[] = [];

  appendActivityScopeConditions(where, params, visibleProjectIds, environmentMode, filters.projectId);
  appendActivityFilterConditions(where, params, filters, availableActions);

  return {
    sql: `WHERE ${where.join(" AND ")}`,
    params,
  };
}

async function loadAvailableActivityActions(
  visibleProjectIds: Set<string>,
  environmentMode: EnvironmentMode,
  projectId?: string,
) {
  const where = buildActivityWhere(visibleProjectIds, environmentMode, { projectId });
  const rows = await select<ActivityActionRow>(
    `SELECT DISTINCT a.action
     ${ACTIVITY_FROM}
     ${where.sql}`,
    where.params,
  );

  return getAvailableActivityActions(rows.map((row) => row.action));
}

async function loadActivityItems(
  visibleProjectIds: Set<string>,
  environmentMode: EnvironmentMode,
  filters: ActivityFilters,
  limit: number,
  offset = 0,
  availableActions: string[] = [],
) {
  const where = buildActivityWhere(visibleProjectIds, environmentMode, filters, availableActions);
  return select<ActivityLog>(
    `${ACTIVITY_SELECT}
     ${where.sql}
     ${ACTIVITY_ORDER}
     LIMIT ? OFFSET ?`,
    [...where.params, limit, offset],
  );
}

async function countActivityItems(
  visibleProjectIds: Set<string>,
  environmentMode: EnvironmentMode,
  filters: ActivityFilters,
  availableActions: string[],
) {
  const where = buildActivityWhere(visibleProjectIds, environmentMode, filters, availableActions);
  const [row] = await select<CountRow>(
    `SELECT COUNT(*) as total
     ${ACTIVITY_FROM}
     ${where.sql}`,
    where.params,
  );

  return row?.total ?? 0;
}

function getAvailableActivityActions(actions: string[]) {
  return Array.from(new Set(actions)).sort((left, right) => (
    getActivityActionLabel(left).localeCompare(getActivityActionLabel(right), "zh-CN")
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
      const safeLimit = Math.max(1, limit);
      const visibleProjectIds = await loadVisibleProjectIds(environmentMode, selectedSshConfigId);
      const activities = await loadActivityItems(
        visibleProjectIds,
        environmentMode,
        { projectId },
        safeLimit,
      );

      set({ recentActivities: activities });
    } catch (error) {
      console.error("Failed to fetch activities:", error);
      set({ recentActivities: [] });
    }
  },

  fetchActivitiesPage: async (environmentMode, selectedSshConfigId, page = 1, pageSize = 20, filters = {}) => {
    const safePage = Math.max(1, page);
    const safePageSize = Math.max(1, pageSize);
    const offset = (safePage - 1) * safePageSize;
    const visibleProjectIds = await loadVisibleProjectIds(environmentMode, selectedSshConfigId);
    const availableActions = await loadAvailableActivityActions(visibleProjectIds, environmentMode, filters.projectId);

    if (isInvalidDateRange(filters)) {
      return {
        items: [],
        total: 0,
        availableActions,
      };
    }

    const [items, total] = await Promise.all([
      loadActivityItems(visibleProjectIds, environmentMode, filters, safePageSize, offset, availableActions),
      countActivityItems(visibleProjectIds, environmentMode, filters, availableActions),
    ]);

    return {
      items,
      total,
      availableActions,
    };
  },
}));
