import { useEffect, useState } from "react";
import type { ActivityLog } from "@/lib/types";
import { useDashboardStore } from "@/stores/dashboardStore";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDate, getActivityActionLabel, getActivityDetailsLabel } from "@/lib/utils";
import { ChevronLeft, ChevronRight } from "lucide-react";
import type { EnvironmentMode } from "@/lib/types";

const PAGE_SIZE = 20;

interface ActivityListDialogProps {
  open: boolean;
  projectId?: string;
  projectName?: string | null;
  environmentMode: EnvironmentMode;
  selectedSshConfigId?: string | null;
  onOpenChange: (open: boolean) => void;
}

export function ActivityListDialog({
  open,
  projectId,
  projectName,
  environmentMode,
  selectedSshConfigId,
  onOpenChange,
}: ActivityListDialogProps) {
  const fetchActivitiesPage = useDashboardStore((state) => state.fetchActivitiesPage);
  const [activities, setActivities] = useState<ActivityLog[]>([]);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);

  const totalPages = total > 0 ? Math.ceil(total / PAGE_SIZE) : 0;
  const rangeStart = total === 0 ? 0 : (page - 1) * PAGE_SIZE + 1;
  const rangeEnd = total === 0 ? 0 : Math.min(page * PAGE_SIZE, total);

  useEffect(() => {
    if (!open) {
      return;
    }
    setPage(1);
  }, [open, projectId]);

  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    setLoading(true);

    void fetchActivitiesPage(environmentMode, selectedSshConfigId, page, PAGE_SIZE, projectId)
      .then((result) => {
        if (!active) {
          return;
        }

        const nextTotalPages = result.total > 0 ? Math.ceil(result.total / PAGE_SIZE) : 0;
        if (nextTotalPages > 0 && page > nextTotalPages) {
          setPage(nextTotalPages);
          return;
        }

        setActivities(result.items);
        setTotal(result.total);
      })
      .catch((error) => {
        console.error("Failed to fetch activity page:", error);
        if (!active) {
          return;
        }
        setActivities([]);
        setTotal(0);
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [environmentMode, fetchActivitiesPage, open, page, projectId, selectedSshConfigId]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-[min(96vw,48rem)] max-w-[min(96vw,48rem)] sm:max-w-[min(96vw,48rem)]"
      >
        <DialogHeader>
          <DialogTitle>全部活动</DialogTitle>
          <DialogDescription>
            {projectName
              ? `查看项目“${projectName}”的活动记录，按时间倒序分页展示。`
              : "查看所有项目的活动记录，按时间倒序分页展示。"}
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-xl border border-border/70">
          {loading ? (
            <div className="flex h-[28rem] items-center justify-center text-sm text-muted-foreground">
              正在加载活动记录...
            </div>
          ) : activities.length === 0 ? (
            <div className="flex h-[28rem] items-center justify-center text-sm text-muted-foreground">
              暂无活动记录
            </div>
          ) : (
            <ScrollArea className="h-[28rem]">
              <div className="space-y-2 p-3">
                {activities.map((activity) => {
                  const details = getActivityDetailsLabel(activity.action, activity.details);

                  return (
                    <div
                      key={activity.id}
                      className="rounded-lg border border-border/60 px-3 py-2.5 text-sm"
                    >
                      <div className="flex items-center gap-2 flex-wrap">
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
                        <p className="mt-1 text-xs text-muted-foreground break-all">
                          {details}
                        </p>
                      )}
                      <span className="mt-1 block text-[10px] text-muted-foreground/70">
                        {formatDate(activity.created_at)}
                      </span>
                    </div>
                  );
                })}
              </div>
            </ScrollArea>
          )}
        </div>

        <div className="flex items-center justify-between gap-3">
          <span className="text-xs text-muted-foreground">
            {total === 0 ? "暂无分页数据" : `显示 ${rangeStart}-${rangeEnd} 条，共 ${total} 条`}
          </span>
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">
              {total === 0 ? "第 0 / 0 页" : `第 ${page} / ${totalPages} 页`}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((current) => Math.max(1, current - 1))}
              disabled={loading || page <= 1}
            >
              <ChevronLeft className="h-3.5 w-3.5" />
              上一页
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((current) => current + 1)}
              disabled={loading || total === 0 || page >= totalPages}
            >
              下一页
              <ChevronRight className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
