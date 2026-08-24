import { getAiProviderCapabilities, type AiProviderCapabilities } from "@/lib/backend";
import i18n from "@/lib/i18n";
import type { AiProvider } from "@/lib/types";

export type EngineCapabilityKey = "start" | "stop" | "restart" | "send_input" | "resume" | "mcp";

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

function localizedProviderNotes(provider: string, fallback?: string | null): string {
  const translated = i18n.t(`settings:page.engineCapabilities.notes.${provider}`, {
    defaultValue: "",
  });
  if (translated.trim()) {
    return translated;
  }
  return fallback?.trim() || "";
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
  const notes = localizedProviderNotes(item.provider, item.notes);
  if (capability === "send_input") {
    return notes || i18n.t("errors:capabilityUnsupportedNonInteractive", { label });
  }
  if (capability === "restart") {
    return notes || i18n.t("errors:capabilityUnsupported", { label });
  }
  return notes || i18n.t("errors:capabilityUnsupported", { label });
}

export function clearAiProviderCapabilitiesCache(): void {
  cache = null;
  loadFailed = false;
  loadPromise = null;
}
