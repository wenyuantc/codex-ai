import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { getCachedAiProviderCapabilities, loadAiProviderCapabilities } from "@/lib/aiCapabilities";
import type { AiProviderCapabilities } from "@/lib/backend";
import type { AiProvider } from "@/lib/types";

interface EngineCapabilityBadgesProps {
  provider?: AiProvider | string | null;
  compact?: boolean;
  className?: string;
}

function CapBadge({
  supported,
  label,
  unsupportedSuffix,
  title,
}: {
  supported: boolean;
  label: string;
  unsupportedSuffix: string;
  title?: string;
}) {
  return (
    <Badge
      variant={supported ? "default" : "secondary"}
      title={title}
      className={!supported ? "opacity-80" : undefined}
    >
      {label}
      {supported ? "" : unsupportedSuffix}
    </Badge>
  );
}

function localizedCapabilityNotes(
  tSettings: (key: string, options?: Record<string, unknown>) => string,
  provider: string,
  fallback?: string | null,
): string {
  const key = `page.engineCapabilities.notes.${provider}`;
  const translated = tSettings(key, { defaultValue: "" });
  if (translated.trim()) {
    return translated;
  }
  return fallback?.trim() || "";
}

export function EngineCapabilityBadges({
  provider,
  compact = false,
  className = "",
}: EngineCapabilityBadgesProps) {
  const { t } = useTranslation("errors");
  const { t: tSettings } = useTranslation("settings");
  const [capabilities, setCapabilities] = useState<AiProviderCapabilities[]>(
    () => getCachedAiProviderCapabilities() ?? [],
  );
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    void loadAiProviderCapabilities()
      .then((items) => {
        setCapabilities(items);
        setLoadError(null);
      })
      .catch((error) => {
        setCapabilities([]);
        setLoadError(error instanceof Error ? error.message : t("capabilityLoadFailed"));
      });
  }, [t]);

  const items = provider ? capabilities.filter((item) => item.provider === provider) : capabilities;
  const unsupportedSuffix = t("capabilityUnsupportedSuffix");

  if (loadError && items.length === 0) {
    return (
      <div
        className={`rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive ${className}`}
      >
        {t("capabilityLoadFailedWithDetail", { detail: loadError })}
      </div>
    );
  }

  if (items.length === 0) {
    return null;
  }

  return (
    <div className={`space-y-2 ${className}`}>
      {items.map((item) => {
        const notesTitle =
          localizedCapabilityNotes(tSettings, item.provider, item.notes) || undefined;
        return (
          <div
            key={item.provider}
            className="rounded-md border border-border bg-card/60 px-3 py-2 text-xs"
            title={notesTitle}
          >
            <div className="mb-1.5 flex items-center gap-2">
              <span className="font-medium">{item.label}</span>
              {!compact && notesTitle && (
                <span className="text-muted-foreground line-clamp-2" title={notesTitle}>
                  {notesTitle}
                </span>
              )}
            </div>
            <div className="flex flex-wrap gap-1">
              <CapBadge
                supported={item.start}
                label={t("capability.start")}
                unsupportedSuffix={unsupportedSuffix}
                title={notesTitle}
              />
              <CapBadge
                supported={item.stop}
                label={t("capability.stop")}
                unsupportedSuffix={unsupportedSuffix}
                title={notesTitle}
              />
              <CapBadge
                supported={item.restart}
                label={t("capability.restart")}
                unsupportedSuffix={unsupportedSuffix}
                title={
                  item.restart
                    ? t("capabilityRestartHint")
                    : notesTitle || t("capabilityUnsupported", { label: t("capability.restart") })
                }
              />
              <CapBadge
                supported={item.send_input}
                label={t("capability.send_input")}
                unsupportedSuffix={unsupportedSuffix}
                title={
                  item.send_input
                    ? notesTitle
                    : notesTitle ||
                      t("capabilityUnsupportedNonInteractive", {
                        label: t("capability.send_input"),
                      })
                }
              />
              <CapBadge
                supported={item.resume}
                label={t("capability.resume")}
                unsupportedSuffix={unsupportedSuffix}
                title={notesTitle}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
