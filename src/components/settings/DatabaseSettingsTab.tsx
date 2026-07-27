import { useEffect, useState } from "react";
import { Download, FolderOpen, Loader2, Upload } from "lucide-react";

import { Button } from "@/components/ui/button";
import { getDatabaseBackupScope, type DatabaseBackupScope } from "@/lib/backend";
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
  const openDatabaseFolderTitle = !isTauriRuntime
    ? "仅桌面端支持打开数据库文件夹"
    : codexHealth?.database_path
      ? "打开数据库所在的文件夹"
      : "数据库路径不可用";

  useEffect(() => {
    void getDatabaseBackupScope()
      .then(setBackupScope)
      .catch(() => setBackupScope(null));
  }, []);

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
    </div>
  );
}
