import { useEffect, useMemo, useState } from "react";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { DatabaseSettingsTab } from "@/components/settings/DatabaseSettingsTab";
import { GitAutomationSettingsTab } from "@/components/settings/GitAutomationSettingsTab";
import { McpSettingsTab } from "@/components/settings/McpSettingsTab";
import { PromptSettingsTab } from "@/components/settings/PromptSettingsTab";
import { RuntimeSettingsTab } from "@/components/settings/RuntimeSettingsTab";
import { SshSettingsTab } from "@/components/settings/SshSettingsTab";
import { EngineCapabilityBadges } from "@/components/ai/EngineCapabilityBadges";
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
  checkGrokHealth,
  getGrokSettings,
  listGrokModels,
  updateGrokSettings,
  validateRemoteGrokHealth,
  createSshConfig as createSshConfigCommand,
  deleteSshConfig as deleteSshConfigCommand,
  getClaudeSettings,
  getCodexSettings,
  getRemoteCodexSettings,
  getRemoteHealthCheck,
  healthCheck,
  installClaudeSdk,
  installCodexSdk,
  installGrokCli,
  installRemoteCodexSdk,
  installRemoteGrokCli,
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
  installRemoteOpenCodeSdk,
  updateOpenCodeSettings,
  validateRemoteOpenCodeHealth,
  type OpenCodeHealthCheck,
  type OpenCodeModelInfo,
  type RemoteOpenCodeHealthCheck,
} from "@/lib/opencode";
import { changeAppLocale } from "@/lib/i18n";
import { getLocalePreference, type AppLocale } from "@/lib/i18n/locale";
import { getEnvironmentModeLabel } from "@/lib/projects";
import {
  normalizeAiProvider,
  normalizeAiCommitMessageLength,
  normalizeClaudeModel,
  normalizeModelForProvider,
  normalizeAiCommitModelSource,
  normalizeReasoningEffortForProvider,
  normalizeTaskAutomationFailureStrategy,
  normalizeWorktreeLocationMode,
  type AiProvider,
  type AiCommitMessageLength,
  type AiCommitModelSource,
  type ClaudeHealthCheck,
  type GrokHealthCheck,
  type GrokModelInfo,
  type RemoteGrokHealthCheck,
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

const SETTINGS_TAB_KEYS: Array<{ value: SettingsTabValue; labelKey: string }> = [
  { value: "runtime", labelKey: "settings:tabs.runtime" },
  { value: "git", labelKey: "settings:tabs.git" },
  { value: "prompts", labelKey: "settings:tabs.prompts" },
  { value: "mcp", labelKey: "settings:tabs.mcp" },
  { value: "ssh", labelKey: "settings:tabs.ssh" },
  { value: "database", labelKey: "settings:tabs.database" },
];

const isTauriRuntime =
  typeof window !== "undefined" &&
  typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
    "undefined";

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

  const lastSeparatorIndex = Math.max(
    databasePath.lastIndexOf("/"),
    databasePath.lastIndexOf("\\"),
  );
  if (lastSeparatorIndex < 0) return fileName;

  const directory = databasePath.slice(0, lastSeparatorIndex);
  const separator = directory.includes("\\") ? "\\" : "/";
  return `${directory}${separator}${fileName}`;
}

export function SettingsPage() {
  const { t } = useTranslation("settings");
  const [searchParams, setSearchParams] = useSearchParams();
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const sshConfigs = useProjectStore((state) => state.sshConfigs);
  const selectedSshConfigId = useProjectStore((state) => state.selectedSshConfigId);
  const sshConfigsLoading = useProjectStore((state) => state.sshConfigsLoading);
  const setSelectedSshConfigId = useProjectStore((state) => state.setSelectedSshConfigId);
  const fetchSshConfigs = useProjectStore((state) => state.fetchSshConfigs);

  const [themeMode, setThemeMode] = useState<ThemeMode>(getThemePreference);
  const [locale, setLocale] = useState<AppLocale>(getLocalePreference);
  const [codexHealth, setCodexHealth] = useState<CodexHealthCheck | RemoteCodexHealthCheck | null>(
    null,
  );
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
  const [testerAutomationEnabled, setTesterAutomationEnabled] = useState(false);
  const [testerAllowAiOnly, setTesterAllowAiOnly] = useState(false);
  const [defaultTestCommand, setDefaultTestCommand] = useState("");
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
  const [sdkActionFeedbackKind, setSdkActionFeedbackKind] = useState<"save" | "install" | null>(
    null,
  );
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
  const [claudeDefaultModel, setClaudeDefaultModel] = useState("sonnet");
  const [claudeDefaultEffort, setClaudeDefaultEffort] = useState("medium");
  const [claudeNodePathOverride, setClaudeNodePathOverride] = useState("");
  const [claudeCliPathOverride, setClaudeCliPathOverride] = useState("");
  const [claudeActionLoading, setClaudeActionLoading] = useState<"save" | "install" | null>(null);
  const [claudeActionMessage, setClaudeActionMessage] = useState<string | null>(null);
  const [claudeActionError, setClaudeActionError] = useState<string | null>(null);
  const [opencodeHealth, setOpenCodeHealth] = useState<OpenCodeHealthCheck | null>(null);
  const [remoteOpenCodeHealth, setRemoteOpenCodeHealth] =
    useState<RemoteOpenCodeHealthCheck | null>(null);
  const [opencodeSdkEnabled, setOpenCodeSdkEnabled] = useState(false);
  const [opencodeDefaultModel, setOpenCodeDefaultModel] = useState("openai/gpt-4o");
  const [opencodeHost, setOpenCodeHost] = useState("127.0.0.1");
  const [opencodePort, setOpenCodePort] = useState(4096);
  const [opencodeNodePathOverride, setOpenCodeNodePathOverride] = useState("");
  const [opencodeActionLoading, setOpenCodeActionLoading] = useState<"save" | "install" | null>(
    null,
  );
  const [opencodeActionMessage, setOpenCodeActionMessage] = useState<string | null>(null);
  const [opencodeActionError, setOpenCodeActionError] = useState<string | null>(null);
  const [opencodeModelList, setOpenCodeModelList] = useState<OpenCodeModelInfo[]>([]);
  const [opencodeModelListLoading, setOpenCodeModelListLoading] = useState(false);
  const [grokHealth, setGrokHealth] = useState<GrokHealthCheck | null>(null);
  const [remoteGrokHealth, setRemoteGrokHealth] = useState<RemoteGrokHealthCheck | null>(null);
  const [grokDefaultModel, setGrokDefaultModel] = useState("grok-4.5");
  const [grokDefaultEffort, setGrokDefaultEffort] = useState("high");
  const [grokCliPathOverride, setGrokCliPathOverride] = useState("");
  const [grokActionLoading, setGrokActionLoading] = useState<"save" | "install" | null>(null);
  const [grokActionMessage, setGrokActionMessage] = useState<string | null>(null);
  const [grokActionError, setGrokActionError] = useState<string | null>(null);
  const [grokModelList, setGrokModelList] = useState<GrokModelInfo[]>([]);
  const [grokModelListLoading, setGrokModelListLoading] = useState(false);

  const selectedSshConfig = useMemo(
    () => sshConfigs.find((config) => config.id === selectedSshConfigId) ?? null,
    [selectedSshConfigId, sshConfigs],
  );

  const isRemoteMode = environmentMode === "ssh";
  const remoteTargetName = selectedSshConfig?.name ?? t("runtime.target.currentSshConfig");
  const passwordAuthBlocked = Boolean(
    isRemoteMode &&
    selectedSshConfig &&
    selectedSshConfig.auth_type === "password" &&
    !selectedSshConfig.password_execution_allowed,
  );
  const activeTab = getSettingsTabFromSection(searchParams.get("section"));
  const requestedSshConfigId = searchParams.get("sshConfigId");
  const selectedSshConfigSummary = selectedSshConfig
    ? `${selectedSshConfig.username}@${selectedSshConfig.host}:${selectedSshConfig.port}`
    : t("runtime.target.noSshConfigSelected");
  const databaseFileFilters = useMemo(
    () => [{ name: t("page.databaseDialogs.fileFilterName"), extensions: ["sql"] }],
    [t],
  );

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
    setTesterAutomationEnabled(settings.tester_automation_enabled ?? false);
    setTesterAllowAiOnly(settings.tester_allow_ai_only ?? false);
    setDefaultTestCommand(settings.default_test_command ?? "");
    setDefaultTaskUseWorktree(gitPreferences.default_task_use_worktree);
    setWorktreeLocationMode(normalizeWorktreeLocationMode(gitPreferences.worktree_location_mode));
    setWorktreeCustomRoot(gitPreferences.worktree_custom_root ?? "");
    setAiCommitMessageLength(
      normalizeAiCommitMessageLength(gitPreferences.ai_commit_message_length),
    );
    setAiCommitModelSource(normalizeAiCommitModelSource(gitPreferences.ai_commit_model_source));
    setAiCommitModel(
      normalizeModelForProvider(
        normalizeAiProvider(gitPreferences.ai_commit_preferred_provider),
        gitPreferences.ai_commit_model,
      ),
    );
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
          setSdkActionError(t("page.messages.runtime.noSshConfigAvailable"));
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
      setSdkActionError(
        error instanceof Error ? error.message : t("page.messages.runtime.loadCodexSettingsFailed"),
      );
    } finally {
      setHealthLoading(false);
    }
  }

  async function loadOpenCodeState() {
    setOpenCodeActionError(null);
    if (isRemoteMode) {
      setOpenCodeHealth(null);
      setOpenCodeSdkEnabled(false);
      setOpenCodeDefaultModel("openai/gpt-4o");
      setOpenCodeHost("127.0.0.1");
      setOpenCodePort(4096);
      setOpenCodeNodePathOverride("");
      if (!selectedSshConfigId) {
        setRemoteOpenCodeHealth(null);
        return;
      }
      try {
        setRemoteOpenCodeHealth(await validateRemoteOpenCodeHealth(selectedSshConfigId));
      } catch (error) {
        setRemoteOpenCodeHealth({
          available: false,
          node_available: false,
          node_version: null,
          sdk_installed: false,
          sdk_version: null,
          sdk_install_dir: "",
          message:
            error instanceof Error
              ? error.message
              : t("page.messages.opencode.remoteHealthCheckFailed"),
          checked_at: new Date().toISOString().slice(0, 19).replace("T", " "),
        });
      }
      return;
    }

    setRemoteOpenCodeHealth(null);
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
      setClaudeDefaultModel("sonnet");
      setClaudeDefaultEffort("medium");
      setClaudeNodePathOverride("");
      setClaudeCliPathOverride("");
      setClaudeActionError(null);
      setClaudeActionMessage(null);
      return;
    }

    try {
      const [health, settings] = await Promise.all([checkClaudeSdkHealth(), getClaudeSettings()]);
      setClaudeHealth(health);
      setClaudeSdkEnabled(settings.sdk_enabled);
      setClaudeDefaultModel(normalizeClaudeModel(settings.default_model));
      setClaudeDefaultEffort(claudeBudgetToDefaultEffort(settings.default_thinking_budget));
      setClaudeNodePathOverride(settings.node_path_override ?? "");
      setClaudeCliPathOverride(settings.cli_path_override ?? "");
    } catch (error) {
      console.error("Failed to load Claude settings:", error);
    }
  }

  async function loadGrokState() {
    try {
      setGrokModelListLoading(true);
      const [health, settings, models] = await Promise.all([
        checkGrokHealth(),
        getGrokSettings(),
        listGrokModels().catch(() => [] as GrokModelInfo[]),
      ]);
      setGrokHealth(health);
      setGrokDefaultModel(settings.default_model || "grok-4.5");
      setGrokDefaultEffort(settings.default_reasoning_effort || "high");
      setGrokCliPathOverride(settings.cli_path_override ?? "");
      setGrokModelList(models);
      setGrokActionError(null);

      if (isRemoteMode && selectedSshConfigId) {
        try {
          setRemoteGrokHealth(await validateRemoteGrokHealth(selectedSshConfigId));
        } catch (error) {
          setRemoteGrokHealth({
            available: false,
            version: null,
            auth_ok: null,
            message:
              error instanceof Error
                ? error.message
                : t("page.messages.grok.remoteHealthCheckFailed"),
            checked_at: new Date().toISOString().slice(0, 19).replace("T", " "),
          });
        }
      } else {
        setRemoteGrokHealth(null);
      }
    } catch (error) {
      console.error("Failed to load Grok settings:", error);
      setGrokHealth(null);
      setGrokActionError(
        error instanceof Error ? error.message : t("page.messages.grok.loadSettingsFailed"),
      );
    } finally {
      setGrokModelListLoading(false);
    }
  }

  async function handleInstallGrokCli() {
    if (isRemoteMode && !selectedSshConfigId) {
      setGrokActionError(t("page.messages.grok.selectSshConfigBeforeRemoteInstall"));
      setGrokActionMessage(null);
      return;
    }

    setGrokActionLoading("install");
    setGrokActionError(null);
    setGrokActionMessage(null);

    try {
      const result =
        isRemoteMode && selectedSshConfigId
          ? await installRemoteGrokCli(selectedSshConfigId)
          : await installGrokCli();
      setGrokActionMessage(
        result.cli_version
          ? t(
              isRemoteMode
                ? "page.messages.grok.remoteCliInstalled"
                : "page.messages.grok.localCliInstalled",
              { version: result.cli_version },
            )
          : result.message,
      );
      await loadGrokState();
    } catch (error) {
      console.error("Failed to install grok cli:", error);
      setGrokActionError(
        error instanceof Error ? error.message : t("page.messages.grok.installFailed"),
      );
    } finally {
      setGrokActionLoading(null);
    }
  }

  async function handleSaveGrokSettings() {
    setGrokActionLoading("save");
    setGrokActionError(null);
    setGrokActionMessage(null);
    try {
      await updateGrokSettings({
        default_model: grokDefaultModel,
        default_reasoning_effort: grokDefaultEffort,
        cli_path_override: grokCliPathOverride.trim() || null,
      });
      setGrokActionMessage(t("page.messages.grok.settingsSaved"));
      await loadGrokState();
    } catch (error) {
      setGrokActionError(
        error instanceof Error ? error.message : t("page.messages.grok.saveFailed"),
      );
    } finally {
      setGrokActionLoading(null);
    }
  }

  useEffect(() => {
    applyTheme(themeMode);
  }, [themeMode]);

  useEffect(() => {
    void changeAppLocale(locale);
  }, [locale]);

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
    void loadGrokState();
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
    if (!SETTINGS_TAB_KEYS.some((tab) => tab.value === value)) {
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
      setSdkActionFeedbackKind("save");
      setSdkActionError(t("page.messages.runtime.customWorktreeRootRequired"));
      setSdkActionMessage(null);
      return;
    }

    setSdkActionLoading("save");
    setSdkActionFeedbackKind("save");
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
        tester_automation_enabled: testerAutomationEnabled,
        tester_allow_ai_only: testerAllowAiOnly,
        default_test_command: defaultTestCommand.trim() || null,
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
      const nextSettings =
        isRemoteMode && selectedSshConfigId
          ? await updateRemoteCodexSettings(selectedSshConfigId, updates)
          : await updateCodexSettings(updates);
      setCodexSettings(nextSettings);
      setSdkActionMessage(
        isRemoteMode
          ? t("page.messages.runtime.remoteConfigSaved", { name: remoteTargetName })
          : t("page.messages.runtime.systemSettingsSaved"),
      );
      await loadRuntimeState();
      await refreshNotificationHealth();
    } catch (error) {
      console.error("Failed to save codex sdk settings:", error);
      setSdkActionError(
        error instanceof Error ? error.message : t("page.messages.runtime.saveCodexConfigFailed"),
      );
    } finally {
      setSdkActionLoading(null);
    }
  }

  async function handleInstallSdk() {
    if (isRemoteMode && !selectedSshConfigId) {
      setSdkActionFeedbackKind("install");
      setSdkActionError(t("page.messages.runtime.selectSshConfigBeforeInstallRemoteSdk"));
      return;
    }

    setSdkActionLoading("install");
    setSdkActionFeedbackKind("install");
    setSdkActionError(null);
    setSdkActionMessage(null);

    try {
      const result =
        isRemoteMode && selectedSshConfigId
          ? await installRemoteCodexSdk(selectedSshConfigId)
          : await installCodexSdk();
      setSdkActionMessage(
        result.sdk_version
          ? t(
              isRemoteMode
                ? "page.messages.runtime.remoteSdkInstalled"
                : "page.messages.runtime.localSdkInstalled",
              { version: result.sdk_version },
            )
          : result.message,
      );
      await loadRuntimeState();
      await refreshNotificationHealth();
    } catch (error) {
      console.error("Failed to install codex sdk:", error);
      setSdkActionError(
        error instanceof Error ? error.message : t("page.messages.runtime.installCodexSdkFailed"),
      );
    } finally {
      setSdkActionLoading(null);
    }
  }

  async function handleSaveOpenCodeSettings() {
    if (isRemoteMode) {
      setOpenCodeActionError(t("page.messages.opencode.localOnly"));
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
      setOpenCodeActionMessage(t("page.messages.opencode.settingsSaved"));
      await loadOpenCodeState();
    } catch (error) {
      setOpenCodeActionError(
        error instanceof Error ? error.message : t("page.messages.opencode.saveFailed"),
      );
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
        models.length > 0 &&
        aiCommitModelSource === "custom" &&
        gitAiProvider === "opencode" &&
        !models.some((m) => m.value === aiCommitModel)
      ) {
        setAiCommitModel(models[0].value);
      }
    } catch (error) {
      setOpenCodeActionError(
        error instanceof Error ? error.message : t("page.messages.opencode.fetchModelsFailed"),
      );
    } finally {
      setOpenCodeModelListLoading(false);
    }
  }

  async function handleInstallOpenCodeSdk() {
    if (isRemoteMode && !selectedSshConfigId) {
      setOpenCodeActionError(t("page.messages.opencode.selectSshConfigBeforeRemoteInstall"));
      setOpenCodeActionMessage(null);
      return;
    }

    setOpenCodeActionLoading("install");
    setOpenCodeActionError(null);
    setOpenCodeActionMessage(null);
    try {
      if (isRemoteMode && selectedSshConfigId) {
        const result = await installRemoteOpenCodeSdk(selectedSshConfigId);
        setOpenCodeActionMessage(
          result.sdk_version
            ? t("page.messages.opencode.remoteSdkInstalled", { version: result.sdk_version })
            : result.message,
        );
        await loadOpenCodeState();
      } else {
        const result = await installOpenCodeSdk();
        setOpenCodeActionMessage(
          result.sdk_version
            ? t("page.messages.opencode.localSdkInstalled", { version: result.sdk_version })
            : result.message,
        );
        await loadOpenCodeState();
        // SDK newly installed, auto-fetch models
        await handleFetchOpenCodeModels();
      }
    } catch (error) {
      setOpenCodeActionError(
        error instanceof Error ? error.message : t("page.messages.opencode.installFailed"),
      );
    } finally {
      setOpenCodeActionLoading(null);
    }
  }

  async function handleSaveClaudeSettings() {
    if (isRemoteMode) {
      setClaudeActionError(t("page.messages.claude.localOnly"));
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
      setClaudeActionMessage(t("page.messages.claude.settingsSaved"));
      await loadClaudeState();
    } catch (error) {
      setClaudeActionError(
        error instanceof Error ? error.message : t("page.messages.claude.saveFailed"),
      );
    } finally {
      setClaudeActionLoading(null);
    }
  }

  async function handleInstallClaudeSdk() {
    if (isRemoteMode) {
      setClaudeActionError(t("page.messages.claude.installLocalOnly"));
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
          ? t("page.messages.claude.sdkInstalled", { version: result.sdk_version })
          : result.message,
      );
      await loadClaudeState();
    } catch (error) {
      setClaudeActionError(
        error instanceof Error ? error.message : t("page.messages.claude.installFailed"),
      );
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
        title: t("page.databaseDialogs.exportTitle"),
        defaultPath: buildBackupDefaultPath(codexHealth),
        filters: databaseFileFilters,
      });

      if (!destination) {
        return;
      }

      const result = await backupDatabase(destination);
      setDatabaseActionMessage(result.message);
    } catch (error) {
      console.error("Failed to backup database:", error);
      setDatabaseActionError(
        error instanceof Error ? error.message : t("page.messages.database.exportFailed"),
      );
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
      setDatabaseActionError(
        error instanceof Error ? error.message : t("page.messages.database.openFolderFailed"),
      );
    } finally {
      setDatabaseActionLoading(null);
    }
  }

  async function handleRestoreDatabase() {
    setDatabaseActionLoading("restore");
    setDatabaseActionError(null);
    setDatabaseActionMessage(null);

    try {
      const confirmed = await confirm(t("page.databaseDialogs.importConfirmMessage"), {
        title: t("page.databaseDialogs.importTitle"),
        kind: "warning",
      });

      if (!confirmed) {
        return;
      }

      const selected = await open({
        title: t("page.databaseDialogs.selectBackupTitle"),
        directory: false,
        multiple: false,
        filters: databaseFileFilters,
      });

      if (typeof selected !== "string") {
        return;
      }

      const result = await restoreDatabase(selected);
      setDatabaseActionMessage(result.message);
      await loadRuntimeState();
      await refreshNotificationHealth();
      await message(
        t("page.databaseDialogs.importCompleteMessage", {
          message: result.message,
          backupPath: result.backup_path,
        }),
        {
          title: t("page.databaseDialogs.importCompleteTitle"),
          kind: "info",
        },
      );
    } catch (error) {
      console.error("Failed to restore database:", error);
      setDatabaseActionError(
        error instanceof Error ? error.message : t("page.messages.database.importFailed"),
      );
    } finally {
      setDatabaseActionLoading(null);
    }
  }

  async function handleSelectPrivateKeyFile() {
    try {
      const selected = await open({
        title: t("page.sshDialogs.selectPrivateKeyTitle"),
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
      setSshFormError(
        error instanceof Error ? error.message : t("page.messages.ssh.selectPrivateKeyFailed"),
      );
    }
  }

  async function handleSaveSshConfig() {
    if (!sshForm.name.trim() || !sshForm.host.trim() || !sshForm.username.trim()) {
      setSshFormError(t("page.messages.ssh.requiredFields"));
      return;
    }
    if (sshForm.authType === "key" && !sshForm.privateKeyPath.trim()) {
      setSshFormError(t("page.messages.ssh.privateKeyRequired"));
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
      setSshFormMessage(
        editingSshConfigId
          ? t("page.messages.ssh.configUpdated")
          : t("page.messages.ssh.configCreated"),
      );
      await refreshNotificationHealth(sshConfig.id);

      if (activeTab === "ssh") {
        replaceSettingsSearchParams("ssh", sshConfig.id);
      }
    } catch (error) {
      console.error("Failed to save SSH config:", error);
      setSshFormError(error instanceof Error ? error.message : t("page.messages.ssh.saveFailed"));
    } finally {
      setSshFormLoading(null);
    }
  }

  async function handleDeleteSshConfig() {
    if (!editingSshConfigId || !selectedSshConfig) {
      return;
    }

    const confirmed = await confirm(
      t("page.sshDialogs.deleteConfirm", { name: selectedSshConfig.name }),
      {
        title: t("page.sshDialogs.deleteTitle"),
        kind: "warning",
      },
    );

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
      setSshFormMessage(t("page.messages.ssh.configDeleted"));
      await refreshNotificationHealth(null);
    } catch (error) {
      console.error("Failed to delete SSH config:", error);
      setSshFormError(error instanceof Error ? error.message : t("page.messages.ssh.deleteFailed"));
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
        setSshFormMessage(t("page.messages.ssh.passwordProbeResult", { message: result.message }));
      } else {
        const health = await getRemoteHealthCheck(selectedSshConfigId);
        setSshFormMessage(
          health.codex_available
            ? t("page.messages.ssh.testConnectionSuccess")
            : t("page.messages.ssh.testConnectionSuccessButCodexUnavailable", {
                message: health.sdk_status_message,
              }),
        );
      }
      await loadRuntimeState();
      await refreshNotificationHealth();
    } catch (error) {
      console.error("Failed to test SSH connection:", error);
      setSshFormError(
        error instanceof Error ? error.message : t("page.messages.ssh.testConnectionFailed"),
      );
    } finally {
      setSshFormLoading(null);
    }
  }

  return (
    <div className="max-w-5xl space-y-6">
      <div>
        <h2 className="text-lg font-semibold">{t("page.title")}</h2>
        <p className="text-sm text-muted-foreground">
          {t("page.description", { mode: getEnvironmentModeLabel(environmentMode) })}
        </p>
      </div>

      <Tabs value={activeTab} onValueChange={handleTabChange} className="gap-6">
        <div className="overflow-x-auto overflow-y-hidden [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
          <TabsList variant="line" className="min-w-max justify-start">
            {SETTINGS_TAB_KEYS.map((tab) => (
              <TabsTrigger key={tab.value} value={tab.value}>
                {t(tab.labelKey)}
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
            actionFeedbackKind={sdkActionFeedbackKind}
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
            locale={locale}
            onLocaleChange={setLocale}
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
            remoteOpenCodeHealth={remoteOpenCodeHealth}
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
            grokHealth={grokHealth}
            remoteGrokHealth={remoteGrokHealth}
            grokDefaultModel={grokDefaultModel}
            grokDefaultEffort={grokDefaultEffort}
            grokCliPathOverride={grokCliPathOverride}
            grokActionLoading={grokActionLoading}
            grokActionMessage={grokActionMessage}
            grokActionError={grokActionError}
            grokModelList={grokModelList}
            grokModelListLoading={grokModelListLoading}
            onGrokDefaultModelChange={setGrokDefaultModel}
            onGrokDefaultEffortChange={setGrokDefaultEffort}
            onGrokCliPathOverrideChange={setGrokCliPathOverride}
            onGrokSave={() => void handleSaveGrokSettings()}
            onGrokInstall={() => void handleInstallGrokCli()}
            onGrokRefresh={() => void loadGrokState()}
          />

          <div className="mt-6 rounded-lg border border-border bg-card p-4 space-y-2">
            <h3 className="text-sm font-medium">{t("page.engineCapabilities.title")}</h3>
            <p className="text-xs text-muted-foreground">
              {t("page.engineCapabilities.description")}
            </p>
            <EngineCapabilityBadges />
          </div>
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
            testerAutomationEnabled={testerAutomationEnabled}
            testerAllowAiOnly={testerAllowAiOnly}
            defaultTestCommand={defaultTestCommand}
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
            onTesterAutomationEnabledChange={setTesterAutomationEnabled}
            onTesterAllowAiOnlyChange={setTesterAllowAiOnly}
            onDefaultTestCommandChange={setDefaultTestCommand}
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

        <TabsContent value="prompts">
          <PromptSettingsTab />
        </TabsContent>

        <TabsContent value="mcp">
          <McpSettingsTab />
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
