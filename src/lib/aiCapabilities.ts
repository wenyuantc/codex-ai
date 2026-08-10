import { getAiProviderCapabilities, type AiProviderCapabilities } from "@/lib/backend";
import i18n from "@/lib/i18n";
import type { AiProvider } from "@/lib/types";

export type EngineCapabilityKey = "start" | "stop" | "restart" | "send_input" | "resume";

function capabilityLabel(capability: EngineCapabilityKey): string {
  return i18n.t(`errors:capability.${capability}`);
}

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
    return i18n.t("errors:capabilityLoadFailed");
  }
  const item = getCap(capabilities, provider);
  if (!item) {
    return i18n.t("errors:capabilityUnknown");
  }
  if (item[capability]) {
    return null;
  }
  const label = capabilityLabel(capability);
  if (capability === "send_input") {
    return item.notes?.trim() || i18n.t("errors:capabilityUnsupportedNonInteractive", { label });
  }
  if (capability === "restart") {
    return item.notes?.trim() || i18n.t("errors:capabilityUnsupported", { label });
  }
  return item.notes?.trim() || i18n.t("errors:capabilityUnsupported", { label });
}

export function clearAiProviderCapabilitiesCache(): void {
  cache = null;
  loadFailed = false;
  loadPromise = null;
}
