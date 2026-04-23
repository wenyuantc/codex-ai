import type { SshAuthType, SshConfig } from "@/lib/types";

export type SettingsTabValue = "runtime" | "git" | "ssh" | "database";

export interface SshConfigFormState {
  name: string;
  host: string;
  port: string;
  username: string;
  authType: SshAuthType;
  privateKeyPath: string;
  password: string;
  passphrase: string;
  knownHostsMode: string;
}

export const EMPTY_SSH_CONFIG_FORM: SshConfigFormState = {
  name: "",
  host: "",
  port: "22",
  username: "",
  authType: "key",
  privateKeyPath: "",
  password: "",
  passphrase: "",
  knownHostsMode: "accept-new",
};

export function buildSshConfigFormState(config: SshConfig | null): SshConfigFormState {
  if (!config) {
    return EMPTY_SSH_CONFIG_FORM;
  }

  return {
    name: config.name,
    host: config.host,
    port: String(config.port || 22),
    username: config.username,
    authType: config.auth_type,
    privateKeyPath: config.private_key_path ?? "",
    password: "",
    passphrase: "",
    knownHostsMode: config.known_hosts_mode ?? "accept-new",
  };
}

export function getSettingsTabFromSection(section: string | null): SettingsTabValue {
  switch (section) {
    case "git":
      return "git";
    case "ssh":
      return "ssh";
    case "database":
      return "database";
    case "sdk":
    default:
      return "runtime";
  }
}

export function getSectionForSettingsTab(tab: SettingsTabValue): string {
  switch (tab) {
    case "git":
      return "git";
    case "ssh":
      return "ssh";
    case "database":
      return "database";
    case "runtime":
    default:
      return "sdk";
  }
}
