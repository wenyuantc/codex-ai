import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useEmployeeStore } from "@/stores/employeeStore";
import { useTaskStore } from "@/stores/taskStore";
import { EmployeeCard } from "./EmployeeCard";

const ACTIVE_TASK_STATUSES = new Set(["in_progress", "review"]);

interface EmployeeListProps {
  projectId?: string;
  highlightedEmployeeId?: string | null;
  highlightedEmployeeNonce?: number | null;
  onCreateEmployee?: () => void;
}

export function EmployeeList({
  projectId,
  highlightedEmployeeId,
  highlightedEmployeeNonce,
  onCreateEmployee,
}: EmployeeListProps) {
  const { t } = useTranslation(["employees", "common", "projects"]);
  const { employees, fetchEmployees } = useEmployeeStore();
  const { tasks, fetchTasks } = useTaskStore();
  const [filter, setFilter] = useState<string>("all");

  useEffect(() => {
    void fetchEmployees();
  }, [fetchEmployees]);

  useEffect(() => {
    void fetchTasks(projectId);
  }, [fetchTasks, projectId]);

  const projectEmployees = projectId
    ? employees.filter((employee) => employee.project_id === projectId)
    : employees;
  const filtered =
    filter === "all"
      ? projectEmployees
      : projectEmployees.filter((employee) => employee.status === filter);

  const taskCountByEmployeeId = useMemo(() => {
    const counts = new Map<string, number>();
    for (const task of tasks) {
      if (!task.assignee_id || !ACTIVE_TASK_STATUSES.has(task.status)) {
        continue;
      }
      if (projectId && task.project_id !== projectId) {
        continue;
      }
      counts.set(task.assignee_id, (counts.get(task.assignee_id) ?? 0) + 1);
    }
    return counts;
  }, [projectId, tasks]);

  useEffect(() => {
    if (!highlightedEmployeeId || filter === "all") {
      return;
    }

    const highlightedEmployee = projectEmployees.find(
      (employee) => employee.id === highlightedEmployeeId,
    );
    if (!highlightedEmployee) {
      return;
    }

    if (highlightedEmployee.status !== filter) {
      setFilter("all");
    }
  }, [filter, highlightedEmployeeId, highlightedEmployeeNonce, projectEmployees]);

  useEffect(() => {
    if (!highlightedEmployeeId) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      document
        .getElementById(`employee-card-${highlightedEmployeeId}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 80);

    return () => window.clearTimeout(timeoutId);
  }, [filtered.length, highlightedEmployeeId, highlightedEmployeeNonce]);

  return (
    <div className="space-y-3">
      {/* Filter */}
      <div className="flex items-center gap-2">
        {["all", "online", "busy", "offline", "error"].map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`px-2.5 py-1 text-xs rounded-md transition-colors ${
              filter === f
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent"
            }`}
          >
            {f === "all" ? t("projects:filterAll") : t(`common:status.${f}`, { defaultValue: f })}
          </button>
        ))}
        <span className="text-xs text-muted-foreground ml-auto">
          {t("countEmployees", { count: filtered.length })}
        </span>
      </div>

      {/* Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {filtered.map((emp) => (
          <EmployeeCard
            key={emp.id}
            employee={emp}
            taskCount={taskCountByEmployeeId.get(emp.id) ?? 0}
            highlighted={emp.id === highlightedEmployeeId}
          />
        ))}
      </div>

      {filtered.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-3 py-12 text-center">
          <p className="text-sm text-muted-foreground">
            {filter === "all"
              ? t("emptyAll")
              : t("emptyStatus", {
                  status: t(`common:status.${filter}`, { defaultValue: filter }),
                })}
          </p>
          {filter === "all" && onCreateEmployee && (
            <button
              type="button"
              onClick={onCreateEmployee}
              className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-primary/90"
            >
              {t("create")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
