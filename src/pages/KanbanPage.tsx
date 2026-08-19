import { useHotkeys } from "react-hotkeys-hook";
import { Kbd } from "@/components/keyboard/Kbd";
import { useTranslation } from "react-i18next";

import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
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
import {
  batchUpdateTasks,
  getCodexSettings,
  listMilestones,
  listTags,
  listTaskDependencies,
  listTaskTags,
} from "@/lib/backend";
import { filterKanbanTaskIds, type TaskTagMap } from "@/lib/kanbanFilters";
import { getProjectWorkingDir } from "@/lib/projects";
import { startTaskRunSession } from "@/lib/taskRunSession";
import { buildTaskExecutionInput } from "@/lib/taskPrompt";
import type { CodexSessionKind, Milestone, Tag, Task } from "@/lib/types";
import { PRIORITIES, TASK_STATUSES } from "@/lib/types";
import { getPriorityLabel, getStatusLabel } from "@/lib/utils";
import { useTaskStore } from "@/stores/taskStore";
import { useProjectStore } from "@/stores/projectStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { Archive, CheckSquare, Play, Plus } from "lucide-react";
const FILTER_ALL = "all";
const FILTER_UNASSIGNED = "__unassigned__";

export function KanbanPage() {
  const { t } = useTranslation("kanban");
  const [searchParams, setSearchParams] = useSearchParams();
  const { fetchTasks, fetchAttachments, fetchSubtasks, tasks, runQueue } = useTaskStore();
  const currentProjectId = useProjectStore((state) => state.currentProject?.id);
  const projects = useProjectStore((state) => state.projects);
  const environmentMode = useProjectStore((state) => state.environmentMode);
  const { employees, employeeRuntime, fetchEmployees } = useEmployeeStore();
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
  const [testerAutomationEnabled, setTesterAutomationEnabled] = useState<boolean | null>(null);
  const [testerTipDismissed, setTesterTipDismissed] = useState(() => {
    if (typeof window === "undefined") {
      return false;
    }
    return window.localStorage.getItem("codex-ai:kanban-tester-tip-dismissed") === "1";
  });
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
    void getCodexSettings()
      .then((settings) => setTesterAutomationEnabled(settings.tester_automation_enabled))
      .catch(() => setTesterAutomationEnabled(null));
  }, []);

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
      setBatchMessage(t("batchNeedSelection"));
      return;
    }
    setBatchLoading(true);
    setBatchMessage(null);
    try {
      await batchUpdateTasks({ task_ids: selectedTaskIds, status: batchStatus });
      await fetchTasks(currentProjectId);
      setSelectedTaskIds([]);
      setBatchMessage(t("batchUpdated", { count: selectedTaskIds.length }));
    } catch (error) {
      setBatchMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBatchLoading(false);
    }
  };

  const isTaskExecutionActive = (taskId: string) =>
    Object.values(employeeRuntime).some((runtime) =>
      runtime.sessions.some((session) => session.task_id === taskId),
    );

  const canBatchRunTask = (task: Task, incompleteDependencyIds: Set<string>) => {
    if (!task.assignee_id || task.status === "archived") {
      return false;
    }
    if (runQueue.some((item) => item.task_id === task.id)) {
      return false;
    }
    if (isTaskExecutionActive(task.id)) {
      return false;
    }
    if (incompleteDependencyIds.has(task.id)) {
      return false;
    }
    return true;
  };

  const handleBatchRun = async () => {
    if (selectedTaskIds.length === 0) {
      setBatchMessage(t("batchRunNeedSelection"));
      return;
    }

    setBatchLoading(true);
    setBatchMessage(null);
    let started = 0;
    let queued = 0;
    let skipped = 0;

    try {
      const selectedTasks = selectedTaskIds
        .map((taskId) => tasks.find((task) => task.id === taskId))
        .filter((task): task is Task => Boolean(task));
      const incompleteDependencyIds = new Set<string>();

      await Promise.all(
        selectedTasks.map(async (task) => {
          try {
            const deps = await listTaskDependencies(task.id);
            const incomplete = deps.some((dep) => {
              const dependency = tasks.find((item) => item.id === dep.depends_on_task_id);
              return !dependency || dependency.status !== "completed";
            });
            if (incomplete) {
              incompleteDependencyIds.add(task.id);
            }
          } catch (error) {
            console.error("Failed to load task dependencies for batch run:", task.id, error);
            incompleteDependencyIds.add(task.id);
          }
        }),
      );

      for (const task of selectedTasks) {
        if (!canBatchRunTask(task, incompleteDependencyIds)) {
          skipped += 1;
          continue;
        }

        const assignee = employees.find((employee) => employee.id === task.assignee_id);
        const project = projects.find((item) => item.id === task.project_id);
        if (!assignee) {
          skipped += 1;
          continue;
        }

        try {
          await Promise.all([fetchSubtasks(task.id), fetchAttachments(task.id)]);
          const executionInput = buildTaskExecutionInput({
            title: task.title,
            description: task.description,
            subtasks: useTaskStore.getState().subtasks[task.id] ?? [],
            attachments: useTaskStore.getState().attachments[task.id] ?? [],
          });
          const outcome = await startTaskRunSession({
            task,
            assigneeId: assignee.id,
            assignee,
            projectRepoPath: getProjectWorkingDir(project),
            executionInput,
            clearTaskOutput: true,
          });
          if (outcome.status === "queued") {
            queued += 1;
          } else {
            started += 1;
          }
        } catch (error) {
          console.error("Failed to start task during batch run:", task.id, error);
          skipped += 1;
        }
      }

      setBatchMessage(t("batchRunSummary", { started, queued, skipped }));
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
          <h2 className="text-lg font-semibold">{t("listTitle")}</h2>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant={overdueOnly ? "default" : "outline"}
              onClick={() => setOverdueOnly((current) => !current)}
            >
              {overdueOnly ? t("overdueOnly") : t("includeNotOverdue")}
            </Button>
            <Button
              variant={blockedOnly ? "default" : "outline"}
              onClick={() => setBlockedOnly((current) => !current)}
            >
              {blockedOnly ? t("blockedOnly") : t("includeNotBlocked")}
            </Button>
            <Button onClick={() => setShowCreateDialog(true)}>
              <Plus className="h-4 w-4" />
              {t("createTask")}
              <Kbd variant="primary" size="xs" className="ml-1.5">
                N
              </Kbd>
            </Button>
            <Button variant="outline" onClick={() => setShowArchiveDialog(true)}>
              <Archive className="h-4 w-4" />
              {t("archiveManage")}
              <Kbd variant="subtle" size="xs" className="ml-1.5">
                A
              </Kbd>
            </Button>
          </div>
        </div>

        {testerAutomationEnabled === false && !testerTipDismissed && (
          <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-950 dark:text-amber-100">
            <p>
              {t("testerTip")}
              <Link className="mx-1 underline" to="/settings?section=git">
                {t("testerTipLink")}
              </Link>
              {t("testerTipSuffix")}
            </p>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2"
              onClick={() => {
                window.localStorage.setItem("codex-ai:kanban-tester-tip-dismissed", "1");
                setTesterTipDismissed(true);
              }}
            >
              {t("gotIt")}
            </Button>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <Input
            className="h-9 w-48"
            placeholder={t("searchPlaceholder")}
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
          />
          <Select
            value={milestoneFilter}
            onValueChange={(value) => setMilestoneFilter(value ?? FILTER_ALL)}
          >
            <SelectTrigger className="h-9 w-40">
              <SelectValue placeholder={t("milestone")}>
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return t("allMilestones");
                  }
                  return milestones.find((item) => item.id === value)?.name ?? t("milestone");
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>{t("allMilestones")}</SelectItem>
              {milestones.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={tagFilter} onValueChange={(value) => setTagFilter(value ?? FILTER_ALL)}>
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder={t("tag")}>
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return t("allTags");
                  }
                  return tags.find((item) => item.id === value)?.name ?? t("tag");
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>{t("allTags")}</SelectItem>
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
              <SelectValue placeholder={t("priority")}>
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return t("allPriorities");
                  }
                  return value ? getPriorityLabel(String(value)) : t("priority");
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>{t("allPriorities")}</SelectItem>
              {PRIORITIES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {getPriorityLabel(item.value)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={assigneeFilter}
            onValueChange={(value) => setAssigneeFilter(value ?? FILTER_ALL)}
          >
            <SelectTrigger className="h-9 w-36">
              <SelectValue placeholder={t("assignee")}>
                {(value) => {
                  if (!value || value === FILTER_ALL) {
                    return t("allAssignees");
                  }
                  if (value === FILTER_UNASSIGNED) {
                    return t("unassigned");
                  }
                  return projectEmployees.find((item) => item.id === value)?.name ?? t("assignee");
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FILTER_ALL}>{t("allAssignees")}</SelectItem>
              <SelectItem value={FILTER_UNASSIGNED}>{t("unassigned")}</SelectItem>
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
              <SelectValue placeholder={t("batchStatus")}>
                {(value) =>
                  typeof value === "string" && value ? getStatusLabel(value) : t("batchStatus")
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {TASK_STATUSES.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {getStatusLabel(item.value)}
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
              setBatchMessage(t("batchSelected", { count: filteredTaskIds.length }));
            }}
          >
            <CheckSquare className="h-4 w-4" />
            {t("selectAllFiltered")}
          </Button>
          <Button size="sm" disabled={batchLoading} onClick={() => void handleBatchUpdate()}>
            {t("batchUpdate", { count: selectedTaskIds.length })}
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={batchLoading || selectedTaskIds.length === 0}
            onClick={() => void handleBatchRun()}
          >
            <Play className="h-4 w-4" />
            {t("batchRun")}
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
          <div className="flex h-full flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border text-sm text-muted-foreground">
            <p>{environmentMode === "ssh" ? t("emptySshProjects") : t("emptyLocalProjects")}</p>
            <div className="flex flex-wrap items-center justify-center gap-2">
              <Link
                to="/projects"
                className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-primary/90"
              >
                {t("goCreateProject")}
              </Link>
              {currentProjectId && (
                <button
                  type="button"
                  onClick={() => setShowCreateDialog(true)}
                  className="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-accent"
                >
                  {t("createTask")}
                </button>
              )}
            </div>
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
