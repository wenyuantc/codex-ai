import { Loader2 } from "lucide-react";

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
  type CodexModelId,
  type ReasoningEffort,
  type TaskAutomationFailureStrategy,
  type WorktreeLocationMode,
} from "@/lib/types";

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
  aiCommitModelSource: AiCommitModelSource;
  aiCommitModel: CodexModelId;
  aiCommitReasoningEffort: ReasoningEffort;
  onTaskAutomationDefaultEnabledChange: (value: boolean) => void;
  onTaskAutomationMaxFixRoundsChange: (value: number) => void;
  onTaskAutomationFailureStrategyChange: (value: TaskAutomationFailureStrategy) => void;
  onDefaultTaskUseWorktreeChange: (value: boolean) => void;
  onWorktreeLocationModeChange: (value: WorktreeLocationMode) => void;
  onWorktreeCustomRootChange: (value: string) => void;
  onAiCommitMessageLengthChange: (value: AiCommitMessageLength) => void;
  onAiCommitModelSourceChange: (value: AiCommitModelSource) => void;
  onAiCommitModelChange: (value: CodexModelId) => void;
  onAiCommitReasoningEffortChange: (value: ReasoningEffort) => void;
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
  aiCommitModelSource,
  aiCommitModel,
  aiCommitReasoningEffort,
  onTaskAutomationDefaultEnabledChange,
  onTaskAutomationMaxFixRoundsChange,
  onTaskAutomationFailureStrategyChange,
  onDefaultTaskUseWorktreeChange,
  onWorktreeLocationModeChange,
  onWorktreeCustomRootChange,
  onAiCommitMessageLengthChange,
  onAiCommitModelSourceChange,
  onAiCommitModelChange,
  onAiCommitReasoningEffortChange,
  onSave,
}: GitAutomationSettingsTabProps) {
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
  const worktreeRootPlaceholder = isRemoteMode ? "~/codex-worktrees" : "/Users/wenyuan/codex-worktrees";

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
              开启后，新任务会默认进入“审核 {"->"} 修复 {"->"} 再审核”的闭环流程。
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
            控制新任务的 Worktree 默认行为，以及 AI 生成 Git 提交信息时的长度、模型和推理强度。
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

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Git AI 模型来源</label>
            <Select<AiCommitModelSource>
              value={aiCommitModelSource}
              onValueChange={(value) => {
                if (value) {
                  onAiCommitModelSourceChange(normalizeAiCommitModelSource(value));
                }
              }}
              disabled={healthLoading || actionLoading !== null}
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
            <p className="text-xs text-muted-foreground">{selectedCommitModelSourceOption?.description}</p>
          </div>
        </div>

        {gitAiUsesCustomModel && (
          <div className="grid gap-3 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Git AI 模型</label>
              <Select<CodexModelId>
                value={aiCommitModel}
                onValueChange={(value) => {
                  if (value) {
                    onAiCommitModelChange(normalizeCodexModel(value));
                  }
                }}
                disabled={healthLoading || actionLoading !== null}
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
                    onAiCommitReasoningEffortChange(normalizeReasoningEffort(value));
                  }
                }}
                disabled={healthLoading || actionLoading !== null}
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
