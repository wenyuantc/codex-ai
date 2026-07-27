import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import {
  getAiProviderCapabilities,
  type AiProviderCapabilities,
} from "@/lib/backend";
import type { AiProvider } from "@/lib/types";

interface EngineCapabilityBadgesProps {
  provider?: AiProvider | string | null;
  compact?: boolean;
  className?: string;
}

export function EngineCapabilityBadges({
  provider,
  compact = false,
  className = "",
}: EngineCapabilityBadgesProps) {
  const [capabilities, setCapabilities] = useState<AiProviderCapabilities[]>([]);

  useEffect(() => {
    void getAiProviderCapabilities()
      .then(setCapabilities)
      .catch(() => setCapabilities([]));
  }, []);

  const items = provider
    ? capabilities.filter((item) => item.provider === provider)
    : capabilities;

  if (items.length === 0) {
    return null;
  }

  return (
    <div className={`space-y-2 ${className}`}>
      {items.map((item) => (
        <div
          key={item.provider}
          className="rounded-md border border-border bg-card/60 px-3 py-2 text-xs"
          title={item.notes}
        >
          <div className="mb-1.5 flex items-center gap-2">
            <span className="font-medium">{item.label}</span>
            {!compact && (
              <span className="text-muted-foreground">{item.notes}</span>
            )}
          </div>
          <div className="flex flex-wrap gap-1">
            <Badge variant={item.start ? "default" : "outline"}>启动</Badge>
            <Badge variant={item.stop ? "default" : "outline"}>停止</Badge>
            <Badge variant={item.restart ? "default" : "secondary"}>
              重启{item.restart ? "" : "（不支持）"}
            </Badge>
            <Badge variant={item.send_input ? "default" : "secondary"}>
              输入{item.send_input ? "" : "（不支持）"}
            </Badge>
            <Badge variant={item.resume ? "default" : "outline"}>续聊</Badge>
          </div>
        </div>
      ))}
    </div>
  );
}
