import { CheckCircle2, Circle, CircleDot, ListTodo, XCircle } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { SessionTodoItem, SessionTodoStatus } from "@/lib/sessionTodos";
import { cn } from "@/lib/utils";

interface SessionTodoListProps {
  todos: SessionTodoItem[];
}

function StatusIcon({ status }: { status: SessionTodoStatus }) {
  switch (status) {
    case "completed":
      return <CheckCircle2 className="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-400" />;
    case "in_progress":
      return <CircleDot className="mt-0.5 h-3.5 w-3.5 shrink-0 text-violet-400" />;
    case "cancelled":
      return <XCircle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-zinc-500" />;
    default:
      return <Circle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-zinc-500" />;
  }
}

export function SessionTodoList({ todos }: SessionTodoListProps) {
  const { t } = useTranslation("sessions");
  if (todos.length === 0) {
    return null;
  }

  const completedCount = todos.filter((item) => item.status === "completed").length;
  const percent = Math.round((completedCount / todos.length) * 100);

  return (
    <div
      className="shrink-0 border-b border-zinc-800 bg-zinc-950 px-2 py-1.5"
      role="status"
      aria-label={t("terminalTodosTitle")}
    >
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5 text-[11px] font-medium text-violet-300">
          <ListTodo className="h-3.5 w-3.5" />
          <span>{t("terminalTodosTitle")}</span>
        </div>
        <span className="font-mono text-[11px] tabular-nums text-zinc-500">
          {t("terminalTodosProgress", { completed: completedCount, total: todos.length })}
        </span>
      </div>
      <div className="mb-1.5 h-1 overflow-hidden rounded-full bg-zinc-800">
        <div
          className="h-full bg-emerald-500/80 transition-all duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
      <ul className="max-h-36 space-y-0.5 overflow-y-auto">
        {todos.map((item, index) => (
          <li
            key={`${index}:${item.content}`}
            className={cn(
              "flex items-start gap-1.5 rounded px-1 py-0.5 text-xs",
              item.status === "in_progress" && "bg-violet-500/10",
              item.status === "completed" && "opacity-70",
              item.status === "cancelled" && "opacity-50",
            )}
          >
            <StatusIcon status={item.status} />
            <span
              className={cn(
                "min-w-0 flex-1 leading-4 text-zinc-200",
                item.status === "in_progress" && "text-violet-200",
                (item.status === "completed" || item.status === "cancelled") &&
                  "text-zinc-500 line-through",
              )}
            >
              {item.content}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
