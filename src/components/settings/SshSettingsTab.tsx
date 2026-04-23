import { FolderOpen, Loader2, Plus, ServerCog, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { type SshAuthType, type SshConfig } from "@/lib/types";
import { formatDate } from "@/lib/utils";

import type { SshConfigFormState } from "./shared";

const KNOWN_HOSTS_OPTIONS = [
  { value: "accept-new", label: "首次连接自动接受" },
  { value: "strict", label: "严格校验" },
  { value: "off", label: "关闭校验（不推荐）" },
];

interface SshSettingsTabProps {
  isTauriRuntime: boolean;
  sshConfigs: SshConfig[];
  sshConfigsLoading: boolean;
  selectedSshConfigId: string | null;
  selectedSshConfig: SshConfig | null;
  editingSshConfigId: string | null;
  sshForm: SshConfigFormState;
  sshFormLoading: "save" | "delete" | "probe" | null;
  sshFormMessage: string | null;
  sshFormError: string | null;
  onResetForm: () => void;
  onSelectConfig: (config: SshConfig) => void;
  onFormChange: (updates: Partial<SshConfigFormState>) => void;
  onSelectPrivateKeyFile: () => void;
  onSave: () => void;
  onTestConnection: () => void;
  onDelete: () => void;
}

export function SshSettingsTab({
  isTauriRuntime,
  sshConfigs,
  sshConfigsLoading,
  selectedSshConfigId,
  selectedSshConfig,
  editingSshConfigId,
  sshForm,
  sshFormLoading,
  sshFormMessage,
  sshFormError,
  onResetForm,
  onSelectConfig,
  onFormChange,
  onSelectPrivateKeyFile,
  onSave,
  onTestConnection,
  onDelete,
}: SshSettingsTabProps) {
  const selectedSshConfigSummary = selectedSshConfig
    ? `${selectedSshConfig.username}@${selectedSshConfig.host}:${selectedSshConfig.port}`
    : "未选择 SSH 配置";

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-sm font-medium">SSH 配置管理</h3>
            <p className="text-xs text-muted-foreground">
              支持多个 SSH 配置；SSH 项目会固定绑定其中一项配置和一个远程仓库目录。
            </p>
          </div>
          <Button variant="outline" onClick={onResetForm}>
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
                <div className="px-3 py-6 text-sm text-muted-foreground">当前还没有 SSH 配置。</div>
              ) : (
                sshConfigs.map((config) => (
                  <button
                    key={config.id}
                    type="button"
                    onClick={() => onSelectConfig(config)}
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
                  onChange={(event) => onFormChange({ name: event.target.value })}
                  placeholder="生产主机"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">主机 *</label>
                <Input
                  value={sshForm.host}
                  onChange={(event) => onFormChange({ host: event.target.value })}
                  placeholder="10.0.0.12"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">端口</label>
                <Input
                  value={sshForm.port}
                  onChange={(event) => onFormChange({ port: event.target.value })}
                  placeholder="22"
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">用户名 *</label>
                <Input
                  value={sshForm.username}
                  onChange={(event) => onFormChange({ username: event.target.value })}
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
                      onFormChange({ authType: value });
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
                  onValueChange={(value) => onFormChange({ knownHostsMode: value ?? "accept-new" })}
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
                  <div className="mt-1 flex gap-2">
                    <Input
                      value={sshForm.privateKeyPath}
                      onChange={(event) => onFormChange({ privateKeyPath: event.target.value })}
                      placeholder="~/.ssh/id_ed25519"
                      className="flex-1"
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={onSelectPrivateKeyFile}
                      disabled={!isTauriRuntime}
                      title={isTauriRuntime ? "选择私钥文件" : "仅桌面端支持选择私钥文件"}
                    >
                      <FolderOpen className="h-4 w-4" />
                      选择
                    </Button>
                  </div>
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">Passphrase（可选）</label>
                  <Input
                    type="password"
                    value={sshForm.passphrase}
                    onChange={(event) => onFormChange({ passphrase: event.target.value })}
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
                    onChange={(event) => onFormChange({ password: event.target.value })}
                    placeholder={selectedSshConfig?.password_configured ? "留空表示保持现有密码" : "输入登录密码"}
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">Passphrase（可选）</label>
                  <Input
                    type="password"
                    value={sshForm.passphrase}
                    onChange={(event) => onFormChange({ passphrase: event.target.value })}
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
              <Button onClick={onSave} disabled={sshFormLoading !== null}>
                {sshFormLoading === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {editingSshConfigId ? "保存 SSH 配置" : "创建 SSH 配置"}
              </Button>
              <Button
                variant="outline"
                onClick={onTestConnection}
                disabled={sshFormLoading !== null || !selectedSshConfigId}
              >
                {sshFormLoading === "probe"
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : <ServerCog className="h-4 w-4" />}
                测试连接
              </Button>
              <Button
                variant="destructive"
                onClick={onDelete}
                disabled={sshFormLoading !== null || !editingSshConfigId}
              >
                {sshFormLoading === "delete"
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : <Trash2 className="h-4 w-4" />}
                删除配置
              </Button>
            </div>

            {sshFormMessage && <p className="text-xs text-green-700">{sshFormMessage}</p>}
            {sshFormError && <p className="text-xs text-destructive">{sshFormError}</p>}
          </div>
        </div>
      </div>
    </div>
  );
}
