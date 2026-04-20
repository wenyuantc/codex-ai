import { useEffect, useMemo, useRef, useState } from "react";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { useSearchParams } from "react-router-dom";
import {
  Download,
  FolderOpen,
  Loader2,
  Monitor,
  Moon,
  Plus,
  RefreshCw,
  ServerCog,
  ShieldAlert,
  Sun,
  Trash2,
  Upload,
} from "lucide-react";

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
  createSshConfig as createSshConfigCommand,
  deleteSshConfig as deleteSshConfigCommand,
  getCodexSettings,
  getRemoteCodexSettings,
  getRemoteHealthCheck,
  healthCheck,
  installCodexSdk,
  installRemoteCodexSdk,
  openDatabaseFolder,
  restoreDatabase,
  syncSystemNotifications,
  updateCodexSettings,
  updateRemoteCodexSettings,
  updateSshConfig as updateSshConfigCommand,
  type CreateSshConfigInput,
  type UpdateSshConfigInput,
} from "@/lib/backend";
import { getEnvironmentModeLabel } from "@/lib/projects";
import {
  AI_COMMIT_MESSAGE_LENGTH_OPTIONS,
  AI_COMMIT_MODEL_SOURCE_OPTIONS,
  CODEX_MODEL_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS,
  WORKTREE_LOCATION_MODE_OPTIONS,
  normalizeAiCommitMessageLength,
  normalizeAiCommitModelSource,
  normalizeCodexModel,
  normalizeReasoningEffort,
  normalizeTaskAutomationFailureStrategy,
  normalizeWorktreeLocationMode,
  type AiCommitMessageLength,
  type AiCommitModelSource,
  type CodexHealthCheck,
  type CodexModelId,
  type CodexSettings,
  type GitPreferences,
  type ReasoningEffort,
  type RemoteCodexHealthCheck,
  type SshAuthType,
  type SshConfig,
  type TaskAutomationFailureStrategy,
  type WorktreeLocationMode,
} from "@/lib/types";
import { applyTheme, getThemePreference, type ThemeMode } from "@/lib/theme";
import { formatDate } from "@/lib/utils";
import { useProjectStore } from "@/stores/projectStore";

const DATABASE_FILE_FILTERS = [
  { name: "SQL 备份", extensions: ["sql"] },
];

const KNOWN_HOSTS_OPTIONS = [
  { value: "accept-new", label: "首次连接自动接受" },
  { value: "strict", label: "严格校验" },
  { value: "off", label: "关闭校验（不推荐）" },
];

const isTauriRuntime =
  typeof window !== "undefined" &&
  typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";

interface SshConfigFormState {
  name: string;
  host: string;
  port: string;
  username: string;
  authType: SshAuthType;
  privateKeyPath: string;
  password: string;
  passphrase: string;
  knownHostsMode: string;
}

const EMPTY_SSH_CONFIG_FORM: SshConfigFormState = {
  name: "",
  host: "",
  port: "22",
  username: "",
  authType: "key",
  privateKeyPath: "",
  password: "",
  passphrase: "",
  knownHostsMode: "accept-new",
};

const DEFAULT_GIT_PREFERENCES: GitPreferences = {
  default_task_use_worktree: false,
  worktree_location_mode: "repo_sibling_hidden",
  worktree_custom_root: null,
  ai_commit_message_length: "title_with_body",
  ai_commit_model_source: "inherit_one_shot",
  ai_commit_model: "gpt-5.4",
  ai_commit_reasoning_effort: "high",
};

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

function buildSshConfigFormState(config: SshConfig | null): SshConfigFormState {
  if (!config) {
    return EMPTY_SSH_CONFIG_FORM;
  }

  return {
    name: config.name,
    host: config.host,
    port: String(config.port || 22),
    username: config.username,
    authType: config.auth_type,
    privateKeyPath: config.private_key_path ?? "",
    password: "",
    passphrase: "",
    knownHostsMode: config.known_hosts_mode ?? "accept-new",
  };
}

export function SettingsPage() {
  const [searchParams] = useSearchParams();
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const sshConfigs = useProjectStore((state) => state.sshConfigs);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const sshConfigsLoading = useProjectStore((state) => state.sshConfigsLoading);
  const setSelectedSshConfigId = useProjectStore((state) => state.setSelectedSshConfigId);
  const fetchSshConfigs = useProjectStore((state) => state.fetchSshConfigs);

  const [themeMode, setThemeMode] = useState<ThemeMode>(getThemePreference);
  const [codexHealth, setCodexHealth] = useState<CodexHealthCheck | RemoteCodexHealthCheck | null>(null);
  const [codexSettings, setCodexSettings] = useState<CodexSettings | null>(null);
  const [taskSdkEnabled, setTaskSdkEnabled] = useState(false);
  const [oneShotSdkEnabled, setOneShotSdkEnabled] = useState(false);
  const [oneShotModel, setOneShotModel] = useState<CodexModelId>("gpt-5.4");
  const [oneShotReasoningEffort, setOneShotReasoningEffort] = useState<ReasoningEffort>("high");
  const [taskAutomationDefaultEnabled, setTaskAutomationDefaultEnabled] = useState(false);
  const [taskAutomationMaxFixRounds, setTaskAutomationMaxFixRounds] = useState(3);
  const [taskAutomationFailureStrategy, setTaskAutomationFailureStrategy] =
    useState<TaskAutomationFailureStrategy>("blocked");
  const [defaultTaskUseWorktree, setDefaultTaskUseWorktree] = useState(
    DEFAULT_GIT_PREFERENCES.default_task_use_worktree,
  );
  const [worktreeLocationMode, setWorktreeLocationMode] = useState<WorktreeLocationMode>(
    DEFAULT_GIT_PREFERENCES.worktree_location_mode,
  );
  const [worktreeCustomRoot, setWorktreeCustomRoot] = useState(
    DEFAULT_GIT_PREFERENCES.worktree_custom_root ?? "",
  );
  const [aiCommitMessageLength, setAiCommitMessageLength] = useState<AiCommitMessageLength>(
    DEFAULT_GIT_PREFERENCES.ai_commit_message_length,
  );
  const [aiCommitModelSource, setAiCommitModelSource] = useState<AiCommitModelSource>(
    DEFAULT_GIT_PREFERENCES.ai_commit_model_source,
  );
  const [aiCommitModel, setAiCommitModel] = useState<CodexModelId>(
    DEFAULT_GIT_PREFERENCES.ai_commit_model,
  );
  const [aiCommitReasoningEffort, setAiCommitReasoningEffort] = useState<ReasoningEffort>(
    DEFAULT_GIT_PREFERENCES.ai_commit_reasoning_effort,
  );
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
  const [editingSshConfigId, setEditingSshConfigId] = useState<string | null>(null);
  const [sshForm, setSshForm] = useState<SshConfigFormState>(EMPTY_SSH_CONFIG_FORM);
  const [sshFormLoading, setSshFormLoading] = useState<"save" | "delete" | "probe" | null>(null);
  const [sshFormMessage, setSshFormMessage] = useState<string | null>(null);
  const [sshFormError, setSshFormError] = useState<string | null>(null);
  const sdkSectionRef = useRef<HTMLDivElement | null>(null);
  const sshSectionRef = useRef<HTMLDivElement | null>(null);
  const databaseSectionRef = useRef<HTMLDivElement | null>(null);

  const selectedSshConfig = useMemo(
    () => sshConfigs.find((config) => config.id === selectedSshConfigId) ?? null,
    [selectedSshConfigId, sshConfigs],
  );

  const isRemoteMode = environmentMode === "ssh";
  const remoteTargetName = selectedSshConfig?.name ?? "当前 SSH 配置";
  const passwordAuthBlocked = Boolean(
    isRemoteMode
    && selectedSshConfig
    && selectedSshConfig.auth_type === "password"
    && !selectedSshConfig.password_execution_allowed,
  );
  const highlightedSection = (() => {
    const section = searchParams.get("section");
    return section === "sdk" || section === "ssh" || section === "database"
      ? section
      : null;
  })();
  const requestedSshConfigId = searchParams.get("sshConfigId");

  function getSectionCardClass(section: "sdk" | "ssh" | "database") {
    return highlightedSection === section
      ? "ring-2 ring-primary/40 ring-offset-2 ring-offset-background"
      : "";
  }

  function applySettingsToFormState(settings: CodexSettings) {
    const gitPreferences = settings.git_preferences ?? DEFAULT_GIT_PREFERENCES;

    setCodexSettings(settings);
    setTaskSdkEnabled(settings.task_sdk_enabled);
    setOneShotSdkEnabled(settings.one_shot_sdk_enabled);
    setOneShotModel(normalizeCodexModel(settings.one_shot_model));
    setOneShotReasoningEffort(normalizeReasoningEffort(settings.one_shot_reasoning_effort));
    setTaskAutomationDefaultEnabled(settings.task_automation_default_enabled);
    setTaskAutomationMaxFixRounds(settings.task_automation_max_fix_rounds);
    setTaskAutomationFailureStrategy(
      normalizeTaskAutomationFailureStrategy(settings.task_automation_failure_strategy),
    );
    setDefaultTaskUseWorktree(gitPreferences.default_task_use_worktree);
    setWorktreeLocationMode(normalizeWorktreeLocationMode(gitPreferences.worktree_location_mode));
    setWorktreeCustomRoot(gitPreferences.worktree_custom_root ?? "");
    setAiCommitMessageLength(
      normalizeAiCommitMessageLength(gitPreferences.ai_commit_message_length),
    );
    setAiCommitModelSource(normalizeAiCommitModelSource(gitPreferences.ai_commit_model_source));
    setAiCommitModel(normalizeCodexModel(gitPreferences.ai_commit_model));
    setAiCommitReasoningEffort(
      normalizeReasoningEffort(gitPreferences.ai_commit_reasoning_effort),
    );
    setNodePathOverride(settings.node_path_override ?? "");
  }

  async function refreshNotificationHealth(nextSshConfigId = selectedSshConfigId) {
    try {
      await syncSystemNotifications(environmentMode, nextSshConfigId);
    } catch (error) {
      console.error("Failed to refresh notification health state:", error);
    }
  }

  async function loadRuntimeState() {
    setHealthLoading(true);
    setSdkActionError(null);

    try {
      if (isRemoteMode) {
        if (!selectedSshConfigId) {
          setCodexHealth(null);
          setCodexSettings(null);
          setSdkActionError("当前没有可用的 SSH 配置，请先创建并选择 SSH 配置。");
          return;
        }

        const [health, settings] = await Promise.all([
          getRemoteHealthCheck(selectedSshConfigId),
          getRemoteCodexSettings(selectedSshConfigId),
        ]);
        setCodexHealth(health);
        applySettingsToFormState(settings);
        return;
      }

      const [health, settings] = await Promise.all([healthCheck(), getCodexSettings()]);
      setCodexHealth(health);
      applySettingsToFormState(settings);
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
  }, [themeMode]);

  useEffect(() => {
    void fetchSshConfigs();
  }, [fetchSshConfigs]);

  useEffect(() => {
    if (requestedSshConfigId) {
      setSelectedSshConfigId(requestedSshConfigId);
      setEditingSshConfigId(requestedSshConfigId);
    }
  }, [requestedSshConfigId, setSelectedSshConfigId]);

  useEffect(() => {
    if (!editingSshConfigId) {
      setSshForm(EMPTY_SSH_CONFIG_FORM);
      return;
    }

    setSshForm(buildSshConfigFormState(selectedSshConfig));
  }, [editingSshConfigId, selectedSshConfig]);

  useEffect(() => {
    void loadRuntimeState();
  }, [environmentMode, selectedSshConfigId]);

  useEffect(() => {
    const sectionRef =
      highlightedSection === "sdk"
        ? sdkSectionRef
        : highlightedSection === "ssh"
          ? sshSectionRef
          : highlightedSection === "database"
            ? databaseSectionRef
            : null;

    if (!sectionRef?.current) {
      return;
    }

    const frameId = window.requestAnimationFrame(() => {
      sectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [highlightedSection]);

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

  const resetSshForm = () => {
    setEditingSshConfigId(null);
    setSshForm(EMPTY_SSH_CONFIG_FORM);
    setSshFormError(null);
    setSshFormMessage(null);
  };

  async function handleSaveSdkSettings() {
    if (worktreeLocationMode === "custom_root" && !worktreeCustomRoot.trim()) {
      setSdkActionError("自定义 Worktree 根目录不能为空。");
      setSdkActionMessage(null);
      return;
    }

    setSdkActionLoading("save");
    setSdkActionError(null);
    setSdkActionMessage(null);

    try {
      const updates = {
        task_sdk_enabled: taskSdkEnabled,
        one_shot_sdk_enabled: oneShotSdkEnabled,
        one_shot_model: oneShotModel,
        one_shot_reasoning_effort: oneShotReasoningEffort,
        task_automation_default_enabled: taskAutomationDefaultEnabled,
        task_automation_max_fix_rounds: taskAutomationMaxFixRounds,
        task_automation_failure_strategy: taskAutomationFailureStrategy,
        git_preferences: {
          default_task_use_worktree: defaultTaskUseWorktree,
          worktree_location_mode: worktreeLocationMode,
          worktree_custom_root: worktreeCustomRoot.trim() || null,
          ai_commit_message_length: aiCommitMessageLength,
          ai_commit_model_source: aiCommitModelSource,
          ai_commit_model: aiCommitModel,
          ai_commit_reasoning_effort: aiCommitReasoningEffort,
        },
        node_path_override: nodePathOverride.trim() || null,
      };
      const nextSettings = isRemoteMode && selectedSshConfigId
        ? await updateRemoteCodexSettings(selectedSshConfigId, updates)
        : await updateCodexSettings(updates);
      setCodexSettings(nextSettings);
      setSdkActionMessage(isRemoteMode ? `远程配置已保存到 ${remoteTargetName}` : "系统设置已保存");
      await loadRuntimeState();
      await refreshNotificationHealth();
    } catch (error) {
      console.error("Failed to save codex sdk settings:", error);
      setSdkActionError(error instanceof Error ? error.message : "保存 Codex 配置失败");
    } finally {
      setSdkActionLoading(null);
    }
  }

  async function handleInstallSdk() {
    if (isRemoteMode && !selectedSshConfigId) {
      setSdkActionError("请先选择 SSH 配置后再安装远程 SDK。");
      return;
    }

    setSdkActionLoading("install");
    setSdkActionError(null);
    setSdkActionMessage(null);

    try {
      const result = isRemoteMode && selectedSshConfigId
        ? await installRemoteCodexSdk(selectedSshConfigId)
        : await installCodexSdk();
      setSdkActionMessage(
        result.sdk_version
          ? `${isRemoteMode ? "远程" : "本地"} SDK 安装完成，版本 ${result.sdk_version}`
          : result.message,
      );
      await loadRuntimeState();
      await refreshNotificationHealth();
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
      await loadRuntimeState();
      await refreshNotificationHealth();
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

  async function handleSaveSshConfig() {
    if (!sshForm.name.trim() || !sshForm.host.trim() || !sshForm.username.trim()) {
      setSshFormError("SSH 配置名称、主机和用户名不能为空。");
      return;
    }
    if (sshForm.authType === "key" && !sshForm.privateKeyPath.trim()) {
      setSshFormError("密钥登录必须填写私钥路径。");
      return;
    }

    setSshFormLoading("save");
    setSshFormError(null);
    setSshFormMessage(null);

    try {
      const payload: CreateSshConfigInput | UpdateSshConfigInput = {
        name: sshForm.name.trim(),
        host: sshForm.host.trim(),
        port: Number(sshForm.port) || 22,
        username: sshForm.username.trim(),
        auth_type: sshForm.authType,
        private_key_path: sshForm.authType === "key" ? sshForm.privateKeyPath.trim() || null : null,
        password: sshForm.authType === "password" && sshForm.password ? sshForm.password : null,
        passphrase: sshForm.passphrase || null,
        known_hosts_mode: sshForm.knownHostsMode,
      };

      const sshConfig = editingSshConfigId
        ? await updateSshConfigCommand(editingSshConfigId, payload)
        : await createSshConfigCommand(payload as CreateSshConfigInput);
      await fetchSshConfigs();
      setSelectedSshConfigId(sshConfig.id);
      setEditingSshConfigId(sshConfig.id);
      setSshForm(buildSshConfigFormState(sshConfig));
      setSshFormMessage(editingSshConfigId ? "SSH 配置已更新。" : "SSH 配置已创建。");
      await refreshNotificationHealth(sshConfig.id);
    } catch (error) {
      console.error("Failed to save SSH config:", error);
      setSshFormError(error instanceof Error ? error.message : "保存 SSH 配置失败");
    } finally {
      setSshFormLoading(null);
    }
  }

  async function handleDeleteSshConfig() {
    if (!editingSshConfigId || !selectedSshConfig) {
      return;
    }

    const confirmed = await confirm(`确认删除 SSH 配置“${selectedSshConfig.name}”？`, {
      title: "删除 SSH 配置",
      kind: "warning",
    });

    if (!confirmed) {
      return;
    }

    setSshFormLoading("delete");
    setSshFormError(null);
    setSshFormMessage(null);

    try {
      await deleteSshConfigCommand(editingSshConfigId);
      await fetchSshConfigs();
      resetSshForm();
      setSshFormMessage("SSH 配置已删除。");
      await refreshNotificationHealth(null);
    } catch (error) {
      console.error("Failed to delete SSH config:", error);
      setSshFormError(error instanceof Error ? error.message : "删除 SSH 配置失败");
    } finally {
      setSshFormLoading(null);
    }
  }

  async function handleTestConnection() {
    if (!selectedSshConfigId) {
      return;
    }

    setSshFormLoading("probe");
    setSshFormError(null);
    setSshFormMessage(null);

    try {
      if (selectedSshConfig?.auth_type === "password") {
        const result = await useProjectStore.getState().runSshPasswordProbe(selectedSshConfigId);
        setSshFormMessage(`测试连接结果：${result.message}`);
      } else {
        const health = await getRemoteHealthCheck(selectedSshConfigId);
        setSshFormMessage(
          health.codex_available
            ? "测试连接成功，远程主机可访问。"
            : `测试连接成功，但远程 Codex 当前不可用：${health.sdk_status_message}`,
        );
      }
      await loadRuntimeState();
      await refreshNotificationHealth();
    } catch (error) {
      console.error("Failed to test SSH connection:", error);
      setSshFormError(error instanceof Error ? error.message : "SSH 测试连接失败");
    } finally {
      setSshFormLoading(null);
    }
  }

  const selectedSshConfigSummary = selectedSshConfig
    ? `${selectedSshConfig.username}@${selectedSshConfig.host}:${selectedSshConfig.port}`
    : "未选择 SSH 配置";
  const showCustomWorktreeRoot = worktreeLocationMode === "custom_root";
  const gitAiUsesCustomModel = aiCommitModelSource === "custom";
  const selectedWorktreeLocationOption = WORKTREE_LOCATION_MODE_OPTIONS.find(
    (option) => option.value === worktreeLocationMode,
  );
  const selectedCommitLengthOption = AI_COMMIT_MESSAGE_LENGTH_OPTIONS.find(
    (option) => option.value === aiCommitMessageLength,
  );
  const selectedCommitModelSourceOption = AI_COMMIT_MODEL_SOURCE_OPTIONS.find(
    (option) => option.value === aiCommitModelSource,
  );
  const worktreeRootPlaceholder = isRemoteMode
    ? "~/codex-worktrees"
    : "/Users/wenyuan/codex-worktrees";

  return (
    <div className="max-w-4xl space-y-6">
      <div>
        <h2 className="text-lg font-semibold">系统设置</h2>
        <p className="text-sm text-muted-foreground">
          当前处于 {getEnvironmentModeLabel(environmentMode)}，Codex 运行配置与 SSH 配置分开保存。
        </p>
      </div>

      <div
        ref={sdkSectionRef}
        className={`space-y-4 rounded-lg border border-border bg-card p-4 ${getSectionCardClass("sdk")}`}
      >
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
              <h3 className="text-sm font-medium">
                {isRemoteMode ? "远程执行目标" : "本地执行目标"}
              </h3>
              <p className="text-xs text-muted-foreground">
                {isRemoteMode
                  ? `当前远程配置：${remoteTargetName}（${selectedSshConfigSummary}）`
                  : "当前保存的是本地运行配置。"}
              </p>
            </div>
            <span className="rounded bg-secondary px-2 py-1 text-xs text-secondary-foreground">
              {isRemoteMode ? "SSH Profile" : "Local Profile"}
            </span>
          </div>

          {isRemoteMode && !selectedSshConfig && (
            <div className="mt-3 rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
              当前是 SSH 模式，但还没有可用的 SSH 配置。请先在下方新增 SSH 配置。
            </div>
          )}

          {passwordAuthBlocked && (
            <div className="mt-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
              <div className="flex items-center gap-2 font-medium">
                <ShieldAlert className="h-4 w-4" />
                配置存在但当前平台不可执行
              </div>
              <p className="mt-1 text-xs leading-5">
                当前 SSH 配置使用密码认证，但测试连接尚未通过。远程 Codex 校验、SDK 安装和实际执行链路都必须保持阻断。
              </p>
              {selectedSshConfig?.password_probe_message && (
                <p className="mt-2 text-xs">{selectedSshConfig.password_probe_message}</p>
              )}
            </div>
          )}
        </div>

        <div className="border-t border-border pt-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h3 className="text-sm font-medium">Codex CLI</h3>
              <p className="text-xs text-muted-foreground">
                {isRemoteMode
                  ? "SSH 模式下会校验当前 SSH 配置对应远程主机的 Codex 环境。"
                  : "作为回退通道保留，用于 SDK 不可用时继续执行任务。"}
              </p>
              {codexHealth?.codex_version && (
                <p className="mt-1 text-xs text-muted-foreground">版本：{codexHealth.codex_version}</p>
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
          {codexHealth?.target_host_label && (
            <p className="mt-2 text-xs text-muted-foreground">主机：{codexHealth.target_host_label}</p>
          )}
          {codexHealth?.last_session_error && (
            <p className="mt-2 text-xs text-amber-700">最近错误：{codexHealth.last_session_error}</p>
          )}
        </div>

        <div className="border-t border-border pt-4">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <h3 className="text-sm font-medium">Codex SDK</h3>
              <p className="text-xs text-muted-foreground">
                {isRemoteMode
                  ? "SSH 模式下任务运行与一次性 AI 会优先使用远程 SDK；如果远程 SDK 不可用，则自动回退到远程 codex exec。"
                  : "任务运行与一次性 AI 优先走 SDK，失败时自动回退到 `codex exec`"}
              </p>
            </div>
            <span
              className={`rounded px-2 py-1 text-xs ${
                codexHealth?.task_execution_effective_provider === "sdk"
                || codexHealth?.one_shot_effective_provider === "sdk"
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

            <div className="space-y-3 rounded-md border border-border px-3 py-3">
              <div className="space-y-1">
                <h4 className="text-sm font-medium">自动质控默认设置</h4>
                <p className="text-xs text-muted-foreground">
                  影响新建任务默认是否开启自动质控，以及自动审核/自动修复闭环的默认策略。
                </p>
              </div>

              <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
                <input
                  type="checkbox"
                  className="mt-0.5 h-4 w-4 rounded border-input"
                  checked={taskAutomationDefaultEnabled}
                  onChange={(event) => setTaskAutomationDefaultEnabled(event.target.checked)}
                  disabled={healthLoading || sdkActionLoading !== null}
                />
                <div className="space-y-1">
                  <p className="text-sm font-medium">新建任务默认开启自动质控</p>
                  <p className="text-xs text-muted-foreground">
                    开启后，新任务会默认进入“审核 {"->"} 修复 {"->"} 再审核”的闭环流程。
                  </p>
                </div>
              </label>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <label className="text-sm font-medium">最大自动修复轮次</label>
                  <Select
                    value={String(taskAutomationMaxFixRounds)}
                    onValueChange={(value) => {
                      const nextValue = Number(value);
                      if (Number.isFinite(nextValue)) {
                        setTaskAutomationMaxFixRounds(nextValue);
                      }
                    }}
                    disabled={healthLoading || sdkActionLoading !== null}
                  >
                    <SelectTrigger className="bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string" && value.trim()
                            ? `${value} 轮`
                            : "选择轮次"
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {Array.from({ length: 10 }, (_, index) => index + 1).map((round) => (
                        <SelectItem key={round} value={String(round)}>
                          {round} 轮
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-medium">失败后处理</label>
                  <Select<TaskAutomationFailureStrategy>
                    value={taskAutomationFailureStrategy}
                    onValueChange={(value) => {
                      if (value) {
                        setTaskAutomationFailureStrategy(
                          normalizeTaskAutomationFailureStrategy(value),
                        );
                      }
                    }}
                    disabled={healthLoading || sdkActionLoading !== null}
                  >
                    <SelectTrigger className="bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string"
                            ? TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS.find(
                                (option) => option.value === value,
                              )?.label ?? value
                            : "选择失败策略"
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <p className="text-xs leading-5 text-muted-foreground">
                当前策略：
                {taskAutomationDefaultEnabled ? " 新任务默认开启自动质控；" : " 新任务默认关闭自动质控；"}
                最多自动修复 {taskAutomationMaxFixRounds} 轮；
                失败后{taskAutomationFailureStrategy === "manual_control" ? "转人工处理" : "转阻塞"}。
              </p>
            </div>

            <div className="space-y-3 rounded-md border border-border px-3 py-3">
              <div className="space-y-1">
                <h4 className="text-sm font-medium">Git 偏好</h4>
                <p className="text-xs text-muted-foreground">
                  控制新任务的 Worktree 默认行为，以及 AI 生成 Git 提交信息时的长度、模型和推理强度。
                </p>
              </div>

              <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
                <input
                  type="checkbox"
                  className="mt-0.5 h-4 w-4 rounded border-input"
                  checked={defaultTaskUseWorktree}
                  onChange={(event) => setDefaultTaskUseWorktree(event.target.checked)}
                  disabled={healthLoading || sdkActionLoading !== null}
                />
                <div className="space-y-1">
                  <p className="text-sm font-medium">新建任务默认启用 Worktree</p>
                  <p className="text-xs text-muted-foreground">
                    开启后，新建任务会默认准备独立 Worktree；仍然可以在任务创建弹窗里单独改掉。
                  </p>
                </div>
              </label>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Worktree 目录规则</label>
                  <Select<WorktreeLocationMode>
                    value={worktreeLocationMode}
                    onValueChange={(value) => {
                      if (value) {
                        setWorktreeLocationMode(normalizeWorktreeLocationMode(value));
                      }
                    }}
                    disabled={healthLoading || sdkActionLoading !== null}
                  >
                    <SelectTrigger className="bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string"
                            ? WORKTREE_LOCATION_MODE_OPTIONS.find((option) => option.value === value)?.label ?? value
                            : "选择 Worktree 目录规则"
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {WORKTREE_LOCATION_MODE_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-muted-foreground">
                    {selectedWorktreeLocationOption?.description}
                  </p>
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-medium">AI 提交信息默认长度</label>
                  <Select<AiCommitMessageLength>
                    value={aiCommitMessageLength}
                    onValueChange={(value) => {
                      if (value) {
                        setAiCommitMessageLength(normalizeAiCommitMessageLength(value));
                      }
                    }}
                    disabled={healthLoading || sdkActionLoading !== null}
                  >
                    <SelectTrigger className="bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string"
                            ? AI_COMMIT_MESSAGE_LENGTH_OPTIONS.find((option) => option.value === value)?.label ?? value
                            : "选择提交信息长度"
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {AI_COMMIT_MESSAGE_LENGTH_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-muted-foreground">
                    {selectedCommitLengthOption?.description}
                  </p>
                </div>
              </div>

              {showCustomWorktreeRoot && (
                <div className="space-y-2">
                  <label htmlFor="worktree-custom-root" className="text-sm font-medium">
                    自定义 Worktree 根目录
                  </label>
                  <Input
                    id="worktree-custom-root"
                    value={worktreeCustomRoot}
                    onChange={(event) => setWorktreeCustomRoot(event.target.value)}
                    placeholder={worktreeRootPlaceholder}
                    disabled={healthLoading || sdkActionLoading !== null}
                  />
                  <p className="text-xs text-muted-foreground">
                    {isRemoteMode
                      ? "SSH 配置下要求绝对路径或 ~/ 开头，最终目录结构为 <root>/<repo>/<task>。"
                      : "本地配置下要求绝对路径，最终目录结构为 <root>/<repo>/<task>。"}
                  </p>
                </div>
              )}

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Git AI 模型来源</label>
                  <Select<AiCommitModelSource>
                    value={aiCommitModelSource}
                    onValueChange={(value) => {
                      if (value) {
                        setAiCommitModelSource(normalizeAiCommitModelSource(value));
                      }
                    }}
                    disabled={healthLoading || sdkActionLoading !== null}
                  >
                    <SelectTrigger className="bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string"
                            ? AI_COMMIT_MODEL_SOURCE_OPTIONS.find((option) => option.value === value)?.label ?? value
                            : "选择 Git AI 模型来源"
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {AI_COMMIT_MODEL_SOURCE_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-muted-foreground">
                    {selectedCommitModelSourceOption?.description}
                  </p>
                </div>
              </div>

              {gitAiUsesCustomModel && (
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Git AI 模型</label>
                    <Select<CodexModelId>
                      value={aiCommitModel}
                      onValueChange={(value) => {
                        if (value) {
                          setAiCommitModel(normalizeCodexModel(value));
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
                    <label className="text-sm font-medium">Git AI 推理强度</label>
                    <Select<ReasoningEffort>
                      value={aiCommitReasoningEffort}
                      onValueChange={(value) => {
                        if (value) {
                          setAiCommitReasoningEffort(normalizeReasoningEffort(value));
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
              )}

              <p className="text-xs leading-5 text-muted-foreground">
                当前策略：
                {defaultTaskUseWorktree ? " 新任务默认启用 Worktree；" : " 新任务默认关闭 Worktree；"}
                Worktree 目录使用{selectedWorktreeLocationOption?.label ?? "仓库同级隐藏目录"}；
                AI 提交信息默认{selectedCommitLengthOption?.label ?? "标题+详情"}；
                Git AI {selectedCommitModelSourceOption?.label ?? "跟随一次性 AI"}
                {gitAiUsesCustomModel ? `（${aiCommitModel} / 推理 ${aiCommitReasoningEffort}）` : ""}。
              </p>
            </div>

            <div className="space-y-2">
              <label htmlFor="node-path-override" className="text-sm font-medium">
                Node 路径覆盖（可选）
              </label>
              <Input
                id="node-path-override"
                value={nodePathOverride}
                onChange={(event) => setNodePathOverride(event.target.value)}
                placeholder={isRemoteMode ? "/usr/local/bin/node" : "/opt/homebrew/bin/node"}
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
              {codexHealth?.checked_at && (
                <p>检测时间：{formatDate(codexHealth.checked_at)}</p>
              )}
              {codexHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">{codexHealth.sdk_status_message}</p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button
                onClick={() => void handleSaveSdkSettings()}
                disabled={healthLoading || sdkActionLoading !== null || (isRemoteMode && !selectedSshConfigId)}
              >
                {sdkActionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                保存配置
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleInstallSdk()}
                disabled={
                  healthLoading
                  || sdkActionLoading !== null
                  || (isRemoteMode && (!selectedSshConfigId || passwordAuthBlocked))
                }
              >
                {sdkActionLoading === "install" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {installButtonLabel}
              </Button>
              <Button
                variant="ghost"
                onClick={() => void loadRuntimeState()}
                disabled={healthLoading || sdkActionLoading !== null}
              >
                <RefreshCw className={`h-4 w-4 ${healthLoading ? "animate-spin" : ""}`} />
                刷新检测
              </Button>
            </div>

            {sdkActionMessage && <p className="text-xs text-green-700">{sdkActionMessage}</p>}
            {sdkActionError && <p className="text-xs text-destructive">{sdkActionError}</p>}
          </div>
        </div>
      </div>

      <div
        ref={sshSectionRef}
        className={`space-y-4 rounded-lg border border-border bg-card p-4 ${getSectionCardClass("ssh")}`}
      >
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-sm font-medium">SSH 配置管理</h3>
            <p className="text-xs text-muted-foreground">
              支持多个 SSH 配置；SSH 项目会固定绑定其中一项配置和一个远程仓库目录。
            </p>
          </div>
          <Button variant="outline" onClick={resetSshForm}>
            <Plus className="mr-1 h-4 w-4" />
            新建配置
          </Button>
        </div>

        <div className="grid gap-4 lg:grid-cols-[18rem,1fr]">
          <div className="space-y-2">
            <div className="rounded-md border border-border">
              {sshConfigsLoading ? (
                <div className="flex h-28 items-center justify-center text-sm text-muted-foreground">
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  正在读取 SSH 配置...
                </div>
              ) : sshConfigs.length === 0 ? (
                <div className="px-3 py-6 text-sm text-muted-foreground">
                  当前还没有 SSH 配置。
                </div>
              ) : (
                sshConfigs.map((config) => (
                  <button
                    key={config.id}
                    type="button"
                    onClick={() => {
                      setSelectedSshConfigId(config.id);
                      setEditingSshConfigId(config.id);
                      setSshForm(buildSshConfigFormState(config));
                      setSshFormError(null);
                      setSshFormMessage(null);
                    }}
                    className={`w-full border-b border-border px-3 py-3 text-left last:border-b-0 ${
                      selectedSshConfigId === config.id ? "bg-primary/5" : "hover:bg-muted/40"
                    }`}
                  >
                    <div className="text-sm font-medium">{config.name}</div>
                    <div className="mt-1 text-xs text-muted-foreground">
                      {config.username}@{config.host}:{config.port}
                    </div>
                    <div className="mt-2 flex flex-wrap gap-2 text-[11px]">
                      <span className="rounded bg-secondary px-1.5 py-0.5 text-secondary-foreground">
                        {config.auth_type === "password" ? "密码登录" : "密钥登录"}
                      </span>
                      {config.last_checked_at && (
                        <span className="rounded border border-border px-1.5 py-0.5 text-muted-foreground">
                          检测于 {formatDate(config.last_checked_at)}
                        </span>
                      )}
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>

          <div className="space-y-3 rounded-md border border-border p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h4 className="text-sm font-medium">
                  {editingSshConfigId ? "编辑 SSH 配置" : "新建 SSH 配置"}
                </h4>
                <p className="text-xs text-muted-foreground">
                  {editingSshConfigId ? "更新后会保留当前配置引用。" : "保存后可用于 SSH 项目和远程运行设置。"}
                </p>
              </div>
              {selectedSshConfig && (
                <span className="rounded bg-secondary px-2 py-1 text-xs text-secondary-foreground">
                  {selectedSshConfig.auth_type === "password" ? "密码认证" : "密钥认证"}
                </span>
              )}
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <label className="text-xs font-medium text-muted-foreground">配置名称 *</label>
                <Input
                  value={sshForm.name}
                  onChange={(event) => setSshForm((current) => ({ ...current, name: event.target.value }))}
                  placeholder="生产主机"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">主机 *</label>
                <Input
                  value={sshForm.host}
                  onChange={(event) => setSshForm((current) => ({ ...current, host: event.target.value }))}
                  placeholder="10.0.0.12"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">端口</label>
                <Input
                  value={sshForm.port}
                  onChange={(event) => setSshForm((current) => ({ ...current, port: event.target.value }))}
                  placeholder="22"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">用户名 *</label>
                <Input
                  value={sshForm.username}
                  onChange={(event) => setSshForm((current) => ({ ...current, username: event.target.value }))}
                  placeholder="deploy"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">认证方式</label>
                <Select<SshAuthType>
                  value={sshForm.authType}
                  onValueChange={(value) => {
                    if (value) {
                      setSshForm((current) => ({ ...current, authType: value }));
                    }
                  }}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="key">密钥登录</SelectItem>
                    <SelectItem value="password">账号密码登录</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Known Hosts 策略</label>
                <Select
                  value={sshForm.knownHostsMode}
                  onValueChange={(value) => setSshForm((current) => ({ ...current, knownHostsMode: value ?? "accept-new" }))}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {KNOWN_HOSTS_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            {sshForm.authType === "key" ? (
              <div className="grid gap-3 md:grid-cols-2">
                <div>
                  <label className="text-xs font-medium text-muted-foreground">私钥路径 *</label>
                  <Input
                    value={sshForm.privateKeyPath}
                    onChange={(event) => setSshForm((current) => ({ ...current, privateKeyPath: event.target.value }))}
                    placeholder="~/.ssh/id_ed25519"
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">Passphrase（可选）</label>
                  <Input
                    type="password"
                    value={sshForm.passphrase}
                    onChange={(event) => setSshForm((current) => ({ ...current, passphrase: event.target.value }))}
                    placeholder={selectedSshConfig?.passphrase_configured ? "留空表示保持现有 passphrase" : "可选"}
                    className="mt-1"
                  />
                </div>
              </div>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                <div>
                  <label className="text-xs font-medium text-muted-foreground">密码</label>
                  <Input
                    type="password"
                    value={sshForm.password}
                    onChange={(event) => setSshForm((current) => ({ ...current, password: event.target.value }))}
                    placeholder={selectedSshConfig?.password_configured ? "留空表示保持现有密码" : "输入登录密码"}
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">Passphrase（可选）</label>
                  <Input
                    type="password"
                    value={sshForm.passphrase}
                    onChange={(event) => setSshForm((current) => ({ ...current, passphrase: event.target.value }))}
                    placeholder={selectedSshConfig?.passphrase_configured ? "留空表示保持现有 passphrase" : "可选"}
                    className="mt-1"
                  />
                </div>
              </div>
            )}

            {selectedSshConfig && (
              <div className="rounded-md border border-border bg-muted/30 px-3 py-3 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">当前配置状态</div>
                <div className="mt-1">主机：{selectedSshConfigSummary}</div>
                <div className="mt-1">
                  连接测试：
                  {(selectedSshConfig.auth_type === "password"
                    ? selectedSshConfig.password_probe_status
                    : selectedSshConfig.last_check_status)
                    ? ` ${selectedSshConfig.auth_type === "password"
                      ? selectedSshConfig.password_probe_status
                      : selectedSshConfig.last_check_status}`
                    : " 未检测"}
                </div>
                {(selectedSshConfig.auth_type === "password"
                  ? selectedSshConfig.password_probe_message
                  : selectedSshConfig.last_check_message) && (
                  <div className="mt-1">
                    {selectedSshConfig.auth_type === "password"
                      ? selectedSshConfig.password_probe_message
                      : selectedSshConfig.last_check_message}
                  </div>
                )}
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <Button onClick={() => void handleSaveSshConfig()} disabled={sshFormLoading !== null}>
                {sshFormLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {editingSshConfigId ? "保存 SSH 配置" : "创建 SSH 配置"}
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleTestConnection()}
                disabled={sshFormLoading !== null || !selectedSshConfigId}
              >
                {sshFormLoading === "probe" ? <Loader2 className="h-4 w-4 animate-spin" /> : <ServerCog className="h-4 w-4" />}
                测试连接
              </Button>
              <Button
                variant="destructive"
                onClick={() => void handleDeleteSshConfig()}
                disabled={sshFormLoading !== null || !editingSshConfigId}
              >
                {sshFormLoading === "delete" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Trash2 className="h-4 w-4" />}
                删除配置
              </Button>
            </div>

            {sshFormMessage && <p className="text-xs text-green-700">{sshFormMessage}</p>}
            {sshFormError && <p className="text-xs text-destructive">{sshFormError}</p>}
          </div>
        </div>
      </div>

      <div
        ref={databaseSectionRef}
        className={`space-y-4 rounded-lg border border-border bg-card p-4 ${getSectionCardClass("database")}`}
      >
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

        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            onClick={() => void handleBackupDatabase()}
            disabled={databaseActionLoading !== null}
          >
            {databaseActionLoading === "backup" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            导出 SQL
          </Button>
          <Button
            variant="outline"
            onClick={() => void handleRestoreDatabase()}
            disabled={databaseActionLoading !== null}
          >
            {databaseActionLoading === "restore" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Upload className="h-4 w-4" />}
            导入 SQL
          </Button>
          <Button
            variant="ghost"
            onClick={() => void handleOpenDatabaseFolder()}
            disabled={databaseActionLoading !== null || !isTauriRuntime || !codexHealth?.database_path}
            title={openDatabaseFolderTitle}
          >
            {databaseActionLoading === "open-folder" ? <Loader2 className="h-4 w-4 animate-spin" /> : <FolderOpen className="h-4 w-4" />}
            打开数据库目录
          </Button>
        </div>

        {databaseActionMessage && <p className="text-xs text-green-700">{databaseActionMessage}</p>}
        {databaseActionError && <p className="text-xs text-destructive">{databaseActionError}</p>}
      </div>
    </div>
  );
}
