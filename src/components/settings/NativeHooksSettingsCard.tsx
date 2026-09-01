import { Plus, Trash2 } from "lucide-react";
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
import type { NativeHook } from "@/lib/native";

interface NativeHooksSettingsCardProps {
  hooks: NativeHook[];
  onChange: (hooks: NativeHook[]) => void;
}

function emptyHook(): NativeHook {
  return {
    id: globalThis.crypto?.randomUUID?.() ?? `hook-${Date.now()}`,
    event: "pre_tool_use",
    matcher: "*",
    command: "",
    timeout_secs: 30,
    enabled: true,
  };
}

export function NativeHooksSettingsCard({ hooks, onChange }: NativeHooksSettingsCardProps) {
  const { t } = useTranslation("settings");

  const update = (id: string, patch: Partial<NativeHook>) => {
    onChange(hooks.map((hook) => (hook.id === id ? { ...hook, ...patch } : hook)));
  };

  return (
    <div className="space-y-3 rounded-lg border border-border bg-card p-4">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h3 className="text-sm font-medium">{t("nativeHooks.title")}</h3>
          <p className="text-xs text-muted-foreground">{t("nativeHooks.description")}</p>
        </div>
        <Button size="sm" variant="outline" onClick={() => onChange([...hooks, emptyHook()])}>
          <Plus className="h-4 w-4" />
          {t("nativeHooks.add")}
        </Button>
      </div>
      {hooks.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("nativeHooks.empty")}</p>
      )}
      {hooks.map((hook) => (
        <div key={hook.id} className="space-y-2 rounded-md border border-border p-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="h-4 w-4 rounded border-input"
                checked={hook.enabled}
                onChange={(event) => update(hook.id, { enabled: event.target.checked })}
              />
              {t("nativeHooks.enabled")}
            </label>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => onChange(hooks.filter((item) => item.id !== hook.id))}
            >
              <Trash2 className="h-4 w-4" />
              {t("nativeHooks.delete")}
            </Button>
          </div>
          <div className="grid gap-2 sm:grid-cols-2">
            <Select
              value={hook.event}
              onValueChange={(value) => {
                if (value) update(hook.id, { event: value });
              }}
            >
              <SelectTrigger>
                <SelectValue>
                  {(value) =>
                    value === "post_tool_use"
                      ? t("nativeHooks.eventPost")
                      : t("nativeHooks.eventPre")
                  }
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="pre_tool_use">{t("nativeHooks.eventPre")}</SelectItem>
                <SelectItem value="post_tool_use">{t("nativeHooks.eventPost")}</SelectItem>
              </SelectContent>
            </Select>
            <Input
              placeholder={t("nativeHooks.matcherPlaceholder")}
              value={hook.matcher}
              onChange={(event) => update(hook.id, { matcher: event.target.value })}
            />
          </div>
          <Input
            placeholder={t("nativeHooks.commandPlaceholder")}
            value={hook.command}
            onChange={(event) => update(hook.id, { command: event.target.value })}
          />
          <div className="max-w-xs space-y-1">
            <label className="text-xs text-muted-foreground">{t("nativeHooks.timeoutLabel")}</label>
            <Input
              type="number"
              min={1}
              max={120}
              value={hook.timeout_secs}
              onChange={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                update(hook.id, {
                  timeout_secs: Number.isNaN(parsed) ? 30 : Math.min(120, Math.max(1, parsed)),
                });
              }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}
