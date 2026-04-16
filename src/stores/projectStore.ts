import { create } from "zustand";

import { select } from "@/lib/database";
import {
  createProject as createProjectCommand,
  createSshConfig as createSshConfigCommand,
  deleteProject as deleteProjectCommand,
  deleteSshConfig as deleteSshConfigCommand,
  listSshConfigs as listSshConfigsCommand,
  runSshPasswordProbe as runSshPasswordProbeCommand,
  updateProject as updateProjectCommand,
  updateSshConfig as updateSshConfigCommand,
  type CreateProjectInput,
  type CreateSshConfigInput,
  type UpdateProjectInput,
  type UpdateSshConfigInput,
} from "@/lib/backend";
import { DEFAULT_ENVIRONMENT_MODE, normalizeProject, projectMatchesEnvironment } from "@/lib/projects";
import type { EnvironmentMode, Project, SshConfig, SshPasswordProbeResult } from "@/lib/types";

const ENVIRONMENT_MODE_STORAGE_KEY = "codex-ai:environment-mode";
const SSH_CONFIG_STORAGE_KEY = "codex-ai:selected-ssh-config-id";

function readStoredEnvironmentMode(): EnvironmentMode {
  if (typeof window === "undefined") {
    return DEFAULT_ENVIRONMENT_MODE;
  }

  return window.localStorage.getItem(ENVIRONMENT_MODE_STORAGE_KEY) === "ssh" ? "ssh" : "local";
}

function persistEnvironmentMode(environmentMode: EnvironmentMode) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(ENVIRONMENT_MODE_STORAGE_KEY, environmentMode);
}

function readStoredSshConfigId(): string | null {
  if (typeof window === "undefined") {
    return null;
  }

  return window.localStorage.getItem(SSH_CONFIG_STORAGE_KEY);
}

function persistSshConfigId(sshConfigId: string | null) {
  if (typeof window === "undefined") {
    return;
  }

  if (sshConfigId) {
    window.localStorage.setItem(SSH_CONFIG_STORAGE_KEY, sshConfigId);
  } else {
    window.localStorage.removeItem(SSH_CONFIG_STORAGE_KEY);
  }
}

function filterProjectsByMode(projects: Project[], environmentMode: EnvironmentMode) {
  return projects.filter((project) => projectMatchesEnvironment(project, environmentMode));
}

function resolveSelectedSshConfigId(
  sshConfigs: SshConfig[],
  selectedSshConfigId: string | null,
  currentProject: Project | null,
) {
  if (currentProject?.project_type === "ssh" && currentProject.ssh_config_id) {
    return currentProject.ssh_config_id;
  }

  if (selectedSshConfigId && sshConfigs.some((config) => config.id === selectedSshConfigId)) {
    return selectedSshConfigId;
  }

  return sshConfigs[0]?.id ?? null;
}

async function selectProjectsFromDatabase(): Promise<Project[]> {
  const rows = await select<Project>("SELECT * FROM projects ORDER BY updated_at DESC");
  return rows.map((project) => normalizeProject(project));
}

interface ProjectStore {
  allProjects: Project[];
  projects: Project[];
  currentProject: Project | null;
  environmentMode: EnvironmentMode;
  sshConfigs: SshConfig[];
  selectedSshConfigId: string | null;
  loading: boolean;
  sshConfigsLoading: boolean;
  fetchProjects: () => Promise<void>;
  fetchSshConfigs: () => Promise<void>;
  setCurrentProject: (project: Project | null) => void;
  setEnvironmentMode: (environmentMode: EnvironmentMode) => Promise<{ redirectToSettings: boolean }>;
  setSelectedSshConfigId: (sshConfigId: string | null) => void;
  createProject: (data: CreateProjectInput) => Promise<void>;
  updateProject: (id: string, updates: UpdateProjectInput) => Promise<void>;
  deleteProject: (id: string) => Promise<void>;
  createSshConfig: (data: CreateSshConfigInput) => Promise<SshConfig>;
  updateSshConfig: (id: string, updates: UpdateSshConfigInput) => Promise<SshConfig>;
  deleteSshConfig: (id: string) => Promise<void>;
  runSshPasswordProbe: (id: string) => Promise<SshPasswordProbeResult>;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
  allProjects: [],
  projects: [],
  currentProject: null,
  environmentMode: readStoredEnvironmentMode(),
  sshConfigs: [],
  selectedSshConfigId: readStoredSshConfigId(),
  loading: false,
  sshConfigsLoading: false,

  fetchProjects: async () => {
    set({ loading: true });
    try {
      const projects = await selectProjectsFromDatabase();
      set((state) => {
        const filteredProjects = filterProjectsByMode(projects, state.environmentMode);
        const currentProjectId = state.currentProject?.id;
        const currentProject = currentProjectId
          ? filteredProjects.find((project) => project.id === currentProjectId) ?? null
          : null;
        const selectedSshConfigId = resolveSelectedSshConfigId(
          state.sshConfigs,
          state.selectedSshConfigId,
          currentProject,
        );

        persistSshConfigId(selectedSshConfigId);

        return {
          allProjects: projects,
          projects: filteredProjects,
          currentProject,
          selectedSshConfigId,
          loading: false,
        };
      });
    } catch (error) {
      console.error("Failed to fetch projects:", error);
      set({ loading: false });
    }
  },

  fetchSshConfigs: async () => {
    set({ sshConfigsLoading: true });
    try {
      const sshConfigs = await listSshConfigsCommand();

      set((state) => {
        const selectedSshConfigId = resolveSelectedSshConfigId(
          sshConfigs,
          state.selectedSshConfigId,
          state.currentProject,
        );
        persistSshConfigId(selectedSshConfigId);

        return {
          sshConfigs,
          selectedSshConfigId,
          sshConfigsLoading: false,
        };
      });
    } catch (error) {
      console.error("Failed to fetch SSH configs:", error);
      set({ sshConfigs: [], selectedSshConfigId: null, sshConfigsLoading: false });
      persistSshConfigId(null);
    }
  },

  setCurrentProject: (project) => {
    const nextProject = project && projectMatchesEnvironment(project, get().environmentMode) ? project : null;
    const nextSshConfigId = nextProject?.project_type === "ssh"
      ? nextProject.ssh_config_id ?? get().selectedSshConfigId
      : get().selectedSshConfigId;

    set({
      currentProject: nextProject,
      selectedSshConfigId: nextSshConfigId ?? null,
    });
    if (nextProject?.project_type === "ssh") {
      persistSshConfigId(nextSshConfigId ?? null);
    }
  },

  setEnvironmentMode: async (environmentMode) => {
    persistEnvironmentMode(environmentMode);

    set((state) => {
      const filteredProjects = filterProjectsByMode(state.allProjects, environmentMode);
      const currentProject = state.currentProject && projectMatchesEnvironment(state.currentProject, environmentMode)
        ? filteredProjects.find((project) => project.id === state.currentProject?.id) ?? null
        : null;
      const selectedSshConfigId = resolveSelectedSshConfigId(
        state.sshConfigs,
        state.selectedSshConfigId,
        currentProject,
      );

      persistSshConfigId(selectedSshConfigId);

      return {
        environmentMode,
        projects: filteredProjects,
        currentProject,
        selectedSshConfigId,
      };
    });

    if (environmentMode === "ssh" && get().sshConfigs.length === 0) {
      await get().fetchSshConfigs();
    }

    return {
      redirectToSettings: environmentMode === "ssh" && get().sshConfigs.length === 0,
    };
  },

  setSelectedSshConfigId: (sshConfigId) => {
    persistSshConfigId(sshConfigId);
    set({ selectedSshConfigId: sshConfigId });
  },

  createProject: async (data) => {
    await createProjectCommand({
      ...data,
      description: data.description ?? null,
      project_type: data.project_type ?? "local",
      repo_path: data.project_type === "ssh" ? null : data.repo_path ?? null,
      ssh_config_id: data.project_type === "ssh" ? data.ssh_config_id ?? null : null,
      remote_repo_path: data.project_type === "ssh" ? data.remote_repo_path ?? null : null,
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

  createSshConfig: async (data) => {
    const sshConfig = await createSshConfigCommand({
      ...data,
      port: data.port ?? 22,
      private_key_path: data.private_key_path ?? null,
      password: data.password ?? null,
      passphrase: data.passphrase ?? null,
      known_hosts_mode: data.known_hosts_mode ?? "accept-new",
    });
    await get().fetchSshConfigs();
    return sshConfig;
  },

  updateSshConfig: async (id, updates) => {
    const sshConfig = await updateSshConfigCommand(id, updates);
    await get().fetchSshConfigs();
    return sshConfig;
  },

  deleteSshConfig: async (id) => {
    await deleteSshConfigCommand(id);
    await get().fetchSshConfigs();
    await get().fetchProjects();
  },

  runSshPasswordProbe: async (id) => {
    const result = await runSshPasswordProbeCommand(id);
    await get().fetchSshConfigs();
    return result;
  },
}));
