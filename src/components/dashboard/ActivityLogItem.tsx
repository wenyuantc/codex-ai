import type { ActivityLog } from "@/lib/types";
import { formatDate, getActivityActionLabel, getActivityDetailsLabel } from "@/lib/utils";

interface ActivityLogItemProps {
  activity: ActivityLog;
  variant?: "feed" | "dialog";
}

export function ActivityLogItem({ activity, variant = "feed" }: ActivityLogItemProps) {
  const details = getActivityDetailsLabel(activity.action, activity.details);
  const content = (
    <div className="min-w-0 flex-1">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-medium">{getActivityActionLabel(activity.action)}</span>
        {activity.project_name && (
          <span className="rounded bg-secondary px-1.5 py-0.5 text-xs">
            {activity.project_name}
          </span>
        )}
        {activity.employee_name && (
          <span className="rounded bg-secondary px-1.5 py-0.5 text-xs">
            {activity.employee_name}
          </span>
        )}
      </div>
      {details && (
        <p className="mt-1 break-all text-xs text-muted-foreground">
          {details}
        </p>
      )}
      <span className="mt-1 block text-[10px] text-muted-foreground/70">
        {formatDate(activity.created_at)}
      </span>
    </div>
  );

  if (variant === "dialog") {
    return (
      <div className="rounded-lg border border-border/60 px-3 py-2.5 text-sm">
        {content}
      </div>
    );
  }

  return (
    <div className="flex items-start gap-3 border-b border-border/50 py-2 text-sm last:border-0">
      <div className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-primary" />
      {content}
    </div>
  );
}
