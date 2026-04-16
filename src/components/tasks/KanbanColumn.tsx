import { useDroppable } from "@dnd-kit/core";
import {
  SortableContext,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import type { CodexSessionKind, Task, TaskStatus } from "@/lib/types";
import { TaskCard } from "./TaskCard";

interface KanbanColumnProps {
  status: TaskStatus;
  label: string;
  color: string;
  tasks: Task[];
  onOpenLog: (taskId: string, sessionKind?: CodexSessionKind) => void;
}

export function KanbanColumn({
  status,
  label,
  color,
  tasks,
  onOpenLog,
}: KanbanColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: status,
    data: { type: "column", status },
  });

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

      <SortableContext
        items={tasks.map((t) => t.id)}
        strategy={verticalListSortingStrategy}
      >
        <div className="flex-1 overflow-y-auto px-2 py-2 space-y-2">
          {tasks.map((task) => (
            <TaskCard
              key={task.id}
              task={task}
              hideRunAction={status === "completed"}
              onOpenLog={onOpenLog}
            />
          ))}
          {tasks.length === 0 && (
            <div className="text-xs text-muted-foreground text-center py-6">
              拖拽任务到此处
            </div>
          )}
        </div>
      </SortableContext>
    </div>
  );
}
