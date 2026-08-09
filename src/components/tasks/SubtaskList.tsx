import { useEffect, useState } from "react";
import { useTaskStore } from "@/stores/taskStore";
import { SubtaskItem } from "./SubtaskItem";
import { Plus } from "lucide-react";

interface SubtaskListProps {
  taskId: string;
  /** Hide the built-in heading (used when a parent section already renders one). */
  hideHeader?: boolean;
}

export function SubtaskList({ taskId, hideHeader = false }: SubtaskListProps) {
  const { subtasks, fetchSubtasks, addSubtask } = useTaskStore();
  const [newTitle, setNewTitle] = useState("");

  useEffect(() => {
    fetchSubtasks(taskId);
  }, [taskId, fetchSubtasks]);

  const items = subtasks[taskId] ?? [];
  const completedCount = items.filter((s) => s.status === "completed").length;

  const handleAdd = async () => {
    if (!newTitle.trim()) return;
    await addSubtask(taskId, newTitle.trim());
    setNewTitle("");
  };

  return (
    <div>
      {!hideHeader && (
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-xs font-medium text-muted-foreground">
            子任务
            {items.length > 0 && (
              <span className="ml-1">
                ({completedCount}/{items.length})
              </span>
            )}
          </h3>
        </div>
      )}

      {items.length > 0 && (
        <div className="space-y-1 mb-2">
          {items.map((subtask) => (
            <SubtaskItem key={subtask.id} subtask={subtask} />
          ))}
        </div>
      )}

      <div className="flex gap-1.5">
        <input
          type="text"
          value={newTitle}
          onChange={(e) => setNewTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAdd()}
          placeholder="添加子任务..."
          className="flex-1 text-xs border border-input rounded px-2 py-1 bg-background"
        />
        <button
          onClick={handleAdd}
          disabled={!newTitle.trim()}
          className="p-1 text-muted-foreground hover:text-foreground disabled:opacity-50"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
