import { Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { AiChannel } from "@/lib/types";

export function selectNativeChannel(channels: AiChannel[], currentId: string): string {
  if (currentId && channels.some((channel) => channel.id === currentId)) {
    return currentId;
  }
  return channels[0]?.id ?? "";
}

const NATIVE_FALLBACK_LEVELS = ["low", "medium", "high"];

export function resolveNativeThinking(
  channel: AiChannel | undefined,
  modelId: string,
): { enabled: boolean; levels: string[]; defaultLevel: string } {
  const model = channel?.models.find((item) => item.id === modelId);
  const enabled = model?.thinking_enabled !== false;
  const levels =
    model?.thinking_levels && model.thinking_levels.length > 0
      ? model.thinking_levels
      : NATIVE_FALLBACK_LEVELS;
  const preferred = model?.thinking_level?.trim();
  const defaultLevel =
    preferred && levels.includes(preferred)
      ? preferred
      : (levels.find((level) => level === "medium") ?? levels[0] ?? "high");
  return { enabled, levels, defaultLevel };
}

export function selectNativeModel(channel: AiChannel | undefined, current: string): string {
  const models =
    channel?.models.map((item) => item.id).filter((item) => item.trim().length > 0) ?? [];
  if (current && models.includes(current)) {
    return current;
  }
  return models[0]?.trim() || current.trim() || "default";
}

interface NativeChannelFieldsProps {
  channels: AiChannel[];
  channelId: string;
  model: string;
  loading: boolean;
  error: string | null;
  onChannelChange: (channelId: string) => void;
  onModelChange: (model: string) => void;
  onRefresh: () => void;
}

export function NativeChannelFields({
  channels,
  channelId,
  model,
  loading,
  error,
  onChannelChange,
  onModelChange,
  onRefresh,
}: NativeChannelFieldsProps) {
  const { t } = useTranslation("employees");
  const selectedChannel = channels.find((channel) => channel.id === channelId) ?? null;
  const modelOptions = selectedChannel?.models.map((item) => item.id) ?? [];

  return (
    <div className="grid grid-cols-2 gap-3">
      <div>
        <label className="text-xs font-medium text-muted-foreground">{t("channel")}</label>
        <div className="mt-1 flex gap-2">
          <div className="flex-1">
            {channels.length > 0 ? (
              <Select
                value={channelId || undefined}
                onValueChange={(value) => {
                  if (value) onChannelChange(value);
                }}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue>
                    {(value) => {
                      if (typeof value !== "string") {
                        return t("selectChannel");
                      }
                      const channel = channels.find((item) => item.id === value);
                      return channel?.name ?? t("selectChannel");
                    }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {channels.map((channel) => (
                    <SelectItem key={channel.id} value={channel.id}>
                      {channel.name} · {channel.protocol}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <Input
                value=""
                placeholder={t("noEnabledChannel")}
                disabled
                className="bg-background"
              />
            )}
          </div>
          <button
            type="button"
            onClick={onRefresh}
            disabled={loading}
            className="px-2 py-1 border border-input rounded-md hover:bg-accent disabled:opacity-50"
            title={t("refreshChannelsTitle")}
          >
            {loading ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
          </button>
        </div>
        {channels.length === 0 && !loading ? (
          <p className="mt-1 text-[11px] text-destructive">{t("noEnabledChannelHint")}</p>
        ) : null}
        {error ? <p className="mt-1 text-[11px] text-destructive">{error}</p> : null}
      </div>

      <div>
        <label className="text-xs font-medium text-muted-foreground">{t("model")}</label>
        {modelOptions.length > 0 ? (
          <Select
            value={model || undefined}
            onValueChange={(value) => {
              if (value) onModelChange(value);
            }}
          >
            <SelectTrigger className="mt-1 bg-background">
              <SelectValue>
                {(value) => (typeof value === "string" ? value : t("selectModel"))}
              </SelectValue>
            </SelectTrigger>
            <SelectContent className="max-h-72">
              {modelOptions.map((item) => (
                <SelectItem key={item} value={item}>
                  {item}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : (
          <Input
            value={model}
            onChange={(event) => onModelChange(event.target.value)}
            placeholder={t("selectModel")}
            disabled={!selectedChannel}
            className="mt-1"
          />
        )}
      </div>
    </div>
  );
}
