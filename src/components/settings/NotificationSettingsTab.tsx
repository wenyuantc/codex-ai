import { useCallback, useEffect, useState } from "react";
import { Volume2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { getNotificationSoundSettings, updateNotificationSoundSettings } from "@/lib/backend";
import {
  NOTIFICATION_SOUND_STORAGE_KEY,
  parseNotificationSoundEnabled,
  playNotificationSound,
  setNotificationSoundEnabled,
} from "@/lib/notificationSound";

interface NotificationSettingsTabProps {
  isTauriRuntime: boolean;
}

export function NotificationSettingsTab({ isTauriRuntime }: NotificationSettingsTabProps) {
  const { t } = useTranslation("settings");
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const persistLocal = useCallback((nextEnabled: boolean) => {
    localStorage.setItem(NOTIFICATION_SOUND_STORAGE_KEY, nextEnabled ? "true" : "false");
    setNotificationSoundEnabled(nextEnabled);
  }, []);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        if (isTauriRuntime) {
          const settings = await getNotificationSoundSettings();
          if (!cancelled) {
            setEnabled(settings.enabled);
            setNotificationSoundEnabled(settings.enabled);
          }
          return;
        }

        const nextEnabled = parseNotificationSoundEnabled(
          localStorage.getItem(NOTIFICATION_SOUND_STORAGE_KEY),
        );
        if (!cancelled) {
          setEnabled(nextEnabled);
          setNotificationSoundEnabled(nextEnabled);
        }
      } catch (loadError: unknown) {
        if (!cancelled) {
          setError(
            t("notifications.loadFailed", {
              detail: loadError instanceof Error ? loadError.message : String(loadError),
            }),
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [isTauriRuntime, t]);

  const handleEnabledChange = async (nextEnabled: boolean) => {
    setEnabled(nextEnabled);
    setSaving(true);
    setMessage(null);
    setError(null);
    try {
      if (isTauriRuntime) {
        const settings = await updateNotificationSoundSettings(nextEnabled);
        setEnabled(settings.enabled);
        setNotificationSoundEnabled(settings.enabled);
      } else {
        persistLocal(nextEnabled);
      }
      setMessage(t("notifications.saved"));
    } catch (saveError: unknown) {
      setEnabled(!nextEnabled);
      setError(
        t("notifications.saveFailed", {
          detail: saveError instanceof Error ? saveError.message : String(saveError),
        }),
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="text-sm font-medium">{t("notifications.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("notifications.description")}</p>
        </div>

        {!isTauriRuntime && (
          <p className="text-xs text-muted-foreground">{t("notifications.browserOnly")}</p>
        )}

        <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
          <input
            type="checkbox"
            className="mt-0.5 h-4 w-4 rounded border-input"
            checked={enabled}
            disabled={loading || saving}
            onChange={(event) => void handleEnabledChange(event.target.checked)}
          />
          <div className="space-y-1">
            <p className="text-sm font-medium">{t("notifications.enableLabel")}</p>
            <p className="text-xs text-muted-foreground">{t("notifications.enableHint")}</p>
          </div>
        </label>

        <div className="flex flex-wrap items-center gap-3">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={loading}
            onClick={() => void playNotificationSound()}
          >
            <Volume2 className="h-4 w-4" />
            {t("notifications.preview")}
          </Button>
          <p className="text-xs text-muted-foreground">{t("notifications.previewHint")}</p>
        </div>

        {message && <p className="text-xs text-emerald-600">{message}</p>}
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>
    </div>
  );
}
