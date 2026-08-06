import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { getCachedAiProviderCapabilities, loadAiProviderCapabilities } from "@/lib/aiCapabilities";
import type { AiProviderCapabilities } from "@/lib/backend";
import type { AiProvider } from "@/lib/types";

interface EngineCapabilityBadgesProps {
  provider?: AiProvider | string | null;
  compact?: boolean;
  className?: string;
}

function CapBadge({
  supported,
  label,
  title,
}: {
  supported: boolean;
  label: string;
  title?: string;
}) {
  return (
    <Badge
      variant={supported ? "default" : "secondary"}
      title={title}
      className={!supported ? "opacity-80" : undefined}
    >
      {label}
      {supported ? "" : "（不支持）"}
    </Badge>
  );
}

export function EngineCapabilityBadges({
  provider,
  compact = false,
  className = "",
}: EngineCapabilityBadgesProps) {
  const [capabilities, setCapabilities] = useState<AiProviderCapabilities[]>(
    () => getCachedAiProviderCapabilities() ?? [],
  );
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    void loadAiProviderCapabilities()
      .then((items) => {
        setCapabilities(items);
        setLoadError(null);
      })
      .catch((error) => {
        setCapabilities([]);
        setLoadError(error instanceof Error ? error.message : "能力信息加载失败");
      });
  }, []);

  const items = provider ? capabilities.filter((item) => item.provider === provider) : capabilities;

  if (loadError && items.length === 0) {
    return (
      <div
        className={`rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive ${className}`}
      >
        能力信息加载失败：{loadError}
      </div>
    );
  }

  if (items.length === 0) {
    return null;
  }

  return (
    <div className={`space-y-2 ${className}`}>
      {items.map((item) => {
        const notesTitle = item.notes || undefined;
        return (
          <div
            key={item.provider}
            className="rounded-md border border-border bg-card/60 px-3 py-2 text-xs"
            title={notesTitle}
          >
            <div className="mb-1.5 flex items-center gap-2">
              <span className="font-medium">{item.label}</span>
              {!compact && item.notes && (
                <span className="text-muted-foreground line-clamp-2" title={item.notes}>
                  {item.notes}
                </span>
              )}
            </div>
            <div className="flex flex-wrap gap-1">
              <CapBadge supported={item.start} label="启动" title={notesTitle} />
              <CapBadge supported={item.stop} label="停止" title={notesTitle} />
              <CapBadge
                supported={item.restart}
                label="重启"
                title={
                  item.restart
                    ? "停止当前运行后重新启动任务（非恢复旧 CLI 会话）"
                    : notesTitle || "当前引擎不支持重启"
                }
              />
              <CapBadge
                supported={item.send_input}
                label="输入"
                title={
                  item.send_input
                    ? notesTitle
                    : notesTitle || "当前引擎不支持会话中输入（非交互模式）"
                }
              />
              <CapBadge supported={item.resume} label="续聊" title={notesTitle} />
            </div>
          </div>
        );
      })}
    </div>
  );
}
