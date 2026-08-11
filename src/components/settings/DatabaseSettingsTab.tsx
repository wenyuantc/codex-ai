import { useCallback, useEffect, useState } from "react";
import { Download, FolderOpen, Loader2, Trash2, Upload } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  getDatabaseBackupScope,
  getSessionEventsPolicy,
  getSessionEventsStats,
  purgeSessionEvents,
  updateSessionEventsPolicy,
  type DatabaseBackupScope,
  type SessionEventsPolicy,
  type SessionEventsStats,
} from "@/lib/backend";
import { type CodexHealthCheck, type RemoteCodexHealthCheck } from "@/lib/types";

interface DatabaseSettingsTabProps {
  codexHealth: CodexHealthCheck | RemoteCodexHealthCheck | null;
  isTauriRuntime: boolean;
  actionLoading: "backup" | "restore" | "open-folder" | null;
  actionMessage: string | null;
  actionError: string | null;
  onBackup: () => void;
  onRestore: () => void;
  onOpenFolder: () => void;
}

const MIN_RETENTION_DAYS = 1;
const MAX_RETENTION_DAYS = 3650;
const DEFAULT_RETENTION_DAYS = 30;

export function DatabaseSettingsTab({
  codexHealth,
  isTauriRuntime,
  actionLoading,
  actionMessage,
  actionError,
  onBackup,
  onRestore,
  onOpenFolder,
}: DatabaseSettingsTabProps) {
  const { t } = useTranslation("settings");
  const [backupScope, setBackupScope] = useState<DatabaseBackupScope | null>(null);
  const [retentionDaysInput, setRetentionDaysInput] = useState(String(DEFAULT_RETENTION_DAYS));
  const [policy, setPolicy] = useState<SessionEventsPolicy | null>(null);
  const [stats, setStats] = useState<SessionEventsStats | null>(null);
  const [retentionLoading, setRetentionLoading] = useState(false);
  const [retentionMessage, setRetentionMessage] = useState<string | null>(null);
  const [retentionError, setRetentionError] = useState<string | null>(null);
  const openDatabaseFolderTitle = !isTauriRuntime
    ? t("database.actions.openDirectoryDesktopOnly")
    : codexHealth?.database_path
      ? t("database.actions.openDirectoryAvailable")
      : t("database.actions.pathUnavailable");

  const refreshSessionEventsState = useCallback(async () => {
    if (!isTauriRuntime) {
      return;
    }
    const [nextPolicy, nextStats] = await Promise.all([
      getSessionEventsPolicy(),
      getSessionEventsStats(),
    ]);
    setPolicy(nextPolicy);
    setStats(nextStats);
    setRetentionDaysInput(String(nextPolicy.retention_days));
  }, [isTauriRuntime]);

  useEffect(() => {
    void getDatabaseBackupScope()
      .then(setBackupScope)
      .catch(() => setBackupScope(null));
  }, []);

  useEffect(() => {
    if (!isTauriRuntime) {
      return;
    }
    void refreshSessionEventsState().catch((error: unknown) => {
      setRetentionError(error instanceof Error ? error.message : String(error));
    });
  }, [isTauriRuntime, refreshSessionEventsState]);

  const handleSaveRetention = async () => {
    if (!isTauriRuntime) {
      return;
    }
    const parsed = Number.parseInt(retentionDaysInput.trim(), 10);
    // Non-finite / out-of-range: still persist via backend normalize → default 30 (R2).
    const isInvalid =
      !Number.isFinite(parsed) || parsed < MIN_RETENTION_DAYS || parsed > MAX_RETENTION_DAYS;
    const days = Number.isFinite(parsed) ? parsed : DEFAULT_RETENTION_DAYS;

    setRetentionLoading(true);
    setRetentionError(null);
    setRetentionMessage(null);
    try {
      const nextPolicy = await updateSessionEventsPolicy(days);
      setPolicy(nextPolicy);
      setRetentionDaysInput(String(nextPolicy.retention_days));
      const nextStats = await getSessionEventsStats();
      setStats(nextStats);
      if (isInvalid || nextPolicy.retention_days !== days) {
        setRetentionMessage(
          t("database.messages.invalidRetentionSaved", {
            min: MIN_RETENTION_DAYS,
            max: MAX_RETENTION_DAYS,
            days: nextPolicy.retention_days,
          }),
        );
      } else {
        setRetentionMessage(
          t("database.messages.retentionSaved", { days: nextPolicy.retention_days }),
        );
      }
    } catch (error) {
      setRetentionError(error instanceof Error ? error.message : String(error));
    } finally {
      setRetentionLoading(false);
    }
  };

  const handlePurge = async () => {
    if (!isTauriRuntime) {
      return;
    }
    const days = policy?.retention_days ?? DEFAULT_RETENTION_DAYS;
    const expired = stats?.expired_events ?? 0;
    const confirmed = window.confirm(
      t("database.retention.confirmPurge", {
        days,
        expired,
      }),
    );
    if (!confirmed) {
      return;
    }

    setRetentionLoading(true);
    setRetentionError(null);
    setRetentionMessage(null);
    try {
      const result = await purgeSessionEvents();
      await refreshSessionEventsState();
      if (result.vacuum_ok) {
        setRetentionMessage(
          t("database.messages.purgeSuccessWithVacuum", {
            deleted: result.deleted,
            days: result.retention_days,
          }),
        );
      } else {
        setRetentionMessage(
          t("database.messages.purgeSuccess", {
            deleted: result.deleted,
            days: result.retention_days,
          }),
        );
        setRetentionError(
          result.vacuum_error
            ? t("database.messages.purgeVacuumFailedWithError", { error: result.vacuum_error })
            : t("database.messages.purgeVacuumFailed"),
        );
      }
    } catch (error) {
      setRetentionError(error instanceof Error ? error.message : String(error));
    } finally {
      setRetentionLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="text-sm font-medium">{t("database.maintenance.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("database.maintenance.description")}</p>
        </div>

        <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
          <p className="break-all">
            {t("database.maintenance.pathLabel")}:
            {codexHealth?.database_path ?? t("database.maintenance.detecting")}
          </p>
          <p>
            {t("database.maintenance.currentVersionLabel")}:
            {codexHealth?.database_current_version ?? t("database.maintenance.unknown")}
          </p>
          <p>
            {t("database.maintenance.latestVersionLabel")}:
            {codexHealth?.database_latest_version ?? t("database.maintenance.unknown")}
          </p>
          {codexHealth?.database_current_description && (
            <p>{codexHealth.database_current_description}</p>
          )}
        </div>

        {backupScope && (
          <div className="space-y-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-xs text-amber-950 dark:text-amber-100">
            <p className="font-medium">{t("database.backupScope.title")}</p>
            <p>{t("database.backupScope.note")}</p>
            <div className="grid gap-2 sm:grid-cols-2">
              <div>
                <p className="mb-1 font-medium text-green-800 dark:text-green-200">
                  {t("database.backupScope.included")}
                </p>
                <ul className="list-disc space-y-1 pl-4">
                  {(
                    t("database.backupScope.includesItems", {
                      returnObjects: true,
                    }) as string[]
                  ).map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
              <div>
                <p className="mb-1 font-medium text-amber-900 dark:text-amber-50">
                  {t("database.backupScope.excluded")}
                </p>
                <ul className="list-disc space-y-1 pl-4">
                  {(
                    t("database.backupScope.excludesItems", {
                      returnObjects: true,
                    }) as string[]
                  ).map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            </div>
            <p className="text-[11px] leading-5 opacity-90">
              {t("database.backupScope.restoreWarning")}
            </p>
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={onBackup} disabled={actionLoading !== null}>
            {actionLoading === "backup" ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Download className="h-4 w-4" />
            )}
            {t("database.actions.exportSql")}
          </Button>
          <Button variant="outline" onClick={onRestore} disabled={actionLoading !== null}>
            {actionLoading === "restore" ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Upload className="h-4 w-4" />
            )}
            {t("database.actions.importSql")}
          </Button>
          <Button
            variant="ghost"
            onClick={onOpenFolder}
            disabled={actionLoading !== null || !isTauriRuntime || !codexHealth?.database_path}
            title={openDatabaseFolderTitle}
          >
            {actionLoading === "open-folder" ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <FolderOpen className="h-4 w-4" />
            )}
            {t("database.actions.openDirectory")}
          </Button>
        </div>

        {actionMessage && <p className="text-xs text-green-700">{actionMessage}</p>}
        {actionError && <p className="text-xs text-destructive">{actionError}</p>}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="text-sm font-medium">{t("database.retention.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("database.retention.description")}</p>
        </div>

        {!isTauriRuntime ? (
          <p className="text-xs text-muted-foreground">{t("database.retention.desktopOnly")}</p>
        ) : (
          <>
            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground sm:grid-cols-2">
              <p>
                {t("database.retention.totalEvents")}:
                {stats?.total_events ?? t("database.retention.notAvailable")}
              </p>
              <p>
                {t("database.retention.expiredEvents")}:
                {stats?.expired_events ?? t("database.retention.notAvailable")}
              </p>
              <p className="break-all">
                {t("database.retention.oldest")}:
                {stats?.oldest_created_at ?? t("database.retention.notAvailable")}
              </p>
              <p className="break-all">
                {t("database.retention.newest")}:
                {stats?.newest_created_at ?? t("database.retention.notAvailable")}
              </p>
              <p className="sm:col-span-2">
                {t("database.retention.currentPolicy", {
                  days: policy?.retention_days ?? DEFAULT_RETENTION_DAYS,
                })}
              </p>
            </div>

            <div className="flex flex-wrap items-end gap-2">
              <label className="space-y-1 text-xs">
                <span className="text-muted-foreground">
                  {t("database.retention.retentionDaysLabel", {
                    min: MIN_RETENTION_DAYS,
                    max: MAX_RETENTION_DAYS,
                  })}
                </span>
                <input
                  type="number"
                  min={MIN_RETENTION_DAYS}
                  max={MAX_RETENTION_DAYS}
                  value={retentionDaysInput}
                  onChange={(event) => setRetentionDaysInput(event.target.value)}
                  disabled={retentionLoading}
                  className="block h-9 w-28 rounded-md border border-input bg-background px-3 text-sm"
                />
              </label>
              <Button
                variant="outline"
                onClick={() => void handleSaveRetention()}
                disabled={retentionLoading}
              >
                {retentionLoading ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {t("database.retention.savePolicy")}
              </Button>
              <Button
                variant="outline"
                onClick={() => void handlePurge()}
                disabled={retentionLoading}
              >
                {retentionLoading ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Trash2 className="h-4 w-4" />
                )}
                {t("database.retention.purgeNow")}
              </Button>
            </div>

            {retentionMessage && <p className="text-xs text-green-700">{retentionMessage}</p>}
            {retentionError && <p className="text-xs text-destructive">{retentionError}</p>}
          </>
        )}
      </div>
    </div>
  );
}
