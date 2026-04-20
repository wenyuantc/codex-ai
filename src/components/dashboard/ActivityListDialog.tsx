import { useEffect, useMemo, useState } from "react";
import type { ActivityLog } from "@/lib/types";
import { useDashboardStore, type ActivityFilters } from "@/stores/dashboardStore";
import { useProjectStore } from "@/stores/projectStore";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { formatDate, getActivityActionLabel, getActivityDetailsLabel } from "@/lib/utils";
import { ChevronLeft, ChevronRight } from "lucide-react";
import type { EnvironmentMode } from "@/lib/types";

const PAGE_SIZE = 20;
const ALL_PROJECTS_VALUE = "__all_projects__";
const ALL_ACTIONS_VALUE = "__all_actions__";

interface ActivityFilterFormState {
  projectId: string;
  action: string;
  keyword: string;
  startDate: string;
  endDate: string;
}

function buildDefaultFilters(projectId?: string): ActivityFilterFormState {
  return {
    projectId: projectId ?? ALL_PROJECTS_VALUE,
    action: ALL_ACTIONS_VALUE,
    keyword: "",
    startDate: "",
    endDate: "",
  };
}

function toActivityFilters(filters: ActivityFilterFormState): ActivityFilters {
  return {
    projectId: filters.projectId === ALL_PROJECTS_VALUE ? undefined : filters.projectId,
    action: filters.action === ALL_ACTIONS_VALUE ? undefined : filters.action,
    keyword: filters.keyword.trim(),
    startDate: filters.startDate || undefined,
    endDate: filters.endDate || undefined,
  };
}

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
  const visibleProjects = useProjectStore((state) => state.projects);
  const [activities, setActivities] = useState<ActivityLog[]>([]);
  const [filters, setFilters] = useState<ActivityFilterFormState>(() => buildDefaultFilters(projectId));
  const [availableActions, setAvailableActions] = useState<string[]>([]);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);

  const defaultFilters = useMemo(() => buildDefaultFilters(projectId), [projectId]);
  const projectOptions = useMemo(() => {
    if (!projectId || !projectName || visibleProjects.some((project) => project.id === projectId)) {
      return visibleProjects;
    }

    return [{ id: projectId, name: projectName }, ...visibleProjects];
  }, [projectId, projectName, visibleProjects]);
  const currentFilters = useMemo(() => toActivityFilters(filters), [filters]);
  const hasInvalidDateRange = Boolean(
    filters.startDate
    && filters.endDate
    && filters.startDate > filters.endDate,
  );
  const resetDisabled = (
    filters.projectId === defaultFilters.projectId
    && filters.action === defaultFilters.action
    && filters.keyword === defaultFilters.keyword
    && filters.startDate === defaultFilters.startDate
    && filters.endDate === defaultFilters.endDate
  );
  const totalPages = total > 0 ? Math.ceil(total / PAGE_SIZE) : 0;
  const rangeStart = total === 0 ? 0 : (page - 1) * PAGE_SIZE + 1;
  const rangeEnd = total === 0 ? 0 : Math.min(page * PAGE_SIZE, total);
  const emptyStateMessage = hasInvalidDateRange
    ? "开始日期不能晚于结束日期"
    : resetDisabled
      ? "暂无活动记录"
      : "没有符合当前筛选条件的活动记录";

  useEffect(() => {
    if (!open) {
      return;
    }

    setPage(1);
    setFilters(buildDefaultFilters(projectId));
  }, [open, projectId]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setPage(1);
  }, [filters.action, filters.endDate, filters.keyword, filters.projectId, filters.startDate, open]);

  useEffect(() => {
    if (!open || filters.projectId === ALL_PROJECTS_VALUE) {
      return;
    }

    if (projectOptions.some((project) => project.id === filters.projectId)) {
      return;
    }

    setFilters((current) => ({ ...current, projectId: defaultFilters.projectId }));
  }, [defaultFilters.projectId, filters.projectId, open, projectOptions]);

  useEffect(() => {
    if (!open || filters.action === ALL_ACTIONS_VALUE) {
      return;
    }

    if (availableActions.includes(filters.action)) {
      return;
    }

    setFilters((current) => ({ ...current, action: ALL_ACTIONS_VALUE }));
  }, [availableActions, filters.action, open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    setLoading(true);

    void fetchActivitiesPage(environmentMode, selectedSshConfigId, page, PAGE_SIZE, currentFilters)
      .then((result) => {
        if (!active) {
          return;
        }

        const nextTotalPages = result.total > 0 ? Math.ceil(result.total / PAGE_SIZE) : 0;
        if (nextTotalPages > 0 && page > nextTotalPages) {
          setPage(nextTotalPages);
          return;
        }
        if (nextTotalPages === 0 && page !== 1) {
          setPage(1);
          return;
        }

        setActivities(result.items);
        setTotal(result.total);
        setAvailableActions(result.availableActions);
      })
      .catch((error) => {
        console.error("Failed to fetch activity page:", error);
        if (!active) {
          return;
        }
        setActivities([]);
        setTotal(0);
        setAvailableActions([]);
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [currentFilters, environmentMode, fetchActivitiesPage, open, page, selectedSshConfigId]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-[min(96vw,56rem)] max-w-[min(96vw,56rem)] sm:max-w-[min(96vw,56rem)]"
      >
        <DialogHeader>
          <DialogTitle>全部活动</DialogTitle>
          <DialogDescription>
            {projectName
              ? `默认展示项目“${projectName}”的活动记录，并支持按项目、活动类型、关键词和时间范围筛选。`
              : "查看当前作用域下的全部活动，并支持按项目、活动类型、关键词和时间范围筛选。"}
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-xl border border-border/70 p-3">
          <div className="grid gap-3 md:grid-cols-2">
            <div className="space-y-1.5">
              <label className="text-sm font-medium text-foreground">项目</label>
              <Select<string>
                value={filters.projectId}
                onValueChange={(value) => setFilters((current) => ({
                  ...current,
                  projectId: value ?? ALL_PROJECTS_VALUE,
                }))}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={ALL_PROJECTS_VALUE}>全部项目</SelectItem>
                  {projectOptions.map((project) => (
                    <SelectItem key={project.id} value={project.id}>
                      {project.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-1.5">
              <label className="text-sm font-medium text-foreground">活动类型</label>
              <Select<string>
                value={filters.action}
                onValueChange={(value) => setFilters((current) => ({
                  ...current,
                  action: value ?? ALL_ACTIONS_VALUE,
                }))}
              >
                <SelectTrigger className="bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={ALL_ACTIONS_VALUE}>全部类型</SelectItem>
                  {availableActions.map((action) => (
                    <SelectItem key={action} value={action}>
                      {getActivityActionLabel(action)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-1.5 md:col-span-2">
              <label className="text-sm font-medium text-foreground" htmlFor="activity-keyword">
                内容关键词
              </label>
              <Input
                id="activity-keyword"
                value={filters.keyword}
                onChange={(event) => setFilters((current) => ({ ...current, keyword: event.target.value }))}
                placeholder="搜索活动类型、详情、项目或员工"
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-sm font-medium text-foreground" htmlFor="activity-start-date">
                开始日期
              </label>
              <Input
                id="activity-start-date"
                type="date"
                value={filters.startDate}
                onChange={(event) => setFilters((current) => ({ ...current, startDate: event.target.value }))}
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-sm font-medium text-foreground" htmlFor="activity-end-date">
                结束日期
              </label>
              <Input
                id="activity-end-date"
                type="date"
                value={filters.endDate}
                onChange={(event) => setFilters((current) => ({ ...current, endDate: event.target.value }))}
              />
            </div>
          </div>

          <div className="mt-3 flex flex-wrap items-center justify-between gap-3">
            <div className="text-xs text-muted-foreground">
              支持组合筛选，时间范围按所选日期的整天边界计算。
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setFilters(defaultFilters);
                setPage(1);
              }}
              disabled={resetDisabled}
            >
              重置筛选
            </Button>
          </div>

          {hasInvalidDateRange && (
            <p className="mt-2 text-xs text-destructive">
              开始日期不能晚于结束日期，请调整后再查看结果。
            </p>
          )}
        </div>

        <div className="rounded-xl border border-border/70">
          {loading ? (
            <div className="flex h-[24rem] items-center justify-center text-sm text-muted-foreground">
              正在加载活动记录...
            </div>
          ) : activities.length === 0 ? (
            <div className="flex h-[24rem] items-center justify-center px-4 text-center text-sm text-muted-foreground">
              {emptyStateMessage}
            </div>
          ) : (
            <ScrollArea className="h-[24rem]">
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
