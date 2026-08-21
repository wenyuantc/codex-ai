import { useTranslation } from "react-i18next";

import {
  buildTokenUsageMetrics,
  formatUsageMetricText,
  type TokenUsageMetricSource,
} from "@/lib/dashboardReport";
import { cn } from "@/lib/utils";

interface TokenUsageBreakdownProps {
  source: TokenUsageMetricSource;
  layout: "stacked" | "inline";
  className?: string;
  title?: string;
}

export function TokenUsageBreakdown({
  source,
  layout,
  className,
  title,
}: TokenUsageBreakdownProps) {
  const { t } = useTranslation(["tasks", "common"]);
  const metrics = buildTokenUsageMetrics(source);
  const labels = {
    unknown: t("common:unknown"),
    empty: t("detail.sidebar.usageEmpty"),
  };
  const items = [
    { key: "input", label: t("detail.sidebar.usageInput"), metric: metrics.input },
    { key: "output", label: t("detail.sidebar.usageOutput"), metric: metrics.output },
    { key: "cached", label: t("detail.sidebar.usageCache"), metric: metrics.cached },
    { key: "total", label: t("detail.sidebar.usageTotal"), metric: metrics.total },
    { key: "cacheRate", label: t("detail.sidebar.usageCacheRate"), metric: metrics.cacheRate },
  ] as const;

  if (layout === "stacked") {
    return (
      <div
        className={cn("space-y-1.5 text-xs", className)}
        role="group"
        aria-label={t("detail.sidebar.usage")}
        title={title}
      >
        {items.map((item) => (
          <div key={item.key} className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">{item.label}</span>
            <span className="font-medium text-foreground">
              {formatUsageMetricText(item.metric, labels)}
            </span>
          </div>
        ))}
      </div>
    );
  }

  return (
    <div
      className={cn("grid grid-cols-5 gap-x-2.5 gap-y-1 text-right", className)}
      role="group"
      aria-label={t("detail.sidebar.usage")}
      title={title}
    >
      {items.map((item) => (
        <div key={item.key} className="min-w-0">
          <div className="truncate text-[10px] text-muted-foreground">{item.label}</div>
          <div className="truncate text-xs font-medium tabular-nums text-foreground">
            {formatUsageMetricText(item.metric, labels)}
          </div>
        </div>
      ))}
    </div>
  );
}
