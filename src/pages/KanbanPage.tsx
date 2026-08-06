import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { KanbanBoard } from "@/components/tasks/KanbanBoard";
import { ArchiveManagementDialog } from "@/components/tasks/ArchiveManagementDialog";
import { CreateTaskDialog } from "@/components/tasks/CreateTaskDialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { batchUpdateTasks, listMilestones, listTags, listTaskTags } from "@/lib/backend";
import { filterKanbanTaskIds, type TaskTagMap } from "@/lib/kanbanFilters";
import type { CodexSessionKind, Milestone, Tag } from "@/lib/types";
import { PRIORITIES, TASK_STATUSES } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Archive, CheckSquare, Plus } from "lucide-react";

const FILTER_ALL = "all";
const FILTER_UNASSIGNED = "__unassigned__";

export function KanbanPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { fetchTasks, tasks } = useTaskStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const projects = useProjectStore((state) => state.projects);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const { employees, fetchEmployees } = useEmployeeStore();
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [showArchiveDialog, setShowArchiveDialog] = useState(false);
  const [pendingLogRequest, setPendingLogRequest] = useState<{
    taskId: string;
    sessionKind?: CodexSessionKind;
  } | null>(null);
  const consumePendingLogRequest = useCallback(() => {
    setPendingLogRequest(null);
  }, []);
  const handleCreateOpenLog = useCallback((taskId: string, sessionKind?: CodexSessionKind) => {
    setPendingLogRequest({ taskId, sessionKind });
  }, []);
  const [overdueOnly, setOverdueOnly] = useState(false);
  const [blockedOnly, setBlockedOnly] = useState(false);
  const [milestoneFilter, setMilestoneFilter] = useState<string>(FILTER_ALL);
  const [tagFilter, setTagFilter] = useState<string>(FILTER_ALL);
  const [priorityFilter, setPriorityFilter] = useState<string>(FILTER_ALL);
  const [assigneeFilter, setAssigneeFilter] = useState<string>(FILTER_ALL);
  const [keyword, setKeyword] = useState("");
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [taskTagMap, setTaskTagMap] = useState<TaskTagMap>(() => new Map());
  /** Bumped after create/tag edits so filter map does not go stale. */
  const [taskTagMapVersion, setTaskTagMapVersion] = useState(0);
  const [selectedTaskIds, setSelectedTaskIds] = useState<string[]>([]);
  const [batchStatus, setBatchStatus] = useState<string>("");
  const [batchMessage, setBatchMessage] = useState<string | null>(null);
  const [batchLoading, setBatchLoading] = useState(false);
  const visibleProjectIdsKey = projects.map((project) => project.id).join(",");
  const targetTaskId = searchParams.get("taskId");

  const activeTaskIdsKey = useMemo(
    () =>
      tasks
        .filter((task) => task.status !== "archived")
        .map((task) => task.id)
        .sort()
        .join(","),
    [tasks],
  );

  const refreshTaskTagMap = useCallback(() => {
    setTaskTagMapVersion((version) => version + 1);
  }, []);

  const handleTaskTagsChange = useCallback((taskId: string, tagIds: string[]) => {
    setTaskTagMap((current) => {
      const next = new Map(current);
      next.set(taskId, tagIds);
      return next;
    });
  }, []);

  useEffect(() => {
    void fetchEmployees();
  }, [fetchEmployees]);

  useEffect(() => {
    void fetchTasks(currentProjectId);
  }, [currentProjectId, environmentMode, visibleProjectIdsKey, fetchTasks]);

  // Load milestones/tags for the current project, or all visible projects when none selected
  // so multi-project boards still resolve milestone names and tag filters.
  useEffect(() => {
    const projectIds = currentProjectId
      ? [currentProjectId]
      : projects.map((project) => project.id);
    if (projectIds.length === 0) {
      setMilestones([]);
      setTags([]);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const [milestoneGroups, tagGroups] = await Promise.all([
          Promise.all(projectIds.map((id) => listMilestones(id).catch(() => [] as Milestone[]))),
          Promise.all(projectIds.map((id) => listTags(id).catch(() => [] as Tag[]))),
        ]);
        if (cancelled) {
          return;
        }
        const mergedMilestones = new Map<string, Milestone>();
        for (const group of milestoneGroups) {
          for (const item of group) {
            mergedMilestones.set(item.id, item);
          }
        }
        const mergedTags = new Map<string, Tag>();
        for (const group of tagGroups) {
          for (const item of group) {
            mergedTags.set(item.id, item);
          }
        }
        setMilestones(Array.from(mergedMilestones.values()));
        setTags(Array.from(mergedTags.values()));
      } catch {
        if (!cancelled) {
          setMilestones([]);
          setTags([]);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [currentProjectId, visibleProjectIdsKey, projects]);

  // Board-level task → tags map to power tag filter and avoid per-card N+1 for tags.
  useEffect(() => {
    const activeTasks = tasks.filter((task) => task.status !== "archived");
    if (activeTasks.length === 0) {
      setTaskTagMap(new Map());
      return;
    }

    let cancelled = false;
    const ids = activeTasks.map((task) => task.id);

    void (async () => {
      const entries = await Promise.all(
        ids.map(async (taskId) => {
          try {
            const taskTags = await listTaskTags(taskId);
            return [taskId, taskTags.map((tag) => tag.id)] as const;
          } catch {
            return [taskId, [] as string[]] as const;
          }
        }),
      );
      if (cancelled) {
        return;
      }
      setTaskTagMap(new Map(entries));
    })();

    return () => {
      cancelled = true;
    };
  }, [activeTaskIdsKey, taskTagMapVersion]);

  const hasProjects = projects.length > 0;

  const kanbanFilters = useMemo(
    () => ({
      keyword,
      overdueOnly,
      blockedOnly,
      milestoneId: milestoneFilter === FILTER_ALL ? null : milestoneFilter,
      tagId: tagFilter === FILTER_ALL ? null : tagFilter,
      priority: priorityFilter === FILTER_ALL ? null : priorityFilter,
      assigneeId: assigneeFilter === FILTER_ALL ? null : assigneeFilter,
    }),
    [keyword, overdueOnly, blockedOnly, milestoneFilter, tagFilter, priorityFilter, assigneeFilter],
  );

  const filteredTaskIds = useMemo(
    () => filterKanbanTaskIds(tasks, kanbanFilters, taskTagMap),
    [tasks, kanbanFilters, taskTagMap],
  );

  const projectEmployees = useMemo(() => {
    if (!currentProjectId) {
      return employees;
    }
    return employees.filter(
      (employee) => !employee.project_id || employee.project_id === currentProjectId,
    );
  }, [employees, currentProjectId]);

  useHotkeys("n", () => setShowCreateDialog(true), { preventDefault: true });
  useHotkeys("a", () => setShowArchiveDialog(true), { preventDefault: true });

  const handleBatchUpdate = async () => {
    if (selectedTaskIds.length === 0 || !batchStatus) {
      setBatchMessage("请选择任务并指定目标状态。");
      return;
    }
    setBatchLoading(true);
    setBatchMessage(null);
    try {
      await batchUpdateTasks({ task_ids: selectedTaskIds, status: batchStatus });
      await fetchTasks(currentProjectId);
      setSelectedTaskIds([]);
      setBatchMessage(`已批量更新 ${selectedTaskIds.length} 个任务状态。`);
    } catch (error) {
      setBatchMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBatchLoading(false);
    }
  };

  return (
    <div className="h-full flex flex-col">
      <div className="mb-4 space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-lg font-semibold">看板列表</h2>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant={overdueOnly ? "default" : "outline"}
              onClick={() => setOverdueOnly((current) => !current)}
            >
              {overdueOnly ? "仅逾期" : "含未逾期"}
            </Button>
            <Button
              variant={blockedOnly ? "default" : "outline"}
              onClick={() => setBlockedOnly((current) => !current)}
            >
              {blockedOnly ? "仅阻塞" : "含未阻塞"}
            </Button>
            <Button onClick={() => setShowCreateDialog(true)}>
              <Plus className="h-4 w-4" />
              新建任务
              <Kbd variant="primary" size="xs" className="ml-1.5">
                N
              </Kbd>
            </Button>
            <Button variant="outline" onClick={() => setShowArchiveDialog(true)}>
              <Archive className="h-4 w-4" />
              归档管理
              <Kbd variant="subtle" size="xs" className="ml-1.5">
                A
              </Kbd>
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Input
            className="h-9 w-48"
            placeholder="搜索标题/描述"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
          />
          <Select
            value={milestoneFilter}
            onValueChange={(value) => setMilestoneFilter(value ?? FILTER_ALL)}
          >
            <SelectTrigger className="h-9 w-40">
              <SelectValue placeholder="里程碑">
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return "全部里程碑";
                  }
                  return milestones.find((item) => item.id === value)?.name ?? "里程碑";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>全部里程碑</SelectItem>
              {milestones.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={tagFilter} onValueChange={(value) => setTagFilter(value ?? FILTER_ALL)}>
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder="标签">
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return "全部标签";
                  }
                  return tags.find((item) => item.id === value)?.name ?? "标签";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>全部标签</SelectItem>
              {tags.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={priorityFilter}
            onValueChange={(value) => setPriorityFilter(value ?? FILTER_ALL)}
          >
            <SelectTrigger className="h-9 w-32">
              <SelectValue placeholder="优先级">
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return "全部优先级";
                  }
                  return PRIORITIES.find((item) => item.value === value)?.label ?? "优先级";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>全部优先级</SelectItem>
              {PRIORITIES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={assigneeFilter}
            onValueChange={(value) => setAssigneeFilter(value ?? FILTER_ALL)}
          >
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder="执行人">
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return "全部执行人";
                  }
                  if (value === FILTER_UNASSIGNED) {
                    return "未指派";
                  }
                  return projectEmployees.find((item) => item.id === value)?.name ?? "执行人";
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>全部执行人</SelectItem>
              <SelectItem value={FILTER_UNASSIGNED}>未指派</SelectItem>
              {projectEmployees.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={batchStatus || null}
            onValueChange={(value) => setBatchStatus(value ?? "")}
          >
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder="批量改状态">
                {(value) =>
                  typeof value === "string" && value
                    ? (TASK_STATUSES.find((item) => item.value === value)?.label ?? value)
                    : "批量改状态"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {TASK_STATUSES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            size="sm"
            disabled={batchLoading}
            onClick={() => {
              setSelectedTaskIds(filteredTaskIds);
              setBatchMessage(`已选中当前筛选下 ${filteredTaskIds.length} 个任务。`);
            }}
          >
            <CheckSquare className="h-4 w-4" />
            全选筛选结果
          </Button>
          <Button size="sm" disabled={batchLoading} onClick={() => void handleBatchUpdate()}>
            批量更新 ({selectedTaskIds.length})
          </Button>
          {batchMessage && <span className="text-xs text-muted-foreground">{batchMessage}</span>}
        </div>
      </div>
      <div className="flex-1 overflow-hidden">
        {hasProjects ? (
          <KanbanBoard
            projectId={currentProjectId}
            overdueOnly={overdueOnly}
            blockedOnly={blockedOnly}
            milestoneId={milestoneFilter === FILTER_ALL ? null : milestoneFilter}
            tagId={tagFilter === FILTER_ALL ? null : tagFilter}
            priority={priorityFilter === FILTER_ALL ? null : priorityFilter}
            assigneeId={assigneeFilter === FILTER_ALL ? null : assigneeFilter}
            keyword={keyword}
            taskTagMap={taskTagMap}
            milestones={milestones}
            tags={tags}
            selectedTaskIds={selectedTaskIds}
            onToggleTaskSelection={(taskId) => {
              setSelectedTaskIds((current) =>
                current.includes(taskId)
                  ? current.filter((id) => id !== taskId)
                  : [...current, taskId],
              );
            }}
            onTaskTagsChange={handleTaskTagsChange}
            targetTaskId={targetTaskId}
            onClearTargetTask={() => {
              if (!targetTaskId) {
                return;
              }

              const nextSearchParams = new URLSearchParams(searchParams);
              nextSearchParams.delete("taskId");
              setSearchParams(nextSearchParams, { replace: true });
              // Global-search detail may have edited tags/milestones.
              refreshTaskTagMap();
            }}
            pendingLogRequest={pendingLogRequest}
            onPendingLogRequestConsumed={consumePendingLogRequest}
          />
        ) : (
          <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-border text-sm text-muted-foreground">
            {environmentMode === "ssh"
              ? "当前 SSH 视图还没有项目，请先到项目管理创建 SSH 项目。"
              : "当前没有可展示的本地项目。"}
          </div>
        )}
      </div>
      {showCreateDialog && (
        <CreateTaskDialog
          open={showCreateDialog}
          onOpenChange={setShowCreateDialog}
          projectId={currentProjectId}
          onOpenLog={handleCreateOpenLog}
          onCreated={() => {
            // Create+setTaskTags races with the initial tag-map load for the new id.
            refreshTaskTagMap();
          }}
        />
      )}
      {showArchiveDialog && (
        <ArchiveManagementDialog
          open={showArchiveDialog}
          onOpenChange={setShowArchiveDialog}
          defaultProjectId={currentProjectId}
        />
      )}
    </div>
  );
}
