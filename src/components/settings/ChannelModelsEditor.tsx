import { Plus, Sparkles, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import { applyCatalogToModel, emptyChannelModel, lookupModelCatalog } from "@/lib/modelCatalog";
import type { AiChannelModel, ModelCatalogEntry } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ChannelModelsEditorProps {
  models: AiChannelModel[];
  catalog: ModelCatalogEntry[];
  disabled?: boolean;
  onChange: (models: AiChannelModel[]) => void;
}

const FALLBACK_LEVELS = ["low", "medium", "high"];

export function ChannelModelsEditor({
  models,
  catalog,
  disabled = false,
  onChange,
}: ChannelModelsEditorProps) {
  const { t } = useTranslation("settings");

  const updateAt = (index: number, next: AiChannelModel) => {
    onChange(models.map((item, itemIndex) => (itemIndex === index ? next : item)));
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <label className="text-xs font-medium text-muted-foreground">
          {t("channels.fields.models")}
        </label>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={disabled}
          onClick={() => onChange([...models, emptyChannelModel()])}
        >
          <Plus className="mr-1 h-3.5 w-3.5" />
          {t("channels.actions.addModel")}
        </Button>
      </div>
      {models.length === 0 ? (
        <p className="text-[11px] text-muted-foreground">{t("channels.fields.modelsEmpty")}</p>
      ) : (
        models.map((model, index) => {
          const entry = lookupModelCatalog(catalog, model.id);
          const levels =
            entry && entry.thinking_levels.length > 0 ? entry.thinking_levels : FALLBACK_LEVELS;
          const thinkingOn = model.thinking_enabled === true;
          return (
            <div
              key={`${model.id}-${index}`}
              className="space-y-2 rounded-md border border-border p-3"
            >
              <div className="flex gap-2">
                <Input
                  value={model.id}
                  disabled={disabled}
                  placeholder={t("channels.fields.modelId")}
                  onChange={(event) => {
                    const id = event.target.value;
                    updateAt(index, applyCatalogToModel(catalog, { ...model, id }));
                  }}
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={disabled || !model.id.trim()}
                  title={t("channels.actions.fillFromCatalog")}
                  onClick={() => updateAt(index, applyCatalogToModel(catalog, model, true))}
                >
                  <Sparkles className="h-3.5 w-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={disabled}
                  onClick={() => onChange(models.filter((_, itemIndex) => itemIndex !== index))}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="text-[11px] text-muted-foreground">
                    {t("channels.fields.contextTokens")}
                  </label>
                  <Input
                    type="number"
                    min={1}
                    disabled={disabled}
                    value={model.context_tokens ?? ""}
                    placeholder={entry ? String(entry.context_tokens) : "128000"}
                    onChange={(event) => {
                      const parsed = Number(event.target.value);
                      updateAt(index, {
                        ...model,
                        context_tokens:
                          event.target.value && Number.isFinite(parsed) && parsed > 0
                            ? parsed
                            : null,
                      });
                    }}
                  />
                </div>
                <div>
                  <label className="text-[11px] text-muted-foreground">
                    {t("channels.fields.maxOutputTokens")}
                  </label>
                  <Input
                    type="number"
                    min={1}
                    disabled={disabled}
                    value={model.max_output_tokens ?? ""}
                    placeholder={entry ? String(entry.max_output_tokens) : "8192"}
                    onChange={(event) => {
                      const parsed = Number(event.target.value);
                      updateAt(index, {
                        ...model,
                        max_output_tokens:
                          event.target.value && Number.isFinite(parsed) && parsed > 0
                            ? parsed
                            : null,
                      });
                    }}
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="text-[11px] text-muted-foreground">
                    {t("channels.fields.thinking")}
                  </label>
                  <Select
                    value={thinkingOn ? "on" : "off"}
                    onValueChange={(value) => {
                      if (value !== "on" && value !== "off") return;
                      updateAt(index, {
                        ...model,
                        thinking_enabled: value === "on",
                        thinking_level:
                          value === "on"
                            ? (model.thinking_level ??
                              levels.find((level) => level === "medium") ??
                              levels[0] ??
                              null)
                            : null,
                      });
                    }}
                  >
                    <SelectTrigger className="mt-1 bg-background">
                      <SelectValue>
                        {(value) =>
                          value === "on"
                            ? t("channels.status.thinkingOn")
                            : t("channels.status.thinkingOff")
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="off">{t("channels.status.thinkingOff")}</SelectItem>
                      <SelectItem value="on">{t("channels.status.thinkingOn")}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div>
                  <label className="text-[11px] text-muted-foreground">
                    {t("channels.fields.thinkingLevel")}
                  </label>
                  <Select
                    value={model.thinking_level ?? undefined}
                    onValueChange={(value) => {
                      if (typeof value === "string") {
                        updateAt(index, { ...model, thinking_level: value });
                      }
                    }}
                    disabled={disabled || !thinkingOn}
                  >
                    <SelectTrigger className="mt-1 bg-background">
                      <SelectValue>
                        {(value) =>
                          typeof value === "string"
                            ? t(`channels.thinkingLevels.${value}`, { defaultValue: value })
                            : t("channels.fields.thinkingLevel")
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {levels.map((level) => (
                        <SelectItem key={level} value={level}>
                          {t(`channels.thinkingLevels.${level}`, { defaultValue: level })}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </div>
          );
        })
      )}
      <p className="text-[11px] text-muted-foreground">{t("channels.fields.modelsHint")}</p>
    </div>
  );
}
