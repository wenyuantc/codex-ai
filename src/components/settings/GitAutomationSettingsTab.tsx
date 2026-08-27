import { Loader2, RefreshCw } from "lucide-react";
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
  AI_COMMIT_MODEL_SOURCE_OPTIONS,
  AI_COMMIT_MESSAGE_LENGTH_OPTIONS,
  AI_PROVIDER_OPTIONS,
  CODEX_MODEL_OPTIONS,
  CLAUDE_MODEL_OPTIONS,
  CLAUDE_THINKING_BUDGET_OPTIONS,
  GROK_MODEL_OPTIONS,
  GROK_EFFORT_OPTIONS,
  OPENCODE_EFFORT_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS,
  WORKTREE_LOCATION_MODE_OPTIONS,
  normalizeAiCommitMessageLength,
  normalizeTaskAutomationFailureStrategy,
  normalizeWorktreeLocationMode,
  type AiChannel,
  type AiProvider,
  type AiCommitMessageLength,
  type AiCommitModelSource,
  type TaskAutomationFailureStrategy,
  type WorktreeLocationMode,
} from "@/lib/types";
import {
  resolveNativeThinking,
  selectNativeModel,
} from "@/components/employees/NativeChannelFields";
import type { OpenCodeModelInfo } from "@/lib/opencode";

const FAILURE_STRATEGY_OPTION_KEY_BY_VALUE: Record<string, string> = {
  blocked: "blocked",
  manual_control: "manualControl",
};

const WORKTREE_LOCATION_OPTION_KEY_BY_VALUE: Record<string, string> = {
  repo_sibling_hidden: "repoSiblingHidden",
  repo_child_hidden: "repoChildHidden",
  custom_root: "customRoot",
};

const COMMIT_MESSAGE_LENGTH_OPTION_KEY_BY_VALUE: Record<string, string> = {
  title_with_body: "titleWithBody",
  title_only: "titleOnly",
};

const COMMIT_MODEL_SOURCE_OPTION_KEY_BY_VALUE: Record<string, string> = {
  inherit_one_shot: "inheritOneShot",
  custom: "custom",
};

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

interface GitAutomationSettingsTabProps {
  isRemoteMode: boolean;
  selectedSshConfigId: string | null;
  healthLoading: boolean;
  actionLoading: "save" | "install" | null;
  actionMessage: string | null;
  actionError: string | null;
  taskAutomationDefaultEnabled: boolean;
  taskAutomationMaxFixRounds: number;
  taskAutomationFailureStrategy: TaskAutomationFailureStrategy;
  testerAutomationEnabled: boolean;
  testerAllowAiOnly: boolean;
  defaultTestCommand: string;
  defaultTaskUseWorktree: boolean;
  worktreeLocationMode: WorktreeLocationMode;
  worktreeCustomRoot: string;
  aiCommitMessageLength: AiCommitMessageLength;
  aiCommitModelSource: AiCommitModelSource;
  gitAiProvider: AiProvider;
  aiCommitModel: string;
  aiCommitReasoningEffort: string;
  aiCommitNativeChannelId: string;
  nativeChannels: AiChannel[];
  opencodeModelList: OpenCodeModelInfo[];
  opencodeModelListLoading: boolean;
  onTaskAutomationDefaultEnabledChange: (value: boolean) => void;
  onTaskAutomationMaxFixRoundsChange: (value: number) => void;
  onTaskAutomationFailureStrategyChange: (value: TaskAutomationFailureStrategy) => void;
  onTesterAutomationEnabledChange: (value: boolean) => void;
  onTesterAllowAiOnlyChange: (value: boolean) => void;
  onDefaultTestCommandChange: (value: string) => void;
  onDefaultTaskUseWorktreeChange: (value: boolean) => void;
  onWorktreeLocationModeChange: (value: WorktreeLocationMode) => void;
  onWorktreeCustomRootChange: (value: string) => void;
  onAiCommitMessageLengthChange: (value: AiCommitMessageLength) => void;
  onAiCommitModelSourceChange: (value: AiCommitModelSource) => void;
  onGitAiProviderChange: (value: AiProvider) => void;
  onAiCommitModelChange: (value: string) => void;
  onAiCommitReasoningEffortChange: (value: string) => void;
  onAiCommitNativeChannelIdChange: (value: string) => void;
  onOpenCodeFetchModels: () => void;
  onSave: () => void;
}

export function GitAutomationSettingsTab({
  isRemoteMode,
  selectedSshConfigId,
  healthLoading,
  actionLoading,
  actionMessage,
  actionError,
  taskAutomationDefaultEnabled,
  taskAutomationMaxFixRounds,
  taskAutomationFailureStrategy,
  testerAutomationEnabled,
  testerAllowAiOnly,
  defaultTestCommand,
  defaultTaskUseWorktree,
  worktreeLocationMode,
  worktreeCustomRoot,
  aiCommitMessageLength,
  aiCommitModelSource,
  gitAiProvider,
  aiCommitModel,
  aiCommitReasoningEffort,
  aiCommitNativeChannelId,
  nativeChannels,
  opencodeModelList,
  opencodeModelListLoading,
  onTaskAutomationDefaultEnabledChange,
  onTaskAutomationMaxFixRoundsChange,
  onTaskAutomationFailureStrategyChange,
  onTesterAutomationEnabledChange,
  onTesterAllowAiOnlyChange,
  onDefaultTestCommandChange,
  onDefaultTaskUseWorktreeChange,
  onWorktreeLocationModeChange,
  onWorktreeCustomRootChange,
  onAiCommitMessageLengthChange,
  onAiCommitModelSourceChange,
  onGitAiProviderChange,
  onAiCommitModelChange,
  onAiCommitReasoningEffortChange,
  onAiCommitNativeChannelIdChange,
  onOpenCodeFetchModels,
  onSave,
}: GitAutomationSettingsTabProps) {
  const { t } = useTranslation("settings");
  const failureStrategyOptions = TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.failureStrategy.${FAILURE_STRATEGY_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const worktreeLocationOptions = WORKTREE_LOCATION_MODE_OPTIONS.map((option) => {
    const key = WORKTREE_LOCATION_OPTION_KEY_BY_VALUE[option.value];
    return {
      ...option,
      label: t(`git.options.worktreeLocation.${key}.label`),
      description: t(`git.options.worktreeLocation.${key}.description`),
    };
  });
  const commitMessageLengthOptions = AI_COMMIT_MESSAGE_LENGTH_OPTIONS.map((option) => {
    const key = COMMIT_MESSAGE_LENGTH_OPTION_KEY_BY_VALUE[option.value];
    return {
      ...option,
      label: t(`git.options.commitMessageLength.${key}.label`),
      description: t(`git.options.commitMessageLength.${key}.description`),
    };
  });
  const commitModelSourceOptions = AI_COMMIT_MODEL_SOURCE_OPTIONS.map((option) => {
    const key = COMMIT_MODEL_SOURCE_OPTION_KEY_BY_VALUE[option.value];
    return {
      ...option,
      label: t(`git.options.commitModelSource.${key}.label`),
      description: t(`git.options.commitModelSource.${key}.description`),
    };
  });
  const providerOptions = AI_PROVIDER_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.providers.${option.value}`),
  }));
  const codexModelOptions = CODEX_MODEL_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.codexModels.${CODEX_MODEL_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const claudeModelOptions = CLAUDE_MODEL_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.claudeModels.${CLAUDE_MODEL_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const grokModelOptions = GROK_MODEL_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.grokModels.${GROK_MODEL_OPTION_KEY_BY_VALUE[option.value]}`),
  }));
  const reasoningEffortOptions = REASONING_EFFORT_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.reasoningEffort.${option.value}`),
  }));
  const openCodeEffortOptions = OPENCODE_EFFORT_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.reasoningEffort.${option.value}`),
  }));
  const grokEffortOptions = GROK_EFFORT_OPTIONS.map((option) => ({
    ...option,
    label: t(`git.options.reasoningEffort.${option.value}`),
  }));
  const claudeThinkingBudgetOptions = CLAUDE_THINKING_BUDGET_OPTIONS.filter(
    (option) => option.value !== "auto",
  ).map((option) => ({
    ...option,
    label: t(`git.options.claudeThinkingBudget.${option.value}`),
  }));
  const showCustomWorktreeRoot = worktreeLocationMode === "custom_root";
  const selectedWorktreeLocationOption = worktreeLocationOptions.find(
    (option) => option.value === worktreeLocationMode,
  );
  const selectedCommitLengthOption = commitMessageLengthOptions.find(
    (option) => option.value === aiCommitMessageLength,
  );
  const selectedCommitModelSourceOption = commitModelSourceOptions.find(
    (option) => option.value === aiCommitModelSource,
  );
  const worktreeRootPlaceholder = t(
    isRemoteMode
      ? "git.preferences.customRootPlaceholderRemote"
      : "git.preferences.customRootPlaceholderLocal",
  );

  const isGitAiCustom = aiCommitModelSource === "custom";
  const isGitClaudeProvider = gitAiProvider === "claude";
  const isGitOpenCodeProvider = gitAiProvider === "opencode";
  const isGitGrokProvider = gitAiProvider === "grok";
  const isGitNativeProvider = gitAiProvider === "native";
  const selectedGitNativeChannel = nativeChannels.find(
    (channel) => channel.id === aiCommitNativeChannelId,
  );
  const gitNativeModelOptions = selectedGitNativeChannel?.models.map((item) => item.id) ?? [];
  const gitNativeThinking = resolveNativeThinking(selectedGitNativeChannel, aiCommitModel);
  const gitNativeEffortOptions = gitNativeThinking.levels.map((level) => ({
    value: level,
    label: t(`runtime.options.nativeThinkingLevels.${level}`, { defaultValue: level }),
  }));
  /** 历史配置的推理强度可能不在当前模型思考等级内，展示时回退到模型默认等级。 */
  const effectiveGitNativeEffort = gitNativeThinking.levels.includes(aiCommitReasoningEffort)
    ? aiCommitReasoningEffort
    : gitNativeThinking.defaultLevel;
  const availableGitProviders = providerOptions;
  const gitOpenCodeModelOptions =
    opencodeModelList.length > 0
      ? opencodeModelList
      : [
          {
            value: aiCommitModel,
            label: aiCommitModel,
            providerId: "opencode",
            providerName: t("git.options.providers.opencode"),
            modelId: aiCommitModel.includes("/")
              ? aiCommitModel.split("/").slice(1).join("/")
              : aiCommitModel,
            capabilities: null,
          },
        ];

  const gitCommitEffortOptions = isGitClaudeProvider
    ? claudeThinkingBudgetOptions
    : isGitOpenCodeProvider
      ? openCodeEffortOptions
      : isGitGrokProvider
        ? grokEffortOptions
        : isGitNativeProvider
          ? gitNativeEffortOptions
          : reasoningEffortOptions;

  const gitModelOptions = isGitClaudeProvider
    ? claudeModelOptions
    : isGitGrokProvider
      ? grokModelOptions
      : isGitNativeProvider
        ? gitNativeModelOptions.map((id) => ({ value: id, label: id }))
        : codexModelOptions;

  const gitProviderLabel =
    providerOptions.find((option) => option.value === gitAiProvider)?.label ?? gitAiProvider;

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-medium">{t("git.automation.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("git.automation.description")}</p>
        </div>

        <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
          <input
            type="checkbox"
            className="mt-0.5 h-4 w-4 rounded border-input"
            checked={taskAutomationDefaultEnabled}
            onChange={(event) => onTaskAutomationDefaultEnabledChange(event.target.checked)}
            disabled={healthLoading || actionLoading !== null}
          />
          <div className="space-y-1">
            <p className="text-sm font-medium">{t("git.automation.enableDefaultTitle")}</p>
            <p className="text-xs text-muted-foreground">
              {t("git.automation.enableDefaultDescription")}
            </p>
          </div>
        </label>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("git.automation.maxFixRoundsLabel")}</label>
            <Select
              value={String(taskAutomationMaxFixRounds)}
              onValueChange={(value) => {
                const nextValue = Number(value);
                if (Number.isFinite(nextValue)) {
                  onTaskAutomationMaxFixRoundsChange(nextValue);
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue>
                  {(value) =>
                    typeof value === "string" && value.trim()
                      ? t("git.automation.roundsValue", { count: Number(value) })
                      : t("git.automation.selectRounds")
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {Array.from({ length: 10 }, (_, index) => index + 1).map((round) => (
                  <SelectItem key={round} value={String(round)}>
                    {t("git.automation.roundsValue", { count: round })}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">
              {t("git.automation.failureStrategyLabel")}
            </label>
            <Select<TaskAutomationFailureStrategy>
              value={taskAutomationFailureStrategy}
              onValueChange={(value) => {
                if (value) {
                  onTaskAutomationFailureStrategyChange(
                    normalizeTaskAutomationFailureStrategy(value),
                  );
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue>
                  {(value) =>
                    typeof value === "string"
                      ? (failureStrategyOptions.find((option) => option.value === value)?.label ??
                        value)
                      : t("git.automation.selectFailureStrategy")
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {failureStrategyOptions.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <p className="text-xs leading-5 text-muted-foreground">
          {t("git.automation.summaryTemplate", {
            defaultMode: t(
              taskAutomationDefaultEnabled
                ? "git.automation.defaultOn"
                : "git.automation.defaultOff",
            ),
            rounds: t("git.automation.roundsValue", { count: taskAutomationMaxFixRounds }),
            strategy:
              failureStrategyOptions.find(
                (option) => option.value === taskAutomationFailureStrategy,
              )?.label ?? taskAutomationFailureStrategy,
          })}
        </p>
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-medium">{t("git.tester.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("git.tester.description")}</p>
        </div>

        {!testerAutomationEnabled && (
          <div className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs leading-5 text-amber-900 dark:text-amber-100">
            {t("git.tester.disabledNotice")}
          </div>
        )}

        <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
          <input
            type="checkbox"
            className="mt-0.5 h-4 w-4 rounded border-input"
            checked={testerAutomationEnabled}
            onChange={(event) => onTesterAutomationEnabledChange(event.target.checked)}
            disabled={healthLoading || actionLoading !== null}
          />
          <div className="space-y-1">
            <p className="text-sm font-medium">{t("git.tester.enableTitle")}</p>
            <p className="text-xs text-muted-foreground">{t("git.tester.enableDescription")}</p>
          </div>
        </label>

        <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
          <input
            type="checkbox"
            className="mt-0.5 h-4 w-4 rounded border-input"
            checked={testerAllowAiOnly}
            onChange={(event) => onTesterAllowAiOnlyChange(event.target.checked)}
            disabled={healthLoading || actionLoading !== null}
          />
          <div className="space-y-1">
            <p className="text-sm font-medium">{t("git.tester.allowAiOnlyTitle")}</p>
            <p className="text-xs text-muted-foreground">
              {t("git.tester.allowAiOnlyDescription")}
            </p>
          </div>
        </label>

        <div className="space-y-2">
          <label className="text-sm font-medium">{t("git.tester.defaultTestCommandLabel")}</label>
          <Input
            value={defaultTestCommand}
            onChange={(event) => onDefaultTestCommandChange(event.target.value)}
            placeholder={t("git.tester.defaultTestCommandPlaceholder")}
            disabled={healthLoading || actionLoading !== null}
            className="bg-background font-mono text-sm"
          />
          <p className="text-xs text-muted-foreground">
            {t("git.tester.defaultTestCommandDescription")}
          </p>
        </div>
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-medium">{t("git.preferences.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("git.preferences.description")}</p>
        </div>

        <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
          <input
            type="checkbox"
            className="mt-0.5 h-4 w-4 rounded border-input"
            checked={defaultTaskUseWorktree}
            onChange={(event) => onDefaultTaskUseWorktreeChange(event.target.checked)}
            disabled={healthLoading || actionLoading !== null}
          />
          <div className="space-y-1">
            <p className="text-sm font-medium">{t("git.preferences.enableWorktreeTitle")}</p>
            <p className="text-xs text-muted-foreground">
              {t("git.preferences.enableWorktreeDescription")}
            </p>
          </div>
        </label>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">
              {t("git.preferences.worktreeLocationLabel")}
            </label>
            <Select<WorktreeLocationMode>
              value={worktreeLocationMode}
              onValueChange={(value) => {
                if (value) {
                  onWorktreeLocationModeChange(normalizeWorktreeLocationMode(value));
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue>
                  {(value) =>
                    typeof value === "string"
                      ? (worktreeLocationOptions.find((option) => option.value === value)?.label ??
                        value)
                      : t("git.preferences.selectWorktreeLocation")
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {worktreeLocationOptions.map((option) => (
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
            <label className="text-sm font-medium">
              {t("git.preferences.commitMessageLengthLabel")}
            </label>
            <Select<AiCommitMessageLength>
              value={aiCommitMessageLength}
              onValueChange={(value) => {
                if (value) {
                  onAiCommitMessageLengthChange(normalizeAiCommitMessageLength(value));
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue>
                  {(value) =>
                    typeof value === "string"
                      ? (commitMessageLengthOptions.find((option) => option.value === value)
                          ?.label ?? value)
                      : t("git.preferences.selectCommitMessageLength")
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {commitMessageLengthOptions.map((option) => (
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
              {t("git.preferences.customRootLabel")}
            </label>
            <Input
              id="worktree-custom-root"
              value={worktreeCustomRoot}
              onChange={(event) => onWorktreeCustomRootChange(event.target.value)}
              placeholder={worktreeRootPlaceholder}
              disabled={healthLoading || actionLoading !== null}
            />
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? t("git.preferences.customRootDescriptionRemote")
                : t("git.preferences.customRootDescriptionLocal")}
            </p>
          </div>
        )}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">{t("git.gitAi.title")}</h3>
            <p className="text-xs text-muted-foreground">{t("git.gitAi.description")}</p>
          </div>
          <span className="rounded bg-secondary px-2 py-1 text-xs text-secondary-foreground">
            {isGitAiCustom ? gitProviderLabel : t("git.gitAi.badgeFollowOneShot")}
          </span>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("git.gitAi.modelSourceLabel")}</label>
            <Select<AiCommitModelSource>
              value={aiCommitModelSource}
              onValueChange={(value) => {
                if (value) {
                  onAiCommitModelSourceChange(value as AiCommitModelSource);
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {commitModelSourceOptions.map((option) => (
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

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("git.gitAi.providerLabel")}</label>
            <Select<AiProvider>
              value={gitAiProvider}
              onValueChange={(value) => {
                if (value) {
                  onGitAiProviderChange(value as AiProvider);
                }
              }}
              disabled={healthLoading || actionLoading !== null || !isGitAiCustom}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {availableGitProviders.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {isGitAiCustom && isGitNativeProvider ? (
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("git.gitAi.channelLabel")}</label>
            <Select
              value={aiCommitNativeChannelId || undefined}
              onValueChange={(value) => {
                if (!value) return;
                onAiCommitNativeChannelIdChange(value);
                const channel = nativeChannels.find((item) => item.id === value);
                const nextModel = selectNativeModel(channel, aiCommitModel);
                onAiCommitModelChange(nextModel);
                const thinking = resolveNativeThinking(channel, nextModel);
                onAiCommitReasoningEffortChange(thinking.defaultLevel);
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue>
                  {(value) => {
                    if (typeof value !== "string") {
                      return t("git.gitAi.selectChannel");
                    }
                    const channel = nativeChannels.find((item) => item.id === value);
                    return channel
                      ? `${channel.name} · ${channel.protocol}`
                      : t("git.gitAi.selectChannel");
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
              <p className="text-xs text-destructive">{t("git.gitAi.noChannelHint")}</p>
            ) : null}
          </div>
        ) : null}

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("git.gitAi.modelLabel")}</label>
            {isGitOpenCodeProvider ? (
              <div className="flex gap-2">
                <div className="flex-1">
                  {opencodeModelList.length > 0 ? (
                    <Select
                      value={aiCommitModel}
                      onValueChange={(value) => {
                        if (value) {
                          onAiCommitModelChange(value);
                        }
                      }}
                      disabled={
                        healthLoading ||
                        actionLoading !== null ||
                        opencodeModelListLoading ||
                        !isGitAiCustom
                      }
                    >
                      <SelectTrigger className="bg-background">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent className="max-h-72">
                        {gitOpenCodeModelOptions.map((model) => (
                          <SelectItem key={model.value} value={model.value}>
                            {`${model.label} · ${model.providerName}`}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  ) : (
                    <Input
                      value={aiCommitModel}
                      onChange={(event) => onAiCommitModelChange(event.target.value)}
                      placeholder={t("runtime.oneShot.modelPlaceholder")}
                      disabled={healthLoading || actionLoading !== null || !isGitAiCustom}
                    />
                  )}
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onOpenCodeFetchModels}
                  disabled={
                    opencodeModelListLoading ||
                    healthLoading ||
                    actionLoading !== null ||
                    !isGitAiCustom
                  }
                  title={t("runtime.opencode.fetchModelsTitle")}
                >
                  {opencodeModelListLoading ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <RefreshCw className="h-3.5 w-3.5" />
                  )}
                </Button>
              </div>
            ) : isGitNativeProvider && gitNativeModelOptions.length === 0 ? (
              <Input
                value={aiCommitModel}
                onChange={(event) => onAiCommitModelChange(event.target.value)}
                placeholder={t("runtime.oneShot.modelPlaceholder")}
                disabled={healthLoading || actionLoading !== null || !isGitAiCustom}
              />
            ) : (
              <Select
                value={aiCommitModel}
                onValueChange={(value) => {
                  if (value) {
                    onAiCommitModelChange(value);
                  }
                }}
                disabled={healthLoading || actionLoading !== null || !isGitAiCustom}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {gitModelOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {isGitOpenCodeProvider && (
              <p className="text-xs text-muted-foreground">
                {opencodeModelList.length > 0
                  ? t("git.gitAi.loadedModels", { count: opencodeModelList.length })
                  : t("git.gitAi.modelFormat")}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("git.gitAi.reasoningLabel")}</label>
            <Select
              value={isGitNativeProvider ? effectiveGitNativeEffort : aiCommitReasoningEffort}
              onValueChange={(value) => {
                if (value) {
                  onAiCommitReasoningEffortChange(value);
                }
              }}
              disabled={healthLoading || actionLoading !== null || !isGitAiCustom}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {gitCommitEffortOptions.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {isGitNativeProvider ? (
              <p className="text-xs text-muted-foreground">
                {gitNativeThinking.enabled
                  ? t("runtime.oneShot.nativeThinkingFromChannel")
                  : t("runtime.oneShot.nativeThinkingDisabled")}
              </p>
            ) : null}
          </div>
        </div>

        <div className="rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
          <p>
            {t("git.gitAi.currentProviderLabel")}:
            {isGitAiCustom ? gitProviderLabel : t("git.gitAi.badgeFollowOneShot")}
          </p>
          {isGitAiCustom && (
            <>
              <p>
                {t("git.gitAi.currentModelLabel")}:{aiCommitModel}
              </p>
              <p>
                {t("git.gitAi.currentReasoningLabel")}:
                {gitCommitEffortOptions.find((option) => option.value === aiCommitReasoningEffort)
                  ?.label ?? aiCommitReasoningEffort}
              </p>
            </>
          )}
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            onClick={onSave}
            disabled={
              healthLoading || actionLoading !== null || (isRemoteMode && !selectedSshConfigId)
            }
          >
            {actionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            {t("git.gitAi.actions.save")}
          </Button>
        </div>

        {actionMessage && <p className="text-xs text-green-700">{actionMessage}</p>}
        {actionError && <p className="text-xs text-destructive">{actionError}</p>}
      </div>
    </div>
  );
}
