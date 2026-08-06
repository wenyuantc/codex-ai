import { getAiProviderCapabilities, type AiProviderCapabilities } from "@/lib/backend";
import type { AiProvider } from "@/lib/types";

export type EngineCapabilityKey = "start" | "stop" | "restart" | "send_input" | "resume";

const CAPABILITY_LABELS: Record<EngineCapabilityKey, string> = {
  start: "启动",
  stop: "停止",
  restart: "重启",
  send_input: "会话中输入",
  resume: "续聊",
};

let cache: AiProviderCapabilities[] | null = null;
let loadPromise: Promise<AiProviderCapabilities[]> | null = null;
let loadFailed = false;

export async function loadAiProviderCapabilities(force = false): Promise<AiProviderCapabilities[]> {
  if (!force && cache) {
    return cache;
  }
  if (!force && loadPromise) {
    return loadPromise;
  }

  loadPromise = getAiProviderCapabilities()
    .then((items) => {
      cache = items;
      loadFailed = false;
      return items;
    })
    .catch((error) => {
      loadFailed = true;
      cache = null;
      throw error;
    })
    .finally(() => {
      loadPromise = null;
    });

  return loadPromise;
}

export function getCachedAiProviderCapabilities(): AiProviderCapabilities[] | null {
  return cache;
}

export function didAiProviderCapabilitiesFail(): boolean {
  return loadFailed;
}

export function getCap(
  capabilities: AiProviderCapabilities[] | null | undefined,
  provider: AiProvider | string | null | undefined,
): AiProviderCapabilities | null {
  if (!capabilities || !provider) {
    return null;
  }
  return capabilities.find((item) => item.provider === provider) ?? null;
}

/** Fail-closed: unknown / missing capability is treated as unsupported. */
export function can(
  capabilities: AiProviderCapabilities[] | null | undefined,
  provider: AiProvider | string | null | undefined,
  capability: EngineCapabilityKey,
): boolean {
  const item = getCap(capabilities, provider);
  if (!item) {
    return false;
  }
  return Boolean(item[capability]);
}

export function capabilityDisabledReason(
  capabilities: AiProviderCapabilities[] | null | undefined,
  provider: AiProvider | string | null | undefined,
  capability: EngineCapabilityKey,
): string | null {
  if (loadFailed && !capabilities) {
    return "能力信息加载失败，操作已禁用";
  }
  const item = getCap(capabilities, provider);
  if (!item) {
    return "当前引擎能力未知，操作已禁用";
  }
  if (item[capability]) {
    return null;
  }
  const label = CAPABILITY_LABELS[capability];
  if (capability === "send_input") {
    return item.notes?.trim() || `当前引擎不支持${label}（非交互模式）`;
  }
  if (capability === "restart") {
    return item.notes?.trim() || `当前引擎不支持${label}`;
  }
  return item.notes?.trim() || `当前引擎不支持${label}`;
}

export function clearAiProviderCapabilitiesCache(): void {
  cache = null;
  loadFailed = false;
  loadPromise = null;
}
