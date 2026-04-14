import { useEffect, useState } from "react";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { Download, FolderOpen, Loader2, Monitor, Moon, RefreshCw, Sun, Upload } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  backupDatabase,
  getCodexSettings,
  healthCheck,
  installCodexSdk,
  openDatabaseFolder,
  restoreDatabase,
  updateCodexSettings,
} from "@/lib/backend";
import {
  CODEX_MODEL_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  normalizeCodexModel,
  normalizeReasoningEffort,
  type CodexHealthCheck,
  type CodexModelId,
  type CodexSettings,
  type ReasoningEffort,
} from "@/lib/types";

type ThemeMode = "light" | "dark" | "system";

const DATABASE_FILE_FILTERS = [
  { name: "SQL 备份", extensions: ["sql"] },
];

const isTauriRuntime =
  typeof window !== "undefined" &&
  typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";

function getThemePreference(): ThemeMode {
  const stored = localStorage.getItem("theme-mode");
  if (stored === "light" || stored === "dark" || stored === "system") return stored;
  return "system";
}

function applyTheme(mode: ThemeMode) {
  let isDark: boolean;
  if (mode === "system") {
    isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  } else {
    isDark = mode === "dark";
  }
  document.documentElement.classList.toggle("dark", isDark);
  localStorage.setItem("theme", isDark ? "dark" : "light");
}

function formatBackupTimestamp(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  return `${year}${month}${day}-${hours}${minutes}${seconds}`;
}

function buildBackupDefaultPath(health: CodexHealthCheck | null) {
  const version = health?.database_current_version ?? health?.database_latest_version ?? 0;
  const fileName = `codex-ai-backup-v${version}-${formatBackupTimestamp()}.sql`;
  const databasePath = health?.database_path;

  if (!databasePath) return fileName;

  const lastSeparatorIndex = Math.max(databasePath.lastIndexOf("/"), databasePath.lastIndexOf("\\"));
  if (lastSeparatorIndex < 0) return fileName;

  const directory = databasePath.slice(0, lastSeparatorIndex);
  const separator = directory.includes("\\") ? "\\" : "/";
  return `${directory}${separator}${fileName}`;
}

export function SettingsPage() {
  const [themeMode, setThemeMode] = useState<ThemeMode>(getThemePreference);
  const [codexHealth, setCodexHealth] = useState<CodexHealthCheck | null>(null);
  const [codexSettings, setCodexSettings] = useState<CodexSettings | null>(null);
  const [taskSdkEnabled, setTaskSdkEnabled] = useState(false);
  const [oneShotSdkEnabled, setOneShotSdkEnabled] = useState(false);
  const [oneShotModel, setOneShotModel] = useState<CodexModelId>("gpt-5.4");
  const [oneShotReasoningEffort, setOneShotReasoningEffort] = useState<ReasoningEffort>("high");
  const [nodePathOverride, setNodePathOverride] = useState("");
  const [healthLoading, setHealthLoading] = useState(false);
  const [sdkActionLoading, setSdkActionLoading] = useState<"save" | "install" | null>(null);
  const [sdkActionMessage, setSdkActionMessage] = useState<string | null>(null);
  const [sdkActionError, setSdkActionError] = useState<string | null>(null);
  const [databaseActionLoading, setDatabaseActionLoading] = useState<
    "backup" | "restore" | "open-folder" | null
  >(null);
  const [databaseActionMessage, setDatabaseActionMessage] = useState<string | null>(null);
  const [databaseActionError, setDatabaseActionError] = useState<string | null>(null);

  async function loadSettingsState() {
    setHealthLoading(true);
    setSdkActionError(null);

    try {
      const [health, settings] = await Promise.all([healthCheck(), getCodexSettings()]);
      setCodexHealth(health);
      setCodexSettings(settings);
      setTaskSdkEnabled(settings.task_sdk_enabled);
      setOneShotSdkEnabled(settings.one_shot_sdk_enabled);
      setOneShotModel(normalizeCodexModel(settings.one_shot_model));
      setOneShotReasoningEffort(normalizeReasoningEffort(settings.one_shot_reasoning_effort));
      setNodePathOverride(settings.node_path_override ?? "");
    } catch (error) {
      console.error("Failed to load codex settings state:", error);
      setCodexHealth(null);
      setCodexSettings(null);
      setSdkActionError(error instanceof Error ? error.message : "读取 Codex 配置失败");
    } finally {
      setHealthLoading(false);
    }
  }

  useEffect(() => {
    applyTheme(themeMode);
    localStorage.setItem("theme-mode", themeMode);
  }, [themeMode]);

  useEffect(() => {
    void loadSettingsState();
  }, []);

  const themeOptions: { value: ThemeMode; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "亮色", icon: Sun },
    { value: "dark", label: "暗色", icon: Moon },
    { value: "system", label: "跟随系统", icon: Monitor },
  ];

  const taskProviderLabel =
    codexHealth?.task_execution_effective_provider === "sdk" ? "SDK" : "exec（自动回退）";
  const oneShotProviderLabel =
    codexHealth?.one_shot_effective_provider === "sdk" ? "SDK" : "exec（自动回退）";
  const installButtonLabel = codexHealth?.sdk_installed ? "重装 SDK" : "安装 SDK";
  const openDatabaseFolderTitle = !isTauriRuntime
    ? "仅桌面端支持打开数据库文件夹"
    : codexHealth?.database_path
      ? "打开数据库所在的文件夹"
      : "数据库路径不可用";

  async function handleSaveSdkSettings() {
    setSdkActionLoading("save");
    setSdkActionError(null);
    setSdkActionMessage(null);

    try {
      const nextSettings = await updateCodexSettings({
        task_sdk_enabled: taskSdkEnabled,
        one_shot_sdk_enabled: oneShotSdkEnabled,
        one_shot_model: oneShotModel,
        one_shot_reasoning_effort: oneShotReasoningEffort,
        node_path_override: nodePathOverride.trim() || null,
      });
      setCodexSettings(nextSettings);
      setSdkActionMessage("Codex SDK 配置已保存");
      await loadSettingsState();
    } catch (error) {
      console.error("Failed to save codex sdk settings:", error);
      setSdkActionError(error instanceof Error ? error.message : "保存 Codex SDK 配置失败");
    } finally {
      setSdkActionLoading(null);
    }
  }

  async function handleInstallSdk() {
    setSdkActionLoading("install");
    setSdkActionError(null);
    setSdkActionMessage(null);

    try {
      const result = await installCodexSdk();
      setSdkActionMessage(
        result.sdk_version
          ? `Codex SDK 安装完成，版本 ${result.sdk_version}`
          : result.message,
      );
      await loadSettingsState();
    } catch (error) {
      console.error("Failed to install codex sdk:", error);
      setSdkActionError(error instanceof Error ? error.message : "安装 Codex SDK 失败");
    } finally {
      setSdkActionLoading(null);
    }
  }

  async function handleBackupDatabase() {
    setDatabaseActionLoading("backup");
    setDatabaseActionError(null);
    setDatabaseActionMessage(null);

    try {
      const destination = await save({
        title: "导出 SQL 备份",
        defaultPath: buildBackupDefaultPath(codexHealth),
        filters: DATABASE_FILE_FILTERS,
      });

      if (!destination) {
        return;
      }

      const result = await backupDatabase(destination);
      setDatabaseActionMessage(result.message);
    } catch (error) {
      console.error("Failed to backup database:", error);
      setDatabaseActionError(error instanceof Error ? error.message : "导出 SQL 备份失败");
    } finally {
      setDatabaseActionLoading(null);
    }
  }

  async function handleOpenDatabaseFolder() {
    setDatabaseActionLoading("open-folder");
    setDatabaseActionError(null);
    setDatabaseActionMessage(null);

    try {
      await openDatabaseFolder();
    } catch (error) {
      console.error("Failed to open database folder:", error);
      setDatabaseActionError(error instanceof Error ? error.message : "打开数据库文件夹失败");
    } finally {
      setDatabaseActionLoading(null);
    }
  }

  async function handleRestoreDatabase() {
    setDatabaseActionLoading("restore");
    setDatabaseActionError(null);
    setDatabaseActionMessage(null);

    try {
      const confirmed = await confirm(
        "导入 SQL 会先自动备份当前数据库，再清空现有数据库并执行导入 SQL。",
        {
          title: "导入 SQL 备份",
          kind: "warning",
        },
      );

      if (!confirmed) {
        return;
      }

      const selected = await open({
        title: "选择 SQL 备份文件",
        directory: false,
        multiple: false,
        filters: DATABASE_FILE_FILTERS,
      });

      if (typeof selected !== "string") {
        return;
      }

      const result = await restoreDatabase(selected);
      setDatabaseActionMessage(result.message);
      await loadSettingsState();
      await message(
        `${result.message}\n\n导入前自动备份：${result.backup_path}`,
        {
          title: "SQL 导入完成",
          kind: "info",
        },
      );
    } catch (error) {
      console.error("Failed to restore database:", error);
      setDatabaseActionError(error instanceof Error ? error.message : "导入 SQL 备份失败");
    } finally {
      setDatabaseActionLoading(null);
    }
  }

  return (
    <div className="max-w-2xl space-y-6">
      <h2 className="text-lg font-semibold">系统设置</h2>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="mb-1 text-sm font-medium">主题模式</h3>
          <p className="mb-3 text-xs text-muted-foreground">选择应用的显示主题</p>
          <div className="flex gap-2">
            {themeOptions.map((opt) => (
              <button
                key={opt.value}
                onClick={() => setThemeMode(opt.value)}
                className={`flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm transition-colors ${
                  themeMode === opt.value
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-input hover:bg-accent"
                }`}
              >
                <opt.icon className="h-4 w-4" />
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h3 className="text-sm font-medium">Codex CLI</h3>
              <p className="text-xs text-muted-foreground">
                作为回退通道保留，用于 SDK 不可用时继续执行任务
              </p>
              {codexHealth?.codex_version && (
                <p className="mt-1 text-xs text-muted-foreground">
                  版本：{codexHealth.codex_version}
                </p>
              )}
            </div>
            <span
              className={`rounded px-2 py-1 text-xs ${
                codexHealth?.codex_available
                  ? "bg-green-100 text-green-700"
                  : "bg-amber-100 text-amber-700"
              }`}
            >
              {healthLoading ? "检测中" : codexHealth?.codex_available ? "已连接" : "不可用"}
            </span>
          </div>
          {codexHealth?.last_session_error && (
            <p className="mt-2 text-xs text-amber-700">
              最近错误：{codexHealth.last_session_error}
            </p>
          )}
        </div>

        <div className="border-t border-border pt-4">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <h3 className="text-sm font-medium">Codex SDK</h3>
              <p className="text-xs text-muted-foreground">
                任务运行与一次性 AI 优先走 SDK，失败时自动回退到 `codex exec`
              </p>
            </div>
            <span
              className={`rounded px-2 py-1 text-xs ${
                codexHealth?.task_execution_effective_provider === "sdk" ||
                codexHealth?.one_shot_effective_provider === "sdk"
                  ? "bg-green-100 text-green-700"
                  : "bg-slate-100 text-slate-700"
              }`}
            >
              {healthLoading
                ? "检测中"
                : `任务 ${taskProviderLabel} / AI ${oneShotProviderLabel}`}
            </span>
          </div>

          <div className="mt-4 space-y-4">
            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={taskSdkEnabled}
                onChange={(event) => setTaskSdkEnabled(event.target.checked)}
                disabled={healthLoading || sdkActionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">运行任务时使用 SDK</p>
                <p className="text-xs text-muted-foreground">
                  影响看板任务运行、员工启动任务，以及相关重启/恢复链路。
                </p>
              </div>
            </label>

            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={oneShotSdkEnabled}
                onChange={(event) => setOneShotSdkEnabled(event.target.checked)}
                disabled={healthLoading || sdkActionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">一次性 AI 使用 SDK</p>
                <p className="text-xs text-muted-foreground">
                  影响任务详情中的 AI 分析、评论生成、计划生成和子任务拆分。
                </p>
              </div>
            </label>

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <label className="text-sm font-medium">一次性 AI 模型</label>
                <Select<CodexModelId>
                  value={oneShotModel}
                  onValueChange={(value) => {
                    if (value) {
                      setOneShotModel(normalizeCodexModel(value));
                    }
                  }}
                  disabled={healthLoading || sdkActionLoading !== null}
                >
                  <SelectTrigger className="bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {CODEX_MODEL_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">一次性 AI 推理强度</label>
                <Select<ReasoningEffort>
                  value={oneShotReasoningEffort}
                  onValueChange={(value) => {
                    if (value) {
                      setOneShotReasoningEffort(normalizeReasoningEffort(value));
                    }
                  }}
                  disabled={healthLoading || sdkActionLoading !== null}
                >
                  <SelectTrigger className="bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {REASONING_EFFORT_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="space-y-2">
              <label htmlFor="node-path-override" className="text-sm font-medium">
                Node 路径覆盖（可选）
              </label>
              <Input
                id="node-path-override"
                value={nodePathOverride}
                onChange={(event) => setNodePathOverride(event.target.value)}
                placeholder="/opt/homebrew/bin/node"
                disabled={healthLoading || sdkActionLoading !== null}
              />
              <p className="text-xs text-muted-foreground">
                留空时自动从系统 PATH 中查找 Node。
              </p>
            </div>

            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p className="break-all">安装目录：{codexSettings?.sdk_install_dir ?? "检测中"}</p>
              <p>
                Node：{codexHealth?.node_available ? "可用" : "不可用"}
                {codexHealth?.node_version ? `（${codexHealth.node_version}）` : ""}
              </p>
              <p>
                SDK：{codexHealth?.sdk_installed ? "已安装" : "未安装"}
                {codexHealth?.sdk_version ? `（${codexHealth.sdk_version}）` : ""}
              </p>
              <p>任务运行引擎：{taskProviderLabel}</p>
              <p>一次性 AI 引擎：{oneShotProviderLabel}</p>
              <p>
                一次性 AI 默认模型：{CODEX_MODEL_OPTIONS.find((option) => option.value === oneShotModel)?.label ?? oneShotModel}
                {" / "}
                推理：{REASONING_EFFORT_OPTIONS.find((option) => option.value === oneShotReasoningEffort)?.label ?? oneShotReasoningEffort}
              </p>
              {codexHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">{codexHealth.sdk_status_message}</p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button
                onClick={() => void handleSaveSdkSettings()}
                disabled={healthLoading || sdkActionLoading !== null}
              >
                {sdkActionLoading === "save" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                保存配置
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleInstallSdk()}
                disabled={healthLoading || sdkActionLoading !== null}
              >
                {sdkActionLoading === "install" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {installButtonLabel}
              </Button>
              <Button
                variant="ghost"
                onClick={() => void loadSettingsState()}
                disabled={healthLoading || sdkActionLoading !== null}
              >
                <RefreshCw className={`h-4 w-4 ${healthLoading ? "animate-spin" : ""}`} />
                刷新检测
              </Button>
            </div>

            {sdkActionMessage && (
              <p className="text-xs text-green-700">{sdkActionMessage}</p>
            )}
            {sdkActionError && (
              <p className="text-xs text-destructive">{sdkActionError}</p>
            )}
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <div>
            <h3 className="text-sm font-medium">数据存储</h3>
            <p className="text-xs text-muted-foreground">
              所有数据存储在本地 SQLite 数据库中，无需网络连接
            </p>
            <div className="mt-3 grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p>
                当前数据版本：
                {codexHealth?.database_current_version != null
                  ? `v${codexHealth.database_current_version}`
                  : "检测中"}
              </p>
              <p>
                当前应用支持的最新版本：
                {codexHealth ? `v${codexHealth.database_latest_version}` : "检测中"}
              </p>
              <p>
                最近一次迁移：
                {codexHealth?.database_current_description ?? "暂无"}
              </p>
              {codexHealth?.database_path && (
                <p className="break-all">数据库：{codexHealth.database_path}</p>
              )}
            </div>

            <div className="mt-3 flex flex-wrap gap-2">
              <Button
                variant="outline"
                onClick={() => void handleOpenDatabaseFolder()}
                disabled={
                  !isTauriRuntime ||
                  !codexHealth?.database_path ||
                  healthLoading ||
                  databaseActionLoading !== null
                }
                title={openDatabaseFolderTitle}
              >
                {databaseActionLoading === "open-folder" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <FolderOpen className="h-4 w-4" />
                )}
                打开文件夹
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleBackupDatabase()}
                disabled={!isTauriRuntime || healthLoading || databaseActionLoading !== null}
                title={isTauriRuntime ? "导出 SQL 备份" : "仅桌面端支持导出 SQL 备份"}
              >
                {databaseActionLoading === "backup" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Download className="h-4 w-4" />
                )}
                导出 SQL
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleRestoreDatabase()}
                disabled={!isTauriRuntime || healthLoading || databaseActionLoading !== null}
                title={isTauriRuntime ? "导入 SQL 备份" : "仅桌面端支持导入 SQL 备份"}
              >
                {databaseActionLoading === "restore" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Upload className="h-4 w-4" />
                )}
                导入 SQL
              </Button>
            </div>

            {databaseActionMessage && (
              <p className="mt-2 text-xs text-green-700">{databaseActionMessage}</p>
            )}
            {databaseActionError && (
              <p className="mt-2 text-xs text-destructive">{databaseActionError}</p>
            )}
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <h3 className="mb-2 text-sm font-medium">键盘快捷键</h3>
          <div className="space-y-1 text-xs text-muted-foreground">
            <div className="flex justify-between">
              <span>跳转到看板</span>
              <kbd className="rounded bg-secondary px-1.5 py-0.5 text-[10px]">⌘N</kbd>
            </div>
            <div className="flex justify-between">
              <span>跳转到员工</span>
              <kbd className="rounded bg-secondary px-1.5 py-0.5 text-[10px]">⌘E</kbd>
            </div>
            <div className="flex justify-between">
              <span>跳转到仪表盘</span>
              <kbd className="rounded bg-secondary px-1.5 py-0.5 text-[10px]">⌘D</kbd>
            </div>
            <div className="flex justify-between">
              <span>跳转到项目</span>
              <kbd className="rounded bg-secondary px-1.5 py-0.5 text-[10px]">⌘P</kbd>
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-lg border border-border bg-card p-4">
        <h3 className="mb-2 text-sm font-medium">关于</h3>
        <div className="space-y-1 text-xs text-muted-foreground">
          <p>AI员工协作系统 v0.1.0</p>
          <p>基于 Tauri 2.0 + React 19 + SQLite</p>
          <p>技术栈：TypeScript, TailwindCSS, shadcn/ui, Zustand, @dnd-kit</p>
        </div>
      </div>
    </div>
  );
}
