import { useCallback, useEffect, useState } from "react";
import { Download, FolderOpen, Loader2, Trash2, Upload } from "lucide-react";

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
  const [backupScope, setBackupScope] = useState<DatabaseBackupScope | null>(null);
  const [retentionDaysInput, setRetentionDaysInput] = useState(String(DEFAULT_RETENTION_DAYS));
  const [policy, setPolicy] = useState<SessionEventsPolicy | null>(null);
  const [stats, setStats] = useState<SessionEventsStats | null>(null);
  const [retentionLoading, setRetentionLoading] = useState(false);
  const [retentionMessage, setRetentionMessage] = useState<string | null>(null);
  const [retentionError, setRetentionError] = useState<string | null>(null);
  const openDatabaseFolderTitle = !isTauriRuntime
    ? "仅桌面端支持打开数据库文件夹"
    : codexHealth?.database_path
      ? "打开数据库所在的文件夹"
      : "数据库路径不可用";

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
          `输入无效（须为 ${MIN_RETENTION_DAYS}–${MAX_RETENTION_DAYS} 的整数），已保存默认保留天数 ${nextPolicy.retention_days} 天`,
        );
      } else {
        setRetentionMessage(`已保存：保留 ${nextPolicy.retention_days} 天`);
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
      `将按当前保留策略（${days} 天）删除约 ${expired} 条过期会话事件，并尝试 VACUUM 回收磁盘。是否继续？`,
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
          `已清理 ${result.deleted} 条过期事件（保留 ${result.retention_days} 天），磁盘回收完成`,
        );
      } else {
        setRetentionMessage(
          `已清理 ${result.deleted} 条过期事件（保留 ${result.retention_days} 天）`,
        );
        setRetentionError(
          result.vacuum_error
            ? `事件已删除，但 VACUUM 失败：${result.vacuum_error}`
            : "事件已删除，但 VACUUM 失败",
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
          <h3 className="text-sm font-medium">数据库维护</h3>
          <p className="text-xs text-muted-foreground">
            数据库仍保留在本地；SSH 模式只切换执行上下文，不切换数据库位置。
          </p>
        </div>

        <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
          <p className="break-all">数据库路径：{codexHealth?.database_path ?? "检测中"}</p>
          <p>当前版本：{codexHealth?.database_current_version ?? "未知"}</p>
          <p>最新版本：{codexHealth?.database_latest_version ?? "未知"}</p>
          {codexHealth?.database_current_description && <p>{codexHealth.database_current_description}</p>}
        </div>

        {backupScope && (
          <div className="space-y-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-xs text-amber-950 dark:text-amber-100">
            <p className="font-medium">备份范围说明（不等于完整灾备）</p>
            <p>{backupScope.note}</p>
            <div className="grid gap-2 sm:grid-cols-2">
              <div>
                <p className="mb-1 font-medium text-green-800 dark:text-green-200">包含</p>
                <ul className="list-disc space-y-1 pl-4">
                  {backupScope.includes.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
              <div>
                <p className="mb-1 font-medium text-amber-900 dark:text-amber-50">不包含</p>
                <ul className="list-disc space-y-1 pl-4">
                  {backupScope.excludes.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            </div>
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={onBackup} disabled={actionLoading !== null}>
            {actionLoading === "backup" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            导出 SQL
          </Button>
          <Button variant="outline" onClick={onRestore} disabled={actionLoading !== null}>
            {actionLoading === "restore" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Upload className="h-4 w-4" />}
            导入 SQL
          </Button>
          <Button
            variant="ghost"
            onClick={onOpenFolder}
            disabled={actionLoading !== null || !isTauriRuntime || !codexHealth?.database_path}
            title={openDatabaseFolderTitle}
          >
            {actionLoading === "open-folder"
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : <FolderOpen className="h-4 w-4" />}
            打开数据库目录
          </Button>
        </div>

        {actionMessage && <p className="text-xs text-green-700">{actionMessage}</p>}
        {actionError && <p className="text-xs text-destructive">{actionError}</p>}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="text-sm font-medium">会话事件保留</h3>
          <p className="text-xs text-muted-foreground">
            按天数清理过期的会话事件日志，避免本地数据库无限膨胀。不会删除会话记录本身。
          </p>
        </div>

        {!isTauriRuntime ? (
          <p className="text-xs text-muted-foreground">仅桌面端支持会话事件保留与清理。</p>
        ) : (
          <>
            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground sm:grid-cols-2">
              <p>事件总数：{stats?.total_events ?? "—"}</p>
              <p>过期事件：{stats?.expired_events ?? "—"}</p>
              <p className="break-all">最早：{stats?.oldest_created_at ?? "—"}</p>
              <p className="break-all">最新：{stats?.newest_created_at ?? "—"}</p>
              <p className="sm:col-span-2">
                当前策略：保留 {policy?.retention_days ?? DEFAULT_RETENTION_DAYS} 天
              </p>
            </div>

            <div className="flex flex-wrap items-end gap-2">
              <label className="space-y-1 text-xs">
                <span className="text-muted-foreground">保留天数（{MIN_RETENTION_DAYS}–{MAX_RETENTION_DAYS}）</span>
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
                保存策略
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
                立即清理
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
