import { FolderOpen, Loader2, Plus, ServerCog, Trash2 } from "lucide-react";
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
import { type SshAuthType, type SshConfig } from "@/lib/types";
import { formatDate } from "@/lib/utils";

import type { SshConfigFormState } from "./shared";

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
  const { t } = useTranslation("settings");
  const knownHostsOptions = [
    { value: "accept-new", label: t("ssh.knownHosts.acceptNew") },
    { value: "strict", label: t("ssh.knownHosts.strict") },
    { value: "off", label: t("ssh.knownHosts.off") },
  ];
  const selectedSshConfigSummary = selectedSshConfig
    ? `${selectedSshConfig.username}@${selectedSshConfig.host}:${selectedSshConfig.port}`
    : t("ssh.list.noSelection");

  return (
    <div className="space-y-6">
      <div className="space-y-4 rounded-lg border border-border bg-card p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-sm font-medium">{t("ssh.title")}</h3>
            <p className="text-xs text-muted-foreground">{t("ssh.description")}</p>
          </div>
          <Button variant="outline" onClick={onResetForm}>
            <Plus className="mr-1 h-4 w-4" />
            {t("ssh.actions.newConfig")}
          </Button>
        </div>

        <div className="grid gap-4 lg:grid-cols-[18rem,1fr]">
          <div className="space-y-2">
            <div className="rounded-md border border-border">
              {sshConfigsLoading ? (
                <div className="flex h-28 items-center justify-center text-sm text-muted-foreground">
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t("ssh.list.loading")}
                </div>
              ) : sshConfigs.length === 0 ? (
                <div className="px-3 py-6 text-sm text-muted-foreground">{t("ssh.list.empty")}</div>
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
                        {t(
                          config.auth_type === "password"
                            ? "ssh.badges.passwordLogin"
                            : "ssh.badges.keyLogin",
                        )}
                      </span>
                      {config.last_checked_at && (
                        <span className="rounded border border-border px-1.5 py-0.5 text-muted-foreground">
                          {t("ssh.list.checkedAt", { date: formatDate(config.last_checked_at) })}
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
                  {t(editingSshConfigId ? "ssh.form.editTitle" : "ssh.form.createTitle")}
                </h4>
                <p className="text-xs text-muted-foreground">
                  {t(
                    editingSshConfigId ? "ssh.form.editDescription" : "ssh.form.createDescription",
                  )}
                </p>
              </div>
              {selectedSshConfig && (
                <span className="rounded bg-secondary px-2 py-1 text-xs text-secondary-foreground">
                  {t(
                    selectedSshConfig.auth_type === "password"
                      ? "ssh.badges.passwordAuth"
                      : "ssh.badges.keyAuth",
                  )}
                </span>
              )}
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("ssh.form.configName")}
                </label>
                <Input
                  value={sshForm.name}
                  onChange={(event) => onFormChange({ name: event.target.value })}
                  placeholder={t("ssh.form.placeholders.configName")}
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("ssh.form.host")}
                </label>
                <Input
                  value={sshForm.host}
                  onChange={(event) => onFormChange({ host: event.target.value })}
                  placeholder={t("ssh.form.placeholders.host")}
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("ssh.form.port")}
                </label>
                <Input
                  value={sshForm.port}
                  onChange={(event) => onFormChange({ port: event.target.value })}
                  placeholder={t("ssh.form.placeholders.port")}
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("ssh.form.username")}
                </label>
                <Input
                  value={sshForm.username}
                  onChange={(event) => onFormChange({ username: event.target.value })}
                  placeholder={t("ssh.form.placeholders.username")}
                  className="mt-1"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("ssh.form.authType")}
                </label>
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
                    <SelectItem value="key">{t("ssh.badges.keyLogin")}</SelectItem>
                    <SelectItem value="password">{t("ssh.badges.passwordLogin")}</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">
                  {t("ssh.form.knownHostsPolicy")}
                </label>
                <Select
                  value={sshForm.knownHostsMode}
                  onValueChange={(value) => onFormChange({ knownHostsMode: value ?? "accept-new" })}
                >
                  <SelectTrigger className="mt-1 bg-background">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {knownHostsOptions.map((option) => (
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
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("ssh.form.privateKeyPath")}
                  </label>
                  <div className="mt-1 flex gap-2">
                    <Input
                      value={sshForm.privateKeyPath}
                      onChange={(event) => onFormChange({ privateKeyPath: event.target.value })}
                      placeholder={t("ssh.form.placeholders.privateKeyPath")}
                      className="flex-1"
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={onSelectPrivateKeyFile}
                      disabled={!isTauriRuntime}
                      title={t(
                        isTauriRuntime
                          ? "ssh.desktop.selectPrivateKeyTitle"
                          : "ssh.desktop.selectPrivateKeyDesktopOnly",
                      )}
                    >
                      <FolderOpen className="h-4 w-4" />
                      {t("ssh.actions.choose")}
                    </Button>
                  </div>
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("ssh.form.passphrase")}
                  </label>
                  <Input
                    type="password"
                    value={sshForm.passphrase}
                    onChange={(event) => onFormChange({ passphrase: event.target.value })}
                    placeholder={
                      selectedSshConfig?.passphrase_configured
                        ? t("ssh.form.placeholders.passphraseKeepExisting")
                        : t("ssh.form.placeholders.passphraseOptional")
                    }
                    className="mt-1"
                  />
                </div>
              </div>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("ssh.form.password")}
                  </label>
                  <Input
                    type="password"
                    value={sshForm.password}
                    onChange={(event) => onFormChange({ password: event.target.value })}
                    placeholder={
                      selectedSshConfig?.password_configured
                        ? t("ssh.form.placeholders.passwordKeepExisting")
                        : t("ssh.form.placeholders.passwordEnter")
                    }
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground">
                    {t("ssh.form.passphrase")}
                  </label>
                  <Input
                    type="password"
                    value={sshForm.passphrase}
                    onChange={(event) => onFormChange({ passphrase: event.target.value })}
                    placeholder={
                      selectedSshConfig?.passphrase_configured
                        ? t("ssh.form.placeholders.passphraseKeepExisting")
                        : t("ssh.form.placeholders.passphraseOptional")
                    }
                    className="mt-1"
                  />
                </div>
              </div>
            )}

            {selectedSshConfig && (
              <div className="rounded-md border border-border bg-muted/30 px-3 py-3 text-xs text-muted-foreground">
                <div className="font-medium text-foreground">{t("ssh.status.title")}</div>
                <div className="mt-1">
                  {t("ssh.status.host")}:{selectedSshConfigSummary}
                </div>
                <div className="mt-1">
                  {t("ssh.status.connectionTest")}:
                  {(
                    selectedSshConfig.auth_type === "password"
                      ? selectedSshConfig.password_probe_status
                      : selectedSshConfig.last_check_status
                  )
                    ? ` ${
                        selectedSshConfig.auth_type === "password"
                          ? selectedSshConfig.password_probe_status
                          : selectedSshConfig.last_check_status
                      }`
                    : ` ${t("ssh.status.notTested")}`}
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
                {t(editingSshConfigId ? "ssh.actions.saveConfig" : "ssh.actions.createConfig")}
              </Button>
              <Button
                variant="outline"
                onClick={onTestConnection}
                disabled={sshFormLoading !== null || !selectedSshConfigId}
              >
                {sshFormLoading === "probe" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <ServerCog className="h-4 w-4" />
                )}
                {t("ssh.actions.testConnection")}
              </Button>
              <Button
                variant="destructive"
                onClick={onDelete}
                disabled={sshFormLoading !== null || !editingSshConfigId}
              >
                {sshFormLoading === "delete" ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Trash2 className="h-4 w-4" />
                )}
                {t("ssh.actions.deleteConfig")}
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
