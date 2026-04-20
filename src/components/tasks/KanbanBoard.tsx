import { useEffect, useMemo, useRef, useState } from "react";
import {
  DndContext,
  DragEndEvent,
  DragOverEvent,
  DragOverlay,
  DragStartEvent,
  PointerSensor,
  closestCorners,
  pointerWithin,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { getProjectGitOverview, listTaskGitContexts } from "@/lib/backend";
import { onCodexExit, onTaskAutomationStateChanged } from "@/lib/codex";
import { TASK_STATUSES, type CodexSessionKind, type ProjectGitOverview, type Task, type TaskGitContext, type TaskStatus } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useProjectStore } from "@/stores/projectStore";
import { KanbanColumn } from "./KanbanColumn";
import { TaskCard } from "./TaskCard";
import { TaskLogDialog } from "./TaskLogDialog";
import { TaskDetailDialog } from "./TaskDetailDialog";
import { ErrorBoundary } from "@/components/ErrorBoundary";

interface KanbanBoardProps {
  projectId?: string;
  targetTaskId?: string | null;
  onClearTargetTask?: () => void;
}

export function KanbanBoard({
  projectId: _projectId,
  targetTaskId,
  onClearTargetTask,
}: KanbanBoardProps) {
  const { tasks, moveTask, updateTaskStatus, fetchTasks } = useTaskStore();
  const employees = useEmployeeStore((s) => s.employees);
  const projects = useProjectStore((s) => s.projects);
  const [activeTask, setActiveTask] = useState<Task | null>(null);
  const dragOriginStatusRef = useRef<TaskStatus | null>(null);
  const [searchTaskOpen, setSearchTaskOpen] = useState(false);
  const [gitOverviewByProjectId, setGitOverviewByProjectId] = useState<Record<string, ProjectGitOverview>>({});
  const [taskGitContextsByProjectId, setTaskGitContextsByProjectId] = useState<Record<string, TaskGitContext[]>>({});
  const [logRequest, setLogRequest] = useState<{
    taskId: string;
    sessionKind?: CodexSessionKind;
  } | null>(null);
  const targetTask = targetTaskId ? tasks.find((task) => task.id === targetTaskId) ?? null : null;
  const projectMap = useMemo(
    () => new Map(projects.map((project) => [project.id, project])),
    [projects],
  );
  const gitProjectIds = useMemo(() => {
    const ids = new Set<string>();
    tasks.forEach((task) => {
      if (projectMap.has(task.project_id)) {
        ids.add(task.project_id);
      }
    });
    return Array.from(ids).sort();
  }, [projectMap, tasks]);
  const gitProjectIdsKey = gitProjectIds.join(",");
  const gitContextRefreshKey = useMemo(
    () =>
      tasks
        .map((task) => `${task.id}:${task.status}:${task.last_codex_session_id ?? ""}:${task.updated_at}`)
        .sort()
        .join("|"),
    [tasks],
  );
  const taskProjectMap = useMemo(
    () => Object.fromEntries(tasks.map((task) => [task.id, task.project_id])),
    [tasks],
  );
  const taskGitContextMap = useMemo(
    () => {
      const entries: Array<[string, TaskGitContext]> = [];

      Object.values(taskGitContextsByProjectId).forEach((contexts) => {
        const seenTaskIds = new Set<string>();
        contexts.forEach((context) => {
          if (seenTaskIds.has(context.task_id)) {
            return;
          }
          seenTaskIds.add(context.task_id);
          entries.push([context.task_id, context]);
        });
      });

      return Object.fromEntries(entries);
    },
    [taskGitContextsByProjectId],
  );
  const projectGitBranchMap = useMemo(
    () =>
      Object.fromEntries(
        Object.entries(gitOverviewByProjectId).map(([projectId, overview]) => [
          projectId,
          overview.project_branches,
        ]),
      ),
    [gitOverviewByProjectId],
  );

  const refreshGitOverviews = async (projectIds: string[]) => {
    if (projectIds.length === 0) {
      setGitOverviewByProjectId({});
      setTaskGitContextsByProjectId({});
      return;
    }

    const results = await Promise.all(
      projectIds.map(async (projectId) => {
        try {
          const [overview, contexts] = await Promise.all([
            getProjectGitOverview(projectId),
            listTaskGitContexts(projectId),
          ]);
          return [projectId, overview, contexts] as const;
        } catch (error) {
          console.error(`Failed to fetch git overview for project ${projectId}:`, error);
          return null;
        }
      }),
    );

    setGitOverviewByProjectId((current) => {
      const next: Record<string, ProjectGitOverview> = {};

      projectIds.forEach((projectId) => {
        if (current[projectId]) {
          next[projectId] = current[projectId];
        }
      });

      results.forEach((entry) => {
        if (!entry) {
          return;
        }
        const [projectId, overview] = entry;
        next[projectId] = overview;
      });

      return next;
    });
    setTaskGitContextsByProjectId((current) => {
      const next: Record<string, TaskGitContext[]> = {};

      projectIds.forEach((projectId) => {
        if (current[projectId]) {
          next[projectId] = current[projectId];
        }
      });

      results.forEach((entry) => {
        if (!entry) {
          return;
        }
        const [projectId, , contexts] = entry;
        next[projectId] = contexts;
      });

      return next;
    });
  };

  const handleGitActionCompleted = async (projectId: string) => {
    await refreshGitOverviews([projectId]);
  };

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    })
  );

  const getTaskById = (taskId: string) =>
    useTaskStore.getState().tasks.find((task) => task.id === taskId);

  const resolveDropStatus = (overId: string): TaskStatus | null => {
    const targetStatus = TASK_STATUSES.find((status) => status.value === overId)?.value;
    if (targetStatus) {
      return targetStatus;
    }

    return getTaskById(overId)?.status as TaskStatus | undefined ?? null;
  };

  const collisionDetection = (args: Parameters<typeof pointerWithin>[0]) => {
    const pointerCollisions = pointerWithin(args);
    if (pointerCollisions.length > 0) {
      return pointerCollisions;
    }

    return closestCorners(args);
  };

  const handleDragStart = (event: DragStartEvent) => {
    const task = getTaskById(event.active.id as string);
    if (!task) {
      return;
    }

    dragOriginStatusRef.current = task.status as TaskStatus;
    setActiveTask(task);
  };

  const handleDragOver = (event: DragOverEvent) => {
    const { active, over } = event;

    if (!over) {
      return;
    }

    const taskId = active.id as string;
    const activeTask = getTaskById(taskId);
    const targetStatus = resolveDropStatus(over.id as string);

    if (!activeTask || !targetStatus || activeTask.status === targetStatus) {
      return;
    }

    moveTask(taskId, targetStatus);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    const taskId = active.id as string;
    const originalStatus = dragOriginStatusRef.current;
    const currentTask = getTaskById(taskId);

    setActiveTask(null);
    dragOriginStatusRef.current = null;

    if (!currentTask || !originalStatus) {
      return;
    }

    const targetStatus = over ? resolveDropStatus(over.id as string) : null;

    if (!targetStatus) {
      if (currentTask.status !== originalStatus) {
        moveTask(taskId, originalStatus);
      }
      return;
    }

    if (currentTask.status !== targetStatus) {
      moveTask(taskId, targetStatus);
    }

    if (originalStatus === targetStatus) {
      return;
    }

    void updateTaskStatus(taskId, targetStatus).catch((error) => {
      console.error("Failed to update task status:", error);
      moveTask(taskId, originalStatus);
      void fetchTasks(_projectId);
    });
  };

  const handleDragCancel = () => {
    if (activeTask && dragOriginStatusRef.current) {
      const currentTask = getTaskById(activeTask.id);
      if (currentTask && currentTask.status !== dragOriginStatusRef.current) {
        moveTask(activeTask.id, dragOriginStatusRef.current);
      }
    }

    dragOriginStatusRef.current = null;
    setActiveTask(null);
  };

  const getTasksByStatus = (status: TaskStatus) =>
    tasks.filter((t) => t.status === status);

  const logTask = logRequest ? tasks.find((task) => task.id === logRequest.taskId) ?? null : null;
  const logAssigneeName = logTask?.assignee_id
    ? employees.find((employee) => employee.id === logTask.assignee_id)?.name
    : undefined;

  useEffect(() => {
    if (!targetTaskId || !targetTask) {
      return;
    }

    setSearchTaskOpen(true);

    const timeoutId = window.setTimeout(() => {
      document
        .getElementById(`task-card-${targetTaskId}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 80);

    return () => window.clearTimeout(timeoutId);
  }, [targetTask, targetTaskId]);

  useEffect(() => {
    let active = true;

    void (async () => {
      if (gitProjectIds.length === 0) {
        if (active) {
          setGitOverviewByProjectId({});
          setTaskGitContextsByProjectId({});
        }
        return;
      }

      const results = await Promise.all(
        gitProjectIds.map(async (projectId) => {
          try {
            const [overview, contexts] = await Promise.all([
              getProjectGitOverview(projectId),
              listTaskGitContexts(projectId),
            ]);
            return [projectId, overview, contexts] as const;
          } catch (error) {
            console.error(`Failed to fetch git overview for project ${projectId}:`, error);
            return null;
          }
        }),
      );

      if (!active) {
        return;
      }

      const next: Record<string, ProjectGitOverview> = {};
      const nextContexts: Record<string, TaskGitContext[]> = {};
      results.forEach((entry) => {
        if (!entry) {
          return;
        }
        const [projectId, overview, contexts] = entry;
        next[projectId] = overview;
        nextContexts[projectId] = contexts;
      });
      setGitOverviewByProjectId(next);
      setTaskGitContextsByProjectId(nextContexts);
    })();

    return () => {
      active = false;
    };
  }, [gitContextRefreshKey, gitProjectIdsKey]);

  useEffect(() => {
    if (gitProjectIds.length === 0) {
      return;
    }

    const handleWindowFocus = () => {
      void refreshGitOverviews(gitProjectIds);
    };

    window.addEventListener("focus", handleWindowFocus);
    return () => {
      window.removeEventListener("focus", handleWindowFocus);
    };
  }, [gitProjectIdsKey]);

  useEffect(() => {
    let active = true;
    let cleanup: (() => void) | null = null;
    let automationCleanup: (() => void) | null = null;

    void onCodexExit((exit) => {
      if (!exit.task_id) {
        return;
      }

      const projectId = taskProjectMap[exit.task_id];
      if (!projectId || !gitProjectIds.includes(projectId)) {
        return;
      }

      void refreshGitOverviews([projectId]);
    })
      .then((unlisten) => {
        if (!active) {
          unlisten();
          return;
        }
        cleanup = unlisten;
      })
      .catch((error) => {
        console.error("Failed to listen codex exit events for kanban git refresh:", error);
      });

    void onTaskAutomationStateChanged((event) => {
      if (!gitProjectIds.includes(event.project_id)) {
        return;
      }
      void refreshGitOverviews([event.project_id]);
    })
      .then((unlisten) => {
        if (!active) {
          unlisten();
          return;
        }
        automationCleanup = unlisten;
      })
      .catch((error) => {
        console.error("Failed to listen task automation state change events:", error);
      });

    return () => {
      active = false;
      cleanup?.();
      automationCleanup?.();
    };
  }, [gitProjectIdsKey, taskProjectMap]);

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={collisionDetection}
        onDragStart={handleDragStart}
        onDragOver={handleDragOver}
        onDragEnd={handleDragEnd}
        onDragCancel={handleDragCancel}
      >
        <div className="flex gap-4 h-full overflow-x-auto pb-4">
          {TASK_STATUSES.map((status) => (
            <KanbanColumn
              key={status.value}
              status={status.value}
              label={status.label}
              color={status.color}
              tasks={getTasksByStatus(status.value)}
              highlightedTaskId={targetTaskId}
              taskGitContextMap={taskGitContextMap}
              projectGitBranchMap={projectGitBranchMap}
              onOpenLog={(taskId, sessionKind) => setLogRequest({ taskId, sessionKind })}
              onGitActionCompleted={handleGitActionCompleted}
            />
          ))}
        </div>
        <DragOverlay>
          {activeTask ? (
            <div className="w-72 rotate-2 opacity-90">
              <TaskCard
                task={activeTask}
                isOverlay
                gitContext={taskGitContextMap[activeTask.id] ?? null}
                projectBranches={projectGitBranchMap[activeTask.project_id] ?? []}
              />
            </div>
          ) : null}
        </DragOverlay>
      </DndContext>

      {logRequest !== null && (
        <TaskLogDialog
          open={logRequest !== null}
          task={logTask}
          assigneeName={logAssigneeName}
          sessionKind={logRequest?.sessionKind}
          onOpenChange={(open) => {
            if (!open) {
              setLogRequest(null);
            }
          }}
        />
      )}

      {targetTask && (
        <ErrorBoundary
          fallbackTitle="任务详情渲染失败"
          fallbackDescription="全局搜索定位到的任务详情弹窗出现运行时异常。"
        >
          <TaskDetailDialog
            task={targetTask}
            open={searchTaskOpen}
            onOpenChange={(nextOpen) => {
              setSearchTaskOpen(nextOpen);
              if (!nextOpen) {
                onClearTargetTask?.();
              }
            }}
          />
        </ErrorBoundary>
      )}
    </>
  );
}
