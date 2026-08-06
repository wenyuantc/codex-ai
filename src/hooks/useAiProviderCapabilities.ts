import { useCallback, useEffect, useState } from "react";

import {
  can,
  capabilityDisabledReason,
  getCap,
  loadAiProviderCapabilities,
  type EngineCapabilityKey,
} from "@/lib/aiCapabilities";
import type { AiProviderCapabilities } from "@/lib/backend";
import type { AiProvider } from "@/lib/types";

interface UseAiProviderCapabilitiesResult {
  capabilities: AiProviderCapabilities[];
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
  getCap: (provider: AiProvider | string | null | undefined) => AiProviderCapabilities | null;
  can: (
    provider: AiProvider | string | null | undefined,
    capability: EngineCapabilityKey,
  ) => boolean;
  disabledReason: (
    provider: AiProvider | string | null | undefined,
    capability: EngineCapabilityKey,
  ) => string | null;
}

export function useAiProviderCapabilities(): UseAiProviderCapabilitiesResult {
  const [capabilities, setCapabilities] = useState<AiProviderCapabilities[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const items = await loadAiProviderCapabilities(true);
      setCapabilities(items);
    } catch (err) {
      setCapabilities([]);
      setError(err instanceof Error ? err.message : "能力信息加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    setLoading(true);
    void loadAiProviderCapabilities()
      .then((items) => {
        if (active) {
          setCapabilities(items);
          setError(null);
        }
      })
      .catch((err) => {
        if (active) {
          setCapabilities([]);
          setError(err instanceof Error ? err.message : "能力信息加载失败");
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  return {
    capabilities,
    loading,
    error,
    reload,
    getCap: (provider) => getCap(capabilities, provider),
    can: (provider, capability) => can(capabilities, provider, capability),
    disabledReason: (provider, capability) =>
      capabilityDisabledReason(capabilities, provider, capability),
  };
}
