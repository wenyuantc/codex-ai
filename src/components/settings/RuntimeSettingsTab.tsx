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
  CODEX_MODEL_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  normalizeCodexModel,
  normalizeReasoningEffort,
  type CodexHealthCheck,
  type CodexModelId,
  type CodexSettings,
  type ReasoningEffort,
  type RemoteCodexHealthCheck,
} from "@/lib/types";
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
  oneShotModel: CodexModelId;
  oneShotReasoningEffort: ReasoningEffort;
  nodePathOverride: string;
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
  onTaskSdkEnabledChange: (value: boolean) => void;
  onOneShotSdkEnabledChange: (value: boolean) => void;
  onOneShotModelChange: (value: CodexModelId) => void;
  onOneShotReasoningEffortChange: (value: ReasoningEffort) => void;
  onNodePathOverrideChange: (value: string) => void;
  onSave: () => void;
  onInstall: () => void;
  onRefresh: () => void;
}

const themeOptions: { value: ThemeMode; label: string; icon: LucideIcon }[] = [
  { value: "light", label: "亮色", icon: Sun },
  { value: "dark", label: "暗色", icon: Moon },
  { value: "system", label: "跟随系统", icon: Monitor },
];

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
  oneShotModel,
  oneShotReasoningEffort,
  nodePathOverride,
  themeMode,
  onThemeModeChange,
  onTaskSdkEnabledChange,
  onOneShotSdkEnabledChange,
  onOneShotModelChange,
  onOneShotReasoningEffortChange,
  onNodePathOverrideChange,
  onSave,
  onInstall,
  onRefresh,
}: RuntimeSettingsTabProps) {
  const taskProviderLabel =
    codexHealth?.task_execution_effective_provider === "sdk" ? "SDK" : "exec（自动回退）";
  const oneShotProviderLabel =
    codexHealth?.one_shot_effective_provider === "sdk" ? "SDK" : "exec（自动回退）";
  const installButtonLabel = codexHealth?.sdk_installed ? "重装 SDK" : "安装 SDK";

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
              {healthLoading ? "检测中" : `任务 ${taskProviderLabel} / AI ${oneShotProviderLabel}`}
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

            <label className="flex items-start gap-3 rounded-md border border-border px-3 py-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 rounded border-input"
                checked={oneShotSdkEnabled}
                onChange={(event) => onOneShotSdkEnabledChange(event.target.checked)}
                disabled={healthLoading || actionLoading !== null}
              />
              <div className="space-y-1">
                <p className="text-sm font-medium">一次性 AI 使用 SDK</p>
                <p className="text-xs text-muted-foreground">
                  影响任务详情中的 AI 分析、评论生成、计划生成和子任务拆分。
                </p>
              </div>
            </label>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">一次性 AI 模型</label>
                <Select<CodexModelId>
                  value={oneShotModel}
                  onValueChange={(value) => {
                    if (value) {
                      onOneShotModelChange(normalizeCodexModel(value));
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
                <label className="text-sm font-medium">一次性 AI 推理强度</label>
                <Select<ReasoningEffort>
                  value={oneShotReasoningEffort}
                  onValueChange={(value) => {
                    if (value) {
                      onOneShotReasoningEffortChange(normalizeReasoningEffort(value));
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
              <p>一次性 AI 引擎：{oneShotProviderLabel}</p>
              {codexHealth?.checked_at && <p>检测时间：{formatDate(codexHealth.checked_at)}</p>}
              {codexHealth?.sdk_status_message && (
                <p className="text-[11px] leading-5">{codexHealth.sdk_status_message}</p>
              )}
            </div>

            <div className="flex flex-wrap gap-2">
              <Button
                onClick={onSave}
                disabled={healthLoading || actionLoading !== null || (isRemoteMode && !hasSelectedSshConfig)}
              >
                {actionLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                保存配置
              </Button>
              <Button
                variant="outline"
                onClick={onInstall}
                disabled={
                  healthLoading
                  || actionLoading !== null
                  || (isRemoteMode && (!hasSelectedSshConfig || passwordAuthBlocked))
                }
              >
                {actionLoading === "install" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {installButtonLabel}
              </Button>
              <Button variant="ghost" onClick={onRefresh} disabled={healthLoading || actionLoading !== null}>
                <RefreshCw className={`h-4 w-4 ${healthLoading ? "animate-spin" : ""}`} />
                刷新检测
              </Button>
            </div>

            {actionMessage && <p className="text-xs text-green-700">{actionMessage}</p>}
            {actionError && <p className="text-xs text-destructive">{actionError}</p>}
          </div>
        </div>
      </div>
    </div>
  );
}
