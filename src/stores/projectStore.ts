import { create } from "zustand";
import { select, execute } from "@/lib/database";
import type { Project } from "@/lib/types";

interface ProjectStore {
  projects: Project[];
  currentProject: Project | null;
  loading: boolean;
  fetchProjects: () => Promise<void>;
  setCurrentProject: (project: Project | null) => void;
  createProject: (data: { name: string; description?: string; repo_path?: string }) => Promise<void>;
  updateProject: (id: string, updates: Partial<Pick<Project, "name" | "description" | "status" | "repo_path">>) => Promise<void>;
  deleteProject: (id: string) => Promise<void>;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
  projects: [],
  currentProject: null,
  loading: false,

  fetchProjects: async () => {
    set({ loading: true });
    try {
      const projects = await select<Project>("SELECT * FROM projects ORDER BY updated_at DESC");
      set({ projects, loading: false });
    } catch (e) {
      console.error("Failed to fetch projects:", e);
      set({ loading: false });
    }
  },

  setCurrentProject: (project) => set({ currentProject: project }),

  createProject: async (data) => {
    const id = crypto.randomUUID();
    await execute(
      "INSERT INTO projects (id, name, description, repo_path) VALUES ($1, $2, $3, $4)",
      [id, data.name, data.description ?? null, data.repo_path ?? null]
    );
    await get().fetchProjects();
  },

  updateProject: async (id, updates) => {
    const fields: string[] = [];
    const values: unknown[] = [];
    let idx = 1;
    for (const [key, value] of Object.entries(updates)) {
      fields.push(`${key} = $${idx}`);
      values.push(value);
      idx++;
    }
    values.push(id);
    await execute(`UPDATE projects SET ${fields.join(", ")} WHERE id = $${idx}`, values);
    await get().fetchProjects();
  },

  deleteProject: async (id) => {
    await execute("DELETE FROM activity_logs WHERE project_id = $1 OR task_id IN (SELECT id FROM tasks WHERE project_id = $1)", [id]);
    await execute("UPDATE employees SET project_id = NULL WHERE project_id = $1", [id]);
    await execute("DELETE FROM project_employees WHERE project_id = $1", [id]);
    await execute("DELETE FROM tasks WHERE project_id = $1", [id]);
    await execute("DELETE FROM projects WHERE id = $1", [id]);
    set((state) => ({
      currentProject: state.currentProject?.id === id ? null : state.currentProject,
    }));
    await get().fetchProjects();
  },
}));
