import { Loader2, Monitor, Moon, RefreshCw, ShieldAlert, Sun, type LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

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
  AI_PROVIDER_OPTIONS,
  CODEX_MODEL_OPTIONS,
  CLAUDE_MODEL_OPTIONS,
  CLAUDE_THINKING_BUDGET_OPTIONS,
  GROK_MODEL_OPTIONS,
  GROK_EFFORT_OPTIONS,
  OPENCODE_EFFORT_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  type AiChannel,
  type AiProvider,
  type ClaudeHealthCheck,
  type CodexHealthCheck,
  type CodexSettings,
  type GrokHealthCheck,
  type GrokModelInfo,
  type RemoteCodexHealthCheck,
  type RemoteGrokHealthCheck,
} from "@/lib/types";
import type {
  OpenCodeHealthCheck,
  OpenCodeModelInfo,
  RemoteOpenCodeHealthCheck,
} from "@/lib/opencode";
import { type AppLocale } from "@/lib/i18n/locale";
import { mapRuntimeStatusMessage } from "@/lib/i18n/mapRuntimeStatusMessage";
import { type ThemeMode } from "@/lib/theme";
import { formatDate } from "@/lib/utils";

import {
  resolveNativeThinking,
  selectNativeModel,
} from "@/components/employees/NativeChannelFields";
import { AboutUpdateSection } from "./AboutUpdateSection";

interface RuntimeSettingsTabProps {
  codexHealth: CodexHealthCheck | RemoteCodexHealthCheck | null;
  codexSettings: CodexSettings | null;
  healthLoading: boolean;
  actionLoading: "save" | "install" | null;
  actionMessage: string | null;
  actionError: string | null;
  /** Which action last produced actionMessage/actionError — controls where feedback is shown. */
  actionFeedbackKind: "save" | "install" | null;
  isRemoteMode: boolean;
  hasSelectedSshConfig: boolean;
  remoteTargetName: string;
  selectedSshConfigSummary: string;
  passwordAuthBlocked: boolean;
  taskSdkEnabled: boolean;
  oneShotSdkEnabled: boolean;
  oneShotPreferredProvider: AiProvider;
  oneShotModel: string;
  oneShotReasoningEffort: string;
  /** 一次性 AI 使用内置 Agent 时绑定的 AI 渠道 id。 */
  oneShotNativeChannelId: string;
  nativeChannels: AiChannel[];
  nodePathOverride: string;
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
  locale: AppLocale;
  onLocaleChange: (locale: AppLocale) => void;
  maxConcurrentSessions: number;
  onMaxConcurrentSessionsChange: (value: number) => void;
  onMaxConcurrentSessionsCommit: (value: number) => void;
  nativeMaxTurns: number;
  onNativeMaxTurnsChange: (value: number) => void;
  onNativeMaxTurnsCommit: (value: number) => void;
  nativeMaxConcurrentSubagents: number;
  onNativeMaxConcurrentSubagentsChange: (value: number) => void;
  onNativeMaxConcurrentSubagentsCommit: (value: number) => void;
  nativeSubagentPolicy: string;
  onNativeSubagentPolicyChange: (value: string) => void;
  nativeConfirmHighRisk: boolean;
  onNativeConfirmHighRiskChange: (value: boolean) => void;
  onTaskSdkEnabledChange: (value: boolean) => void;
  onOneShotSdkEnabledChange: (value: boolean) => void;
  onOneShotPreferredProviderChange: (value: AiProvider) => void;
  onOneShotModelChange: (value: string) => void;
  onOneShotReasoningEffortChange: (value: string) => void;
  onOneShotNativeChannelIdChange: (value: string) => void;
  onNodePathOverrideChange: (value: string) => void;
  onSave: () => void;
  onInstall: () => void;
  onRefresh: () => void;
  claudeHealth: ClaudeHealthCheck | null;
  claudeSdkEnabled: boolean;
  claudeDefaultModel: string;
  claudeDefaultEffort: string;
  claudeNodePathOverride: string;
  claudeCliPathOverride: string;
  claudeActionLoading: "save" | "install" | null;
  claudeActionMessage: string | null;
  claudeActionError: string | null;
  onClaudeSdkEnabledChange: (enabled: boolean) => void;
  onClaudeDefaultModelChange: (model: string) => void;
  onClaudeDefaultEffortChange: (effort: string) => void;
  onClaudeNodePathOverrideChange: (path: string) => void;
  onClaudeCliPathOverrideChange: (path: string) => void;
  onClaudeSave: () => void;
  onClaudeInstall: () => void;
  onClaudeRefresh: () => void;
  opencodeHealth: OpenCodeHealthCheck | null;
  remoteOpenCodeHealth: RemoteOpenCodeHealthCheck | null;
  opencodeSdkEnabled: boolean;
  opencodeDefaultModel: string;
  opencodeHost: string;
  opencodePort: number;
  opencodeNodePathOverride: string;
  opencodeActionLoading: "save" | "install" | null;
  opencodeActionMessage: string | null;
  opencodeActionError: string | null;
  onOpenCodeSdkEnabledChange: (enabled: boolean) => void;
  onOpenCodeDefaultModelChange: (model: string) => void;
  onOpenCodeHostChange: (host: string) => void;
  onOpenCodePortChange: (port: number) => void;
  opencodeModelList: OpenCodeModelInfo[];
  opencodeModelListLoading: boolean;
  onOpenCodeNodePathOverrideChange: (path: string) => void;
  onOpenCodeFetchModels: () => void;
  onOpenCodeSave: () => void;
  onOpenCodeInstall: () => void;
  onOpenCodeRefresh: () => void;
  grokHealth: GrokHealthCheck | null;
  remoteGrokHealth: RemoteGrokHealthCheck | null;
  grokDefaultModel: string;
  grokDefaultEffort: string;
  grokCliPathOverride: string;
  grokActionLoading: "save" | "install" | null;
  grokActionMessage: string | null;
  grokActionError: string | null;
  grokModelList: GrokModelInfo[];
  grokModelListLoading: boolean;
  onGrokDefaultModelChange: (model: string) => void;
  onGrokDefaultEffortChange: (effort: string) => void;
  onGrokCliPathOverrideChange: (path: string) => void;
  onGrokSave: () => void;
  onGrokInstall: () => void;
  onGrokRefresh: () => void;
}

const themeOptionDefs: { value: ThemeMode; labelKey: string; icon: LucideIcon }[] = [
  { value: "light", labelKey: "settings:theme.light", icon: Sun },
  { value: "dark", labelKey: "settings:theme.dark", icon: Moon },
  { value: "system", labelKey: "settings:theme.system", icon: Monitor },
];

const localeOptions: { value: AppLocale; labelKey: string }[] = [
  { value: "zh-CN", labelKey: "settings:language.zhCN" },
  { value: "en", labelKey: "settings:language.en" },
];

const CODEX_MODEL_OPTION_KEY_BY_VALUE: Record<string, string> = {
  "gpt-5.6-sol": "gpt56Sol",
  "gpt-5.6-terra": "gpt56Terra",
  "gpt-5.6-luna": "gpt56Luna",
  "gpt-5.5": "gpt55",
  "gpt-5.4": "gpt54",
  "gpt-5.2-codex": "gpt52Codex",
  "gpt-5.1-codex-max": "gpt51CodexMax",
  "gpt-5.4-mini": "gpt54Mini",
  "gpt-5.3-codex": "gpt53Codex",
  "gpt-5.3-codex-spark": "gpt53CodexSpark",
  "gpt-5.2": "gpt52",
  "gpt-5.1-codex-mini": "gpt51CodexMini",
};

const CLAUDE_MODEL_OPTION_KEY_BY_VALUE: Record<string, string> = {
  opus: "opus",
  "opus[1m]": "opus1m",
  sonnet: "sonnet",
  "sonnet[1m]": "sonnet1m",
  haiku: "haiku",
};

const GROK_MODEL_OPTION_KEY_BY_VALUE: Record<string, string> = {
  "grok-4.5": "grok45",
};

export function RuntimeSettingsTab({
  codexHealth,
  codexSettings,
  healthLoading,
  actionLoading,
  actionMessage,
  actionError,
  actionFeedbackKind,
  isRemoteMode,
  hasSelectedSshConfig,
  remoteTargetName,
  selectedSshConfigSummary,
  passwordAuthBlocked,
  taskSdkEnabled,
  oneShotSdkEnabled,
  oneShotPreferredProvider,
  oneShotModel,
  oneShotReasoningEffort,
  oneShotNativeChannelId,
  nativeChannels,
  nodePathOverride,
  themeMode,
  onThemeModeChange,
  locale,
  onLocaleChange,
  maxConcurrentSessions,
  onMaxConcurrentSessionsChange,
  onMaxConcurrentSessionsCommit,
  nativeMaxTurns,
  onNativeMaxTurnsChange,
  onNativeMaxTurnsCommit,
  nativeMaxConcurrentSubagents,
  onNativeMaxConcurrentSubagentsChange,
  onNativeMaxConcurrentSubagentsCommit,
  nativeSubagentPolicy,
  onNativeSubagentPolicyChange,
  nativeConfirmHighRisk,
  onNativeConfirmHighRiskChange,
  onTaskSdkEnabledChange,
  onOneShotSdkEnabledChange,
  onOneShotPreferredProviderChange,
  onOneShotModelChange,
  onOneShotReasoningEffortChange,
  onOneShotNativeChannelIdChange,
  onNodePathOverrideChange,
  onSave,
  onInstall,
  onRefresh,
  claudeHealth,
  claudeSdkEnabled,
  claudeDefaultModel,
  claudeDefaultEffort,
  claudeNodePathOverride,
  claudeCliPathOverride,
  claudeActionLoading,
  claudeActionMessage,
  claudeActionError,
  onClaudeSdkEnabledChange,
  onClaudeDefaultModelChange,
  onClaudeDefaultEffortChange,
  onClaudeNodePathOverrideChange,
  onClaudeCliPathOverrideChange,
  onClaudeSave,
  onClaudeInstall,
  onClaudeRefresh,
  opencodeHealth,
  remoteOpenCodeHealth,
  opencodeSdkEnabled,
  opencodeDefaultModel,
  opencodeHost,
  opencodePort,
  opencodeNodePathOverride,
  opencodeActionLoading,
  opencodeActionMessage,
  opencodeActionError,
  opencodeModelList,
  opencodeModelListLoading,
  onOpenCodeSdkEnabledChange,
  onOpenCodeDefaultModelChange,
  onOpenCodeHostChange,
  onOpenCodePortChange,
  onOpenCodeNodePathOverrideChange,
  onOpenCodeFetchModels,
  onOpenCodeSave,
  onOpenCodeInstall,
  onOpenCodeRefresh,
  grokHealth,
  remoteGrokHealth,
  grokDefaultModel,
  grokDefaultEffort,
  grokCliPathOverride,
  grokActionLoading,
  grokActionMessage,
  grokActionError,
  grokModelList,
  grokModelListLoading,
  onGrokDefaultModelChange,
  onGrokDefaultEffortChange,
  onGrokCliPathOverrideChange,
  onGrokSave,
  onGrokInstall,
  onGrokRefresh,
}: RuntimeSettingsTabProps) {
  const { t } = useTranslation(["settings", "common"]);
  const availableOneShotProviders = AI_PROVIDER_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.providers.${option.value}`),
  }));
  const codexModelOptions = CODEX_MODEL_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.codexModels.${CODEX_MODEL_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const claudeModelOptions = CLAUDE_MODEL_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.claudeModels.${CLAUDE_MODEL_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const grokModelOptions = GROK_MODEL_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.grokModels.${GROK_MODEL_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const reasoningEffortOptions = REASONING_EFFORT_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.reasoningEffort.${option.value}`),
  }));
  const openCodeEffortOptions = OPENCODE_EFFORT_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.reasoningEffort.${option.value}`),
  }));
  const grokEffortOptions = GROK_EFFORT_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.reasoningEffort.${option.value}`),
  }));
  const claudeThinkingBudgetOptions = CLAUDE_THINKING_BUDGET_OPTIONS.map((option) => ({
    ...option,
    label: t(`runtime.options.claudeThinkingBudget.${option.value}`),
  }));
  const claudeDefaultThinkingBudgetOptions = claudeThinkingBudgetOptions.filter(
    (option) => option.value !== "auto",
  );
  const taskProviderLabel = t(
    codexHealth?.task_execution_effective_provider === "sdk"
      ? "runtime.codexSdk.taskProvider.sdk"
      : "runtime.codexSdk.taskProvider.execFallback",
  );
  const oneShotProviderLabel =
    availableOneShotProviders.find((option) => option.value === oneShotPreferredProvider)?.label ??
    oneShotPreferredProvider;
  const oneShotChannelLabel = (() => {
    const channel = codexHealth?.one_shot_effective_channel;
    if (channel === "sdk") return t("runtime.oneShot.channel.sdk");
    if (channel === "cli") {
      return t(isRemoteMode ? "runtime.oneShot.channel.cliRemote" : "runtime.oneShot.channel.cli");
    }
    if (channel === "exec") {
      return t(
        isRemoteMode
          ? "runtime.oneShot.channel.execRemote"
          : "runtime.oneShot.channel.execFallback",
      );
    }
    if (channel === "channel") {
      const selected = nativeChannels.find((item) => item.id === oneShotNativeChannelId);
      return selected?.name ?? t("runtime.oneShot.channel.channel");
    }
    return t("runtime.oneShot.channel.unavailable");
  })();
  const installButtonLabel = t(
    codexHealth?.sdk_installed
      ? "runtime.codexSdk.actions.reinstall"
      : "runtime.codexSdk.actions.install",
  );
  const grokCliInstalled = isRemoteMode
    ? Boolean(remoteGrokHealth?.available)
    : Boolean(grokHealth?.cli_available);
  const grokInstallButtonLabel = t(
    grokCliInstalled ? "runtime.grok.actions.reinstall" : "runtime.grok.actions.install",
  );
  const grokInstallDisabled =
    healthLoading || grokActionLoading !== null || (isRemoteMode && !hasSelectedSshConfig);
  const saveDisabled =
    healthLoading || actionLoading !== null || (isRemoteMode && !hasSelectedSshConfig);
  const installDisabled =
    healthLoading ||
    actionLoading !== null ||
    (isRemoteMode && (!hasSelectedSshConfig || passwordAuthBlocked));
  const isOneShotCodexProvider = oneShotPreferredProvider === "codex";
  const isOneShotClaudeProvider = oneShotPreferredProvider === "claude";
  const isOneShotOpenCodeProvider = oneShotPreferredProvider === "opencode";
  const isOneShotGrokProvider = oneShotPreferredProvider === "grok";
  const isOneShotNativeProvider = oneShotPreferredProvider === "native";
  const selectedNativeChannel = nativeChannels.find(
    (channel) => channel.id === oneShotNativeChannelId,
  );
  const nativeModelOptions = selectedNativeChannel?.models.map((item) => item.id) ?? [];
  const nativeThinking = resolveNativeThinking(selectedNativeChannel, oneShotModel);
  const nativeEffortOptions = nativeThinking.levels.map((level) => ({
    value: level,
    label: t(`runtime.options.nativeThinkingLevels.${level}`, { defaultValue: level }),
  }));
  /** 历史配置的推理强度可能不在当前模型思考等级内，展示时回退到模型默认等级。 */
  const effectiveNativeEffort = nativeThinking.levels.includes(oneShotReasoningEffort)
    ? oneShotReasoningEffort
    : nativeThinking.defaultLevel;
  const oneShotGrokModelOptions =
    grokModelList.length > 0
      ? grokModelList
      : [
          {
            value: oneShotModel,
            label: grokModelListLoading
              ? t("runtime.oneShot.fallbacks.loadingModels")
              : oneShotModel || t("runtime.oneShot.fallbacks.currentModel"),
            is_default: false,
          },
        ];
  const defaultGrokModelOptions =
    grokModelList.length > 0
      ? grokModelList
      : grokModelOptions.map((option) => ({
          value: option.value,
          label: option.label,
          is_default: option.value === "grok-4.5",
        }));
  const oneShotOpenCodeModelOptions =
    opencodeModelList.length > 0
      ? opencodeModelList
      : [
          {
            value: oneShotModel,
            label: opencodeModelListLoading
              ? t("runtime.oneShot.fallbacks.loadingModels")
              : t("runtime.oneShot.fallbacks.currentModel"),
            providerId: "opencode",
            providerName: t("runtime.options.providers.opencode"),
            modelId: oneShotModel.includes("/")
              ? oneShotModel.split("/").slice(1).join("/")
              : oneShotModel,
            capabilities: null,
          },
        ];
  const defaultOpenCodeModelOptions =
    opencodeModelList.length > 0
      ? opencodeModelList
      : [
          {
            value: opencodeDefaultModel,
            label: opencodeModelListLoading
              ? t("runtime.opencode.fallbacks.loadingModels")
              : t("runtime.opencode.fallbacks.currentModel"),
            providerId: "opencode",
            providerName: t("runtime.options.providers.opencode"),
            modelId: opencodeDefaultModel.includes("/")
              ? opencodeDefaultModel.split("/").slice(1).join("/")
              : opencodeDefaultModel,
            capabilities: null,
          },
        ];
  const canUseOneShotSdkToggle = !isRemoteMode
    ? isOneShotCodexProvider || isOneShotClaudeProvider || isOneShotOpenCodeProvider
    : isOneShotCodexProvider || isOneShotOpenCodeProvider;
  const selectedOneShotStatusMessage = mapRuntimeStatusMessage(
    codexHealth?.one_shot_status_message,
  );

  return (
    <div className="space-y-6">
      <AboutUpdateSection />
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="mb-1 text-sm font-medium">{t("settings:theme.title")}</h3>
          <p className="mb-3 text-xs text-muted-foreground">{t("settings:theme.description")}</p>
          <div className="flex flex-wrap gap-2">
            {themeOptionDefs.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => onThemeModeChange(option.value)}
                className={`flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm transition-colors ${
                  themeMode === option.value
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-input hover:bg-accent"
                }`}
              >
                <option.icon className="h-4 w-4" />
                {t(option.labelKey)}
              </button>
            ))}
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <h3 className="mb-1 text-sm font-medium">{t("settings:language.title")}</h3>
          <p className="mb-3 text-xs text-muted-foreground">{t("settings:language.description")}</p>
          <div className="flex flex-wrap gap-2">
            {localeOptions.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => onLocaleChange(option.value)}
                className={`rounded-md border px-3 py-1.5 text-sm transition-colors ${
                  locale === option.value
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-input hover:bg-accent"
                }`}
              >
                {t(option.labelKey)}
              </button>
            ))}
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <h3 className="mb-1 text-sm font-medium">{t("settings:concurrency.title")}</h3>
          <p className="mb-3 text-xs text-muted-foreground">
            {t("settings:concurrency.description")}
          </p>
          <div className="max-w-xs space-y-2">
            <label htmlFor="max-concurrent-sessions" className="text-sm font-medium">
              {t("settings:concurrency.label")}
            </label>
            <Input
              id="max-concurrent-sessions"
              type="number"
              min={0}
              max={64}
              step={1}
              value={maxConcurrentSessions}
              onChange={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                onMaxConcurrentSessionsChange(
                  Number.isNaN(parsed) ? 0 : Math.min(64, Math.max(0, parsed)),
                );
              }}
              onBlur={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                onMaxConcurrentSessionsCommit(
                  Number.isNaN(parsed) ? 0 : Math.min(64, Math.max(0, parsed)),
                );
              }}
            />
            <p className="text-xs text-muted-foreground">{t("settings:concurrency.hint")}</p>
          </div>
        </div>

        <div className="border-t border-border pt-4">
          <h3 className="mb-1 text-sm font-medium">{t("settings:nativeAgent.title")}</h3>
          <p className="mb-3 text-xs text-muted-foreground">
            {t("settings:nativeAgent.description")}
          </p>
          <div className="max-w-xs space-y-2">
            <label htmlFor="native-max-turns" className="text-sm font-medium">
              {t("settings:nativeAgent.label")}
            </label>
            <Input
              id="native-max-turns"
              type="number"
              min={0}
              max={500}
              step={1}
              value={nativeMaxTurns}
              onChange={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                onNativeMaxTurnsChange(
                  Number.isNaN(parsed) ? 0 : Math.min(500, Math.max(0, parsed)),
                );
              }}
              onBlur={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                onNativeMaxTurnsCommit(
                  Number.isNaN(parsed) ? 0 : Math.min(500, Math.max(0, parsed)),
                );
              }}
            />
            <p className="text-xs text-muted-foreground">{t("settings:nativeAgent.hint")}</p>
          </div>
          <div className="mt-4 max-w-xs space-y-2">
            <label htmlFor="native-max-concurrent-subagents" className="text-sm font-medium">
              {t("settings:nativeAgent.maxConcurrentSubagentsLabel")}
            </label>
            <Input
              id="native-max-concurrent-subagents"
              type="number"
              min={1}
              max={16}
              step={1}
              value={nativeMaxConcurrentSubagents}
              onChange={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                onNativeMaxConcurrentSubagentsChange(
                  Number.isNaN(parsed) ? 3 : Math.min(16, Math.max(1, parsed)),
                );
              }}
              onBlur={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                onNativeMaxConcurrentSubagentsCommit(
                  Number.isNaN(parsed) ? 3 : Math.min(16, Math.max(1, parsed)),
                );
              }}
            />
            <p className="text-xs text-muted-foreground">
              {t("settings:nativeAgent.maxConcurrentSubagentsHint")}
            </p>
          </div>
          <div className="mt-4 max-w-xs space-y-2">
            <label htmlFor="native-subagent-policy" className="text-sm font-medium">
              {t("settings:nativeAgent.subagentPolicyLabel")}
            </label>
            <Select
              value={nativeSubagentPolicy}
              onValueChange={(value) => {
                if (value) {
                  onNativeSubagentPolicyChange(value);
                }
              }}
            >
              <SelectTrigger id="native-subagent-policy" className="bg-background">
                <SelectValue>
                  {(value) => {
                    if (value === "conservative") {
                      return t("settings:nativeAgent.subagentPolicyConservative");
                    }
                    if (value === "aggressive") {
                      return t("settings:nativeAgent.subagentPolicyAggressive");
                    }
                    return t("settings:nativeAgent.subagentPolicyBalanced");
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="conservative">
                  {t("settings:nativeAgent.subagentPolicyConservative")}
                </SelectItem>
                <SelectItem value="balanced">
                  {t("settings:nativeAgent.subagentPolicyBalanced")}
                </SelectItem>
                <SelectItem value="aggressive">
                  {t("settings:nativeAgent.subagentPolicyAggressive")}
                </SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {t("settings:nativeAgent.subagentPolicyHint")}
            </p>
          </div>
          <label className="mt-4 flex items-start gap-3 rounded-md border border-border px-3 py-2">
            <input
              type="checkbox"
              className="mt-0.5 h-4 w-4 rounded border-input"
              checked={nativeConfirmHighRisk}
              onChange={(event) => onNativeConfirmHighRiskChange(event.target.checked)}
            />
            <div className="space-y-1">
              <p className="text-sm font-medium">
                {t("settings:nativeAgent.confirmHighRiskLabel")}
              </p>
              <p className="text-xs text-muted-foreground">
                {t("settings:nativeAgent.confirmHighRiskHint")}
              </p>
            </div>
          </label>
        </div>
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-sm font-medium">
              {t(isRemoteMode ? "runtime.target.remoteTitle" : "runtime.target.localTitle")}
            </h3>
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? t("runtime.target.remoteSummary", {
                    name: remoteTargetName,
                    summary: selectedSshConfigSummary,
                  })
                : t("runtime.target.localSummary")}
            </p>
          </div>
          <span className="rounded bg-secondary px-2 py-1 text-xs text-secondary-foreground">
            {t(isRemoteMode ? "runtime.target.remoteBadge" : "runtime.target.localBadge")}
          </span>
        </div>

        {isRemoteMode && !hasSelectedSshConfig && (
          <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            {t("runtime.target.missingSshConfig")}
          </div>
        )}

        {passwordAuthBlocked && (
          <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
            <div className="flex items-center gap-2 font-medium">
              <ShieldAlert className="h-4 w-4" />
              {t("runtime.target.blockedTitle")}
            </div>
            <p className="mt-1 text-xs leading-5">{t("runtime.target.blockedDescription")}</p>
          </div>
        )}

        <div className="border-t border-border pt-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h3 className="text-sm font-medium">{t("runtime.codexCli.title")}</h3>
              <p className="text-xs text-muted-foreground">
                {isRemoteMode
                  ? t("runtime.codexCli.descriptionRemote")
                  : t("runtime.codexCli.descriptionLocal")}
              </p>
              {codexHealth?.codex_version && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {t("runtime.common.version")}:{codexHealth.codex_version}
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
              {healthLoading
                ? t("runtime.status.checking")
                : codexHealth?.codex_available
                  ? t("runtime.status.connected")
                  : t("runtime.status.unavailable")}
            </span>
          </div>
          {codexHealth?.target_host_label && (
            <p className="mt-2 text-xs text-muted-foreground">
              {t("runtime.common.host")}:{codexHealth.target_host_label}
            </p>
          )}
          {codexHealth?.last_session_error && (
            <p className="mt-2 text-xs text-amber-700">
              {t("runtime.common.lastError")}:
              {mapRuntimeStatusMessage(codexHealth.last_session_error)}
            </p>
          )}
        </div>

        <div className="border-t border-border pt-4">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <h3 className="text-sm font-medium">{t("runtime.codexSdk.title")}</h3>
              <p className="text-xs text-muted-foreground">
                {isRemoteMode
                  ? t("runtime.codexSdk.descriptionRemote")
                  : t("runtime.codexSdk.descriptionLocal")}
              </p>
            </div>
            <span
              className={`rounded px-2 py-1 text-xs ${
                codexHealth?.task_execution_effective_provider === "sdk"
                  ? "bg-green-100 text-green-700"
                  : "bg-slate-100 text-slate-700"
              }`}
            >
              {healthLoading
                ? t("runtime.status.checking")
                : t("runtime.codexSdk.taskStatus", { provider: taskProviderLabel })}
            </span>
          </div>

          <div className="mt-4 space-y-4">
            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={taskSdkEnabled}
                onChange={(event) => onTaskSdkEnabledChange(event.target.checked)}
                disabled={healthLoading || actionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">{t("runtime.codexSdk.useSdkTitle")}</p>
                <p className="text-xs text-muted-foreground">
                  {t("runtime.codexSdk.useSdkDescription")}
                </p>
              </div>
            </label>

            <div className="space-y-2">
              <label htmlFor="node-path-override" className="text-sm font-medium">
                {t("runtime.codexSdk.nodePathTitle")}
              </label>
              <Input
                id="node-path-override"
                value={nodePathOverride}
                onChange={(event) => onNodePathOverrideChange(event.target.value)}
                placeholder={t(
                  isRemoteMode
                    ? "runtime.codexSdk.nodePathPlaceholderRemote"
                    : "runtime.codexSdk.nodePathPlaceholderLocal",
                )}
                disabled={healthLoading || actionLoading !== null}
              />
              <p className="text-xs text-muted-foreground">
                {t("runtime.codexSdk.nodePathDescription")}
              </p>
            </div>

            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p className="break-all">
                {t("runtime.common.installDir")}:
                {codexSettings?.sdk_install_dir ?? t("runtime.status.checking")}
              </p>
              <p>
                {t("runtime.common.node")}:
                {codexHealth?.node_available
                  ? t("runtime.status.available")
                  : t("runtime.status.unavailable")}
                {codexHealth?.node_version ? `（${codexHealth.node_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.sdk")}:
                {codexHealth?.sdk_installed
                  ? t("runtime.status.installed")
                  : t("runtime.status.notInstalled")}
                {codexHealth?.sdk_version ? `（${codexHealth.sdk_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.taskEngine")}:{taskProviderLabel}
              </p>
              {codexHealth?.checked_at && (
                <p>
                  {t("runtime.common.checkedAt")}:{formatDate(codexHealth.checked_at)}
                </p>
              )}
              {codexHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">
                  {mapRuntimeStatusMessage(codexHealth.sdk_status_message)}
                </p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button variant="outline" onClick={onInstall} disabled={installDisabled}>
                {actionLoading === "install" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {installButtonLabel}
              </Button>
              <Button
                variant="ghost"
                onClick={onRefresh}
                disabled={healthLoading || actionLoading !== null}
              >
                <RefreshCw className={`h-4 w-4 ${healthLoading ? "animate-spin" : ""}`} />
                {t("runtime.codexSdk.actions.refresh")}
              </Button>
            </div>

            {actionFeedbackKind === "install" && actionMessage ? (
              <p className="text-xs text-green-700">{actionMessage}</p>
            ) : null}
            {actionFeedbackKind === "install" && actionError ? (
              <p className="text-xs text-destructive">{actionError}</p>
            ) : null}
          </div>
        </div>
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">{t("runtime.oneShot.title")}</h3>
            <p className="text-xs text-muted-foreground">{t("runtime.oneShot.description")}</p>
          </div>
          <span
            className={`rounded px-2 py-1 text-xs ${
              codexHealth?.one_shot_effective_channel !== "unavailable"
                ? "bg-green-100 text-green-700"
                : "bg-slate-100 text-slate-700"
            }`}
          >
            {healthLoading
              ? t("runtime.status.checking")
              : t("runtime.oneShot.statusBadge", {
                  provider: oneShotProviderLabel,
                  channel: oneShotChannelLabel,
                })}
          </span>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("runtime.oneShot.providerLabel")}</label>
            <Select<AiProvider>
              value={oneShotPreferredProvider}
              onValueChange={(value) => {
                if (value) {
                  onOneShotPreferredProviderChange(value as AiProvider);
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {availableOneShotProviders.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {isOneShotNativeProvider ? (
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("runtime.oneShot.channelLabel")}</label>
              <Select
                value={oneShotNativeChannelId || undefined}
                onValueChange={(value) => {
                  if (!value) return;
                  onOneShotNativeChannelIdChange(value);
                  const channel = nativeChannels.find((item) => item.id === value);
                  const nextModel = selectNativeModel(channel, oneShotModel);
                  onOneShotModelChange(nextModel);
                  const thinking = resolveNativeThinking(channel, nextModel);
                  onOneShotReasoningEffortChange(thinking.defaultLevel);
                }}
                disabled={healthLoading || actionLoading !== null}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>
                    {(value) => {
                      if (typeof value !== "string") {
                        return t("runtime.oneShot.selectChannel");
                      }
                      const channel = nativeChannels.find((item) => item.id === value);
                      return channel
                        ? `${channel.name} · ${channel.protocol}`
                        : t("runtime.oneShot.selectChannel");
                    }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {nativeChannels.map((channel) => (
                    <SelectItem key={channel.id} value={channel.id}>
                      {channel.name} · {channel.protocol}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {nativeChannels.length === 0 ? (
                <p className="text-xs text-destructive">{t("runtime.oneShot.noChannelHint")}</p>
              ) : null}
            </div>
          ) : canUseOneShotSdkToggle ? (
            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={oneShotSdkEnabled}
                onChange={(event) => onOneShotSdkEnabledChange(event.target.checked)}
                disabled={healthLoading || actionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">
                  {t(
                    isOneShotOpenCodeProvider
                      ? "runtime.oneShot.enableOpenCodeSdkTitle"
                      : "runtime.oneShot.preferSdkTitle",
                  )}
                </p>
                <p className="text-xs text-muted-foreground">
                  {isOneShotCodexProvider
                    ? isRemoteMode
                      ? t("runtime.oneShot.preferSdkDescriptionCodexRemote")
                      : t("runtime.oneShot.preferSdkDescriptionCodexLocal")
                    : isOneShotClaudeProvider
                      ? t("runtime.oneShot.preferSdkDescriptionClaude")
                      : t("runtime.oneShot.preferSdkDescriptionOpenCode")}
                </p>
              </div>
            </label>
          ) : (
            <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
              {isOneShotGrokProvider
                ? t("runtime.oneShot.sshGrokNotice")
                : t("runtime.oneShot.sshClaudeNotice")}
            </div>
          )}
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("runtime.oneShot.modelLabel")}</label>
            {isOneShotNativeProvider ? (
              nativeModelOptions.length > 0 ? (
                <Select
                  value={oneShotModel}
                  onValueChange={(value) => {
                    if (value) {
                      onOneShotModelChange(value);
                      const thinking = resolveNativeThinking(selectedNativeChannel, value);
                      onOneShotReasoningEffortChange(thinking.defaultLevel);
                    }
                  }}
                  disabled={healthLoading || actionLoading !== null}
                >
                  <SelectTrigger className="bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent className="max-h-72">
                    {nativeModelOptions.map((item) => (
                      <SelectItem key={item} value={item}>
                        {item}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              ) : (
                <Input
                  value={oneShotModel}
                  onChange={(event) => onOneShotModelChange(event.target.value)}
                  placeholder={t("runtime.oneShot.modelPlaceholder")}
                  disabled={!selectedNativeChannel || healthLoading || actionLoading !== null}
                />
              )
            ) : isOneShotOpenCodeProvider ? (
              <div className="flex gap-2">
                <div className="flex-1">
                  {opencodeHealth?.sdk_installed ? (
                    <Select
                      value={oneShotModel}
                      onValueChange={(value) => {
                        if (value) {
                          onOneShotModelChange(value);
                        }
                      }}
                      disabled={healthLoading || actionLoading !== null || opencodeModelListLoading}
                    >
                      <SelectTrigger className="bg-background">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent className="max-h-72">
                        {oneShotOpenCodeModelOptions.map((model) => (
                          <SelectItem key={model.value} value={model.value}>
                            {`${model.label} · ${model.providerName}`}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  ) : (
                    <Input
                      value={oneShotModel}
                      onChange={(event) => onOneShotModelChange(event.target.value)}
                      placeholder={t("runtime.oneShot.modelPlaceholder")}
                      disabled={healthLoading || actionLoading !== null}
                    />
                  )}
                </div>
                {!isRemoteMode && opencodeHealth?.sdk_installed && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={onOpenCodeFetchModels}
                    disabled={opencodeModelListLoading || actionLoading !== null}
                    title={t("runtime.oneShot.fetchModelsTitle")}
                  >
                    {opencodeModelListLoading ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="h-3.5 w-3.5" />
                    )}
                  </Button>
                )}
              </div>
            ) : (
              <Select
                value={oneShotModel}
                onValueChange={(value) => {
                  if (value) {
                    onOneShotModelChange(value);
                  }
                }}
                disabled={healthLoading || actionLoading !== null}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {(isOneShotClaudeProvider
                    ? claudeModelOptions
                    : isOneShotGrokProvider
                      ? oneShotGrokModelOptions
                      : codexModelOptions
                  ).map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {isOneShotOpenCodeProvider && (
              <p className="text-xs text-muted-foreground">
                {opencodeModelList.length > 0
                  ? t("runtime.oneShot.loadedModels", { count: opencodeModelList.length })
                  : t("runtime.oneShot.modelFormat")}
              </p>
            )}
            {isOneShotGrokProvider && (
              <p className="text-xs text-muted-foreground">
                {grokModelList.length > 0
                  ? t("runtime.oneShot.loadedGrokModels", { count: grokModelList.length })
                  : grokModelListLoading
                    ? t("runtime.oneShot.loadingGrokModels")
                    : t("runtime.oneShot.usingStaticModelList")}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("runtime.oneShot.reasoningLabel")}</label>
            <Select
              value={isOneShotNativeProvider ? effectiveNativeEffort : oneShotReasoningEffort}
              onValueChange={(value) => {
                if (value) {
                  onOneShotReasoningEffortChange(value);
                }
              }}
              disabled={
                healthLoading ||
                actionLoading !== null ||
                (isOneShotNativeProvider && !nativeThinking.enabled)
              }
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(isOneShotNativeProvider
                  ? nativeEffortOptions
                  : isOneShotClaudeProvider
                    ? claudeDefaultThinkingBudgetOptions
                    : isOneShotOpenCodeProvider
                      ? openCodeEffortOptions
                      : isOneShotGrokProvider
                        ? grokEffortOptions
                        : reasoningEffortOptions
                ).map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {isOneShotNativeProvider ? (
              <p className="text-xs text-muted-foreground">
                {nativeThinking.enabled
                  ? t("runtime.oneShot.nativeThinkingFromChannel")
                  : t("runtime.oneShot.nativeThinkingDisabled")}
              </p>
            ) : null}
          </div>
        </div>

        <div className="rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
          <p>
            {t("runtime.common.currentProvider")}:{oneShotProviderLabel}
          </p>
          <p>
            {t("runtime.common.executionChannel")}:{oneShotChannelLabel}
          </p>
          {selectedOneShotStatusMessage ? (
            <p className="mt-1 leading-5">{selectedOneShotStatusMessage}</p>
          ) : null}
        </div>

        <div className="flex flex-wrap gap-2">
          <Button onClick={onSave} disabled={saveDisabled}>
            {actionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            {t("runtime.oneShot.actions.save")}
          </Button>
        </div>

        {actionFeedbackKind === "save" && actionMessage ? (
          <p className="text-xs text-green-700">{actionMessage}</p>
        ) : null}
        {actionFeedbackKind === "save" && actionError ? (
          <p className="text-xs text-destructive">{actionError}</p>
        ) : null}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">
              {t(isRemoteMode ? "runtime.claude.titleRemote" : "runtime.claude.titleLocal")}
            </h3>
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? t("runtime.claude.descriptionRemote")
                : t("runtime.claude.descriptionLocal")}
            </p>
          </div>
          <span
            className={`rounded px-2 py-1 text-xs ${
              !isRemoteMode && claudeHealth?.sdk_installed
                ? "bg-green-100 text-green-700"
                : "bg-slate-100 text-slate-700"
            }`}
          >
            {isRemoteMode
              ? t("runtime.claude.badgeLocalOnly")
              : claudeHealth?.sdk_installed
                ? t("runtime.claude.badgeInstalled")
                : t("runtime.claude.badgeNotInstalled")}
          </span>
        </div>

        {isRemoteMode ? (
          <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            {t("runtime.claude.remoteNotice")}
          </div>
        ) : (
          <div className="mt-4 space-y-4">
            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={claudeSdkEnabled}
                onChange={(event) => onClaudeSdkEnabledChange(event.target.checked)}
                disabled={claudeActionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">{t("runtime.claude.enableSdkTitle")}</p>
                <p className="text-xs text-muted-foreground">
                  {t("runtime.claude.enableSdkDescription")}
                </p>
              </div>
            </label>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  {t("runtime.claude.defaultModelLabel")}
                </label>
                <Select
                  value={claudeDefaultModel}
                  onValueChange={(value) => {
                    if (value) {
                      onClaudeDefaultModelChange(value);
                    }
                  }}
                  disabled={claudeActionLoading !== null}
                >
                  <SelectTrigger className="bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {claudeModelOptions.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">
                  {t("runtime.claude.defaultEffortLabel")}
                </label>
                <Select
                  value={claudeDefaultEffort}
                  onValueChange={(value) => {
                    if (value) {
                      onClaudeDefaultEffortChange(value);
                    }
                  }}
                  disabled={claudeActionLoading !== null}
                >
                  <SelectTrigger className="bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {claudeDefaultThinkingBudgetOptions.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label htmlFor="claude-node-path-override" className="text-sm font-medium">
                  {t("runtime.claude.nodePathTitle")}
                </label>
                <Input
                  id="claude-node-path-override"
                  value={claudeNodePathOverride}
                  onChange={(event) => onClaudeNodePathOverrideChange(event.target.value)}
                  placeholder={t("runtime.claude.nodePathPlaceholder")}
                  disabled={claudeActionLoading !== null}
                />
                <p className="text-xs text-muted-foreground">
                  {t("runtime.claude.nodePathDescription")}
                </p>
              </div>

              <div className="space-y-2">
                <label htmlFor="claude-cli-path-override" className="text-sm font-medium">
                  {t("runtime.claude.cliPathTitle")}
                </label>
                <Input
                  id="claude-cli-path-override"
                  value={claudeCliPathOverride}
                  onChange={(event) => onClaudeCliPathOverrideChange(event.target.value)}
                  placeholder={t("runtime.claude.cliPathPlaceholder")}
                  disabled={claudeActionLoading !== null}
                />
                <p className="text-xs text-muted-foreground">
                  {t("runtime.claude.cliPathDescription")}
                </p>
              </div>
            </div>

            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p className="break-all">
                {t("runtime.common.installDir")}:
                {claudeHealth?.sdk_install_dir ?? t("runtime.status.checking")}
              </p>
              <p>
                {t("runtime.common.node")}:
                {claudeHealth?.node_available
                  ? t("runtime.status.available")
                  : t("runtime.status.unavailable")}
                {claudeHealth?.node_version ? `（${claudeHealth.node_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.sdk")}:
                {claudeHealth?.sdk_installed
                  ? t("runtime.status.installed")
                  : t("runtime.status.notInstalled")}
                {claudeHealth?.sdk_version ? `（${claudeHealth.sdk_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.cli")}:
                {claudeHealth?.cli_available
                  ? t("runtime.status.available")
                  : t("runtime.status.unavailable")}
                {claudeHealth?.cli_version ? `（${claudeHealth.cli_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.currentChannel")}:
                {claudeHealth?.effective_provider === "sdk"
                  ? t("runtime.claude.channel.sdkAgent")
                  : claudeHealth?.effective_provider === "cli"
                    ? t("runtime.claude.channel.cli")
                    : t("runtime.claude.channel.unavailable")}
              </p>
              {claudeHealth?.checked_at && (
                <p>
                  {t("runtime.common.checkedAt")}:{formatDate(claudeHealth.checked_at)}
                </p>
              )}
              {claudeHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">
                  {mapRuntimeStatusMessage(claudeHealth.sdk_status_message)}
                </p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button onClick={onClaudeSave} disabled={claudeActionLoading !== null}>
                {claudeActionLoading === "save" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {t("runtime.claude.actions.save")}
              </Button>
              <Button
                variant="outline"
                onClick={onClaudeInstall}
                disabled={claudeActionLoading !== null}
              >
                {claudeActionLoading === "install" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {t(
                  claudeHealth?.sdk_installed
                    ? "runtime.claude.actions.reinstall"
                    : "runtime.claude.actions.install",
                )}
              </Button>
              <Button
                variant="ghost"
                onClick={onClaudeRefresh}
                disabled={claudeActionLoading !== null}
              >
                <RefreshCw className={`h-4 w-4`} />
                {t("runtime.claude.actions.refresh")}
              </Button>
            </div>

            {claudeActionMessage && <p className="text-xs text-green-700">{claudeActionMessage}</p>}
            {claudeActionError && <p className="text-xs text-destructive">{claudeActionError}</p>}
          </div>
        )}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">
              {t(isRemoteMode ? "runtime.opencode.titleRemote" : "runtime.opencode.titleLocal")}
            </h3>
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? t("runtime.opencode.descriptionRemote")
                : t("runtime.opencode.descriptionLocal")}
            </p>
          </div>
          <span
            className={`rounded px-2 py-1 text-xs ${
              isRemoteMode
                ? remoteOpenCodeHealth?.available
                  ? "bg-green-100 text-green-700"
                  : "bg-slate-100 text-slate-700"
                : opencodeHealth?.sdk_installed
                  ? "bg-green-100 text-green-700"
                  : "bg-slate-100 text-slate-700"
            }`}
          >
            {isRemoteMode
              ? remoteOpenCodeHealth?.available
                ? t("runtime.status.remoteAvailable")
                : t("runtime.status.notReady")
              : opencodeHealth?.sdk_installed
                ? t("runtime.status.installed")
                : t("runtime.status.notInstalled")}
          </span>
        </div>

        {isRemoteMode ? (
          <div className="space-y-3">
            <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground space-y-1">
              <p>
                {t("runtime.common.remoteStatus")}:
                {remoteOpenCodeHealth?.message
                  ? mapRuntimeStatusMessage(remoteOpenCodeHealth.message)
                  : t("runtime.opencode.remoteStatusPending")}
              </p>
              {remoteOpenCodeHealth?.node_version ? (
                <p>
                  {t("runtime.common.remoteNode")}:{remoteOpenCodeHealth.node_version}
                </p>
              ) : null}
              {remoteOpenCodeHealth?.sdk_version ? (
                <p>
                  {t("runtime.common.remoteSdkVersion")}:{remoteOpenCodeHealth.sdk_version}
                </p>
              ) : null}
              {remoteOpenCodeHealth?.sdk_install_dir ? (
                <p>
                  {t("runtime.common.installDir")}:{remoteOpenCodeHealth.sdk_install_dir}
                </p>
              ) : null}
              {remoteOpenCodeHealth?.checked_at ? (
                <p>
                  {t("runtime.common.checkedAt")}:{formatDate(remoteOpenCodeHealth.checked_at)}
                </p>
              ) : null}
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={onOpenCodeRefresh}
                disabled={healthLoading || opencodeActionLoading !== null || !hasSelectedSshConfig}
              >
                {opencodeActionLoading !== null ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="mr-1.5 h-3.5 w-3.5" />
                )}
                {t("runtime.opencode.actions.recheck")}
              </Button>
              <Button
                size="sm"
                onClick={onOpenCodeInstall}
                disabled={
                  healthLoading ||
                  opencodeActionLoading !== null ||
                  !hasSelectedSshConfig ||
                  passwordAuthBlocked
                }
              >
                {opencodeActionLoading === "install" ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : null}
                {t(
                  remoteOpenCodeHealth?.sdk_installed
                    ? "runtime.opencode.actions.reinstallRemote"
                    : "runtime.opencode.actions.installRemote",
                )}
              </Button>
            </div>
            {opencodeActionMessage && (
              <p className="text-xs text-green-700">{opencodeActionMessage}</p>
            )}
            {opencodeActionError && (
              <p className="text-xs text-destructive">{opencodeActionError}</p>
            )}
          </div>
        ) : (
          <div className="mt-4 space-y-4">
            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={opencodeSdkEnabled}
                onChange={(event) => onOpenCodeSdkEnabledChange(event.target.checked)}
                disabled={opencodeActionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">{t("runtime.opencode.enableSdkTitle")}</p>
                <p className="text-xs text-muted-foreground">
                  {t("runtime.opencode.enableSdkDescription")}
                </p>
              </div>
            </label>

            <div className="space-y-2">
              <label htmlFor="opencode-default-model" className="text-sm font-medium">
                {t("runtime.opencode.defaultModelLabel")}
              </label>
              <div className="flex gap-2">
                <div className="flex-1">
                  {opencodeHealth?.sdk_installed ? (
                    <Select
                      value={opencodeDefaultModel}
                      onValueChange={(value) => {
                        if (value) onOpenCodeDefaultModelChange(value);
                      }}
                      disabled={opencodeActionLoading !== null || opencodeModelListLoading}
                    >
                      <SelectTrigger className="bg-background">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent className="max-h-72">
                        {defaultOpenCodeModelOptions.map((m) => (
                          <SelectItem key={m.value} value={m.value}>
                            {`${m.label} · ${m.providerName}`}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  ) : (
                    <Input
                      id="opencode-default-model"
                      value={opencodeDefaultModel}
                      onChange={(event) => onOpenCodeDefaultModelChange(event.target.value)}
                      placeholder={t("runtime.oneShot.modelPlaceholder")}
                      disabled={opencodeActionLoading !== null}
                    />
                  )}
                </div>
                {opencodeHealth?.sdk_installed && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={onOpenCodeFetchModels}
                    disabled={opencodeModelListLoading || opencodeActionLoading !== null}
                    title={t("runtime.opencode.fetchModelsTitle")}
                  >
                    {opencodeModelListLoading ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="h-3.5 w-3.5" />
                    )}
                  </Button>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                {opencodeModelList.length > 0
                  ? t("runtime.opencode.loadedModels", { count: opencodeModelList.length })
                  : t("runtime.opencode.modelFormatOrFetch")}
              </p>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label htmlFor="opencode-host" className="text-sm font-medium">
                  {t("runtime.opencode.hostLabel")}
                </label>
                <Input
                  id="opencode-host"
                  value={opencodeHost}
                  onChange={(event) => onOpenCodeHostChange(event.target.value)}
                  placeholder="127.0.0.1"
                  disabled={opencodeActionLoading !== null}
                />
              </div>

              <div className="space-y-2">
                <label htmlFor="opencode-port" className="text-sm font-medium">
                  {t("runtime.opencode.portLabel")}
                </label>
                <Input
                  id="opencode-port"
                  type="number"
                  value={String(opencodePort)}
                  onChange={(event) => onOpenCodePortChange(Number(event.target.value) || 4096)}
                  placeholder="4096"
                  disabled={opencodeActionLoading !== null}
                />
              </div>
            </div>

            <div className="space-y-2">
              <label htmlFor="opencode-node-path-override" className="text-sm font-medium">
                {t("runtime.opencode.nodePathTitle")}
              </label>
              <Input
                id="opencode-node-path-override"
                value={opencodeNodePathOverride}
                onChange={(event) => onOpenCodeNodePathOverrideChange(event.target.value)}
                placeholder={t("runtime.opencode.nodePathPlaceholder")}
                disabled={opencodeActionLoading !== null}
              />
              <p className="text-xs text-muted-foreground">
                {t("runtime.opencode.nodePathDescription")}
              </p>
            </div>

            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p className="break-all">
                {t("runtime.common.installDir")}:
                {opencodeHealth?.sdk_install_dir ?? t("runtime.status.checking")}
              </p>
              <p>
                {t("runtime.common.node")}:
                {opencodeHealth?.node_available
                  ? t("runtime.status.available")
                  : t("runtime.status.unavailable")}
                {opencodeHealth?.node_version ? `（${opencodeHealth.node_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.sdk")}:
                {opencodeHealth?.sdk_installed
                  ? t("runtime.status.installed")
                  : t("runtime.status.notInstalled")}
                {opencodeHealth?.sdk_version ? `（${opencodeHealth.sdk_version}）` : ""}
              </p>
              <p>
                {t("runtime.common.currentChannel")}:
                {opencodeHealth?.effective_provider === "sdk"
                  ? t("runtime.opencode.channel.sdk")
                  : t("runtime.opencode.channel.unavailable")}
              </p>
              {opencodeHealth?.checked_at && (
                <p>
                  {t("runtime.common.checkedAt")}:{formatDate(opencodeHealth.checked_at)}
                </p>
              )}
              {opencodeHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">
                  {mapRuntimeStatusMessage(opencodeHealth.sdk_status_message)}
                </p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button onClick={onOpenCodeSave} disabled={opencodeActionLoading !== null}>
                {opencodeActionLoading === "save" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {t("runtime.opencode.actions.save")}
              </Button>
              <Button
                variant="outline"
                onClick={onOpenCodeInstall}
                disabled={opencodeActionLoading !== null}
              >
                {opencodeActionLoading === "install" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {t(
                  opencodeHealth?.sdk_installed
                    ? "runtime.opencode.actions.reinstall"
                    : "runtime.opencode.actions.install",
                )}
              </Button>
              <Button
                variant="ghost"
                onClick={onOpenCodeRefresh}
                disabled={opencodeActionLoading !== null}
              >
                <RefreshCw className={`h-4 w-4`} />
                {t("runtime.opencode.actions.refresh")}
              </Button>
            </div>

            {opencodeActionMessage && (
              <p className="text-xs text-green-700">{opencodeActionMessage}</p>
            )}
            {opencodeActionError && (
              <p className="text-xs text-destructive">{opencodeActionError}</p>
            )}
          </div>
        )}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h3 className="mb-1 text-sm font-medium">{t("runtime.grok.title")}</h3>
            <p className="text-xs text-muted-foreground">{t("runtime.grok.description")}</p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={onGrokRefresh}
            disabled={healthLoading || grokActionLoading !== null}
          >
            <RefreshCw className="mr-1.5 h-3.5 w-3.5" />
            {t("runtime.grok.actions.refresh")}
          </Button>
        </div>

        <div className="rounded-md border border-border px-3 py-3 text-xs text-muted-foreground space-y-1">
          <p>
            {t("runtime.common.localStatus")}:
            {grokHealth?.status_message
              ? mapRuntimeStatusMessage(grokHealth.status_message)
              : t("runtime.grok.notChecked")}
          </p>
          {grokHealth?.cli_path ? (
            <p>
              {t("runtime.common.localPath")}:{grokHealth.cli_path}
            </p>
          ) : null}
          {grokHealth?.cli_version ? (
            <p>
              {t("runtime.common.localVersion")}:{grokHealth.cli_version}
            </p>
          ) : null}
          {grokHealth?.auth_ok === true ? (
            <p>
              {t("runtime.common.localLogin")}:{t("runtime.status.loggedIn")}
            </p>
          ) : null}
          {grokHealth?.auth_ok === false ? (
            <p className="text-destructive">
              {t("runtime.common.localLogin")}:{t("runtime.grok.localNotLoggedIn")}
            </p>
          ) : null}
          {isRemoteMode ? (
            <>
              <p>
                {t("runtime.common.remoteStatus")}:
                {remoteGrokHealth?.message
                  ? mapRuntimeStatusMessage(remoteGrokHealth.message)
                  : t("runtime.opencode.remoteStatusPending")}
              </p>
              {remoteGrokHealth?.version ? (
                <p>
                  {t("runtime.common.remoteVersion")}:{remoteGrokHealth.version}
                </p>
              ) : null}
              {remoteGrokHealth?.auth_ok === true ? (
                <p>
                  {t("runtime.common.remoteLogin")}:{t("runtime.status.loggedIn")}
                </p>
              ) : null}
              {remoteGrokHealth?.auth_ok === false ? (
                <p className="text-destructive">
                  {t("runtime.common.remoteLogin")}:{t("runtime.grok.remoteNotLoggedIn")}
                </p>
              ) : null}
            </>
          ) : null}
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("runtime.grok.defaultModelLabel")}</label>
            <Select
              value={grokDefaultModel}
              onValueChange={(value) => {
                if (value) onGrokDefaultModelChange(value);
              }}
              disabled={healthLoading || grokActionLoading !== null || grokModelListLoading}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(defaultGrokModelOptions.some((option) => option.value === grokDefaultModel)
                  ? defaultGrokModelOptions
                  : [
                      ...defaultGrokModelOptions,
                      { value: grokDefaultModel, label: grokDefaultModel, is_default: false },
                    ]
                ).map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                    {option.is_default ? t("runtime.grok.defaultSuffix") : ""}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {grokModelList.length > 0
                ? t("runtime.grok.loadedModels", { count: grokModelList.length })
                : grokModelListLoading
                  ? t("runtime.grok.loadingModels")
                  : t("runtime.grok.usingStaticModelList")}
            </p>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("runtime.grok.defaultEffortLabel")}</label>
            <Select
              value={grokDefaultEffort}
              onValueChange={(value) => {
                if (value) onGrokDefaultEffortChange(value);
              }}
              disabled={healthLoading || grokActionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {grokEffortOptions.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">{t("runtime.grok.cliPathTitle")}</label>
          <Input
            value={grokCliPathOverride}
            onChange={(event) => onGrokCliPathOverrideChange(event.target.value)}
            placeholder={t("runtime.grok.cliPathPlaceholder")}
            disabled={healthLoading || grokActionLoading !== null}
          />
          <p className="text-xs text-muted-foreground">{t("runtime.grok.cliPathDescription")}</p>
        </div>

        {(grokActionMessage || grokActionError) && (
          <div
            className={`rounded-md border px-3 py-2 text-sm ${grokActionError ? "border-destructive/40 text-destructive" : "border-border text-muted-foreground"}`}
          >
            {grokActionError ?? grokActionMessage}
          </div>
        )}

        <div className="flex flex-wrap items-center justify-end gap-2">
          <Button
            variant="outline"
            onClick={onGrokInstall}
            disabled={grokInstallDisabled}
            title={
              isRemoteMode && !hasSelectedSshConfig
                ? t("runtime.grok.tooltips.installNoSsh")
                : isRemoteMode
                  ? t("runtime.grok.tooltips.installRemote")
                  : t("runtime.grok.tooltips.installLocal")
            }
          >
            {grokActionLoading === "install" ? (
              <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
            ) : null}
            {grokInstallButtonLabel}
          </Button>
          <Button onClick={onGrokSave} disabled={healthLoading || grokActionLoading !== null}>
            {grokActionLoading === "save" ? (
              <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
            ) : null}
            {t("runtime.grok.actions.save")}
          </Button>
        </div>
      </div>
    </div>
  );
}
