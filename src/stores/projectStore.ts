import { create } from "zustand";
import { select } from "@/lib/database";
import {
  createProject as createProjectCommand,
  deleteProject as deleteProjectCommand,
  updateProject as updateProjectCommand,
} from "@/lib/backend";
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
    await createProjectCommand({
      name: data.name,
      description: data.description ?? null,
      repo_path: data.repo_path ?? null,
    });
    await get().fetchProjects();
  },

  updateProject: async (id, updates) => {
    await updateProjectCommand(id, updates);
    await get().fetchProjects();
  },

  deleteProject: async (id) => {
    await deleteProjectCommand(id);
    set((state) => ({
      currentProject: state.currentProject?.id === id ? null : state.currentProject,
    }));
    await get().fetchProjects();
  },
}));
