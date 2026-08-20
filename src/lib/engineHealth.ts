import {
  checkClaudeSdkHealth,
  checkGrokHealth,
  getRemoteHealthCheck,
  healthCheck,
  validateRemoteGrokHealth,
} from "@/lib/backend";
import { checkOpenCodeSdkHealth, validateRemoteOpenCodeHealth } from "@/lib/opencode";
import type { EnvironmentMode } from "@/lib/types";

export type EngineReadyFlags = {
  codex: boolean;
  claude: boolean;
  grok: boolean;
  opencode: boolean;
};

export const ENGINE_READY_ORDER = ["codex", "claude", "grok", "opencode"] as const;

export function isAnyEngineReady(flags: EngineReadyFlags): boolean {
  return flags.codex || flags.claude || flags.grok || flags.opencode;
}

export function readyEngineIds(
  flags: EngineReadyFlags,
): Array<(typeof ENGINE_READY_ORDER)[number]> {
  return ENGINE_READY_ORDER.filter((id) => flags[id]);
}

async function settledTrue(probe: Promise<boolean>): Promise<boolean> {
  try {
    return await probe;
  } catch {
    return false;
  }
}

export async function probeEngineReadiness(input: {
  environmentMode: EnvironmentMode;
  sshConfigId: string | null;
}): Promise<EngineReadyFlags> {
  if (input.environmentMode === "ssh") {
    const sshConfigId = input.sshConfigId;
    if (!sshConfigId) {
      return { codex: false, claude: false, grok: false, opencode: false };
    }
    const [codex, grok, opencode] = await Promise.all([
      settledTrue(
        getRemoteHealthCheck(sshConfigId).then((health) =>
          Boolean(health.sdk_installed || health.codex_available),
        ),
      ),
      settledTrue(
        validateRemoteGrokHealth(sshConfigId).then((health) => Boolean(health.available)),
      ),
      settledTrue(
        validateRemoteOpenCodeHealth(sshConfigId).then((health) =>
          Boolean(health.available || health.sdk_installed),
        ),
      ),
    ]);
    return { codex, claude: false, grok, opencode };
  }

  const [codex, claude, grok, opencode] = await Promise.all([
    settledTrue(
      healthCheck().then((health) => Boolean(health.sdk_installed || health.codex_available)),
    ),
    settledTrue(
      checkClaudeSdkHealth().then((health) =>
        Boolean(health.cli_available || health.sdk_installed),
      ),
    ),
    settledTrue(checkGrokHealth().then((health) => Boolean(health.cli_available))),
    settledTrue(checkOpenCodeSdkHealth().then((health) => Boolean(health.sdk_installed))),
  ]);
  return { codex, claude, grok, opencode };
}
