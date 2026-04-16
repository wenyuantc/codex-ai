import { create } from "zustand";

import { select } from "@/lib/database";
import { normalizeProject, projectMatchesEnvironment } from "@/lib/projects";
import type { ActivityLog, EnvironmentMode, Project, Task } from "@/lib/types";

interface DashboardStats {
  totalProjects: number;
  activeProjects: number;
  totalTasks: number;
  tasksByStatus: Record<string, number>;
  totalEmployees: number;
  onlineEmployees: number;
  completionRate: number;
}

interface ActivityPageResult {
  items: ActivityLog[];
  total: number;
}

interface EmployeeLookup {
  id: string;
  project_id: string | null;
  status: string;
}

interface DashboardStore {
  stats: DashboardStats | null;
  recentActivities: ActivityLog[];
  loading: boolean;
  fetchStats: (environmentMode: EnvironmentMode, projectId?: string) => Promise<void>;
  fetchRecentActivities: (environmentMode: EnvironmentMode, limit?: number, projectId?: string) => Promise<void>;
  fetchActivitiesPage: (
    environmentMode: EnvironmentMode,
    page?: number,
    pageSize?: number,
    projectId?: string,
  ) => Promise<ActivityPageResult>;
}

const ACTIVITY_SELECT = `SELECT a.*, e.name as employee_name
  FROM activity_logs a
  LEFT JOIN employees e ON a.employee_id = e.id
  ORDER BY a.created_at DESC, a.id DESC`;

async function loadProjects() {
  const rows = await select<Project>("SELECT * FROM projects ORDER BY updated_at DESC");
  return rows.map((project) => normalizeProject(project));
}

async function loadActivities() {
  return select<ActivityLog>(ACTIVITY_SELECT);
}

export const useDashboardStore = create<DashboardStore>((set) => ({
  stats: null,
  recentActivities: [],
  loading: false,

  fetchStats: async (environmentMode, projectId) => {
    set({ loading: true });
    try {
      const [projects, tasks, employees] = await Promise.all([
        loadProjects(),
        select<Task>("SELECT * FROM tasks ORDER BY updated_at DESC"),
        select<EmployeeLookup>("SELECT id, project_id, status FROM employees"),
      ]);

      const visibleProjects = projects.filter((project) => projectMatchesEnvironment(project, environmentMode));
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

      set({
        stats: {
          totalProjects: scopedProjects.length,
          activeProjects: scopedProjects.filter((project) => project.status === "active").length,
          totalTasks: filteredTasks.length,
          tasksByStatus,
          totalEmployees: filteredEmployees.length,
          onlineEmployees: onlineEmployees.length,
          completionRate: filteredTasks.length > 0 ? Math.round((completed / filteredTasks.length) * 100) : 0,
        },
        loading: false,
      });
    } catch (error) {
      console.error("Failed to fetch dashboard stats:", error);
      set({ loading: false });
    }
  },

  fetchRecentActivities: async (environmentMode, limit = 20, projectId) => {
    try {
      const projects = await loadProjects();
      const visibleProjectIds = new Set(
        projects
          .filter((project) => projectMatchesEnvironment(project, environmentMode))
          .map((project) => project.id),
      );

      const activities = (await loadActivities()).filter((activity) => {
        if (projectId) {
          return activity.project_id === projectId;
        }

        return activity.project_id ? visibleProjectIds.has(activity.project_id) : environmentMode === "local";
      });

      set({ recentActivities: activities.slice(0, limit) });
    } catch (error) {
      console.error("Failed to fetch activities:", error);
      set({ recentActivities: [] });
    }
  },

  fetchActivitiesPage: async (environmentMode, page = 1, pageSize = 20, projectId) => {
    const safePage = Math.max(1, page);
    const safePageSize = Math.max(1, pageSize);
    const offset = (safePage - 1) * safePageSize;
    const projects = await loadProjects();
    const visibleProjectIds = new Set(
      projects
        .filter((project) => projectMatchesEnvironment(project, environmentMode))
        .map((project) => project.id),
    );
    const items = (await loadActivities()).filter((activity) => {
      if (projectId) {
        return activity.project_id === projectId;
      }

      return activity.project_id ? visibleProjectIds.has(activity.project_id) : environmentMode === "local";
    });

    return {
      items: items.slice(offset, offset + safePageSize),
      total: items.length,
    };
  },
}));
