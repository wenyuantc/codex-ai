import { describe, it, expect } from "vitest";

import type { Project, SshConfig } from "@/lib/types";
import { resolveSelectedSshConfigId } from "@/stores/projectStore";

function makeSshConfig(id: string): SshConfig {
  return {
    id,
    name: `host-${id}`,
    host: "example.com",
    port: 22,
    username: "deploy",
    auth_type: "key",
    private_key_path: null,
    known_hosts_mode: "accept-new",
    password_configured: false,
    passphrase_configured: false,
    password_probe_status: null,
    password_probe_message: null,
    password_execution_allowed: true,
    last_checked_at: null,
    last_check_status: null,
    last_check_message: null,
    created_at: "2026-08-06 00:00:00",
    updated_at: "2026-08-06 00:00:00",
  };
}

function makeProject(overrides: Partial<Project>): Project {
  return {
    id: "p1",
    name: "demo",
    description: null,
    status: "active",
    repo_path: null,
    project_type: "local",
    ssh_config_id: null,
    remote_repo_path: null,
    test_command: null,
    deleted_at: null,
    created_at: "2026-08-06 00:00:00",
    updated_at: "2026-08-06 00:00:00",
    ...overrides,
  };
}

const configs = [makeSshConfig("a"), makeSshConfig("b")];

describe("resolveSelectedSshConfigId", () => {
  it("prefers the SSH config bound to the current SSH project", () => {
    const project = makeProject({ project_type: "ssh", ssh_config_id: "b" });
    expect(resolveSelectedSshConfigId(configs, "a", project)).toBe("b");
  });

  it("ignores an SSH project that has no bound config", () => {
    const project = makeProject({ project_type: "ssh", ssh_config_id: null });
    expect(resolveSelectedSshConfigId(configs, "a", project)).toBe("a");
  });

  it("ignores the binding for a local project", () => {
    const project = makeProject({ project_type: "local", ssh_config_id: "b" });
    expect(resolveSelectedSshConfigId(configs, "a", project)).toBe("a");
  });

  it("keeps the current selection when it still exists", () => {
    expect(resolveSelectedSshConfigId(configs, "b", null)).toBe("b");
  });

  it("falls back to the first config when the selection is stale", () => {
    // e.g. the selected host was deleted in settings while it was active.
    expect(resolveSelectedSshConfigId(configs, "deleted-id", null)).toBe("a");
  });

  it("falls back to the first config when nothing is selected", () => {
    expect(resolveSelectedSshConfigId(configs, null, null)).toBe("a");
  });

  it("returns null when there is no SSH config at all", () => {
    expect(resolveSelectedSshConfigId([], "a", null)).toBeNull();
  });
});
