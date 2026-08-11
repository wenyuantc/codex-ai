import { useEffect, useState } from "react";
import { Loader2, RotateCcw, Save } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  getAiPromptTemplates,
  resetAiPromptTemplates,
  updateAiPromptTemplates,
  type AiPromptTemplate,
} from "@/lib/backend";
import { cn } from "@/lib/utils";

export function PromptSettingsTab() {
  const { t } = useTranslation("settings");
  const [templates, setTemplates] = useState<AiPromptTemplate[]>([]);
  const [selectedScene, setSelectedScene] = useState<string | null>(null);
  const [draft, setDraft] = useState<AiPromptTemplate | null>(null);
  const [loading, setLoading] = useState(true);
  const [actionLoading, setActionLoading] = useState<"save" | "reset" | "reset-all" | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function loadTemplates(preferredScene?: string | null) {
    setLoading(true);
    setError(null);
    try {
      const document = await getAiPromptTemplates();
      const localized = document.templates.map((template) => ({
        ...template,
        label: t(`prompts.scenes.${template.scene}`, { defaultValue: template.label }),
      }));
      setTemplates(localized);
      const nextScene =
        preferredScene && localized.some((template) => template.scene === preferredScene)
          ? preferredScene
          : (localized[0]?.scene ?? null);
      setSelectedScene(nextScene);
      setDraft(localized.find((template) => template.scene === nextScene) ?? null);
    } catch (loadError) {
      console.error("Failed to load AI prompt templates:", loadError);
      setError(loadError instanceof Error ? loadError.message : t("prompts.errors.loadFailed"));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadTemplates(selectedScene);
    // Re-localize scene labels when UI language changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only reload on locale `t` identity change
  }, [t]);

  function selectTemplate(scene: string) {
    const template = templates.find((item) => item.scene === scene) ?? null;
    setSelectedScene(scene);
    setDraft(template ? { ...template } : null);
    setMessage(null);
    setError(null);
  }

  function updateDraft(updates: Partial<AiPromptTemplate>) {
    setDraft((current) => (current ? { ...current, ...updates } : current));
  }

  async function handleSave() {
    if (!draft) {
      return;
    }

    setActionLoading("save");
    setMessage(null);
    setError(null);
    try {
      const nextTemplates = templates.map((template) =>
        template.scene === draft.scene ? { ...draft } : template,
      );
      const document = await updateAiPromptTemplates(nextTemplates);
      setTemplates(document.templates);
      setDraft(document.templates.find((template) => template.scene === draft.scene) ?? draft);
      setMessage(t("prompts.messages.savedNamed", { label: draft.label }));
    } catch (saveError) {
      console.error("Failed to save AI prompt template:", saveError);
      setError(saveError instanceof Error ? saveError.message : t("prompts.errors.saveFailed"));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleResetCurrent() {
    if (!selectedScene) {
      return;
    }

    setActionLoading("reset");
    setMessage(null);
    setError(null);
    try {
      const document = await resetAiPromptTemplates(selectedScene);
      const localized = document.templates.map((template) => ({
        ...template,
        label: t(`prompts.scenes.${template.scene}`, { defaultValue: template.label }),
      }));
      setTemplates(localized);
      const restored = localized.find((template) => template.scene === selectedScene) ?? null;
      setDraft(restored ? { ...restored } : null);
      setMessage(
        restored
          ? t("prompts.messages.resetNamedToDefault", { label: restored.label })
          : t("prompts.messages.resetCurrent"),
      );
    } catch (resetError) {
      console.error("Failed to reset AI prompt template:", resetError);
      setError(resetError instanceof Error ? resetError.message : t("prompts.errors.resetFailed"));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleResetAll() {
    setActionLoading("reset-all");
    setMessage(null);
    setError(null);
    try {
      const document = await resetAiPromptTemplates(null);
      const localized = document.templates.map((template) => ({
        ...template,
        label: t(`prompts.scenes.${template.scene}`, { defaultValue: template.label }),
      }));
      setTemplates(localized);
      const nextScene = selectedScene ?? localized[0]?.scene ?? null;
      setSelectedScene(nextScene);
      setDraft(localized.find((template) => template.scene === nextScene) ?? null);
      setMessage(t("prompts.messages.resetAll"));
    } catch (resetError) {
      console.error("Failed to reset all AI prompt templates:", resetError);
      setError(
        resetError instanceof Error ? resetError.message : t("prompts.errors.resetAllFailed"),
      );
    } finally {
      setActionLoading(null);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-8 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        {t("prompts.states.loading")}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-border bg-card p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium">{t("prompts.title")}</h3>
            <p className="mt-1 text-xs text-muted-foreground">{t("prompts.description")}</p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void handleResetAll()}
            disabled={actionLoading !== null || templates.length === 0}
          >
            {actionLoading === "reset-all" ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <RotateCcw className="h-4 w-4" />
            )}
            {t("prompts.actions.resetAll")}
          </Button>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-[220px_minmax(0,1fr)]">
        <div className="rounded-lg border border-border bg-card p-2">
          <div className="space-y-1">
            {templates.map((template) => (
              <button
                key={template.scene}
                type="button"
                onClick={() => selectTemplate(template.scene)}
                className={cn(
                  "w-full rounded-md px-3 py-2 text-left text-sm transition-colors",
                  selectedScene === template.scene
                    ? "bg-primary text-primary-foreground"
                    : "text-foreground hover:bg-accent",
                )}
              >
                <div className="font-medium">
                  {t(`prompts.scenes.${template.scene}`, { defaultValue: template.label })}
                </div>
                <div
                  className={cn(
                    "mt-0.5 text-[11px]",
                    selectedScene === template.scene
                      ? "text-primary-foreground/80"
                      : "text-muted-foreground",
                  )}
                >
                  {template.scene}
                </div>
              </button>
            ))}
          </div>
        </div>

        <div className="space-y-4 rounded-lg border border-border bg-card p-4">
          {draft ? (
            <>
              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">
                  {t("prompts.fields.displayName")}
                </label>
                <input
                  value={draft.label}
                  onChange={(event) => updateDraft({ label: event.target.value })}
                  className="h-9 w-full rounded-md border border-border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">
                  {t("prompts.fields.outputGoal")}
                </label>
                <textarea
                  value={draft.output_goal}
                  onChange={(event) => updateDraft({ output_goal: event.target.value })}
                  rows={4}
                  className="w-full resize-y rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">
                  {t("prompts.fields.sceneRequirement")}
                </label>
                <textarea
                  value={draft.scene_requirement}
                  onChange={(event) => updateDraft({ scene_requirement: event.target.value })}
                  rows={12}
                  className="w-full resize-y rounded-md border border-border bg-background px-3 py-2 font-mono text-xs leading-5 outline-none focus:ring-2 focus:ring-ring"
                />
              </div>

              <div className="flex flex-wrap gap-2">
                <Button onClick={() => void handleSave()} disabled={actionLoading !== null}>
                  {actionLoading === "save" ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Save className="h-4 w-4" />
                  )}
                  {t("prompts.actions.saveCurrent")}
                </Button>
                <Button
                  variant="outline"
                  onClick={() => void handleResetCurrent()}
                  disabled={actionLoading !== null}
                >
                  {actionLoading === "reset" ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <RotateCcw className="h-4 w-4" />
                  )}
                  {t("prompts.actions.resetCurrent")}
                </Button>
              </div>
            </>
          ) : (
            <p className="text-sm text-muted-foreground">{t("prompts.states.empty")}</p>
          )}

          {message && <p className="text-xs text-green-700">{message}</p>}
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
      </div>
    </div>
  );
}
