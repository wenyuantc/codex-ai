import { Loader2, RefreshCw } from "lucide-react";

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
  AI_COMMIT_MESSAGE_LENGTH_OPTIONS,
  AI_PROVIDER_OPTIONS,
  CODEX_MODEL_OPTIONS,
  CLAUDE_MODEL_OPTIONS,
  CLAUDE_THINKING_BUDGET_OPTIONS,
  OPENCODE_EFFORT_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS,
  WORKTREE_LOCATION_MODE_OPTIONS,
  normalizeAiCommitMessageLength,
  normalizeTaskAutomationFailureStrategy,
  normalizeWorktreeLocationMode,
  type AiProvider,
  type AiCommitMessageLength,
  type TaskAutomationFailureStrategy,
  type WorktreeLocationMode,
} from "@/lib/types";
import type { OpenCodeModelInfo } from "@/lib/opencode";

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
  defaultTaskUseWorktree: boolean;
  worktreeLocationMode: WorktreeLocationMode;
  worktreeCustomRoot: string;
  aiCommitMessageLength: AiCommitMessageLength;
  gitAiProvider: AiProvider;
  aiCommitModel: string;
  aiCommitReasoningEffort: string;
  opencodeModelList: OpenCodeModelInfo[];
  opencodeModelListLoading: boolean;
  onTaskAutomationDefaultEnabledChange: (value: boolean) => void;
  onTaskAutomationMaxFixRoundsChange: (value: number) => void;
  onTaskAutomationFailureStrategyChange: (value: TaskAutomationFailureStrategy) => void;
  onDefaultTaskUseWorktreeChange: (value: boolean) => void;
  onWorktreeLocationModeChange: (value: WorktreeLocationMode) => void;
  onWorktreeCustomRootChange: (value: string) => void;
  onAiCommitMessageLengthChange: (value: AiCommitMessageLength) => void;
  onGitAiProviderChange: (value: AiProvider) => void;
  onAiCommitModelChange: (value: string) => void;
  onAiCommitReasoningEffortChange: (value: string) => void;
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
  defaultTaskUseWorktree,
  worktreeLocationMode,
  worktreeCustomRoot,
  aiCommitMessageLength,
  gitAiProvider,
  aiCommitModel,
  aiCommitReasoningEffort,
  opencodeModelList,
  opencodeModelListLoading,
  onTaskAutomationDefaultEnabledChange,
  onTaskAutomationMaxFixRoundsChange,
  onTaskAutomationFailureStrategyChange,
  onDefaultTaskUseWorktreeChange,
  onWorktreeLocationModeChange,
  onWorktreeCustomRootChange,
  onAiCommitMessageLengthChange,
  onGitAiProviderChange,
  onAiCommitModelChange,
  onAiCommitReasoningEffortChange,
  onOpenCodeFetchModels,
  onSave,
}: GitAutomationSettingsTabProps) {
  const showCustomWorktreeRoot = worktreeLocationMode === "custom_root";
  const selectedWorktreeLocationOption = WORKTREE_LOCATION_MODE_OPTIONS.find(
    (option) => option.value === worktreeLocationMode,
  );
  const selectedCommitLengthOption = AI_COMMIT_MESSAGE_LENGTH_OPTIONS.find(
    (option) => option.value === aiCommitMessageLength,
  );
  const worktreeRootPlaceholder = isRemoteMode ? "~/codex-worktrees" : "/Users/wenyuan/codex-worktrees";

  const isGitClaudeProvider = gitAiProvider === "claude";
  const isGitOpenCodeProvider = gitAiProvider === "opencode";
  const availableGitProviders = AI_PROVIDER_OPTIONS.filter(
    (option) => !(isRemoteMode && option.value === "opencode"),
  );
  const gitOpenCodeModelOptions = opencodeModelList.length > 0
    ? opencodeModelList
    : [{
      value: aiCommitModel,
      label: opencodeModelListLoading ? "正在加载模型..." : "当前模型",
      providerId: "opencode",
      providerName: "OpenCode",
      modelId: aiCommitModel.includes("/") ? aiCommitModel.split("/").slice(1).join("/") : aiCommitModel,
      capabilities: null,
    }];

  const gitCommitEffortOptions = isGitClaudeProvider
    ? CLAUDE_THINKING_BUDGET_OPTIONS.filter((option) => option.value !== "auto")
    : isGitOpenCodeProvider
      ? OPENCODE_EFFORT_OPTIONS
      : REASONING_EFFORT_OPTIONS;

  const gitProviderLabel = isGitClaudeProvider
    ? "Claude"
    : isGitOpenCodeProvider
      ? "OpenCode"
      : "Codex";

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-medium">自动质控默认设置</h3>
          <p className="text-xs text-muted-foreground">
            影响新建任务默认是否开启自动质控，以及自动审核/自动修复闭环的默认策略。
          </p>
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
            <p className="text-sm font-medium">新建任务默认开启自动质控</p>
            <p className="text-xs text-muted-foreground">
              开启后，新任务会默认进入"审核 {"->"} 修复 {"->"} 再审核"的闭环流程。
            </p>
          </div>
        </label>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">最大自动修复轮次</label>
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
                  {(value) => (typeof value === "string" && value.trim() ? `${value} 轮` : "选择轮次")}
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
                  onTaskAutomationFailureStrategyChange(normalizeTaskAutomationFailureStrategy(value));
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue>
                  {(value) =>
                    typeof value === "string"
                      ? TASK_AUTOMATION_FAILURE_STRATEGY_OPTIONS.find((option) => option.value === value)?.label ?? value
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

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="space-y-1">
          <h3 className="text-sm font-medium">Git 偏好</h3>
          <p className="text-xs text-muted-foreground">
            控制新任务的 Worktree 默认行为，以及 AI 生成 Git 提交信息时的长度。
          </p>
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
            <p className="text-sm font-medium">新建任务默认启用 Worktree</p>
            <p className="text-xs text-muted-foreground">
              开启后，新建任务会默认准备独立 Worktree；仍然可以在任务创建弹窗里单独改掉。
            </p>
          </div>
        </label>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Worktree 目录规则</label>
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
            <p className="text-xs text-muted-foreground">{selectedWorktreeLocationOption?.description}</p>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">AI 提交信息默认长度</label>
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
            <p className="text-xs text-muted-foreground">{selectedCommitLengthOption?.description}</p>
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
              onChange={(event) => onWorktreeCustomRootChange(event.target.value)}
              placeholder={worktreeRootPlaceholder}
              disabled={healthLoading || actionLoading !== null}
            />
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? "SSH 配置下要求绝对路径或 ~/ 开头，最终目录结构为 <root>/<repo>/<task>。"
                : "本地配置下要求绝对路径，最终目录结构为 <root>/<repo>/<task>。"}
            </p>
          </div>
        )}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">Git AI</h3>
            <p className="text-xs text-muted-foreground">
              控制 AI 自动生成 Git 提交信息时使用的提供商、模型和推理强度。
            </p>
          </div>
          <span className="rounded bg-secondary px-2 py-1 text-xs text-secondary-foreground">
            {gitProviderLabel}
          </span>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">AI 提供商</label>
            <Select<AiProvider>
              value={gitAiProvider}
              onValueChange={(value) => {
                if (value) {
                  onGitAiProviderChange(value as AiProvider);
                }
              }}
              disabled={healthLoading || actionLoading !== null}
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

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Git AI 模型</label>
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
                      disabled={healthLoading || actionLoading !== null || opencodeModelListLoading}
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
                      placeholder="openai/gpt-4o"
                      disabled={healthLoading || actionLoading !== null}
                    />
                  )}
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onOpenCodeFetchModels}
                  disabled={opencodeModelListLoading || healthLoading || actionLoading !== null}
                  title="从 OpenCode SDK 获取模型列表"
                >
                  {opencodeModelListLoading
                    ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    : <RefreshCw className="h-3.5 w-3.5" />
                  }
                </Button>
              </div>
            ) : (
              <Select
                value={aiCommitModel}
                onValueChange={(value) => {
                  if (value) {
                    onAiCommitModelChange(value);
                  }
                }}
                disabled={healthLoading || actionLoading !== null}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {(isGitClaudeProvider ? CLAUDE_MODEL_OPTIONS : CODEX_MODEL_OPTIONS).map((option) => (
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
                  ? `已加载 ${opencodeModelList.length} 个可用模型`
                  : "格式: provider/modelID（例如 openai/gpt-4o）"}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">Git AI 推理强度</label>
            <Select
              value={aiCommitReasoningEffort}
              onValueChange={(value) => {
                if (value) {
                  onAiCommitReasoningEffortChange(value);
                }
              }}
              disabled={healthLoading || actionLoading !== null}
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
          </div>
        </div>

        <div className="rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
          <p>当前 Git AI 提供商：{gitProviderLabel}</p>
          <p>当前模型：{aiCommitModel}</p>
          <p>当前推理强度：{aiCommitReasoningEffort}</p>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            onClick={onSave}
            disabled={healthLoading || actionLoading !== null || (isRemoteMode && !selectedSshConfigId)}
          >
            {actionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            保存配置
          </Button>
        </div>

        {actionMessage && <p className="text-xs text-green-700">{actionMessage}</p>}
        {actionError && <p className="text-xs text-destructive">{actionError}</p>}
      </div>
    </div>
  );
}
