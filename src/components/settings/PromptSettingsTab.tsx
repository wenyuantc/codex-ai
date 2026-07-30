import { useEffect, useState } from "react";
import { Loader2, RotateCcw, Save } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  getAiPromptTemplates,
  resetAiPromptTemplates,
  updateAiPromptTemplates,
  type AiPromptTemplate,
} from "@/lib/backend";
import { cn } from "@/lib/utils";

export function PromptSettingsTab() {
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
      setTemplates(document.templates);
      const nextScene =
        preferredScene && document.templates.some((template) => template.scene === preferredScene)
          ? preferredScene
          : (document.templates[0]?.scene ?? null);
      setSelectedScene(nextScene);
      setDraft(document.templates.find((template) => template.scene === nextScene) ?? null);
    } catch (loadError) {
      console.error("Failed to load AI prompt templates:", loadError);
      setError(loadError instanceof Error ? loadError.message : "加载提示词模板失败");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadTemplates();
  }, []);

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
      setMessage(`已保存「${draft.label}」`);
    } catch (saveError) {
      console.error("Failed to save AI prompt template:", saveError);
      setError(saveError instanceof Error ? saveError.message : "保存提示词模板失败");
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
      setTemplates(document.templates);
      const restored =
        document.templates.find((template) => template.scene === selectedScene) ?? null;
      setDraft(restored ? { ...restored } : null);
      setMessage(restored ? `已重置「${restored.label}」为默认` : "已重置当前模板");
    } catch (resetError) {
      console.error("Failed to reset AI prompt template:", resetError);
      setError(resetError instanceof Error ? resetError.message : "重置提示词模板失败");
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
      setTemplates(document.templates);
      const nextScene = selectedScene ?? document.templates[0]?.scene ?? null;
      setSelectedScene(nextScene);
      setDraft(document.templates.find((template) => template.scene === nextScene) ?? null);
      setMessage("已重置全部提示词模板为默认");
    } catch (resetError) {
      console.error("Failed to reset all AI prompt templates:", resetError);
      setError(resetError instanceof Error ? resetError.message : "重置全部提示词模板失败");
    } finally {
      setActionLoading(null);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-8 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        正在加载提示词模板…
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-border bg-card p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium">AI 提示词模板</h3>
            <p className="mt-1 text-xs text-muted-foreground">
              配置各场景的输出目标与补充要求。保存后会写入本地
              `ai-prompt-templates.json`，并在生成提示词时优先使用。
            </p>
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
            全部重置
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
                <div className="font-medium">{template.label}</div>
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
                <label className="text-xs text-muted-foreground">显示名称</label>
                <input
                  value={draft.label}
                  onChange={(event) => updateDraft({ label: event.target.value })}
                  className="h-9 w-full rounded-md border border-border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">输出目标 (output_goal)</label>
                <textarea
                  value={draft.output_goal}
                  onChange={(event) => updateDraft({ output_goal: event.target.value })}
                  rows={4}
                  className="w-full resize-y rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">
                  场景补充要求 (scene_requirement)
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
                  保存当前模板
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
                  重置当前
                </Button>
              </div>
            </>
          ) : (
            <p className="text-sm text-muted-foreground">暂无可用模板</p>
          )}

          {message && <p className="text-xs text-green-700">{message}</p>}
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
      </div>
    </div>
  );
}
