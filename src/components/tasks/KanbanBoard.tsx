import { useEffect, useState } from "react";
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  PointerSensor,
  useSensor,
  useSensors,
  closestCorners,
} from "@dnd-kit/core";
import { TASK_STATUSES, type CodexSessionKind, type Task, type TaskStatus } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";
import { useEmployeeStore } from "@/stores/employeeStore";
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
  const [activeTask, setActiveTask] = useState<Task | null>(null);
  const [searchTaskOpen, setSearchTaskOpen] = useState(false);
  const [logRequest, setLogRequest] = useState<{
    taskId: string;
    sessionKind?: CodexSessionKind;
  } | null>(null);
  const targetTask = targetTaskId ? tasks.find((task) => task.id === targetTaskId) ?? null : null;

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    })
  );

  const handleDragStart = (event: DragEndEvent) => {
    const task = tasks.find((t) => t.id === event.active.id);
    if (task) setActiveTask(task);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveTask(null);

    if (!over) return;

    const taskId = active.id as string;
    const overId = over.id as string;

    // Check if dropped on a column (status id like "todo", "in_progress", etc.)
    const targetStatus = TASK_STATUSES.find((s) => s.value === overId)?.value;
    if (targetStatus) {
      const task = tasks.find((t) => t.id === taskId);
      if (task && task.status !== targetStatus) {
        const previousStatus = task.status;
        moveTask(taskId, targetStatus as TaskStatus);
        void updateTaskStatus(taskId, targetStatus as TaskStatus).catch((error) => {
          console.error("Failed to update task status:", error);
          moveTask(taskId, previousStatus as TaskStatus);
          void fetchTasks(_projectId);
        });
      }
      return;
    }

    // Dropped on another task - find that task's column status
    const targetTask = tasks.find((t) => t.id === overId);
    if (targetTask) {
      const task = tasks.find((t) => t.id === taskId);
      if (task && task.status !== targetTask.status) {
        const previousStatus = task.status;
        moveTask(taskId, targetTask.status as TaskStatus);
        void updateTaskStatus(taskId, targetTask.status as TaskStatus).catch((error) => {
          console.error("Failed to update task status:", error);
          moveTask(taskId, previousStatus as TaskStatus);
          void fetchTasks(_projectId);
        });
      }
    }
  };

  const handleDragCancel = () => {
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

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCorners}
        onDragStart={handleDragStart}
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
              onOpenLog={(taskId, sessionKind) => setLogRequest({ taskId, sessionKind })}
            />
          ))}
        </div>
        <DragOverlay>
          {activeTask ? (
            <div className="w-72 rotate-2 opacity-90">
              <TaskCard task={activeTask} isOverlay />
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
