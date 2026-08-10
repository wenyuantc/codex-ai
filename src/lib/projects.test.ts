import { describe, expect, it } from "vitest";

import {
  filterProjectsByScope,
  getProjectTypeLabel,
  getProjectWorkingDir,
  normalizeProjectType,
  projectMatchesScope,
} from "@/lib/projects";
import type { Project } from "@/lib/types";

function project(partial: Partial<Project> & Pick<Project, "id" | "name">): Project {
  return {
    description: null,
    status: "active",
    repo_path: "/local/repo",
    project_type: "local",
    ssh_config_id: null,
    remote_repo_path: null,
    test_command: null,
    deleted_at: null,
    created_at: "2026-08-01",
    updated_at: "2026-08-01",
    ...partial,
  };
}

describe("projects helpers", () => {
  it("normalizes project type and labels", () => {
    expect(normalizeProjectType("ssh")).toBe("ssh");
    expect(normalizeProjectType("other")).toBe("local");
    expect(getProjectTypeLabel("ssh")).toBe("SSH 项目");
    expect(getProjectTypeLabel("local")).toBe("本地项目");
  });

  it("resolves working dir by project type", () => {
    expect(
      getProjectWorkingDir(
        project({ id: "1", name: "L", repo_path: "/a", remote_repo_path: "/b" }),
      ),
    ).toBe("/a");
    expect(
      getProjectWorkingDir(
        project({
          id: "2",
          name: "S",
          project_type: "ssh",
          repo_path: "/a",
          remote_repo_path: "/remote",
        }),
      ),
    ).toBe("/remote");
  });

  it("scopes projects by environment and selected SSH host", () => {
    const local = project({ id: "l", name: "Local" });
    const ssh = project({
      id: "s",
      name: "SSH",
      project_type: "ssh",
      ssh_config_id: "host-1",
      remote_repo_path: "/r",
    });
    expect(projectMatchesScope(local, "local")).toBe(true);
    expect(projectMatchesScope(ssh, "local")).toBe(false);
    expect(projectMatchesScope(ssh, "ssh", null)).toBe(false);
    expect(projectMatchesScope(ssh, "ssh", "host-1")).toBe(true);
    expect(filterProjectsByScope([local, ssh], "ssh", "host-1").map((item) => item.id)).toEqual([
      "s",
    ]);
  });
});
