import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

interface DetailSectionProps {
  icon?: LucideIcon;
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  className?: string;
  contentClassName?: string;
}

export function DetailSection({
  icon: Icon,
  title,
  description,
  actions,
  children,
  className,
  contentClassName,
}: DetailSectionProps) {
  return (
    <section className={cn("rounded-lg border border-border/60 bg-background/60 p-4", className)}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          <div className="flex items-center gap-2 text-sm font-medium text-foreground">
            {Icon ? <Icon className="h-4 w-4 shrink-0 text-primary" /> : null}
            {title}
          </div>
          {description ? (
            <p className="text-[11px] leading-relaxed text-muted-foreground">{description}</p>
          ) : null}
        </div>
        {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
      {children ? <div className={cn("mt-3", contentClassName)}>{children}</div> : null}
    </section>
  );
}

interface DetailStatProps {
  label: ReactNode;
  value: ReactNode;
  className?: string;
}

export function DetailStat({ label, value, className }: DetailStatProps) {
  return (
    <div className={cn("rounded-md border border-border/60 bg-background/70 px-3 py-2", className)}>
      <p className="text-[11px] text-muted-foreground">{label}</p>
      <p className="mt-0.5 truncate text-xs font-medium text-foreground">{value}</p>
    </div>
  );
}
