import { memo, useRef } from "react";
import { useDroppable } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { useVirtualizer } from "@tanstack/react-virtual";
import type {
  CodexSessionKind,
  Milestone,
  Tag,
  Task,
  TaskGitContext,
  TaskStatus,
} from "@/lib/types";
import { TaskCard } from "./TaskCard";

/** Columns at or above this size use virtualization. */
const VIRTUALIZE_THRESHOLD = 25;
/** Estimated TaskCard height including gap; overscan absorbs variance. */
const ESTIMATED_CARD_HEIGHT = 140;
const VIRTUAL_OVERSCAN = 4;

interface KanbanColumnProps {
  status: TaskStatus;
  label: string;
  color: string;
  tasks: Task[];
  highlightedTaskId?: string | null;
  selectedTaskIds?: string[];
  onToggleTaskSelection?: (taskId: string) => void;
  taskGitContextMap: Record<string, TaskGitContext | null>;
  projectGitBranchMap: Record<string, string[]>;
  taskTagsByTaskId?: Map<string, Tag[]>;
  milestonesById?: Map<string, Milestone>;
  onTaskTagsChange?: (taskId: string, tagIds: string[]) => void;
  onOpenLog: (taskId: string, sessionKind?: CodexSessionKind) => void;
  onGitActionCompleted: (projectId: string, message: string) => Promise<void> | void;
}

export const KanbanColumn = memo(function KanbanColumn({
  status,
  label,
  color,
  tasks,
  highlightedTaskId,
  selectedTaskIds = [],
  onToggleTaskSelection,
  taskGitContextMap,
  projectGitBranchMap,
  taskTagsByTaskId,
  milestonesById,
  onTaskTagsChange,
  onOpenLog,
  onGitActionCompleted,
}: KanbanColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: status,
    data: { type: "column", status },
  });
  const scrollParentRef = useRef<HTMLDivElement | null>(null);
  const shouldVirtualize = tasks.length >= VIRTUALIZE_THRESHOLD;
  const virtualizer = useVirtualizer({
    count: shouldVirtualize ? tasks.length : 0,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => ESTIMATED_CARD_HEIGHT,
    overscan: VIRTUAL_OVERSCAN,
    enabled: shouldVirtualize,
  });

  const renderTaskCard = (task: Task) => {
    const selected = selectedTaskIds.includes(task.id);
    const boardTags = taskTagsByTaskId?.get(task.id);
    const milestoneName = task.milestone_id
      ? (milestonesById?.get(task.milestone_id)?.name ?? null)
      : null;
    return (
      <TaskCard
        key={task.id}
        task={task}
        hideRunAction={status === "completed"}
        highlighted={task.id === highlightedTaskId || selected}
        selected={selected}
        onToggleSelected={onToggleTaskSelection}
        gitContext={taskGitContextMap[task.id] ?? null}
        projectBranches={projectGitBranchMap[task.project_id] ?? []}
        tags={boardTags}
        milestoneName={milestoneName}
        onTaskTagsChange={onTaskTagsChange}
        onOpenLog={onOpenLog}
        onGitActionCompleted={onGitActionCompleted}
      />
    );
  };

  return (
    <div
      ref={setNodeRef}
      className={`flex flex-col w-72 min-w-[288px] bg-muted/50 rounded-lg transition-colors ${
        isOver ? "ring-2 ring-primary/50 bg-muted" : ""
      }`}
    >
      <div className="flex items-center gap-2 px-3 py-2.5 border-b border-border/50">
        <div className={`w-2.5 h-2.5 rounded-full ${color}`} />
        <span className="text-sm font-medium">{label}</span>
        <span className="text-xs text-muted-foreground ml-auto bg-muted px-1.5 py-0.5 rounded-full">
          {tasks.length}
        </span>
      </div>

      <SortableContext items={tasks.map((t) => t.id)} strategy={verticalListSortingStrategy}>
        <div ref={scrollParentRef} className="flex-1 overflow-y-auto px-2 py-2">
          {shouldVirtualize ? (
            <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const task = tasks[virtualRow.index];
                if (!task) {
                  return null;
                }
                return (
                  <div
                    key={task.id}
                    data-index={virtualRow.index}
                    className="absolute left-0 top-0 w-full pb-2"
                    style={{
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    {renderTaskCard(task)}
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="space-y-2">
              {tasks.map((task) => renderTaskCard(task))}
              {tasks.length === 0 && (
                <div className="text-xs text-muted-foreground text-center py-6">拖拽任务到此处</div>
              )}
            </div>
          )}
        </div>
      </SortableContext>
    </div>
  );
});
