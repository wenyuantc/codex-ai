import { useEffect, useState } from "react";
import { Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import {
  AI_PROVIDER_OPTIONS,
  CODEX_MODEL_OPTIONS,
  CLAUDE_MODEL_OPTIONS,
  CLAUDE_THINKING_BUDGET_OPTIONS,
  GROK_MODEL_OPTIONS,
  GROK_EFFORT_OPTIONS,
  REASONING_EFFORT_OPTIONS,
  OPENCODE_EFFORT_OPTIONS,
  type AiChannel,
  type AiProvider,
  getDefaultModelForProvider,
  getDefaultReasoningEffortForProvider,
  normalizeReasoningEffortForProvider,
} from "@/lib/types";
import { listAiChannels } from "@/lib/backend";
import { getOpenCodeModels, type OpenCodeModelInfo } from "@/lib/opencode";
import { getReasoningEffortLabel } from "@/lib/utils";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { EmployeeSystemPromptField } from "./EmployeeSystemPromptField";
import { EMPLOYEE_ROLE_OPTIONS, EmployeeRoleHint } from "./employeeRoles";
import {
  NativeChannelFields,
  resolveNativeThinking,
  selectNativeChannel,
  selectNativeModel,
} from "./NativeChannelFields";
import { selectOpenCodeModel, selectOpenCodeReasoningEffort } from "./openCodeModelSelection";

const NO_PROJECT_VALUE = "__no_project__";

interface CreateEmployeeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultProjectId?: string;
}

export function CreateEmployeeDialog({
  open,
  onOpenChange,
  defaultProjectId,
}: CreateEmployeeDialogProps) {
  const { t } = useTranslation(["employees", "common"]);
  const { createEmployee } = useEmployeeStore();
  const { projects, fetchProjects } = useProjectStore();
  const [name, setName] = useState("");
  const [role, setRole] = useState("developer");
  const [aiProvider, setAiProvider] = useState<AiProvider>("codex");
  const [model, setModel] = useState<string>("gpt-5.4");
  const [reasoningEffort, setReasoningEffort] = useState<string>("high");
  const [specialization, setSpecialization] = useState("");
  const [systemPrompt, setSystemPrompt] = useState("");
  const [projectId, setProjectId] = useState("");
  const [saving, setSaving] = useState(false);
  const [opencodeModels, setOpenCodeModels] = useState<OpenCodeModelInfo[]>([]);
  const [opencodeModelsLoading, setOpenCodeModelsLoading] = useState(false);
  const [opencodeModelError, setOpenCodeModelError] = useState<string | null>(null);
  const [nativeChannels, setNativeChannels] = useState<AiChannel[]>([]);
  const [nativeChannelId, setNativeChannelId] = useState("");
  const [nativeChannelsLoading, setNativeChannelsLoading] = useState(false);
  const [nativeChannelError, setNativeChannelError] = useState<string | null>(null);

  const modelOptions =
    aiProvider === "claude"
      ? CLAUDE_MODEL_OPTIONS
      : aiProvider === "opencode"
        ? opencodeModels
        : aiProvider === "grok"
          ? GROK_MODEL_OPTIONS
          : CODEX_MODEL_OPTIONS;
  const nativeThinking = resolveNativeThinking(
    nativeChannels.find((channel) => channel.id === nativeChannelId),
    model,
  );
  const effortOptions =
    aiProvider === "claude"
      ? CLAUDE_THINKING_BUDGET_OPTIONS
      : aiProvider === "opencode"
        ? OPENCODE_EFFORT_OPTIONS
        : aiProvider === "native"
          ? nativeThinking.levels.map((value) => ({ value, label: value }))
          : aiProvider === "grok"
            ? GROK_EFFORT_OPTIONS
            : REASONING_EFFORT_OPTIONS;
  const nativeSaveBlocked = aiProvider === "native" && !nativeChannelId;

  const selectedModelCapabilities =
    aiProvider === "opencode"
      ? (opencodeModels.find((m) => m.value === model)?.capabilities ?? null)
      : null;

  const modelSupportsReasoning =
    aiProvider === "native"
      ? nativeThinking.enabled
      : selectedModelCapabilities === null || selectedModelCapabilities.reasoning;

  const resetForm = () => {
    setName("");
    setRole("developer");
    setAiProvider("codex");
    setModel("gpt-5.4");
    setReasoningEffort("high");
    setSpecialization("");
    setSystemPrompt("");
    setProjectId(defaultProjectId ?? "");
    setOpenCodeModels([]);
    setNativeChannels([]);
    setNativeChannelId("");
    setNativeChannelError(null);
  };

  useEffect(() => {
    if (open) {
      void fetchProjects();
      resetForm();
    }
  }, [defaultProjectId, fetchProjects, open]);

  const fetchOpenCodeModels = async () => {
    setOpenCodeModelsLoading(true);
    setOpenCodeModelError(null);
    try {
      const models = await getOpenCodeModels();
      const selectedModel = selectOpenCodeModel(models, model);
      setOpenCodeModels(models);
      setModel(selectedModel);
      setReasoningEffort((current) =>
        selectOpenCodeReasoningEffort(models, selectedModel, current),
      );
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setOpenCodeModelError(msg);
      console.error("获取 OpenCode 模型列表失败:", msg);
    } finally {
      setOpenCodeModelsLoading(false);
    }
  };

  const fetchNativeChannels = async (preferredChannelId?: string) => {
    setNativeChannelsLoading(true);
    setNativeChannelError(null);
    try {
      const channels = (await listAiChannels()).filter((channel) => channel.enabled);
      const nextChannelId = selectNativeChannel(channels, preferredChannelId ?? nativeChannelId);
      const nextChannel = channels.find((channel) => channel.id === nextChannelId);
      setNativeChannels(channels);
      setNativeChannelId(nextChannelId);
      const nextModel = selectNativeModel(nextChannel, model);
      setModel(nextModel);
      const thinking = resolveNativeThinking(nextChannel, nextModel);
      setReasoningEffort((current) =>
        thinking.enabled && thinking.levels.includes(current) ? current : thinking.defaultLevel,
      );
      if (channels.length === 0) {
        setNativeChannelError(t("noEnabledChannelHint"));
      }
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setNativeChannelError(msg);
      console.error("获取 AI 渠道列表失败:", msg);
    } finally {
      setNativeChannelsLoading(false);
    }
  };

  useEffect(() => {
    if (!open || aiProvider !== "opencode") return;
    void fetchOpenCodeModels();
  }, [aiProvider, open]);

  useEffect(() => {
    if (!open || aiProvider !== "native") return;
    void fetchNativeChannels();
  }, [aiProvider, open]);

  const handleProviderChange = (value: AiProvider | null) => {
    if (!value) return;
    setAiProvider(value);
    setModel(getDefaultModelForProvider(value) as string);
    setReasoningEffort(getDefaultReasoningEffortForProvider(value));
    setOpenCodeModelError(null);
    setNativeChannelError(null);
    if (value !== "native") {
      setNativeChannelId("");
    }
  };

  const handleNativeChannelChange = (channelId: string) => {
    const channel = nativeChannels.find((item) => item.id === channelId);
    const nextModel = selectNativeModel(channel, "");
    setNativeChannelId(channelId);
    setModel(nextModel);
    const thinking = resolveNativeThinking(channel, nextModel);
    setReasoningEffort(thinking.enabled ? thinking.defaultLevel : reasoningEffort);
  };

  const handleModelChange = (value: string) => {
    const selectedModel = value.trim();
    setModel(selectedModel);
    if (aiProvider === "opencode") {
      setReasoningEffort((current) =>
        selectOpenCodeReasoningEffort(opencodeModels, selectedModel, current),
      );
    }
    if (aiProvider === "native") {
      const channel = nativeChannels.find((item) => item.id === nativeChannelId);
      const thinking = resolveNativeThinking(channel, selectedModel);
      setReasoningEffort((current) =>
        thinking.enabled && thinking.levels.includes(current) ? current : thinking.defaultLevel,
      );
    }
  };

  const handleCreate = async () => {
    if (!name.trim() || nativeSaveBlocked) return;
    setSaving(true);
    try {
      await createEmployee({
        name: name.trim(),
        role,
        model,
        reasoning_effort: normalizeReasoningEffortForProvider(aiProvider, reasoningEffort),
        specialization: specialization.trim() || undefined,
        system_prompt: systemPrompt.trim() || undefined,
        project_id: projectId || undefined,
        ai_provider: aiProvider,
        ai_channel_id: aiProvider === "native" ? nativeChannelId : undefined,
      });
      resetForm();
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{t("create")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-muted-foreground">{t("nameRequired")}</label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("namePlaceholder")}
              className="mt-1"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">{t("role")}</label>
              <Select
                value={role}
                onValueChange={(value) => {
                  if (value) {
                    setRole(value);
                  }
                }}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) => {
                      const option = EMPLOYEE_ROLE_OPTIONS.find((o) => o.value === value);
                      return typeof value === "string"
                        ? option
                          ? t(`common:role.${option.value}`)
                          : value
                        : t("selectRole");
                    }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {EMPLOYEE_ROLE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {t(`common:role.${option.value}`)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <div className="mt-1.5">
                <EmployeeRoleHint role={role} />
              </div>
            </div>

            <div>
              <label className="text-xs font-medium text-muted-foreground">{t("aiProvider")}</label>
              <Select value={aiProvider} onValueChange={handleProviderChange}>
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? (AI_PROVIDER_OPTIONS.find((option) => option.value === value)?.label ??
                          value)
                        : t("selectProvider")
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {AI_PROVIDER_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {aiProvider === "native" ? (
            <NativeChannelFields
              channels={nativeChannels}
              channelId={nativeChannelId}
              model={model}
              loading={nativeChannelsLoading}
              error={nativeChannelError}
              onChannelChange={handleNativeChannelChange}
              onModelChange={handleModelChange}
              onRefresh={() => void fetchNativeChannels(nativeChannelId)}
            />
          ) : null}

          <div className="grid grid-cols-2 gap-3">
            {aiProvider !== "native" ? (
              <div>
                <label className="text-xs font-medium text-muted-foreground">{t("model")}</label>
                {modelOptions.length > 0 && aiProvider !== "opencode" ? (
                  <Select
                    value={model}
                    onValueChange={(value) => {
                      if (value) handleModelChange(value);
                    }}
                  >
                    <SelectTrigger className="mt-1 bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string"
                            ? (modelOptions.find((option) => option.value === value)?.label ??
                              value)
                            : t("selectModel")
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {modelOptions.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : aiProvider === "opencode" ? (
                  <div className="flex flex-col gap-1 mt-1">
                    <div className="flex gap-2">
                      <div className="flex-1">
                        {opencodeModels.length > 0 ? (
                          <Select
                            value={model}
                            onValueChange={(value) => {
                              if (value) handleModelChange(value);
                            }}
                          >
                            <SelectTrigger className="bg-background">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent className="max-h-72">
                              {opencodeModels.map((m) => (
                                <SelectItem key={m.value} value={m.value}>
                                  {`${m.label} · ${m.providerName}`}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                        ) : (
                          <Input
                            value={model}
                            onChange={(e) => handleModelChange(e.target.value)}
                            placeholder={t("selectModel")}
                          />
                        )}
                      </div>
                      <button
                        type="button"
                        onClick={fetchOpenCodeModels}
                        disabled={opencodeModelsLoading}
                        className="px-2 py-1 border border-input rounded-md hover:bg-accent disabled:opacity-50"
                        title={t("refreshModelsTitle")}
                      >
                        {opencodeModelsLoading ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <RefreshCw className="h-3.5 w-3.5" />
                        )}
                      </button>
                    </div>
                    {opencodeModels.length > 0 && (
                      <p className="text-[11px] text-muted-foreground">
                        {t("opencodeLoaded", { count: opencodeModels.length })}
                      </p>
                    )}
                    {opencodeModelError && (
                      <p className="text-[11px] text-destructive">{opencodeModelError}</p>
                    )}
                  </div>
                ) : (
                  <Input
                    value={model}
                    onChange={(e) => handleModelChange(e.target.value)}
                    placeholder={t("selectModel")}
                    className="mt-1"
                  />
                )}
              </div>
            ) : null}

            <div className={aiProvider === "native" ? "col-span-2" : undefined}>
              <label className="text-xs font-medium text-muted-foreground">
                {t("reasoningEffort")}
              </label>
              <Select
                value={reasoningEffort}
                onValueChange={(value) => {
                  if (value && modelSupportsReasoning) {
                    setReasoningEffort(value);
                  }
                }}
                disabled={!modelSupportsReasoning}
              >
                <SelectTrigger className="mt-1 bg-background">
                  <SelectValue>
                    {(value) =>
                      typeof value === "string"
                        ? getReasoningEffortLabel(value, aiProvider)
                        : t("selectReasoning")
                    }
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {effortOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {getReasoningEffortLabel(option.value, aiProvider)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {aiProvider === "opencode" && selectedModelCapabilities?.reasoning && (
                <p className="text-[10px] text-muted-foreground mt-0.5">{t("reasoningHint")}</p>
              )}
              {aiProvider === "native" && !nativeThinking.enabled ? (
                <p className="text-[10px] text-muted-foreground mt-0.5">
                  {t("nativeThinkingDisabled")}
                </p>
              ) : null}
              {aiProvider === "native" && nativeThinking.enabled ? (
                <p className="text-[10px] text-muted-foreground mt-0.5">
                  {t("nativeThinkingFromChannel")}
                </p>
              ) : null}
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("specialization")}
            </label>
            <Input
              value={specialization}
              onChange={(e) => setSpecialization(e.target.value)}
              placeholder={t("specializationPlaceholder")}
              className="mt-1"
            />
          </div>

          <EmployeeSystemPromptField
            open={open}
            role={role}
            specialization={specialization}
            systemPrompt={systemPrompt}
            projectId={projectId || undefined}
            disabled={saving}
            onSystemPromptChange={setSystemPrompt}
          />

          <div>
            <label className="text-xs font-medium text-muted-foreground">
              {t("linkedProject")}
            </label>
            <Select
              value={projectId || NO_PROJECT_VALUE}
              onValueChange={(value) => {
                setProjectId(!value || value === NO_PROJECT_VALUE ? "" : value);
              }}
            >
              <SelectTrigger className="mt-1 bg-background">
                <SelectValue>
                  {(value) => {
                    if (!value || value === NO_PROJECT_VALUE) {
                      return t("none");
                    }

                    return projects.find((project) => project.id === value)?.name ?? t("none");
                  }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_PROJECT_VALUE}>{t("none")}</SelectItem>
                {projects.map((project) => (
                  <SelectItem key={project.id} value={project.id}>
                    {project.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={() => onOpenChange(false)}
              className="px-3 py-1.5 text-sm border border-input rounded-md hover:bg-accent"
            >
              {t("common:cancel")}
            </button>
            <button
              onClick={handleCreate}
              disabled={!name.trim() || nativeSaveBlocked || saving}
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50"
            >
              {saving ? t("creating") : t("common:create")}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
