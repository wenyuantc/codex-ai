import { Check, Copy } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { cn, formatDate, getStatusColor, getStatusLabel } from "@/lib/utils";

interface TaskDetailHeaderProps {
  title: string;
  onTitleChange: (value: string) => void;
  onTitleBlur: () => void;
  taskId: string;
  taskIdCopied: boolean;
  onCopyTaskId: () => void;
  status: string;
  projectName?: string;
  createdAt: string;
}

export function TaskDetailHeader({
  title,
  onTitleChange,
  onTitleBlur,
  taskId,
  taskIdCopied,
  onCopyTaskId,
  status,
  projectName,
  createdAt,
}: TaskDetailHeaderProps) {
  const { t } = useTranslation("tasks");

  return (
    <div className="shrink-0 border-b border-border/70 px-5 pb-3 pr-12 pt-4">
      <Input
        value={title}
        onChange={(e) => onTitleChange(e.target.value)}
        onBlur={onTitleBlur}
        className="h-auto border-none bg-transparent px-0 text-base font-semibold shadow-none focus-visible:ring-0 dark:bg-transparent"
        placeholder={t("detail.header.titlePlaceholder")}
      />
      <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px] text-muted-foreground">
        <button
          type="button"
          onClick={onCopyTaskId}
          className="inline-flex cursor-pointer items-center rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          title={taskIdCopied ? t("detail.header.taskIdCopied") : t("detail.header.copyTaskId")}
          aria-label={
            taskIdCopied ? t("detail.header.taskIdCopied") : t("detail.header.copyTaskId")
          }
        >
          <Badge
            variant="outline"
            className="h-5.5 cursor-pointer gap-1.5 rounded-md px-2 font-mono text-[11px] transition-colors hover:border-primary/40 hover:bg-primary/5"
          >
            {taskId}
            {taskIdCopied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
          </Badge>
        </button>
        {status ? (
          <span className="inline-flex items-center gap-1.5">
            <span className={cn("h-1.5 w-1.5 rounded-full", getStatusColor(status))} />
            {getStatusLabel(status)}
          </span>
        ) : null}
        {projectName ? (
          <span className="truncate">{t("detail.header.project", { name: projectName })}</span>
        ) : null}
        <span>{t("detail.header.createdAt", { date: formatDate(createdAt) })}</span>
      </div>
    </div>
  );
}
