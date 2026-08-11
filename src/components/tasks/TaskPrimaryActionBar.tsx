import {
  AlertTriangle,
  ClipboardCheck,
  GitBranch,
  Loader2,
  MoreHorizontal,
  Play,
  ScrollText,
  Square,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { TaskPrimaryCta } from "@/lib/taskPrimaryCta";
import { cn } from "@/lib/utils";

interface SecondaryAction {
  key: string;
  label: string;
  disabled?: boolean;
  onSelect: () => void;
}

interface TaskPrimaryActionBarProps {
  primaryCta: TaskPrimaryCta;
  automationLabel?: string | null;
  loading?: boolean;
  onPrimaryAction: () => void;
  secondaryActions?: SecondaryAction[];
  className?: string;
}

function primaryButtonVariant(
  tone: TaskPrimaryCta["tone"],
): "default" | "destructive" | "secondary" | "outline" {
  switch (tone) {
    case "danger":
      return "destructive";
    case "warning":
      return "secondary";
    case "muted":
      return "outline";
    case "primary":
    default:
      return "default";
  }
}

function PrimaryIcon({ kind, loading }: { kind: TaskPrimaryCta["kind"]; loading?: boolean }) {
  if (loading || kind === "starting" || kind === "running_locked") {
    return <Loader2 className="h-4 w-4 animate-spin" />;
  }
  switch (kind) {
    case "stop":
      return <Square className="h-4 w-4" />;
    case "review":
      return <ScrollText className="h-4 w-4" />;
    case "blocked":
      return <AlertTriangle className="h-4 w-4" />;
    case "commit":
      return <GitBranch className="h-4 w-4" />;
    case "acceptance":
      return <ClipboardCheck className="h-4 w-4" />;
    case "run":
      return <Play className="h-4 w-4" />;
    default:
      return null;
  }
}

export function TaskPrimaryActionBar({
  primaryCta,
  automationLabel,
  loading = false,
  onPrimaryAction,
  secondaryActions = [],
  className,
}: TaskPrimaryActionBarProps) {
  const { t } = useTranslation("tasks");
  if (primaryCta.kind === "none") {
    return null;
  }

  const disabled = primaryCta.disabled || loading;
  const visibleSecondary = secondaryActions.filter(Boolean);

  return (
    <div
      className={cn("flex flex-wrap items-center justify-between gap-3 px-1 py-3", className)}
      data-slot="task-primary-action-bar"
    >
      <div className="min-w-0 flex-1">
        {automationLabel ? (
          <p className="truncate text-xs text-muted-foreground">
            {t("primaryActionBar.automationPrefix", { label: automationLabel })}
          </p>
        ) : (
          <p className="truncate text-xs text-muted-foreground">
            {t("primaryActionBar.primaryPath")}
          </p>
        )}
        {primaryCta.reason && primaryCta.disabled ? (
          <p className="truncate text-[11px] text-amber-700 dark:text-amber-200/90">
            {primaryCta.reason}
          </p>
        ) : null}
      </div>

      <div className="flex shrink-0 items-center gap-2">
        <Button
          type="button"
          size="sm"
          variant={primaryButtonVariant(primaryCta.tone)}
          disabled={disabled}
          title={primaryCta.reason ?? primaryCta.label}
          aria-label={primaryCta.label}
          onClick={onPrimaryAction}
          className={cn(
            primaryCta.tone === "warning" &&
              "border-amber-500/40 bg-amber-500 text-black hover:bg-amber-400",
            primaryCta.tone === "danger" && "bg-red-600 text-white hover:bg-red-700",
            primaryCta.tone === "primary" &&
              primaryCta.kind === "run" &&
              "bg-green-600 text-white hover:bg-green-700",
          )}
        >
          <PrimaryIcon kind={primaryCta.kind} loading={loading && !primaryCta.disabled} />
          {primaryCta.label}
        </Button>

        {visibleSecondary.length > 0 && (
          <DropdownMenu>
            <DropdownMenuTrigger
              className="inline-flex size-7 items-center justify-center rounded-lg border border-border bg-background text-foreground outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
              aria-label={t("primaryActionBar.moreActionsAria")}
            >
              <MoreHorizontal className="h-4 w-4" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="min-w-44">
              {visibleSecondary.map((action) => (
                <DropdownMenuItem
                  key={action.key}
                  disabled={action.disabled}
                  onClick={action.onSelect}
                >
                  {action.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </div>
  );
}
