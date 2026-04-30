import { useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { select } from "@/lib/database";
import type { Task } from "@/lib/types";
import { formatDate, getPriorityLabel, parseDateValue } from "@/lib/utils";
import { useProjectStore } from "@/stores/projectStore";

const ALL_PROJECTS_VALUE = "__all_projects__";

interface ArchiveManagementDialogProps {
  open: boolean;
  defaultProjectId?: string;
  onOpenChange: (open: boolean) => void;
}

interface ArchiveFilterState {
  projectId: string;
  keyword: string;
  startDate: string;
  endDate: string;
}

function buildDefaultFilters(defaultProjectId?: string): ArchiveFilterState {
  return {
    projectId: defaultProjectId ?? ALL_PROJECTS_VALUE,
    keyword: "",
    startDate: "",
    endDate: "",
  };
}

function createDayBoundary(date: string, endOfDay: boolean) {
  if (!date) {
    return null;
  }

  const normalized = endOfDay ? `${date}T23:59:59.999` : `${date}T00:00:00.000`;
  const parsed = new Date(normalized);
  return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
}

function normalizeSearchText(value: string | null | undefined) {
  return (value ?? "").trim().toLocaleLowerCase();
}

export function ArchiveManagementDialog({
  open,
  defaultProjectId,
  onOpenChange,
}: ArchiveManagementDialogProps) {
  const projects = useProjectStore((state) => state.projects);
  const [archivedTasks, setArchivedTasks] = useState<Task[]>([]);
  const [filters, setFilters] = useState<ArchiveFilterState>(() => buildDefaultFilters(defaultProjectId));
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const projectMap = useMemo(
    () => new Map(projects.map((project) => [project.id, project.name])),
    [projects],
  );
  const defaultFilters = useMemo(
    () => buildDefaultFilters(defaultProjectId),
    [defaultProjectId],
  );
  const visibleProjectIdsKey = useMemo(
    () => projects.map((project) => project.id).sort().join(","),
    [projects],
  );
  const hasInvalidDateRange = Boolean(
    filters.startDate
    && filters.endDate
    && filters.startDate > filters.endDate,
  );

  useEffect(() => {
    if (!open) {
      return;
    }

    setFilters(buildDefaultFilters(defaultProjectId));
  }, [defaultProjectId, open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    let active = true;
    const visibleProjectIds = projects.map((project) => project.id);
    if (visibleProjectIds.length === 0) {
      setArchivedTasks([]);
      setLoading(false);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);

    const projectPlaceholders = visibleProjectIds
      .map((_, index) => `$${index + 2}`)
      .join(", ");

    void select<Task>(
      `SELECT * FROM tasks
       WHERE status = $1
         AND project_id IN (${projectPlaceholders})
         AND deleted_at IS NULL
       ORDER BY updated_at DESC, id DESC`,
      ["archived", ...visibleProjectIds],
    )
      .then((rows) => {
        if (!active) {
          return;
        }

        setArchivedTasks(rows);
      })
      .catch((loadError) => {
        console.error("Failed to load archived tasks:", loadError);
        if (!active) {
          return;
        }

        setArchivedTasks([]);
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [open, visibleProjectIdsKey, projects]);

  const filteredTasks = useMemo(() => {
    if (hasInvalidDateRange) {
      return [];
    }

    const normalizedKeyword = normalizeSearchText(filters.keyword);
    const startTimestamp = createDayBoundary(filters.startDate, false);
    const endTimestamp = createDayBoundary(filters.endDate, true);

    return archivedTasks.filter((task) => {
      if (
        filters.projectId !== ALL_PROJECTS_VALUE
        && task.project_id !== filters.projectId
      ) {
        return false;
      }

      if (normalizedKeyword) {
        const haystack = normalizeSearchText([
          task.title,
          task.description,
          projectMap.get(task.project_id),
        ].join("\n"));

        if (!haystack.includes(normalizedKeyword)) {
          return false;
        }
      }

      if (startTimestamp === null && endTimestamp === null) {
        return true;
      }

      const taskTimestamp = parseDateValue(task.updated_at)?.getTime();
      if (taskTimestamp === undefined) {
        return false;
      }

      if (startTimestamp !== null && taskTimestamp < startTimestamp) {
        return false;
      }

      if (endTimestamp !== null && taskTimestamp > endTimestamp) {
        return false;
      }

      return true;
    });
  }, [archivedTasks, filters, hasInvalidDateRange, projectMap]);

  const isResetDisabled = (
    filters.projectId === defaultFilters.projectId
    && filters.keyword === defaultFilters.keyword
    && filters.startDate === defaultFilters.startDate
    && filters.endDate === defaultFilters.endDate
  );

  const emptyStateMessage = hasInvalidDateRange
    ? "开始时间不能晚于结束时间"
    : archivedTasks.length === 0
      ? "当前没有已归档任务"
      : "没有符合当前筛选条件的归档任务";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,72rem)] max-w-[min(96vw,72rem)] sm:max-w-[min(96vw,72rem)]">
        <DialogHeader>
          <DialogTitle>归档管理</DialogTitle>
          <DialogDescription>
            集中查看当前作用域下的已归档任务，并支持按项目、内容和时间范围筛选。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="rounded-xl border border-border/70 p-3">
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
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
                    <SelectValue>
                      {(value) => {
                        if (value === ALL_PROJECTS_VALUE) {
                          return "全部项目";
                        }

                        if (typeof value === "string") {
                          return projectMap.get(value) ?? value;
                        }

                        return "全部项目";
                      }}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={ALL_PROJECTS_VALUE}>全部项目</SelectItem>
                    {projects.map((project) => (
                      <SelectItem key={project.id} value={project.id}>
                        {project.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-1.5">
                <label className="text-sm font-medium text-foreground">内容</label>
                <Input
                  value={filters.keyword}
                  onChange={(event) => setFilters((current) => ({
                    ...current,
                    keyword: event.target.value,
                  }))}
                  placeholder="搜索标题或描述"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-sm font-medium text-foreground">开始时间</label>
                <Input
                  type="date"
                  value={filters.startDate}
                  onChange={(event) => setFilters((current) => ({
                    ...current,
                    startDate: event.target.value,
                  }))}
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-sm font-medium text-foreground">结束时间</label>
                <Input
                  type="date"
                  value={filters.endDate}
                  onChange={(event) => setFilters((current) => ({
                    ...current,
                    endDate: event.target.value,
                  }))}
                />
              </div>
            </div>

            <div className="mt-3 flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
              <span>
                共 {filteredTasks.length} 条归档任务
              </span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={isResetDisabled}
                onClick={() => setFilters(defaultFilters)}
              >
                重置筛选
              </Button>
            </div>
          </div>

          <div className="overflow-hidden rounded-xl border border-border/70">
            <div className="max-h-[28rem] overflow-auto">
              <table className="min-w-full text-sm">
                <thead className="sticky top-0 bg-muted/40 text-left">
                  <tr className="border-b border-border">
                    <th className="px-3 py-2 text-xs font-medium text-muted-foreground">项目</th>
                    <th className="px-3 py-2 text-xs font-medium text-muted-foreground">内容</th>
                    <th className="px-3 py-2 text-xs font-medium text-muted-foreground">优先级</th>
                    <th className="px-3 py-2 text-xs font-medium text-muted-foreground">更新时间</th>
                  </tr>
                </thead>
                <tbody>
                  {loading ? (
                    <tr>
                      <td colSpan={4} className="px-3 py-8 text-center text-sm text-muted-foreground">
                        归档任务加载中...
                      </td>
                    </tr>
                  ) : error ? (
                    <tr>
                      <td colSpan={4} className="px-3 py-8 text-center text-sm text-destructive">
                        {error}
                      </td>
                    </tr>
                  ) : filteredTasks.length === 0 ? (
                    <tr>
                      <td colSpan={4} className="px-3 py-8 text-center text-sm text-muted-foreground">
                        {emptyStateMessage}
                      </td>
                    </tr>
                  ) : (
                    filteredTasks.map((task) => (
                      <tr key={task.id} className="border-b border-border/50 align-top last:border-0">
                        <td className="px-3 py-3 text-muted-foreground">
                          {projectMap.get(task.project_id) ?? task.project_id}
                        </td>
                        <td className="px-3 py-3">
                          <div className="space-y-1">
                            <div className="font-medium text-foreground">{task.title}</div>
                            {task.description && (
                              <div className="line-clamp-2 text-xs text-muted-foreground">
                                {task.description}
                              </div>
                            )}
                          </div>
                        </td>
                        <td className="px-3 py-3 text-muted-foreground">
                          {getPriorityLabel(task.priority)}
                        </td>
                        <td className="px-3 py-3 text-muted-foreground">
                          {formatDate(task.updated_at)}
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
