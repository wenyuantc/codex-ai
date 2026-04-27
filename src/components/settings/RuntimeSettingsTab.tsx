import { Loader2, Monitor, Moon, RefreshCw, ShieldAlert, Sun, type LucideIcon } from "lucide-react";

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
  OPENCODE_EFFORT_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  type AiProvider,
  type ClaudeHealthCheck,
  type CodexHealthCheck,
  type CodexSettings,
  type RemoteCodexHealthCheck,
} from "@/lib/types";
import type { OpenCodeHealthCheck, OpenCodeModelInfo } from "@/lib/opencode";
import { type ThemeMode } from "@/lib/theme";
import { formatDate } from "@/lib/utils";

interface RuntimeSettingsTabProps {
  codexHealth: CodexHealthCheck | RemoteCodexHealthCheck | null;
  codexSettings: CodexSettings | null;
  healthLoading: boolean;
  actionLoading: "save" | "install" | null;
  actionMessage: string | null;
  actionError: string | null;
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
  nodePathOverride: string;
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
  onTaskSdkEnabledChange: (value: boolean) => void;
  onOneShotSdkEnabledChange: (value: boolean) => void;
  onOneShotPreferredProviderChange: (value: AiProvider) => void;
  onOneShotModelChange: (value: string) => void;
  onOneShotReasoningEffortChange: (value: string) => void;
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
}

const themeOptions: { value: ThemeMode; label: string; icon: LucideIcon }[] = [
  { value: "light", label: "亮色", icon: Sun },
  { value: "dark", label: "暗色", icon: Moon },
  { value: "system", label: "跟随系统", icon: Monitor },
];

const CLAUDE_DEFAULT_THINKING_BUDGET_OPTIONS = CLAUDE_THINKING_BUDGET_OPTIONS.filter(
  (option) => option.value !== "auto",
);

export function RuntimeSettingsTab({
  codexHealth,
  codexSettings,
  healthLoading,
  actionLoading,
  actionMessage,
  actionError,
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
  nodePathOverride,
  themeMode,
  onThemeModeChange,
  onTaskSdkEnabledChange,
  onOneShotSdkEnabledChange,
  onOneShotPreferredProviderChange,
  onOneShotModelChange,
  onOneShotReasoningEffortChange,
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
}: RuntimeSettingsTabProps) {
  const taskProviderLabel =
    codexHealth?.task_execution_effective_provider === "sdk" ? "SDK" : "exec（自动回退）";
  const oneShotProviderLabel =
    oneShotPreferredProvider === "claude"
      ? "Claude"
      : oneShotPreferredProvider === "opencode"
        ? "OpenCode"
        : "Codex";
  const oneShotChannelLabel = (() => {
    const channel = codexHealth?.one_shot_effective_channel;
    if (channel === "sdk") return "SDK";
    if (channel === "cli") return isRemoteMode ? "CLI（远程）" : "CLI";
    if (channel === "exec") return isRemoteMode ? "exec（远程）" : "exec（自动回退）";
    return "不可用";
  })();
  const installButtonLabel = codexHealth?.sdk_installed ? "重装 SDK" : "安装 SDK";
  const saveDisabled = healthLoading || actionLoading !== null || (isRemoteMode && !hasSelectedSshConfig);
  const installDisabled =
    healthLoading || actionLoading !== null || (isRemoteMode && (!hasSelectedSshConfig || passwordAuthBlocked));
  const availableOneShotProviders = AI_PROVIDER_OPTIONS.filter(
    (option) => !(isRemoteMode && option.value === "opencode"),
  );
  const isOneShotCodexProvider = oneShotPreferredProvider === "codex";
  const isOneShotClaudeProvider = oneShotPreferredProvider === "claude";
  const isOneShotOpenCodeProvider = oneShotPreferredProvider === "opencode";
  const oneShotOpenCodeModelOptions = opencodeModelList.length > 0
    ? opencodeModelList
    : [{
      value: oneShotModel,
      label: opencodeModelListLoading ? "正在加载模型..." : "当前模型",
      providerId: "opencode",
      providerName: "OpenCode",
      modelId: oneShotModel.includes("/") ? oneShotModel.split("/").slice(1).join("/") : oneShotModel,
      capabilities: null,
    }];
  const defaultOpenCodeModelOptions = opencodeModelList.length > 0
    ? opencodeModelList
    : [{
      value: opencodeDefaultModel,
      label: opencodeModelListLoading ? "正在加载模型..." : "当前模型",
      providerId: "opencode",
      providerName: "OpenCode",
      modelId: opencodeDefaultModel.includes("/") ? opencodeDefaultModel.split("/").slice(1).join("/") : opencodeDefaultModel,
      capabilities: null,
    }];
  const canUseOneShotSdkToggle = !isRemoteMode || isOneShotCodexProvider || isOneShotOpenCodeProvider;
  const selectedOneShotStatusMessage = codexHealth?.one_shot_status_message;

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div>
          <h3 className="mb-1 text-sm font-medium">主题模式</h3>
          <p className="mb-3 text-xs text-muted-foreground">选择应用的显示主题</p>
          <div className="flex flex-wrap gap-2">
            {themeOptions.map((option) => (
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
                {option.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
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

        {isRemoteMode && !hasSelectedSshConfig && (
          <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            当前是 SSH 模式，但还没有可用的 SSH 配置。请先切到“SSH 配置”tab 新增或选择配置。
          </div>
        )}

        {passwordAuthBlocked && (
          <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-3 text-sm text-amber-800">
            <div className="flex items-center gap-2 font-medium">
              <ShieldAlert className="h-4 w-4" />
              配置存在但当前平台不可执行
            </div>
            <p className="mt-1 text-xs leading-5">
              当前 SSH 配置使用密码认证，但测试连接尚未通过。远程 Codex 校验、SDK 安装和实际执行链路都必须保持阻断。
            </p>
          </div>
        )}

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
                  ? "SSH 模式下任务运行会优先使用远程 SDK；如果远程 SDK 不可用，则自动回退到远程 codex exec。"
                  : "任务运行优先走 SDK，失败时自动回退到 `codex exec`。一次性 AI 在下方单独配置。"}
              </p>
            </div>
            <span
              className={`rounded px-2 py-1 text-xs ${
                codexHealth?.task_execution_effective_provider === "sdk"
                  ? "bg-green-100 text-green-700"
                  : "bg-slate-100 text-slate-700"
              }`}
            >
              {healthLoading ? "检测中" : `任务 ${taskProviderLabel}`}
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
                <p className="text-sm font-medium">运行任务时使用 SDK</p>
                <p className="text-xs text-muted-foreground">
                  影响看板任务运行、员工启动任务，以及相关重启/恢复链路。
                </p>
              </div>
            </label>

            <div className="space-y-2">
              <label htmlFor="node-path-override" className="text-sm font-medium">
                Node 路径覆盖（可选）
              </label>
              <Input
                id="node-path-override"
                value={nodePathOverride}
                onChange={(event) => onNodePathOverrideChange(event.target.value)}
                placeholder={isRemoteMode ? "/usr/local/bin/node" : "/opt/homebrew/bin/node"}
                disabled={healthLoading || actionLoading !== null}
              />
              <p className="text-xs text-muted-foreground">留空时自动从系统 PATH 中查找 Node。</p>
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
              {codexHealth?.checked_at && <p>检测时间：{formatDate(codexHealth.checked_at)}</p>}
              {codexHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">{codexHealth.sdk_status_message}</p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                onClick={onInstall}
                disabled={installDisabled}
              >
                {actionLoading === "install" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {installButtonLabel}
              </Button>
              <Button variant="ghost" onClick={onRefresh} disabled={healthLoading || actionLoading !== null}>
                <RefreshCw className={`h-4 w-4 ${healthLoading ? "animate-spin" : ""}`} />
                刷新检测
              </Button>
            </div>
          </div>
        </div>
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">一次性 AI</h3>
            <p className="text-xs text-muted-foreground">
              控制任务详情中的 AI 分析、评论生成、计划生成和子任务拆分默认通道。
            </p>
          </div>
          <span
            className={`rounded px-2 py-1 text-xs ${
              codexHealth?.one_shot_effective_channel !== "unavailable"
                ? "bg-green-100 text-green-700"
                : "bg-slate-100 text-slate-700"
            }`}
          >
            {healthLoading ? "检测中" : `${oneShotProviderLabel} / ${oneShotChannelLabel}`}
          </span>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">AI 提供商</label>
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

          {canUseOneShotSdkToggle ? (
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
                  {isOneShotOpenCodeProvider ? "启用 OpenCode SDK" : "优先使用 SDK"}
                </p>
                <p className="text-xs text-muted-foreground">
                  {isOneShotCodexProvider
                    ? (isRemoteMode
                      ? "SSH 模式下优先使用远程 Codex SDK，失败时自动回退到远程 codex exec。"
                      : "优先通过 Codex SDK 执行，失败时自动回退到 `codex exec`。")
                    : isOneShotClaudeProvider
                      ? "优先通过 Claude SDK 执行，失败时自动回退到 Claude CLI。"
                      : "OpenCode 当前仅支持本地 SDK；关闭后一次性 AI 将不可用。"}
                </p>
              </div>
            </label>
          ) : (
            <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
              SSH 模式下 Claude 一次性 AI 固定通过远端 Claude CLI 执行。
            </div>
          )}
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">一次性 AI 模型</label>
            {isOneShotOpenCodeProvider ? (
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
                      placeholder="openai/gpt-4o"
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
                    title="从 OpenCode SDK 获取模型列表"
                  >
                    {opencodeModelListLoading
                      ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      : <RefreshCw className="h-3.5 w-3.5" />
                    }
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
                  {(isOneShotClaudeProvider ? CLAUDE_MODEL_OPTIONS : CODEX_MODEL_OPTIONS).map((option) => (
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
                  ? `已加载 ${opencodeModelList.length} 个可用模型`
                  : "格式: provider/modelID（例如 openai/gpt-4o）"}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">一次性 AI 推理强度</label>
            <Select
              value={oneShotReasoningEffort}
              onValueChange={(value) => {
                if (value) {
                  onOneShotReasoningEffortChange(value);
                }
              }}
              disabled={healthLoading || actionLoading !== null}
            >
              <SelectTrigger className="bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(isOneShotClaudeProvider
                  ? CLAUDE_DEFAULT_THINKING_BUDGET_OPTIONS
                  : isOneShotOpenCodeProvider
                    ? OPENCODE_EFFORT_OPTIONS
                    : REASONING_EFFORT_OPTIONS).map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
          <p>当前一次性 AI 提供商：{oneShotProviderLabel}</p>
          <p>当前执行通道：{oneShotChannelLabel}</p>
          {selectedOneShotStatusMessage ? <p className="mt-1 leading-5">{selectedOneShotStatusMessage}</p> : null}
        </div>

        <div className="flex flex-wrap gap-2">
          <Button onClick={onSave} disabled={saveDisabled}>
            {actionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            保存配置
          </Button>
        </div>

        {actionMessage && <p className="text-xs text-green-700">{actionMessage}</p>}
        {actionError && <p className="text-xs text-destructive">{actionError}</p>}
      </div>

      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">
              {isRemoteMode ? "Claude（仅本地配置）" : "Claude SDK 配置"}
            </h3>
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? "SSH 模式下 Claude 任务会在远端通过 Claude CLI 执行，本地 SDK 配置不会应用到当前 SSH 目标。"
                : "Claude SDK 用于运行 Anthropic Claude 模型的任务与 AI 功能。"}
            </p>
          </div>
          <span
            className={`rounded px-2 py-1 text-xs ${
              !isRemoteMode && claudeHealth?.sdk_installed
                ? "bg-green-100 text-green-700"
                : "bg-slate-100 text-slate-700"
            }`}
          >
            {isRemoteMode ? "本地设置已禁用" : claudeHealth?.sdk_installed ? "已安装" : "未安装"}
          </span>
        </div>

        {isRemoteMode ? (
          <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            当前 SSH 配置不会读取本机 Claude SDK 设置。请在远端主机安装并配置 Claude CLI 后，再运行 Claude 员工任务。
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
                <p className="text-sm font-medium">启用 Claude SDK</p>
                <p className="text-xs text-muted-foreground">
                  启用后，使用 Claude 作为 AI 提供商的员工将通过 SDK 运行任务。
                </p>
              </div>
            </label>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">默认模型</label>
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
                    {CLAUDE_MODEL_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">默认推理强度</label>
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
                    {CLAUDE_DEFAULT_THINKING_BUDGET_OPTIONS.map((option) => (
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
                  Node 路径覆盖（可选）
                </label>
                <Input
                  id="claude-node-path-override"
                  value={claudeNodePathOverride}
                  onChange={(event) => onClaudeNodePathOverrideChange(event.target.value)}
                  placeholder="/opt/homebrew/bin/node"
                  disabled={claudeActionLoading !== null}
                />
                <p className="text-xs text-muted-foreground">留空时自动从系统 PATH 中查找 Node。</p>
              </div>

              <div className="space-y-2">
                <label htmlFor="claude-cli-path-override" className="text-sm font-medium">
                  Claude CLI 路径覆盖（可选）
                </label>
                <Input
                  id="claude-cli-path-override"
                  value={claudeCliPathOverride}
                  onChange={(event) => onClaudeCliPathOverrideChange(event.target.value)}
                  placeholder="/opt/homebrew/bin/claude"
                  disabled={claudeActionLoading !== null}
                />
                <p className="text-xs text-muted-foreground">SDK 不可用时会回退到该 CLI。</p>
              </div>
            </div>

            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p className="break-all">安装目录：{claudeHealth?.sdk_install_dir ?? "检测中"}</p>
              <p>
                Node：{claudeHealth?.node_available ? "可用" : "不可用"}
                {claudeHealth?.node_version ? `（${claudeHealth.node_version}）` : ""}
              </p>
              <p>
                SDK：{claudeHealth?.sdk_installed ? "已安装" : "未安装"}
                {claudeHealth?.sdk_version ? `（${claudeHealth.sdk_version}）` : ""}
              </p>
              <p>
                CLI：{claudeHealth?.cli_available ? "可用" : "不可用"}
                {claudeHealth?.cli_version ? `（${claudeHealth.cli_version}）` : ""}
              </p>
              <p>
                当前通道：
                {claudeHealth?.effective_provider === "sdk"
                  ? "Claude Agent SDK"
                  : claudeHealth?.effective_provider === "cli"
                    ? "Claude CLI"
                    : "不可用"}
              </p>
              {claudeHealth?.checked_at && <p>检测时间：{formatDate(claudeHealth.checked_at)}</p>}
              {claudeHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">{claudeHealth.sdk_status_message}</p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button onClick={onClaudeSave} disabled={claudeActionLoading !== null}>
                {claudeActionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                保存配置
              </Button>
              <Button
                variant="outline"
                onClick={onClaudeInstall}
                disabled={claudeActionLoading !== null}
              >
                {claudeActionLoading === "install" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {claudeHealth?.sdk_installed ? "重装 SDK" : "安装 SDK"}
              </Button>
              <Button variant="ghost" onClick={onClaudeRefresh} disabled={claudeActionLoading !== null}>
                <RefreshCw className={`h-4 w-4`} />
                刷新检测
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
              {isRemoteMode ? "OpenCode（仅本地配置）" : "OpenCode SDK 配置"}
            </h3>
            <p className="text-xs text-muted-foreground">
              {isRemoteMode
                ? "SSH 模式下 OpenCode SDK 仅支持本地执行，远程支持将在后续版本添加。"
                : "OpenCode SDK 用于运行开源 AI 编码代理的任务。"}
            </p>
          </div>
          <span
            className={`rounded px-2 py-1 text-xs ${
              !isRemoteMode && opencodeHealth?.sdk_installed
                ? "bg-green-100 text-green-700"
                : "bg-slate-100 text-slate-700"
            }`}
          >
            {isRemoteMode ? "本地设置已禁用" : opencodeHealth?.sdk_installed ? "已安装" : "未安装"}
          </span>
        </div>

        {isRemoteMode ? (
          <div className="rounded-md border border-dashed border-border px-3 py-3 text-sm text-muted-foreground">
            当前 SSH 配置不会读取本机 OpenCode SDK 设置。请在本地模式下使用 OpenCode 员工。
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
                <p className="text-sm font-medium">启用 OpenCode SDK</p>
                <p className="text-xs text-muted-foreground">
                  启用后，使用 OpenCode 作为 AI 提供商的员工将通过 SDK 运行任务。
                </p>
              </div>
            </label>

            <div className="space-y-2">
              <label htmlFor="opencode-default-model" className="text-sm font-medium">
                默认模型
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
                      placeholder="openai/gpt-4o"
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
                    title="从 SDK 获取可用模型列表"
                  >
                    {opencodeModelListLoading
                      ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      : <RefreshCw className="h-3.5 w-3.5" />
                    }
                  </Button>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                {opencodeModelList.length > 0
                  ? `已加载 ${opencodeModelList.length} 个可用模型`
                  : "格式: provider/modelID（例如 openai/gpt-4o），或点击右侧按钮从 SDK 获取"}
              </p>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label htmlFor="opencode-host" className="text-sm font-medium">
                  服务器主机
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
                  服务器端口
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
                Node 路径覆盖（可选）
              </label>
              <Input
                id="opencode-node-path-override"
                value={opencodeNodePathOverride}
                onChange={(event) => onOpenCodeNodePathOverrideChange(event.target.value)}
                placeholder="/opt/homebrew/bin/node"
                disabled={opencodeActionLoading !== null}
              />
              <p className="text-xs text-muted-foreground">留空时自动从系统 PATH 中查找 Node。</p>
            </div>

            <div className="grid gap-2 rounded-md border border-border px-3 py-3 text-xs text-muted-foreground">
              <p className="break-all">安装目录：{opencodeHealth?.sdk_install_dir ?? "检测中"}</p>
              <p>
                Node：{opencodeHealth?.node_available ? "可用" : "不可用"}
                {opencodeHealth?.node_version ? `（${opencodeHealth.node_version}）` : ""}
              </p>
              <p>
                SDK：{opencodeHealth?.sdk_installed ? "已安装" : "未安装"}
                {opencodeHealth?.sdk_version ? `（${opencodeHealth.sdk_version}）` : ""}
              </p>
              <p>当前通道：{opencodeHealth?.effective_provider === "sdk" ? "SDK" : "不可用"}</p>
              {opencodeHealth?.checked_at && <p>检测时间：{formatDate(opencodeHealth.checked_at)}</p>}
              {opencodeHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">{opencodeHealth.sdk_status_message}</p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button onClick={onOpenCodeSave} disabled={opencodeActionLoading !== null}>
                {opencodeActionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                保存配置
              </Button>
              <Button
                variant="outline"
                onClick={onOpenCodeInstall}
                disabled={opencodeActionLoading !== null}
              >
                {opencodeActionLoading === "install" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {opencodeHealth?.sdk_installed ? "重装 SDK" : "安装 SDK"}
              </Button>
              <Button variant="ghost" onClick={onOpenCodeRefresh} disabled={opencodeActionLoading !== null}>
                <RefreshCw className={`h-4 w-4`} />
                刷新检测
              </Button>
            </div>

            {opencodeActionMessage && <p className="text-xs text-green-700">{opencodeActionMessage}</p>}
            {opencodeActionError && <p className="text-xs text-destructive">{opencodeActionError}</p>}
          </div>
        )}
      </div>
    </div>
  );
}
