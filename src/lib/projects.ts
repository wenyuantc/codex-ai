import type { EnvironmentMode, Project, ProjectType } from "./types";

export const DEFAULT_ENVIRONMENT_MODE: EnvironmentMode = "local";

export function normalizeProjectType(value: string | null | undefined): ProjectType {
  return value === "ssh" ? "ssh" : "local";
}

export function normalizeProject(project: Partial<Project> & Pick<Project, "id" | "name" | "status" | "created_at" | "updated_at">): Project {
  const projectType = normalizeProjectType(project.project_type);

  return {
    id: project.id,
    name: project.name,
    description: project.description ?? null,
    status: project.status,
    repo_path: project.repo_path ?? null,
    project_type: projectType,
    ssh_config_id: project.ssh_config_id ?? null,
    remote_repo_path: project.remote_repo_path ?? null,
    created_at: project.created_at,
    updated_at: project.updated_at,
  };
}

export function getProjectTypeLabel(projectType: ProjectType | null | undefined): string {
  return normalizeProjectType(projectType) === "ssh" ? "SSH 项目" : "本地项目";
}

export function getProjectWorkingDir(project: Pick<Project, "project_type" | "repo_path" | "remote_repo_path"> | null | undefined): string | null {
  if (!project) {
    return null;
  }

  return normalizeProjectType(project.project_type) === "ssh"
    ? project.remote_repo_path ?? null
    : project.repo_path ?? null;
}

export function projectMatchesEnvironment(
  project: Pick<Project, "project_type"> | null | undefined,
  environmentMode: EnvironmentMode,
): boolean {
  if (!project) {
    return false;
  }

  return normalizeProjectType(project.project_type) === environmentMode;
}

export function getEnvironmentModeLabel(environmentMode: EnvironmentMode): string {
  return environmentMode === "ssh" ? "SSH 模式" : "本地模式";
}
