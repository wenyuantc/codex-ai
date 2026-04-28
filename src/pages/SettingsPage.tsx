import { useEffect, useMemo, useState } from "react";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { useSearchParams } from "react-router-dom";

import { DatabaseSettingsTab } from "@/components/settings/DatabaseSettingsTab";
import { GitAutomationSettingsTab } from "@/components/settings/GitAutomationSettingsTab";
import { RuntimeSettingsTab } from "@/components/settings/RuntimeSettingsTab";
import { SshSettingsTab } from "@/components/settings/SshSettingsTab";
import {
  EMPTY_SSH_CONFIG_FORM,
  buildSshConfigFormState,
  getSectionForSettingsTab,
  getSettingsTabFromSection,
  type SettingsTabValue,
  type SshConfigFormState,
} from "@/components/settings/shared";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  backupDatabase,
  checkClaudeSdkHealth,
  createSshConfig as createSshConfigCommand,
  deleteSshConfig as deleteSshConfigCommand,
  getClaudeSettings,
  getCodexSettings,
  getRemoteCodexSettings,
  getRemoteHealthCheck,
  healthCheck,
  installClaudeSdk,
  installCodexSdk,
  installRemoteCodexSdk,
  openDatabaseFolder,
  restoreDatabase,
  syncSystemNotifications,
  updateClaudeSettings,
  updateCodexSettings,
  updateRemoteCodexSettings,
  updateSshConfig as updateSshConfigCommand,
  type CreateSshConfigInput,
  type UpdateSshConfigInput,
} from "@/lib/backend";
import {
  checkOpenCodeSdkHealth,
  getOpenCodeModels,
  getOpenCodeSettings,
  installOpenCodeSdk,
  updateOpenCodeSettings,
  type OpenCodeHealthCheck,
  type OpenCodeModelInfo,
} from "@/lib/opencode";
import { getEnvironmentModeLabel } from "@/lib/projects";
import {
  normalizeAiProvider,
  normalizeAiCommitMessageLength,
  normalizeModelForProvider,
  normalizeAiCommitModelSource,
  normalizeReasoningEffortForProvider,
  normalizeTaskAutomationFailureStrategy,
  normalizeWorktreeLocationMode,
  type AiProvider,
  type AiCommitMessageLength,
  type AiCommitModelSource,
  type ClaudeHealthCheck,
  type CodexHealthCheck,
  type CodexSettings,
  type GitPreferences,
  type RemoteCodexHealthCheck,
  type SshConfig,
  type TaskAutomationFailureStrategy,
  type WorktreeLocationMode,
} from "@/lib/types";
import { applyTheme, getThemePreference, type ThemeMode } from "@/lib/theme";
import { useProjectStore } from "@/stores/projectStore";

const DATABASE_FILE_FILTERS = [
  { name: "SQL 备份", extensions: ["sql"] },
];

const SETTINGS_TABS: Array<{ value: SettingsTabValue; label: string }> = [
  { value: "runtime", label: "界面与运行" },
  { value: "git", label: "Git 与自动质控" },
  { value: "ssh", label: "SSH 配置" },
  { value: "database", label: "数据库维护" },
];

const isTauriRuntime =
  typeof window !== "undefined" &&
  typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";

const DEFAULT_GIT_PREFERENCES: GitPreferences = {
  default_task_use_worktree: false,
  worktree_location_mode: "repo_sibling_hidden",
  worktree_custom_root: null,
  ai_commit_message_length: "title_with_body",
  ai_commit_preferred_provider: "codex",
  ai_commit_model_source: "inherit_one_shot",
  ai_commit_model: "gpt-5.4",
  ai_commit_reasoning_effort: "high",
};

const CLAUDE_DEFAULT_EFFORT_TO_BUDGET: Record<string, number> = {
  low: 5000,
  medium: 10000,
  high: 16000,
  xhigh: 32000,
  max: 128000,
};

function claudeBudgetToDefaultEffort(budget: number): string {
  if (budget >= CLAUDE_DEFAULT_EFFORT_TO_BUDGET.max) return "max";
  if (budget >= CLAUDE_DEFAULT_EFFORT_TO_BUDGET.xhigh) return "xhigh";
  if (budget >= CLAUDE_DEFAULT_EFFORT_TO_BUDGET.high) return "high";
  if (budget >= CLAUDE_DEFAULT_EFFORT_TO_BUDGET.medium) return "medium";
  return "low";
}

function claudeDefaultEffortToBudget(effort: string): number {
  return CLAUDE_DEFAULT_EFFORT_TO_BUDGET[effort] ?? CLAUDE_DEFAULT_EFFORT_TO_BUDGET.medium;
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
  const [searchParams, setSearchParams] = useSearchParams();
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
  const [oneShotPreferredProvider, setOneShotPreferredProvider] = useState<AiProvider>("codex");
  const [oneShotModel, setOneShotModel] = useState("gpt-5.4");
  const [oneShotReasoningEffort, setOneShotReasoningEffort] = useState("high");
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
  const [aiCommitModel, setAiCommitModel] = useState(DEFAULT_GIT_PREFERENCES.ai_commit_model);
  const [aiCommitReasoningEffort, setAiCommitReasoningEffort] = useState(
    DEFAULT_GIT_PREFERENCES.ai_commit_reasoning_effort,
  );
  const [gitAiProvider, setGitAiProvider] = useState<AiProvider>(
    DEFAULT_GIT_PREFERENCES.ai_commit_preferred_provider,
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
  const [claudeHealth, setClaudeHealth] = useState<ClaudeHealthCheck | null>(null);
  const [claudeSdkEnabled, setClaudeSdkEnabled] = useState(false);
  const [claudeDefaultModel, setClaudeDefaultModel] = useState("claude-sonnet-4-6");
  const [claudeDefaultEffort, setClaudeDefaultEffort] = useState("medium");
  const [claudeNodePathOverride, setClaudeNodePathOverride] = useState("");
  const [claudeCliPathOverride, setClaudeCliPathOverride] = useState("");
  const [claudeActionLoading, setClaudeActionLoading] = useState<"save" | "install" | null>(null);
  const [claudeActionMessage, setClaudeActionMessage] = useState<string | null>(null);
  const [claudeActionError, setClaudeActionError] = useState<string | null>(null);
  const [opencodeHealth, setOpenCodeHealth] = useState<OpenCodeHealthCheck | null>(null);
  const [opencodeSdkEnabled, setOpenCodeSdkEnabled] = useState(false);
  const [opencodeDefaultModel, setOpenCodeDefaultModel] = useState("openai/gpt-4o");
  const [opencodeHost, setOpenCodeHost] = useState("127.0.0.1");
  const [opencodePort, setOpenCodePort] = useState(4096);
  const [opencodeNodePathOverride, setOpenCodeNodePathOverride] = useState("");
  const [opencodeActionLoading, setOpenCodeActionLoading] = useState<"save" | "install" | null>(null);
  const [opencodeActionMessage, setOpenCodeActionMessage] = useState<string | null>(null);
  const [opencodeActionError, setOpenCodeActionError] = useState<string | null>(null);
  const [opencodeModelList, setOpenCodeModelList] = useState<OpenCodeModelInfo[]>([]);
  const [opencodeModelListLoading, setOpenCodeModelListLoading] = useState(false);

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
  const activeTab = getSettingsTabFromSection(searchParams.get("section"));
  const requestedSshConfigId = searchParams.get("sshConfigId");
  const selectedSshConfigSummary = selectedSshConfig
    ? `${selectedSshConfig.username}@${selectedSshConfig.host}:${selectedSshConfig.port}`
    : "未选择 SSH 配置";

  function replaceSettingsSearchParams(tab: SettingsTabValue, sshConfigId?: string | null) {
    const nextSearchParams = new URLSearchParams(searchParams);
    nextSearchParams.set("section", getSectionForSettingsTab(tab));

    if (tab === "ssh") {
      const nextSshConfigId = sshConfigId === undefined ? selectedSshConfigId : sshConfigId;
      if (nextSshConfigId) {
        nextSearchParams.set("sshConfigId", nextSshConfigId);
      } else {
        nextSearchParams.delete("sshConfigId");
      }
    } else {
      nextSearchParams.delete("sshConfigId");
    }

    setSearchParams(nextSearchParams, { replace: true });
  }

  function applySettingsToFormState(settings: CodexSettings) {
    const gitPreferences = settings.git_preferences ?? DEFAULT_GIT_PREFERENCES;
    const oneShotProvider = normalizeAiProvider(settings.one_shot_preferred_provider);

    setCodexSettings(settings);
    setTaskSdkEnabled(settings.task_sdk_enabled);
    setOneShotSdkEnabled(settings.one_shot_sdk_enabled);
    setOneShotPreferredProvider(oneShotProvider);
    setOneShotModel(normalizeModelForProvider(oneShotProvider, settings.one_shot_model));
    setOneShotReasoningEffort(
      normalizeReasoningEffortForProvider(oneShotProvider, settings.one_shot_reasoning_effort),
    );
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
    setAiCommitModel(normalizeModelForProvider(
      normalizeAiProvider(gitPreferences.ai_commit_preferred_provider),
      gitPreferences.ai_commit_model,
    ));
    setGitAiProvider(normalizeAiProvider(gitPreferences.ai_commit_preferred_provider));
    setAiCommitReasoningEffort(
      normalizeReasoningEffortForProvider(
        normalizeAiProvider(gitPreferences.ai_commit_preferred_provider),
        gitPreferences.ai_commit_reasoning_effort,
      ),
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

  async function loadOpenCodeState() {
    if (isRemoteMode) {
      setOpenCodeHealth(null);
      setOpenCodeSdkEnabled(false);
      setOpenCodeDefaultModel("openai/gpt-4o");
      setOpenCodeHost("127.0.0.1");
      setOpenCodePort(4096);
      setOpenCodeNodePathOverride("");
      setOpenCodeActionError(null);
      setOpenCodeActionMessage(null);
      return;
    }

    try {
      const [health, settings] = await Promise.all([
        checkOpenCodeSdkHealth(),
        getOpenCodeSettings(),
      ]);
      setOpenCodeHealth(health);
      setOpenCodeSdkEnabled(settings.sdk_enabled);
      setOpenCodeDefaultModel(settings.default_model);
      setOpenCodeHost(settings.host);
      setOpenCodePort(settings.port);
      setOpenCodeNodePathOverride(settings.node_path_override ?? "");
    } catch (error) {
      console.error("Failed to load OpenCode settings:", error);
    }
  }

  async function loadClaudeState() {
    if (isRemoteMode) {
      setClaudeHealth(null);
      setClaudeSdkEnabled(false);
      setClaudeDefaultModel("claude-sonnet-4-6");
      setClaudeDefaultEffort("medium");
      setClaudeNodePathOverride("");
      setClaudeCliPathOverride("");
      setClaudeActionError(null);
      setClaudeActionMessage(null);
      return;
    }

    try {
      const [health, settings] = await Promise.all([
        checkClaudeSdkHealth(),
        getClaudeSettings(),
      ]);
      setClaudeHealth(health);
      setClaudeSdkEnabled(settings.sdk_enabled);
      setClaudeDefaultModel(settings.default_model);
      setClaudeDefaultEffort(claudeBudgetToDefaultEffort(settings.default_thinking_budget));
      setClaudeNodePathOverride(settings.node_path_override ?? "");
      setClaudeCliPathOverride(settings.cli_path_override ?? "");
    } catch (error) {
      console.error("Failed to load Claude settings:", error);
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
    void loadClaudeState();
    void loadOpenCodeState();
  }, [environmentMode, selectedSshConfigId]);

  useEffect(() => {
    if (isRemoteMode || oneShotPreferredProvider !== "opencode") return;
    void handleFetchOpenCodeModels();
  }, [isRemoteMode, oneShotPreferredProvider]);

  const resetSshForm = () => {
    setEditingSshConfigId(null);
    setSshForm(EMPTY_SSH_CONFIG_FORM);
    setSshFormError(null);
    setSshFormMessage(null);

    if (activeTab === "ssh") {
      replaceSettingsSearchParams("ssh", null);
    }
  };

  const handleTabChange = (value: string) => {
    if (!SETTINGS_TABS.some((tab) => tab.value === value)) {
      return;
    }

    replaceSettingsSearchParams(value as SettingsTabValue);
  };

  const handleSelectSshConfig = (config: SshConfig) => {
    setSelectedSshConfigId(config.id);
    setEditingSshConfigId(config.id);
    setSshForm(buildSshConfigFormState(config));
    setSshFormError(null);
    setSshFormMessage(null);

    if (activeTab === "ssh") {
      replaceSettingsSearchParams("ssh", config.id);
    }
  };

  const handleSshFormChange = (updates: Partial<SshConfigFormState>) => {
    setSshForm((current) => ({ ...current, ...updates }));
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
        one_shot_preferred_provider: oneShotPreferredProvider,
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
          ai_commit_preferred_provider: gitAiProvider,
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

  async function handleSaveOpenCodeSettings() {
    if (isRemoteMode) {
      setOpenCodeActionError("OpenCode SDK 配置仅适用于本地执行目标。");
      setOpenCodeActionMessage(null);
      return;
    }

    setOpenCodeActionLoading("save");
    setOpenCodeActionError(null);
    setOpenCodeActionMessage(null);
    try {
      await updateOpenCodeSettings({
        sdk_enabled: opencodeSdkEnabled,
        default_model: opencodeDefaultModel,
        host: opencodeHost,
        port: opencodePort,
        node_path_override: opencodeNodePathOverride.trim() || null,
      });
      setOpenCodeActionMessage("OpenCode 设置已保存");
      await loadOpenCodeState();
    } catch (error) {
      setOpenCodeActionError(error instanceof Error ? error.message : "保存 OpenCode 设置失败");
    } finally {
      setOpenCodeActionLoading(null);
    }
  }

  async function handleFetchOpenCodeModels() {
    setOpenCodeModelListLoading(true);
    setOpenCodeActionError(null);
    try {
      const models = await getOpenCodeModels();
      setOpenCodeModelList(models);
      if (models.length > 0 && !models.some((m) => m.value === opencodeDefaultModel)) {
        setOpenCodeDefaultModel(models[0].value);
      }
      if (models.length > 0 && !models.some((m) => m.value === oneShotModel)) {
        setOneShotModel(models[0].value);
      }
      if (
        models.length > 0
        && aiCommitModelSource === "custom"
        && gitAiProvider === "opencode"
        && !models.some((m) => m.value === aiCommitModel)
      ) {
        setAiCommitModel(models[0].value);
      }
    } catch (error) {
      setOpenCodeActionError(error instanceof Error ? error.message : "获取模型列表失败");
    } finally {
      setOpenCodeModelListLoading(false);
    }
  }

  async function handleInstallOpenCodeSdk() {
    if (isRemoteMode) {
      setOpenCodeActionError("OpenCode SDK 安装仅适用于本地执行目标。");
      setOpenCodeActionMessage(null);
      return;
    }

    setOpenCodeActionLoading("install");
    setOpenCodeActionError(null);
    setOpenCodeActionMessage(null);
    try {
      const result = await installOpenCodeSdk();
      setOpenCodeActionMessage(
        result.sdk_version
          ? `OpenCode SDK 安装完成，版本 ${result.sdk_version}`
          : result.message,
      );
      await loadOpenCodeState();
      // SDK newly installed, auto-fetch models
      await handleFetchOpenCodeModels();
    } catch (error) {
      setOpenCodeActionError(error instanceof Error ? error.message : "安装 OpenCode SDK 失败");
    } finally {
      setOpenCodeActionLoading(null);
    }
  }

  async function handleSaveClaudeSettings() {
    if (isRemoteMode) {
      setClaudeActionError("Claude SDK 配置仅适用于本地执行目标，SSH 目标会使用远端 Claude CLI。");
      setClaudeActionMessage(null);
      return;
    }

    setClaudeActionLoading("save");
    setClaudeActionError(null);
    setClaudeActionMessage(null);
    try {
      await updateClaudeSettings({
        sdk_enabled: claudeSdkEnabled,
        default_model: claudeDefaultModel,
        default_thinking_budget: claudeDefaultEffortToBudget(claudeDefaultEffort),
        node_path_override: claudeNodePathOverride.trim() || null,
        cli_path_override: claudeCliPathOverride.trim() || null,
      });
      setClaudeActionMessage("Claude 设置已保存");
      await loadClaudeState();
    } catch (error) {
      setClaudeActionError(error instanceof Error ? error.message : "保存 Claude 设置失败");
    } finally {
      setClaudeActionLoading(null);
    }
  }

  async function handleInstallClaudeSdk() {
    if (isRemoteMode) {
      setClaudeActionError("Claude SDK 安装仅适用于本地执行目标，SSH 目标请在远端安装 Claude CLI。");
      setClaudeActionMessage(null);
      return;
    }

    setClaudeActionLoading("install");
    setClaudeActionError(null);
    setClaudeActionMessage(null);
    try {
      const result = await installClaudeSdk();
      setClaudeActionMessage(
        result.sdk_version
          ? `Claude SDK 安装完成，版本 ${result.sdk_version}`
          : result.message,
      );
      await loadClaudeState();
    } catch (error) {
      setClaudeActionError(error instanceof Error ? error.message : "安装 Claude SDK 失败");
    } finally {
      setClaudeActionLoading(null);
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
      await message(`${result.message}\n\n导入前自动备份：${result.backup_path}`, {
        title: "SQL 导入完成",
        kind: "info",
      });
    } catch (error) {
      console.error("Failed to restore database:", error);
      setDatabaseActionError(error instanceof Error ? error.message : "导入 SQL 备份失败");
    } finally {
      setDatabaseActionLoading(null);
    }
  }

  async function handleSelectPrivateKeyFile() {
    try {
      const selected = await open({
        title: "选择私钥文件",
        directory: false,
        multiple: false,
        defaultPath: sshForm.privateKeyPath.trim() || undefined,
      });

      if (typeof selected !== "string") {
        return;
      }

      setSshForm((current) => ({ ...current, privateKeyPath: selected }));
      setSshFormError(null);
    } catch (error) {
      console.error("Failed to select SSH private key file:", error);
      setSshFormError(error instanceof Error ? error.message : "选择私钥文件失败");
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

      if (activeTab === "ssh") {
        replaceSettingsSearchParams("ssh", sshConfig.id);
      }
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

  return (
    <div className="max-w-5xl space-y-6">
      <div>
        <h2 className="text-lg font-semibold">系统设置</h2>
        <p className="text-sm text-muted-foreground">
          当前处于 {getEnvironmentModeLabel(environmentMode)}，Codex 运行配置与 SSH 配置分开保存。
        </p>
      </div>

      <Tabs value={activeTab} onValueChange={handleTabChange} className="gap-6">
        <div className="overflow-x-auto overflow-y-hidden [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
          <TabsList variant="line" className="min-w-max justify-start">
            {SETTINGS_TABS.map((tab) => (
              <TabsTrigger key={tab.value} value={tab.value}>
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>

        <TabsContent value="runtime">
          <RuntimeSettingsTab
            codexHealth={codexHealth}
            codexSettings={codexSettings}
            healthLoading={healthLoading}
            actionLoading={sdkActionLoading}
            actionMessage={sdkActionMessage}
            actionError={sdkActionError}
            isRemoteMode={isRemoteMode}
            hasSelectedSshConfig={Boolean(selectedSshConfig)}
            remoteTargetName={remoteTargetName}
            selectedSshConfigSummary={selectedSshConfigSummary}
            passwordAuthBlocked={passwordAuthBlocked}
            taskSdkEnabled={taskSdkEnabled}
            oneShotSdkEnabled={oneShotSdkEnabled}
            oneShotPreferredProvider={oneShotPreferredProvider}
            oneShotModel={oneShotModel}
            oneShotReasoningEffort={oneShotReasoningEffort}
            nodePathOverride={nodePathOverride}
            themeMode={themeMode}
            onThemeModeChange={setThemeMode}
            onTaskSdkEnabledChange={setTaskSdkEnabled}
            onOneShotSdkEnabledChange={setOneShotSdkEnabled}
            onOneShotPreferredProviderChange={(provider) => {
              setOneShotPreferredProvider(provider);
              setOneShotModel((current) => normalizeModelForProvider(provider, current));
              setOneShotReasoningEffort((current) =>
                normalizeReasoningEffortForProvider(provider, current),
              );
            }}
            onOneShotModelChange={setOneShotModel}
            onOneShotReasoningEffortChange={setOneShotReasoningEffort}
            onNodePathOverrideChange={setNodePathOverride}
            onSave={() => void handleSaveSdkSettings()}
            onInstall={() => void handleInstallSdk()}
            onRefresh={() => void loadRuntimeState()}
            claudeHealth={claudeHealth}
            claudeSdkEnabled={claudeSdkEnabled}
            claudeDefaultModel={claudeDefaultModel}
            claudeDefaultEffort={claudeDefaultEffort}
            claudeNodePathOverride={claudeNodePathOverride}
            claudeCliPathOverride={claudeCliPathOverride}
            claudeActionLoading={claudeActionLoading}
            claudeActionMessage={claudeActionMessage}
            claudeActionError={claudeActionError}
            onClaudeSdkEnabledChange={setClaudeSdkEnabled}
            onClaudeDefaultModelChange={setClaudeDefaultModel}
            onClaudeDefaultEffortChange={setClaudeDefaultEffort}
            onClaudeNodePathOverrideChange={setClaudeNodePathOverride}
            onClaudeCliPathOverrideChange={setClaudeCliPathOverride}
            onClaudeSave={() => void handleSaveClaudeSettings()}
            onClaudeInstall={() => void handleInstallClaudeSdk()}
            onClaudeRefresh={() => void loadClaudeState()}
            opencodeHealth={opencodeHealth}
            opencodeSdkEnabled={opencodeSdkEnabled}
            opencodeDefaultModel={opencodeDefaultModel}
            opencodeHost={opencodeHost}
            opencodePort={opencodePort}
            opencodeNodePathOverride={opencodeNodePathOverride}
            opencodeActionLoading={opencodeActionLoading}
            opencodeActionMessage={opencodeActionMessage}
            opencodeActionError={opencodeActionError}
            opencodeModelList={opencodeModelList}
            opencodeModelListLoading={opencodeModelListLoading}
            onOpenCodeSdkEnabledChange={setOpenCodeSdkEnabled}
            onOpenCodeDefaultModelChange={setOpenCodeDefaultModel}
            onOpenCodeHostChange={setOpenCodeHost}
            onOpenCodePortChange={setOpenCodePort}
            onOpenCodeNodePathOverrideChange={setOpenCodeNodePathOverride}
            onOpenCodeFetchModels={() => void handleFetchOpenCodeModels()}
            onOpenCodeSave={() => void handleSaveOpenCodeSettings()}
            onOpenCodeInstall={() => void handleInstallOpenCodeSdk()}
            onOpenCodeRefresh={() => void loadOpenCodeState()}
          />
        </TabsContent>

        <TabsContent value="git">
          <GitAutomationSettingsTab
            isRemoteMode={isRemoteMode}
            selectedSshConfigId={selectedSshConfigId}
            healthLoading={healthLoading}
            actionLoading={sdkActionLoading}
            actionMessage={sdkActionMessage}
            actionError={sdkActionError}
            taskAutomationDefaultEnabled={taskAutomationDefaultEnabled}
            taskAutomationMaxFixRounds={taskAutomationMaxFixRounds}
            taskAutomationFailureStrategy={taskAutomationFailureStrategy}
            defaultTaskUseWorktree={defaultTaskUseWorktree}
            worktreeLocationMode={worktreeLocationMode}
            worktreeCustomRoot={worktreeCustomRoot}
            aiCommitMessageLength={aiCommitMessageLength}
            aiCommitModelSource={aiCommitModelSource}
            gitAiProvider={gitAiProvider}
            aiCommitModel={aiCommitModel}
            aiCommitReasoningEffort={aiCommitReasoningEffort}
            opencodeModelList={opencodeModelList}
            opencodeModelListLoading={opencodeModelListLoading}
            onTaskAutomationDefaultEnabledChange={setTaskAutomationDefaultEnabled}
            onTaskAutomationMaxFixRoundsChange={setTaskAutomationMaxFixRounds}
            onTaskAutomationFailureStrategyChange={setTaskAutomationFailureStrategy}
            onDefaultTaskUseWorktreeChange={setDefaultTaskUseWorktree}
            onWorktreeLocationModeChange={setWorktreeLocationMode}
            onWorktreeCustomRootChange={setWorktreeCustomRoot}
            onAiCommitMessageLengthChange={setAiCommitMessageLength}
            onAiCommitModelSourceChange={setAiCommitModelSource}
            onGitAiProviderChange={(provider) => {
              setGitAiProvider(provider);
              setAiCommitModel((current) => normalizeModelForProvider(provider, current));
              setAiCommitReasoningEffort((current) =>
                normalizeReasoningEffortForProvider(provider, current),
              );
            }}
            onAiCommitModelChange={setAiCommitModel}
            onAiCommitReasoningEffortChange={setAiCommitReasoningEffort}
            onOpenCodeFetchModels={() => void handleFetchOpenCodeModels()}
            onSave={() => void handleSaveSdkSettings()}
          />
        </TabsContent>

        <TabsContent value="ssh">
          <SshSettingsTab
            isTauriRuntime={isTauriRuntime}
            sshConfigs={sshConfigs}
            sshConfigsLoading={sshConfigsLoading}
            selectedSshConfigId={selectedSshConfigId}
            selectedSshConfig={selectedSshConfig}
            editingSshConfigId={editingSshConfigId}
            sshForm={sshForm}
            sshFormLoading={sshFormLoading}
            sshFormMessage={sshFormMessage}
            sshFormError={sshFormError}
            onResetForm={resetSshForm}
            onSelectConfig={handleSelectSshConfig}
            onFormChange={handleSshFormChange}
            onSelectPrivateKeyFile={() => void handleSelectPrivateKeyFile()}
            onSave={() => void handleSaveSshConfig()}
            onTestConnection={() => void handleTestConnection()}
            onDelete={() => void handleDeleteSshConfig()}
          />
        </TabsContent>

        <TabsContent value="database">
          <DatabaseSettingsTab
            codexHealth={codexHealth}
            isTauriRuntime={isTauriRuntime}
            actionLoading={databaseActionLoading}
            actionMessage={databaseActionMessage}
            actionError={databaseActionError}
            onBackup={() => void handleBackupDatabase()}
            onRestore={() => void handleRestoreDatabase()}
            onOpenFolder={() => void handleOpenDatabaseFolder()}
          />
        </TabsContent>
      </Tabs>
    </div>
  );
}
