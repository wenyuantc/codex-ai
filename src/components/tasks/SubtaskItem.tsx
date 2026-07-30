import type { Subtask } from "@/lib/types";
import { useTaskStore } from "@/stores/taskStore";
import { Check, X } from "lucide-react";

interface SubtaskItemProps {
  subtask: Subtask;
}

export function SubtaskItem({ subtask }: SubtaskItemProps) {
  const { toggleSubtask, deleteSubtask } = useTaskStore();
  const isCompleted = subtask.status === "completed";

  return (
    <div className="flex items-center gap-2 group py-0.5">
      <button
        onClick={() => toggleSubtask(subtask.id, isCompleted ? "todo" : "completed")}
        className={`shrink-0 w-4 h-4 rounded border flex items-center justify-center transition-colors ${
          isCompleted
            ? "bg-primary border-primary text-primary-foreground"
            : "border-input hover:border-primary"
        }`}
      >
        {isCompleted && <Check className="h-3 w-3" />}
      </button>
      <span className={`text-xs flex-1 ${isCompleted ? "line-through text-muted-foreground" : ""}`}>
        {subtask.title}
      </span>
      <button
        onClick={() => deleteSubtask(subtask.id)}
        className="shrink-0 p-0.5 text-muted-foreground/0 group-hover:text-muted-foreground hover:text-destructive transition-colors"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  );
}
